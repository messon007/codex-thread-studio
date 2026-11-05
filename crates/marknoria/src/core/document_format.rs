//! Shared supported-format registry. Recognizing a file is separate from
//! granting authority to open its path or deciding which operations it permits.
use std::path::Path;

pub const EXTENSIONS: &[&str] = &[
    "md",
    "markdown",
    "mdown",
    "mkd",
    "mdx",
    "csv",
    "tsv",
    "xlsx",
    "pdf",
    "epub",
    "png",
    "apng",
    "jpg",
    "jpeg",
    "gif",
    "webp",
    "svg",
    "html",
    "htm",
    "xhtml",
    "txt",
    "text",
    "json",
    "json5",
    "jsonc",
    "jsonl",
    "ndjson",
    "yaml",
    "yml",
    "toml",
    "xml",
    "plist",
    "rss",
    "atom",
    "ini",
    "cfg",
    "conf",
    "properties",
    "env",
    "log",
    "lock",
    "rs",
    "py",
    "pyi",
    "js",
    "mjs",
    "cjs",
    "ts",
    "tsx",
    "jsx",
    "css",
    "scss",
    "sass",
    "less",
    "c",
    "h",
    "cpp",
    "hpp",
    "cc",
    "cxx",
    "cs",
    "go",
    "java",
    "kt",
    "kts",
    "swift",
    "rb",
    "php",
    "lua",
    "sh",
    "bash",
    "zsh",
    "fish",
    "ps1",
    "bat",
    "sql",
    "diff",
    "patch",
    "adoc",
    "asciidoc",
    "rst",
    "cmake",
    "mk",
    "gradle",
    "groovy",
    "proto",
    "graphql",
    "gql",
    "d2",
    "dist",
    "editorconfig",
    "example",
    "gitattributes",
    "gitignore",
    "in",
    "npmrc",
    "nvmrc",
    "sample",
    "template",
    "tmpl",
];
pub const MIME_TYPES: &[&str] = &[
    "text/plain",
    "text/markdown",
    "text/csv",
    "text/html",
    "application/pdf",
    "application/epub+zip",
    "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
    "image/png",
    "image/apng",
    "image/jpeg",
    "image/gif",
    "image/webp",
    "image/svg+xml",
];

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum DocumentFormat {
    #[default]
    Markdown,
    Csv,
    Tsv,
    Xlsx,
    Text,
    Html,
    Pdf,
    Epub,
    Image,
}

impl DocumentFormat {
    pub fn for_name(name: &str) -> Option<Self> {
        if name.is_empty()
            || matches!(name, "." | "..")
            || name.len() > 255
            || name.contains(['/', '\\', '\0'])
        {
            return None;
        }
        let extension = Path::new(name)
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("");
        match extension.to_ascii_lowercase().as_str() {
            "csv" => Some(Self::Csv),
            "tsv" => Some(Self::Tsv),
            "xlsx" => Some(Self::Xlsx),
            "pdf" => Some(Self::Pdf),
            "epub" => Some(Self::Epub),
            "png" | "apng" | "jpg" | "jpeg" | "gif" | "webp" | "svg" => Some(Self::Image),
            "html" | "htm" | "xhtml" => Some(Self::Html),
            "md" | "markdown" | "mdown" | "mkd" => Some(Self::Markdown),
            "7z" | "a" | "apk" | "avi" | "avif" | "bin" | "bmp" | "bz2" | "class" | "db"
            | "deb" | "dll" | "dmg" | "doc" | "docx" | "eot" | "exe" | "flac" | "gz" | "ico"
            | "iso" | "jar" | "mov" | "mp3" | "mp4" | "o" | "obj" | "ogg" | "otf" | "parquet"
            | "ppt" | "pptx" | "rar" | "rpm" | "so" | "sqlite" | "sqlite3" | "tar" | "tif"
            | "tiff" | "ttf" | "wav" | "wasm" | "webm" | "woff" | "woff2" | "xls" | "xz"
            | "zip" | "zst" => None,
            _ => Some(Self::Text),
        }
    }

    pub fn for_path(path: &Path) -> Option<Self> {
        Self::for_name(path.file_name()?.to_str()?)
    }

    pub fn is_text(self) -> bool {
        matches!(
            self,
            Self::Markdown | Self::Csv | Self::Tsv | Self::Text | Self::Html
        )
    }
    pub fn delimiter(self) -> Option<u8> {
        match self {
            Self::Csv => Some(b','),
            Self::Tsv => Some(b'\t'),
            _ => None,
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            Self::Markdown => "Markdown",
            Self::Csv => "CSV",
            Self::Tsv => "TSV",
            Self::Xlsx => "XLSX",
            Self::Text => "Text",
            Self::Html => "HTML",
            Self::Pdf => "PDF",
            Self::Epub => "EPUB",
            Self::Image => "Image",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_advertised_extension_has_a_case_insensitive_handler() {
        for extension in EXTENSIONS {
            for suffix in [extension.to_string(), extension.to_uppercase()] {
                let name = format!("中文 😀.{suffix}");
                assert!(DocumentFormat::for_name(&name).is_some());
                assert!(
                    DocumentFormat::for_path(&Path::new("folder.with.dots").join(&name)).is_some()
                );
            }
        }
        assert_eq!(
            DocumentFormat::for_name("table.CsV"),
            Some(DocumentFormat::Csv)
        );
    }

    #[test]
    fn recognition_does_not_accept_paths_binary_extensions_or_oversized_names() {
        for name in [
            "",
            "binary.exe",
            "../table.csv",
            "/tmp/table.csv",
            "dir\\table.csv",
            "nul\0.csv",
        ] {
            assert_eq!(DocumentFormat::for_name(name), None, "{name:?}");
        }
        assert_eq!(
            DocumentFormat::for_name(&format!("{}.csv", "a".repeat(252))),
            None
        );
        assert_eq!(
            DocumentFormat::for_name(&format!("{}.csv", "a".repeat(251))),
            Some(DocumentFormat::Csv)
        );
    }
}
