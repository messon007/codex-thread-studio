use std::{
    fs,
    io::Read,
    sync::atomic::{AtomicBool, Ordering},
    thread,
    time::{Duration, Instant},
};

pub(super) fn supervise(
    child: &mut std::process::Child,
    cancelled: &AtomicBool,
) -> Result<(), String> {
    let stderr = child.stderr.take();
    let diagnostic = thread::spawn(move || {
        let mut bytes = Vec::new();
        if let Some(stderr) = stderr {
            let _ = stderr.take(4096).read_to_end(&mut bytes);
        }
        String::from_utf8_lossy(&bytes).into_owned()
    });
    let started = Instant::now();
    let status = loop {
        let resident = fs::read_to_string(format!("/proc/{}/status", child.id()))
            .ok()
            .and_then(|status| {
                status.lines().find_map(|line| {
                    line.strip_prefix("VmRSS:")?
                        .split_whitespace()
                        .next()?
                        .parse::<usize>()
                        .ok()
                })
            });
        let error = if cancelled.load(Ordering::Relaxed) {
            Some("Document closed")
        } else if started.elapsed() > Duration::from_secs(60)
            || resident.is_some_and(|kib| kib > 1024 * 1024)
        {
            Some("Indexer exceeded 60 seconds or the Linux 1 GiB RSS budget")
        } else {
            None
        };
        if let Some(error) = error {
            let _ = child.kill();
            let _ = child.wait();
            break Err(error.to_owned());
        }
        match child.try_wait() {
            Ok(Some(status)) if status.success() => break Ok(()),
            Ok(Some(status)) => break Err(format!("Indexing failed ({status})")),
            Err(error) => {
                let _ = child.kill();
                let _ = child.wait();
                break Err(error.to_string());
            }
            Ok(None) => thread::sleep(Duration::from_millis(50)),
        }
    };
    let diagnostic = diagnostic.join().unwrap_or_default();
    status.map_err(|error| {
        format!(
            "{error}: {}",
            diagnostic.chars().take(800).collect::<String>()
        )
    })
}
