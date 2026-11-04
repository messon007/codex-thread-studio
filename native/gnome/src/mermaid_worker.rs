//! Parent-side process boundary; parsing/layout remain in the bundled worker.
//! Invoke from the existing single diagram CPU lane, never from GTK callbacks.
use std::{
    path::{Path, PathBuf},
    process::Command,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::Duration,
};
#[path = "../../mermaid-worker/src/protocol.rs"]
pub mod protocol;

pub fn render(
    worker: &Path,
    fonts: &[PathBuf],
    request: &protocol::Request,
    cancel: Arc<AtomicBool>,
) -> Result<String, String> {
    render_until(worker, fonts, request, cancel, Duration::from_secs(4))
}

pub fn font_plan(
    worker: &Path,
    request: &protocol::Request,
    cancel: Arc<AtomicBool>,
) -> Result<protocol::FontPlan, String> {
    let bytes = transport(worker, &[], request, cancel, Duration::from_secs(4), true)?;
    let plan: protocol::FontPlan = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
    if plan.version != protocol::VERSION
        || plan.families.len() > 16
        || plan.characters.len() > 4096
        || plan
            .families
            .iter()
            .chain([&plan.primary, &plan.mono])
            .any(|s| s.is_empty() || s.len() > 8192 || s.chars().any(char::is_control))
        || plan.characters.iter().any(|c| char::from_u32(*c).is_none())
    {
        return Err("Native Mermaid returned an invalid font plan".into());
    }
    Ok(plan)
}

pub(crate) fn render_until(
    worker: &Path,
    fonts: &[PathBuf],
    request: &protocol::Request,
    cancel: Arc<AtomicBool>,
    timeout: Duration,
) -> Result<String, String> {
    let output = transport(worker, fonts, request, cancel, timeout, false)?;
    let text = String::from_utf8(output).map_err(|_| "Mermaid returned invalid UTF-8")?;
    if !text.contains("<svg") {
        return Err("Mermaid returned no SVG".into());
    }
    protocol::math_labels(&text)?;
    Ok(text)
}

fn transport(
    worker: &Path,
    fonts: &[PathBuf],
    request: &protocol::Request,
    cancel: Arc<AtomicBool>,
    timeout: Duration,
    plan: bool,
) -> Result<Vec<u8>, String> {
    request.validate()?;
    if cancel.load(Ordering::Relaxed) {
        return Err("Mermaid rendering cancelled".into());
    }
    if (!plan && fonts.is_empty()) || fonts.len() > protocol::FONT_LIMIT {
        return Err("Mermaid requires one to twenty-four selected fonts".into());
    }
    let worker = worker
        .canonicalize()
        .map_err(|_| "Bundled native Mermaid worker is unavailable")?;
    if !worker.is_file() {
        return Err("Bundled native Mermaid worker is not a file".into());
    }
    let mut total = 0u64;
    let mut selected = Vec::new();
    for file in fonts {
        let file = file
            .canonicalize()
            .map_err(|_| "Selected Mermaid font is unavailable")?;
        let stat = file.metadata().map_err(|e| e.to_string())?;
        total = total.saturating_add(stat.len());
        if !stat.is_file() || total > protocol::FONT_BYTES {
            return Err("Selected Mermaid fonts exceed their file/64 MiB budget".into());
        }
        selected.push(file);
    }
    let input = serde_json::to_vec(request).map_err(|e| e.to_string())?;
    if input.len() > protocol::INPUT_LIMIT {
        return Err("Serialized Mermaid request exceeds 800 KiB".into());
    }
    // Fonts are mounted by ordinal, with no host pathname in diagram input. No
    // home/profile/workspace files, system font discovery or network are exposed.
    let mut command = Command::new("/usr/bin/bwrap");
    command.env_clear().args([
        "--unshare-all",
        "--die-with-parent",
        "--new-session",
        "--clearenv",
        "--setenv",
        "LANG",
        "C.UTF-8",
        "--setenv",
        "HOME",
        "/nonexistent",
        "--ro-bind",
        "/usr/lib",
        "/usr/lib",
        "--symlink",
        "usr/lib",
        "/lib",
        "--symlink",
        "usr/lib64",
        "/lib64",
        "--proc",
        "/proc",
        "--dev",
        "/dev",
        "--tmpfs",
        "/tmp",
        "--dir",
        "/fonts",
        "--dir",
        "/app",
        "--chdir",
        "/tmp",
    ]);
    if Path::new("/usr/lib64").is_dir() {
        command.args(["--ro-bind", "/usr/lib64", "/usr/lib64"]);
    }
    if Path::new("/etc/ld.so.cache").is_file() {
        command.args(["--ro-bind", "/etc/ld.so.cache", "/etc/ld.so.cache"]);
    }
    command.arg("--ro-bind").arg(worker).arg("/app/mermaid");
    for (i, file) in selected.iter().enumerate() {
        command
            .arg("--ro-bind")
            .arg(file)
            .arg(format!("/fonts/{i}"));
    }
    command.args(["--", "/app/mermaid"]);
    if plan {
        command.arg("--font-plan");
    }
    for i in 0..selected.len() {
        command.arg("--font").arg(format!("/fonts/{i}"));
    }
    crate::engine_process::bytes(
        &mut command,
        input,
        cancel,
        timeout,
        if plan { 65536 } else { protocol::SVG_LIMIT },
    )
    .map_err(|e| format!("Mermaid: {e}"))
}
