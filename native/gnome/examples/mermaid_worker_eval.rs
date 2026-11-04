//! Probe the actual parent sandbox/transport without GTK or any live backend.
#[path = "../src/byte_cache.rs"]
mod byte_cache;
#[path = "../src/engine_process.rs"]
mod engine_process;
#[path = "../src/mermaid_fonts.rs"]
mod mermaid_fonts;
#[path = "../src/mermaid_worker.rs"]
mod mermaid_worker;
use std::{
    io::Read,
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::Duration,
};
fn main() {
    let result = (|| -> Result<(), String> {
        let mut args = std::env::args_os().skip(1);
        let worker = PathBuf::from(args.next().ok_or("worker path required")?);
        let mut timeout = None;
        let mut cancel_ms = None;
        let mut fonts = Vec::new();
        let mut automatic = false;
        let mut plan_only = false;
        let mut repeats = 1;
        while let Some(flag) = args.next() {
            if flag == "--font-repeat" {
                repeats = args
                    .next()
                    .ok_or("repeat required")?
                    .to_str()
                    .ok_or("invalid repeat")?
                    .parse::<usize>()
                    .map_err(|e| e.to_string())?
                    .clamp(1, 8);
            } else if flag == "--auto-fonts" {
                automatic = true;
            } else if flag == "--font-plan" {
                plan_only = true;
            } else if flag == "--timeout-ms" {
                timeout = Some(
                    args.next()
                        .ok_or("timeout required")?
                        .to_str()
                        .ok_or("invalid timeout")?
                        .parse::<u64>()
                        .map_err(|e| e.to_string())?,
                );
            } else if flag == "--cancel-ms" {
                cancel_ms = Some(
                    args.next()
                        .ok_or("cancel delay required")?
                        .to_str()
                        .ok_or("invalid delay")?
                        .parse::<u64>()
                        .map_err(|e| e.to_string())?,
                );
            } else if flag == "--font" {
                fonts.push(PathBuf::from(args.next().ok_or("font path required")?));
            } else {
                return Err("Unknown probe argument".into());
            }
        }
        let mut input = Vec::new();
        std::io::stdin()
            .take(mermaid_worker::protocol::INPUT_LIMIT as u64 + 1)
            .read_to_end(&mut input)
            .map_err(|e| e.to_string())?;
        if input.len() > mermaid_worker::protocol::INPUT_LIMIT {
            return Err("Probe input exceeds budget".into());
        }
        let request: mermaid_worker::protocol::Request =
            serde_json::from_slice(&input).map_err(|e| e.to_string())?;
        let cancel = Arc::new(AtomicBool::new(false));
        if let Some(delay) = cancel_ms {
            let token = cancel.clone();
            std::thread::spawn(move || {
                std::thread::sleep(Duration::from_millis(delay));
                token.store(true, Ordering::Relaxed);
            });
        }
        if plan_only {
            let plan = mermaid_worker::font_plan(&worker, &request, cancel)?;
            println!(
                "{}",
                serde_json::to_string(&plan).map_err(|e| e.to_string())?
            );
            return Ok(());
        }
        if automatic {
            let mut selection = None;
            let mut timings = Vec::new();
            for _ in 0..repeats {
                let start = std::time::Instant::now();
                let next = mermaid_fonts::prepare(&worker, &request, cancel.clone())?;
                timings.push(
                    serde_json::json!({"us":start.elapsed().as_micros(),"cacheHit":next.cache_hit}),
                );
                selection = Some(next);
            }
            let selection = selection.unwrap();
            eprintln!(
                "{}",
                serde_json::json!({"fontFingerprint":selection.fingerprint,"fontDefaults":selection.defaults,"fontFiles":selection.files,"fontPrepare":timings})
            );
            let svg = selection.render(&worker, &request, cancel)?;
            print!("{svg}");
            return Ok(());
        }
        let svg = if let Some(ms) = timeout {
            mermaid_worker::render_until(
                &worker,
                &fonts,
                &request,
                cancel,
                Duration::from_millis(ms),
            )
        } else {
            mermaid_worker::render(&worker, &fonts, &request, cancel)
        }?;
        print!("{svg}");
        Ok(())
    })();
    if let Err(error) = result {
        eprintln!("{error}");
        std::process::exit(1);
    }
}
