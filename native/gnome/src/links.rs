//! Classify clicked references without launching arbitrary URI schemes.
use std::path::{Path, PathBuf};

#[derive(Debug, PartialEq)]
pub enum Target {
    Document {
        path: PathBuf,
        line: Option<usize>,
        heading: Option<String>,
    },
    External(String),
    Heading(String),
}

pub fn resolve(reference: &str, project: &Path, document: Option<&Path>) -> Result<Target, String> {
    let reference = reference.trim();
    if reference.is_empty() || reference.chars().any(char::is_control) {
        return Err("Invalid link".into());
    }
    if let Some(heading) = reference.strip_prefix('#') {
        return Ok(Target::Heading(decode(heading)?));
    }
    let fragment;
    let before_fragment = reference.split('#').next().unwrap_or(reference);
    let (local_path, local_line) = line_suffix(before_fragment);
    let local_line_reference = local_line.is_some()
        && !local_path.contains("://")
        && (Path::new(local_path).extension().is_some()
            || local_path.contains('/')
            || matches!(
                local_path,
                "Makefile" | "Dockerfile" | "Justfile" | "LICENSE"
            ));
    let parsed = if local_line_reference {
        None
    } else {
        url::Url::parse(reference).ok()
    };
    let path = if let Some(url) = parsed {
        match url.scheme() {
            "http" | "https" => {
                let local = matches!(
                    url.host_str(),
                    Some("127.0.0.1" | "localhost" | "[::1]" | "::1")
                );
                if !local || !document_path(&decode(url.path())?) {
                    return Ok(Target::External(url.into()));
                }
                fragment = url.fragment().map(str::to_owned);
                decode(url.path())?
            }
            "file" => {
                fragment = url.fragment().map(str::to_owned);
                url.to_file_path()
                    .map_err(|_| "Only local file links are supported")?
                    .to_string_lossy()
                    .into_owned()
            }
            _ => return Err("This link scheme is not supported".into()),
        }
    } else {
        let (path, hash) = reference
            .split_once('#')
            .map_or((reference, None), |(path, hash)| (path, Some(hash)));
        fragment = hash.map(str::to_owned);
        decode(path)?
    };
    let (path, suffix_line) = line_suffix(&path);
    let (line, heading) = if let Some(fragment) = fragment {
        let fragment = decode(&fragment)?;
        match fragment
            .strip_prefix('L')
            .unwrap_or(&fragment)
            .parse::<usize>()
            .ok()
            .filter(|line| *line > 0)
        {
            Some(line) => (Some(line), None),
            None => (suffix_line, Some(fragment)),
        }
    } else {
        (suffix_line, None)
    };
    let path = Path::new(path.strip_prefix('@').unwrap_or(path));
    let base = document.and_then(Path::parent).unwrap_or(project);
    let path = if path.is_absolute() {
        path.to_owned()
    } else {
        base.join(path)
    };
    // Workspace/shared-root access is checked again by the Rust file service;
    // a model-provided absolute path never silently chooses its own read root.
    Ok(Target::Document {
        path,
        line,
        heading,
    })
}

fn decode(value: &str) -> Result<String, String> {
    let decoded = percent_encoding::percent_decode_str(value)
        .decode_utf8()
        .map(|value| value.into_owned())
        .map_err(|_| "Link path is not valid UTF-8".to_owned())?;
    if decoded.chars().any(char::is_control) {
        return Err("Invalid link path".into());
    }
    Ok(decoded)
}
fn line_suffix(path: &str) -> (&str, Option<usize>) {
    if let Some((path, line)) = path.rsplit_once(':') {
        if let Ok(line) = line.parse::<usize>() {
            if line > 0 {
                if let Some((file, previous)) = path.rsplit_once(':') {
                    if let Some(previous) = previous.parse::<usize>().ok().filter(|line| *line > 0)
                    {
                        return (file, Some(previous));
                    }
                }
                return (path, Some(line));
            }
        }
    }
    (path, None)
}
fn document_path(path: &str) -> bool {
    let (path, _) = line_suffix(path);
    Path::new(path)
        .extension()
        .and_then(|value| value.to_str())
        .is_some_and(|extension| {
            matches!(
                extension.to_ascii_lowercase().as_str(),
                "md" | "markdown"
                    | "mdown"
                    | "epub"
                    | "pdf"
                    | "png"
                    | "jpg"
                    | "jpeg"
                    | "gif"
                    | "webp"
                    | "svg"
                    | "csv"
                    | "tsv"
                    | "xlsx"
            )
        })
}

pub fn heading_slug(label: &str) -> String {
    label
        .to_lowercase()
        .chars()
        .filter_map(|character| {
            if character.is_whitespace() {
                Some('-')
            } else if character.is_alphanumeric() || matches!(character, '-' | '_') {
                Some(character)
            } else {
                None
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn local_studio_links_keep_line_hints_and_workspace_context() {
        assert_eq!(
            resolve("README.md:1", Path::new("/work"), None).unwrap(),
            Target::Document {
                path: "/work/README.md".into(),
                line: Some(1),
                heading: None
            }
        );
        assert_eq!(
            resolve("src/lib.rs:20:3", Path::new("/work"), None).unwrap(),
            Target::Document {
                path: "/work/src/lib.rs".into(),
                line: Some(20),
                heading: None
            }
        );
        assert_eq!(
            resolve(
                "http://127.0.0.1:38669/work/README.md:1",
                Path::new("/work"),
                None
            )
            .unwrap(),
            Target::Document {
                path: "/work/README.md".into(),
                line: Some(1),
                heading: None
            }
        );
        assert_eq!(
            resolve(
                "../%E4%B8%AD%E6%96%87.md#L20",
                Path::new("/work"),
                Some(Path::new("/work/docs/read.md"))
            )
            .unwrap(),
            Target::Document {
                path: "/work/docs/../中文.md".into(),
                line: Some(20),
                heading: None
            }
        );
        assert_eq!(
            resolve("https://example.com/README.md", Path::new("/work"), None).unwrap(),
            Target::External("https://example.com/README.md".into())
        );
        assert!(resolve("javascript:alert(1)", Path::new("/work"), None).is_err());
        assert!(resolve("data:text/html,hi", Path::new("/work"), None).is_err());
    }
}
