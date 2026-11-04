//! Reading a bounded excerpt is not whole-document syntax validation. Retain
//! Adwaita's native code colors without reporting missing page context as errors.
struct Schemes {
    manager: sourceview::StyleSchemeManager,
    _files: tempfile::TempDir,
}
impl Schemes {
    fn new() -> std::io::Result<Self> {
        let files = tempfile::Builder::new()
            .prefix("studio-preview-styles-")
            .tempdir()?;
        for parent in ["Adwaita", "Adwaita-dark"] {
            let xml = format!(
                "<style-scheme id=\"studio-preview-{parent}\" name=\"Preview {parent}\" version=\"1.0\" parent-scheme=\"{parent}\"><style name=\"def:error\" underline=\"none\"/></style-scheme>"
            );
            std::fs::write(files.path().join(format!("{parent}.xml")), xml)?;
        }
        let manager = sourceview::StyleSchemeManager::new();
        let path = files.path().to_str().ok_or_else(|| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "Preview style path is not UTF-8",
            )
        })?;
        manager.prepend_search_path(path);
        Ok(Self {
            manager,
            _files: files,
        })
    }
}
thread_local! {
    static SCHEMES: Option<Schemes> = Schemes::new().map_err(|error| {
        eprintln!("Native Preview colors: {error}");
    }).ok();
}
pub fn get(dark: bool) -> Option<sourceview::StyleScheme> {
    let parent = if dark { "Adwaita-dark" } else { "Adwaita" };
    SCHEMES
        .with(|schemes| {
            schemes
                .as_ref()
                .and_then(|s| s.manager.scheme(&format!("studio-preview-{parent}")))
        })
        .or_else(|| sourceview::StyleSchemeManager::default().scheme(parent))
}
