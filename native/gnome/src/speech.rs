//! One utterance per local SSIP connection, with cancellation scoped to that connection.
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

#[derive(Clone, Copy, Debug)]
pub enum Language {
    English,
    Mandarin,
    Pinyin,
}
impl Language {
    fn code(self) -> &'static str {
        match self {
            Self::English => "en-US",
            Self::Mandarin => "cmn",
            Self::Pinyin => "cmn-LATN-PINYIN",
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    Finished,
    Stopped,
}
const MAX_CHARS: usize = 16_000;
fn lines(text: &str) -> Result<Vec<String>, String> {
    if text.trim().is_empty() || text.chars().count() > MAX_CHARS || text.contains('\0') {
        return Err("Speech text must contain 1 to 16000 characters without NUL".into());
    }
    // SSIP data is dot-stuffed, including after CR/CRLF normalization. Selected
    // text can never terminate SPEAK data and become a protocol command.
    Ok(text
        .replace("\r\n", "\n")
        .replace('\r', "\n")
        .split('\n')
        .map(|line| {
            if line.starts_with('.') {
                format!(".{line}")
            } else {
                line.to_owned()
            }
        })
        .collect())
}

#[cfg(target_os = "linux")]
mod linux {
    use super::*;
    use ssip_client::{
        fifo, ClientName, ClientScope, NotificationType, Priority, QueuedClient, Request, Response,
    };
    use std::{
        io::Write,
        net::Shutdown,
        os::unix::{fs::FileTypeExt, net::UnixStream},
        path::PathBuf,
        sync::Mutex,
        time::Duration,
    };
    use tokio::sync::oneshot;
    type Client = QueuedClient<UnixStream>;
    struct Control {
        socket: UnixStream,
        data_sent: Arc<Mutex<bool>>,
        cleanup: bool,
    }
    impl Drop for Control {
        fn drop(&mut self) {
            // Emergency future-drop path. Ordinary Stop is acknowledged below.
            // Never block a reactor in Drop, and never open another connection.
            if self.cleanup
                && self.socket.set_nonblocking(true).is_ok()
                && self.data_sent.try_lock().is_ok_and(|sent| *sent)
            {
                let _ = self.socket.write_all(b"CANCEL self\r\n");
            }
            let _ = self.socket.shutdown(Shutdown::Both);
        }
    }
    fn socket_path() -> Result<(PathBuf, bool), String> {
        let (path, spawn) = if let Some(address) = std::env::var_os("SPEECHD_ADDRESS") {
            let address = address
                .to_str()
                .ok_or("Invalid Speech Dispatcher address")?;
            let path = address
                .strip_prefix("unix_socket:")
                .ok_or("Native speech requires a local Speech Dispatcher Unix socket")?;
            (PathBuf::from(path), false)
        } else {
            (
                PathBuf::from(
                    std::env::var_os("XDG_RUNTIME_DIR")
                        .ok_or("Speech Dispatcher requires XDG_RUNTIME_DIR")?,
                )
                .join("speech-dispatcher/speechd.sock"),
                true,
            )
        };
        if !path.is_absolute()
            || path.as_os_str().len() > 4096
            || path.to_string_lossy().chars().any(char::is_control)
        {
            return Err("Invalid Speech Dispatcher socket path".into());
        }
        Ok((path, spawn))
    }
    async fn prepare(cancel: &AtomicBool) -> Result<PathBuf, String> {
        let (path, may_spawn) = socket_path()?;
        let check = path.clone();
        let exists = crate::worker::compute(move || {
            Ok(std::fs::metadata(check).is_ok_and(|m| m.file_type().is_socket()))
        })
        .await?;
        if !exists && may_spawn && !cancel.load(Ordering::Acquire) {
            let mut command = tokio::process::Command::new("speech-dispatcher");
            command
                .arg("--spawn")
                .kill_on_drop(true)
                .stdin(std::process::Stdio::null())
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null());
            let status = tokio::time::timeout(Duration::from_secs(5), command.status()).await;
            if !matches!(status, Ok(Ok(s)) if s.success()) {
                return Err("Unable to start Speech Dispatcher. Install speech-dispatcher and a synthesis voice.".into());
            }
        }
        Ok(path)
    }
    fn error(_: impl std::fmt::Display) -> String {
        "Speech Dispatcher could not complete the request".into()
    }
    struct Session {
        client: Client,
        client_id: String,
        terminal: Option<Outcome>,
    }
    impl Session {
        fn observe(&mut self, value: Response) -> Option<Response> {
            match value {
                Response::EventEnd(id) if id.client == self.client_id => {
                    self.terminal = Some(Outcome::Finished);
                    None
                }
                Response::EventCanceled(id) if id.client == self.client_id => {
                    self.terminal = Some(Outcome::Stopped);
                    None
                }
                Response::EventBegin(_) | Response::EventEnd(_) | Response::EventCanceled(_) => {
                    None
                }
                value => Some(value),
            }
        }
        fn request(
            &mut self,
            request: Request,
            expected: impl Fn(&Response) -> bool,
        ) -> Result<Response, String> {
            self.client.push(request);
            self.client.send_next().map_err(error)?;
            for _ in 0..32 {
                let reply = self.client.receive_next().map_err(error)?;
                if let Some(reply) = self.observe(reply) {
                    return if expected(&reply) {
                        Ok(reply)
                    } else {
                        Err("Unexpected Speech Dispatcher reply".into())
                    };
                }
            }
            Err("Speech Dispatcher notification limit reached".into())
        }
    }
    fn worker(
        path: PathBuf,
        data: Vec<String>,
        language: Language,
        cancel: Arc<AtomicBool>,
        control: oneshot::Sender<Control>,
    ) -> Result<Outcome, String> {
        let client = fifo::Builder::new().path(path).timeout(Duration::from_secs(2)).build().map_err(|_|"Speech Dispatcher is unavailable. Start it and install an English/Chinese synthesis voice.".to_owned())?;
        let mut session = Session {
            client: QueuedClient::new(client),
            client_id: String::new(),
            terminal: None,
        };
        session
            .client
            .output_source()
            .set_write_timeout(Some(Duration::from_secs(2)))
            .map_err(error)?;
        let gate = Arc::new(Mutex::new(false));
        let socket = session.client.output_source().try_clone().map_err(error)?;
        if control
            .send(Control {
                socket,
                data_sent: gate.clone(),
                cleanup: true,
            })
            .is_err()
        {
            return Ok(Outcome::Stopped);
        }
        if cancel.load(Ordering::Acquire) {
            return Ok(Outcome::Stopped);
        }
        session.request(
            Request::SetName(ClientName::with_component(
                "studio",
                "codex-thread-studio",
                "translation",
            )),
            |r| matches!(r, Response::ClientNameSet),
        )?;
        if let Response::HistoryClientIdSent(id) = session
            .request(Request::HistoryGetClientId, |r| {
                matches!(r, Response::HistoryClientIdSent(_))
            })?
        {
            session.client_id = id.to_string();
        }
        session.request(Request::SetSsmlMode(false), |r| {
            matches!(r, Response::SsmlModeSet)
        })?;
        session.request(
            Request::SetLanguage(ClientScope::Current, language.code().into()),
            |r| matches!(r, Response::LanguageSet),
        )?;
        session.request(Request::SetPriority(Priority::Text), |r| {
            matches!(r, Response::PrioritySet)
        })?;
        session.request(Request::SetNotification(NotificationType::End, true), |r| {
            matches!(r, Response::NotificationSet)
        })?;
        session.request(
            Request::SetNotification(NotificationType::Cancel, true),
            |r| matches!(r, Response::NotificationSet),
        )?;
        // The gate prevents Stop from writing in the middle of SPEAK data. It is
        // held only through the bounded initial reply and a <=64 KiB data flush.
        {
            let mut sent = gate.lock().map_err(|_| "Speech control is unavailable")?;
            if cancel.load(Ordering::Acquire) {
                return Ok(Outcome::Stopped);
            }
            session.request(Request::Speak, |r| matches!(r, Response::ReceivingData))?;
            session.client.push(Request::SendLines(data));
            session.client.send_next().map_err(error)?;
            *sent = true;
        }
        session
            .client
            .input_source()
            .set_read_timeout(Some(Duration::from_secs(15 * 60)))
            .map_err(error)?;
        // ACK and terminal notifications may arrive in either order. There is
        // exactly one utterance on this connection; foreign client events do not
        // end it. No retries replay an ambiguous SPEAK.
        for _ in 0..64 {
            let reply = session.client.receive_next().map_err(error)?;
            if let Some(reply) = session.observe(reply) {
                match reply {
                    Response::MessageQueued => {
                        if let Some(done) = session.terminal {
                            return Ok(done);
                        }
                    }
                    Response::Canceled => return Ok(Outcome::Stopped),
                    _ => return Err("Unexpected speech playback reply".into()),
                }
            } else if let Some(done) = session.terminal {
                return Ok(done);
            }
        }
        Err("Speech Dispatcher notification limit reached".into())
    }
    async fn stop(control: &Control) -> Result<(), String> {
        let mut socket = control.socket.try_clone().map_err(error)?;
        let gate = control.data_sent.clone();
        let thread = std::thread::Builder::new()
            .name("studio-speech-stop".into())
            .spawn(move || {
                let sent = gate.lock().map_err(|_| "Speech control is unavailable")?;
                if *sent {
                    // The library exposes only a single mutable client. Its blocked
                    // notification reader owns parsing; this fixed command uses a
                    // clone of that exact socket, never a new/global connection.
                    socket.write_all(b"CANCEL self\r\n").map_err(error)?;
                    socket.flush().map_err(error)?;
                } else {
                    let _ = socket.shutdown(Shutdown::Both);
                }
                Ok(())
            })
            .map_err(|_| "Unable to stop the speech worker")?;
        joined(thread).await?
    }
    async fn joined<T>(thread: std::thread::JoinHandle<T>) -> Result<T, String> {
        // Stop and joining do not queue behind Markdown parsing/indexing in the
        // shared CPU pool. There is one playback worker and at most one stopper.
        while !thread.is_finished() {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
        thread
            .join()
            .map_err(|_| "Speech worker stopped unexpectedly".into())
    }
    struct CancelOnDrop(Arc<AtomicBool>);
    impl Drop for CancelOnDrop {
        fn drop(&mut self) {
            self.0.store(true, Ordering::Release);
        }
    }
    pub async fn run(
        text: String,
        language: Language,
        cancel: Arc<AtomicBool>,
    ) -> Result<Outcome, String> {
        let _guard = CancelOnDrop(cancel.clone());
        let data = lines(&text)?;
        if cancel.load(Ordering::Acquire) {
            return Ok(Outcome::Stopped);
        }
        let path = prepare(&cancel).await?;
        let (ready, mut control_rx) = oneshot::channel();
        let (finished, mut result) = oneshot::channel();
        let flag = cancel.clone();
        let thread = std::thread::Builder::new()
            .name("studio-speech".into())
            .spawn(move || {
                let _ = finished.send(worker(path, data, language, flag, ready));
            })
            .map_err(|_| "Unable to start speech worker")?;
        let mut control = None;
        let mut ready_done = false;
        let mut stopping = None;
        let began = std::time::Instant::now();
        let outcome = loop {
            tokio::select! {
                value = &mut control_rx, if !ready_done => { ready_done = true; control = value.ok(); }
                value = &mut result => break value.map_err(|_|"Speech worker stopped".to_owned())?,
                _ = tokio::time::sleep(Duration::from_millis(25)) => {
                    if began.elapsed() > Duration::from_secs(15 * 60) {
                        cancel.store(true, Ordering::Release);
                    }
                    if cancel.load(Ordering::Acquire) && stopping.is_none() {
                        if let Some(control) = control.as_ref() { let _ = stop(control).await; stopping = Some(std::time::Instant::now()); }
                    }
                    if stopping.is_some_and(|time: std::time::Instant|time.elapsed() > Duration::from_secs(3)) {
                        if let Some(control) = control.as_ref() { let _ = control.socket.shutdown(Shutdown::Both); }
                        break Err("Speech playback stop was not confirmed".into());
                    }
                }
            }
        };
        if control.is_none() {
            control = control_rx.try_recv().ok();
        }
        if outcome.is_err() && stopping.is_none() {
            if let Some(control) = control.as_ref() {
                let _ = stop(control).await;
                let _ = control.socket.shutdown(Shutdown::Both);
            }
        }
        // Socket shutdown wakes the notification reader, including when a server
        // disappears. Join off GTK/the async reactor rather than detach it.
        joined(thread).await?;
        if let Some(control) = control.as_mut() {
            control.cleanup = false;
        }
        outcome
    }
}
#[cfg(target_os = "linux")]
pub use linux::run;
#[cfg(not(target_os = "linux"))]
pub async fn run(_: String, _: Language, _: Arc<AtomicBool>) -> Result<Outcome, String> {
    Err("Native speech synthesis is unavailable on this system".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn untrusted_multiline_data_is_dot_stuffed_and_bounded() {
        assert_eq!(
            lines("hello\r\n.\rCANCEL all\n..\n<b>中文😀</b>").unwrap(),
            ["hello", "..", "CANCEL all", "...", "<b>中文😀</b>"]
        );
        assert!(lines(" ").is_err());
        assert!(lines("a\0b").is_err());
        assert!(lines(&"😀".repeat(MAX_CHARS + 1)).is_err());
        assert_eq!(
            lines(&"😀".repeat(MAX_CHARS)).unwrap()[0].chars().count(),
            MAX_CHARS
        );
        assert_eq!(Language::Pinyin.code(), "cmn-LATN-PINYIN");
    }
}
