//! Generate the public synthetic corpus used by headless and browser contracts.
#[path = "../tests/support/markdown.rs"]
mod fixtures;
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let root = std::path::PathBuf::from(
        std::env::args_os()
            .nth(1)
            .ok_or("Pass a private output directory")?,
    );
    std::fs::create_dir_all(&root)?;
    let markdown = root.join("gfm-unicode-crlf.md");
    std::fs::write(&markdown, fixtures::gfm())?;
    println!("{}", serde_json::json!({"markdown": markdown}));
    Ok(())
}
