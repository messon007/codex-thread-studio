//! Cross-platform Poppler utilities run in owned, deadline-bound subprocesses.
use super::*;
use std::{
    process::{Command, Stdio},
    thread,
    time::Duration,
};
fn program(name: &str) -> PathBuf {
    if let Some(dir) = std::env::var_os("MARKNORIA_POPPLER_DIR") {
        return PathBuf::from(dir).join(if cfg!(windows) {
            format!("{name}.exe")
        } else {
            name.into()
        });
    }
    #[cfg(target_os = "macos")]
    for base in ["/opt/homebrew/bin", "/usr/local/bin"] {
        let path = Path::new(base).join(name);
        if path.is_file() {
            return path;
        }
    }
    name.into()
}
fn run(name: &str, args: &[&std::ffi::OsStr], output: &Path, limit: u64) -> io::Result<()> {
    let log = tempfile::NamedTempFile::new_in(output.parent().unwrap())?;
    let mut command = Command::new(program(name));
    #[cfg(windows)]
    { use std::os::windows::process::CommandExt; command.creation_flags(0x08000000); }
    let mut child = command.args(args)
        .env("LC_ALL", "C")
        .stdin(Stdio::null())
        .stdout(Stdio::from(log.reopen()?))
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| {
            error(format!(
                "{name}: {e}; install Poppler utilities or set MARKNORIA_POPPLER_DIR"
            ))
        })?;
    let deadline = Instant::now();
    loop {
        if let Some(status) = child.try_wait()? {
            if !status.success() {
                return Err(error(format!(
                    "{name} failed; the PDF may be encrypted, malformed or unsupported"
                )));
            }
            break;
        }
        let rss = crate::runtime::process::resident_kib(child.id()).unwrap_or(0);
        if deadline.elapsed() > Duration::from_secs(20)
            || rss > 512 * 1024
            || output.metadata().is_ok_and(|m| m.len() > limit)
            || log.as_file().metadata()?.len() > 128 * 1024
        {
            let _ = child.kill();
            let _ = child.wait();
            return Err(error(
                "PDF worker exceeded its time, memory or output budget",
            ));
        }
        thread::sleep(Duration::from_millis(20));
    }
    if !output.is_file() {
        fs::copy(log.path(), output)?;
    }
    if output.metadata()?.len() > limit {
        return Err(error("PDF output exceeds budget"));
    }
    Ok(())
}
pub(super) fn index(builder: &mut Builder<'_>) -> io::Result<()> {
    let original = builder.directory.join("original.bin");
    let meta = builder.directory.join("pdf-info.txt");
    run("pdfinfo", &[original.as_os_str()], &meta, 128 * 1024)?;
    let info = fs::read_to_string(&meta)?;
    let pages = info
        .lines()
        .find_map(|l| {
            l.strip_prefix("Pages:")
                .and_then(|v| v.trim().parse::<usize>().ok())
        })
        .ok_or_else(|| error("PDF page count unavailable"))?;
    if pages == 0 || pages > 2000 {
        return Err(error("PDF exceeds the 2,000 page budget"));
    }
    let extracted = builder.directory.join("pdf-text.txt");
    run(
        "pdftotext",
        &[
            "-layout".as_ref(),
            "-enc".as_ref(),
            "UTF-8".as_ref(),
            original.as_os_str(),
            extracted.as_os_str(),
        ],
        &extracted,
        crate::core::MAX_SOURCE,
    )?;
    let text = fs::read_to_string(extracted)?;
    let mut contents = text.split('\u{c}');
    for page in 0..pages {
        builder.heading(&format!("Page {}", page + 1), 2)?;
        let id = builder.index.assets.len();
        builder.index.assets.push(Asset {
            alt: format!("Page {}", page + 1),
            width: 1200,
            height: 1600,
            frames: vec![AssetFrame {
                file: format!("asset-{id}-0.png"),
                delay_ms: 100,
            }],
            plays: 1,
            pdf_page: Some(page),
        });
        builder.rich(&builder.figure(id), "")?;
        let source = contents.next().unwrap_or("");
        let mut at = 0;
        while at < source.len() {
            let mut end = (at + 8192).min(source.len());
            while !source.is_char_boundary(end) {
                end -= 1;
            }
            let part = &source[at..end];
            builder.rich(
                &format!(
                    "<details><summary>Page text</summary><pre>{}</pre></details>",
                    escaped(part)
                ),
                part,
            )?;
            at = end;
        }
    }
    // First useful pixels are ready at publication; later pages stay lazy.
    render(
        builder.directory,
        0,
        &builder.directory.join("asset-0-0.png"),
    )?;
    Ok(())
}
pub(super) fn render(directory: &Path, page: usize, target: &Path) -> io::Result<()> {
    let pending = tempfile::Builder::new()
        .prefix("pdf-page-")
        .tempdir_in(directory)?;
    let prefix = pending.path().join("page");
    let output = prefix.with_extension("png");
    let original = directory.join("original.bin");
    let number = (page + 1).to_string();
    run(
        "pdftoppm",
        &[
            "-f".as_ref(),
            number.as_ref(),
            "-l".as_ref(),
            number.as_ref(),
            "-singlefile".as_ref(),
            "-scale-to".as_ref(),
            "1800".as_ref(),
            "-png".as_ref(),
            original.as_os_str(),
            prefix.as_os_str(),
        ],
        &output,
        20 * 1024 * 1024,
    )?;
    let (w, h) = image::image_dimensions(&output).map_err(error)?;
    if w > 1800 || h > 1800 {
        return Err(error("PDF raster exceeds its pixel budget"));
    }
    // Bound retained raster files without touching any other application's data.
    let mut retained = fs::read_dir(directory)?
        .filter_map(Result::ok)
        .filter(|e| {
            e.file_name().to_string_lossy().starts_with("asset-")
                && e.path().extension().is_some_and(|e| e == "png")
        })
        .collect::<Vec<_>>();
    retained.sort_by_key(|e| e.metadata().and_then(|m| m.modified()).ok());
    for old in retained.iter().take(retained.len().saturating_sub(7)) {
        if old.path() != target {
            fs::remove_file(old.path())?;
        }
    }
    fs::rename(output, target)?;
    Ok(())
}
