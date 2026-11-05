//! Linux acceptance host for the same Studio WSL script/authority adapter.
//! This validates the portable transport path, not Windows/WSL execution itself.
#[cfg(target_os = "linux")]
#[allow(dead_code)]
#[path = "../src/backend_runtime.rs"]
mod backend_runtime;
#[cfg(target_os = "linux")]
#[allow(dead_code)]
#[path = "../src/wsl_reader.rs"]
mod wsl_reader;
#[cfg(target_os = "linux")]
fn main() -> std::io::Result<()> {
    if let Some(result) = marknoria::reader::worker_entry() {
        return result;
    }
    use std::{
        io::BufRead,
        sync::{Arc, Mutex},
    };
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.len() < 2 {
        return Err(std::io::Error::other(
            "Pass an authorized project root and file",
        ));
    }
    let directories = Arc::new(Mutex::new(
        args.iter()
            .filter_map(|value| value.strip_prefix("--shared=").map(str::to_owned))
            .collect::<Vec<_>>(),
    ));
    let notes = args
        .iter()
        .find_map(|value| value.strip_prefix("--notes=").map(std::path::PathBuf::from));
    let runtime = tokio::runtime::Runtime::new()?;
    let transport = backend_runtime::BackendRuntime::new(
        std::env::var_os("PATH").unwrap_or_default(),
        backend_runtime::WslSettings::default(),
    );
    let opened = runtime.block_on(wsl_reader::open(
        transport,
        &args[0],
        &args[1],
        directories.clone(),
        notes,
    ))?;
    println!("{}", opened.service.url());
    // Trusted owner controls only: JSON arrays replace the current shared policy;
    // an empty line closes the owned service. No document request can do this.
    for line in std::io::stdin().lock().lines() {
        let line = line?;
        if line.is_empty() {
            break;
        }
        let policy: Vec<String> = serde_json::from_str(&line).map_err(std::io::Error::other)?;
        *directories.lock().unwrap() = policy;
    }
    runtime.block_on(opened.service.close())
}
#[cfg(not(target_os = "linux"))]
fn main() {
    eprintln!("This portable transport acceptance host runs on Linux.");
}
