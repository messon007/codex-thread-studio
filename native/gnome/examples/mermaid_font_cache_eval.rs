//! Private font fixture: validate warm reuse and content identity after a font changes.
#[path = "../src/byte_cache.rs"]
mod byte_cache;
#[path = "../src/engine_process.rs"]
mod engine_process;
#[path = "../src/mermaid_fonts.rs"]
mod mermaid_fonts;
#[path = "../src/mermaid_worker.rs"]
mod mermaid_worker;
use std::{io::Write, path::PathBuf};
fn main() {
    let result = (|| -> Result<(), String> {
        let mut args = std::env::args_os().skip(1);
        let worker = PathBuf::from(args.next().ok_or("worker required")?);
        let root = PathBuf::from(args.next().ok_or("new fixture directory required")?);
        std::fs::create_dir(&root).map_err(|e| e.to_string())?;
        let fonts = root.join("fonts");
        std::fs::create_dir(&fonts).map_err(|e| e.to_string())?;
        let font = fonts.join("DejaVuSans.ttf");
        std::fs::copy("/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf", &font)
            .map_err(|e| e.to_string())?;
        let escape = |p: &std::path::Path| {
            p.to_string_lossy()
                .replace('&', "&amp;")
                .replace('<', "&lt;")
                .replace('>', "&gt;")
        };
        let config = root.join("fonts.conf");
        std::fs::write(
            &config,
            format!(
                "<fontconfig><dir>{}</dir><cachedir>{}</cachedir></fontconfig>",
                escape(&fonts),
                escape(&root.join("cache"))
            ),
        )
        .map_err(|e| e.to_string())?;
        // This standalone single-threaded probe sets its configuration before any
        // transport threads start; the running desktop application's environment is untouched.
        unsafe {
            std::env::set_var("FONTCONFIG_FILE", &config);
            std::env::remove_var("FONTCONFIG_PATH");
        }
        let request = mermaid_worker::protocol::Request {
            version: mermaid_worker::protocol::VERSION,
            source: "flowchart LR\nA[Hello] --> B[World]".into(),
            dark: false,
            style: mermaid_worker::protocol::Style {
                font_family: "DejaVu Sans".into(),
                font_family_mono: "DejaVu Sans".into(),
                ..Default::default()
            },
            font_defaults: None,
        };
        let first = mermaid_fonts::prepare(&worker, &request, Default::default())?;
        let second = mermaid_fonts::prepare(&worker, &request, Default::default())?;
        assert!(!first.cache_hit && second.cache_hit);
        assert_eq!(first.fingerprint, second.fingerprint);
        assert_eq!(
            first.files,
            vec![font.canonicalize().map_err(|e| e.to_string())?]
        );
        let size = font.metadata().map_err(|e| e.to_string())?.len();
        std::fs::OpenOptions::new()
            .append(true)
            .open(&font)
            .map_err(|e| e.to_string())?
            .write_all(&[0; 16])
            .map_err(|e| e.to_string())?;
        assert!(first.validate_current().is_err());
        let changed = mermaid_fonts::prepare(&worker, &request, Default::default())?;
        assert!(!changed.cache_hit);
        assert_ne!(first.fingerprint, changed.fingerprint);
        std::fs::OpenOptions::new()
            .write(true)
            .open(&font)
            .map_err(|e| e.to_string())?
            .set_len(size)
            .map_err(|e| e.to_string())?;
        let restored = mermaid_fonts::prepare(&worker, &request, Default::default())?;
        assert!(!restored.cache_hit);
        assert_eq!(first.fingerprint, restored.fingerprint);
        println!(
            "{}",
            serde_json::json!({"status":"passed","warmCacheReuse":true,"changedFontInvalidates":true,"restoredBytesRestoreIdentity":true,"stalePreparedSelectionRejected":true,"onlyOwnedFontModified":font})
        );
        Ok(())
    })();
    if let Err(e) = result {
        eprintln!("{e}");
        std::process::exit(1);
    }
}
