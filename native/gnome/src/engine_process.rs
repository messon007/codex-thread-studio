//! Bounded process transport shared by native render engines.
use std::{
    io::{Read, Write},
    process::{Command, Stdio},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    thread,
    time::{Duration, Instant},
};
pub fn bytes(
    command: &mut Command,
    input: Vec<u8>,
    cancel: Arc<AtomicBool>,
    timeout: Duration,
    limit: usize,
) -> Result<Vec<u8>, String> {
    if cancel.load(Ordering::Relaxed) {
        return Err("Rendering cancelled".into());
    }
    let mut child = command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| e.to_string())?;
    let mut stdin = child.stdin.take().unwrap();
    let input = thread::spawn(move || stdin.write_all(&input));
    let exceeded = Arc::new(AtomicBool::new(false));
    let budget = exceeded.clone();
    let mut stdout = child.stdout.take().unwrap();
    let output = thread::spawn(move || {
        let mut bytes = Vec::new();
        let result = stdout
            .by_ref()
            .take(limit as u64 + 1)
            .read_to_end(&mut bytes);
        if bytes.len() > limit {
            budget.store(true, Ordering::Relaxed);
        }
        result.map(|_| bytes)
    });
    let mut stderr = child.stderr.take().unwrap();
    let errors = thread::spawn(move || {
        let mut bytes = Vec::new();
        let _ = stderr.by_ref().take(16384).read_to_end(&mut bytes);
        bytes
    });
    let start = Instant::now();
    let status = loop {
        if cancel.load(Ordering::Relaxed)
            || start.elapsed() > timeout
            || exceeded.load(Ordering::Relaxed)
        {
            let _ = child.kill();
            let _ = child.wait();
            break Err("Render engine exceeded its time/output budget or was cancelled".to_owned());
        }
        match child.try_wait() {
            Ok(Some(status)) => break Ok(status),
            Ok(None) => thread::sleep(Duration::from_millis(5)),
            Err(e) => {
                let _ = child.kill();
                let _ = child.wait();
                break Err(e.to_string());
            }
        }
    };
    let _ = input.join();
    let output = output
        .join()
        .map_err(|_| "Render output worker stopped")?
        .map_err(|e| e.to_string())?;
    let errors = errors.join().unwrap_or_default();
    let status = status?;
    if !status.success() {
        let errors = String::from_utf8_lossy(&errors);
        return Err(if errors.trim().is_empty() {
            format!("Render engine failed: {status}")
        } else {
            errors.trim().to_owned()
        });
    }
    if output.len() > limit {
        return Err("Rendered SVG exceeds its output budget".into());
    }
    Ok(output)
}
pub fn svg(
    command: &mut Command,
    input: Vec<u8>,
    cancel: Arc<AtomicBool>,
    timeout: Duration,
    limit: usize,
) -> Result<String, String> {
    let output = bytes(command, input, cancel, timeout, limit)?;
    let text = String::from_utf8(output).map_err(|_| "Render engine returned invalid UTF-8")?;
    if !text.contains("<svg") {
        return Err("Render engine returned no SVG".into());
    }
    Ok(text)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn slow_or_excessive_engines_are_stopped_and_reaped() {
        let start = Instant::now();
        let mut command = Command::new("/bin/sleep");
        command.arg("30");
        assert!(svg(
            &mut command,
            Vec::new(),
            Default::default(),
            Duration::from_millis(30),
            1024
        )
        .is_err());
        assert!(start.elapsed() < Duration::from_secs(2));
        let mut command = Command::new("/usr/bin/python3");
        command.args(["-c", "print('<svg>'+'x'*10000+'</svg>')"]);
        assert!(svg(
            &mut command,
            Vec::new(),
            Default::default(),
            Duration::from_secs(2),
            100
        )
        .is_err());
        let cancel = Arc::new(AtomicBool::new(true));
        assert!(svg(
            &mut Command::new("/path-not-started"),
            Vec::new(),
            cancel,
            Duration::from_secs(1),
            100
        )
        .unwrap_err()
        .contains("cancelled"));
    }
}
