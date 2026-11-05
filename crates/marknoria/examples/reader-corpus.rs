//! Generate the public synthetic corpus used by headless and browser contracts.
#[path = "../tests/support/markdown.rs"]
mod fixtures;
#[path = "../tests/support/references.rs"]
mod references;
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let root = std::path::PathBuf::from(
        std::env::args_os()
            .nth(1)
            .ok_or("Pass a private output directory")?,
    );
    std::fs::create_dir_all(&root)?;
    let markdown = root.join("gfm-unicode-crlf.md");
    std::fs::write(&markdown, fixtures::gfm())?;
    let raw = root.join("raw-resources.md");
    std::fs::write(&raw,references::raw_html_document())?;
    use base64::Engine;
    std::fs::write(root.join("中文 pic.png"),base64::engine::general_purpose::STANDARD.decode("iVBORw0KGgoAAAANSUhEUgAAAAIAAAABCAYAAAD0In+KAAAADklEQVR4nGP4z8AAQv8BD/kD/YURmXYAAAAASUVORK5CYII=")?)?;
    std::fs::write(root.join("next.md"),"# Next document\n")?;
    std::fs::create_dir_all(root.join("images"))?;
    std::fs::copy(root.join("中文 pic.png"),root.join("images/picture.png"))?;
    let source_image=root.join("source-resolution.png");references::source_image().save(&source_image)?;
    let resolution=root.join("source-resolution.md");
    std::fs::write(&resolution,"# Source pixels\n\n![Markdown](source-resolution.png)\n\n<img src=\"source-resolution.png\" alt=\"HTML pixels\">\n")?;
    println!("{}", serde_json::json!({"markdown": markdown,"rawMarkdown":raw,"sourceImage":source_image,"resolutionMarkdown":resolution}));
    Ok(())
}
