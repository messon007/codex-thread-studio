//! Studio-owned Linux/WSL file authority and transport; shared SDK owns reading.
use crate::backend_runtime::{BackendRuntime, WslEnvironment};
use marknoria::{
    reader::DocumentFormat,
    web::{DiagramMode, HostImage, Service, ServiceOptions},
};
use sha2::{Digest, Sha256};
use std::{
    io,
    path::PathBuf,
    sync::{Arc, Mutex},
};

const FILE_PATH_ENCODE: &percent_encoding::AsciiSet = &percent_encoding::CONTROLS
    .add(b'%')
    .add(b'#')
    .add(b'?')
    .add(b'\\');

pub struct Opened {
    pub service: Service,
    pub root: String,
    pub path: String,
    pub document_root: String,
    pub relative_path: String,
    pub size: u64,
    pub format: DocumentFormat,
}

fn invalid(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}
fn shared(directories: &Mutex<Vec<String>>) -> io::Result<Vec<String>> {
    directories
        .lock()
        .map(|value| value.clone())
        .map_err(|_| io::Error::other("Shared directory policy unavailable"))
}

/// URI resolution chooses a candidate only. The existing file script performs
/// canonical project/shared authorization before any candidate bytes are read.
pub fn resolve_reference(source: &str, document_root: &str, target: &str) -> io::Result<String> {
    if target.len() > 8192 || target.chars().any(char::is_control) {
        return Err(invalid("Invalid WSL document reference"));
    }
    let target = target.replace('\\', "/");
    let mut base = url::Url::parse("file:///").map_err(io::Error::other)?;
    let target = if target.starts_with('/') && !target.starts_with("//") {
        base.set_path(
            &percent_encoding::utf8_percent_encode(
                &format!("{}/", document_root.trim_end_matches('/')),
                FILE_PATH_ENCODE,
            )
            .to_string(),
        );
        target.trim_start_matches('/')
    } else {
        base.set_path(&percent_encoding::utf8_percent_encode(source, FILE_PATH_ENCODE).to_string());
        target.as_str()
    };
    let target = base.join(target).map_err(io::Error::other)?;
    if target.scheme() != "file" || target.host_str().is_some_and(|host| host != "localhost") {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "WSL references require an authorized local file",
        ));
    }
    let path = percent_encoding::percent_decode_str(target.path())
        .decode_utf8()
        .map_err(io::Error::other)?
        .into_owned();
    if !path.starts_with('/') || path.contains('\0') {
        return Err(invalid("Invalid WSL reference path"));
    }
    Ok(path)
}

pub fn resource_identity(environment: &WslEnvironment, path: &str) -> io::Result<String> {
    let key = serde_json::to_vec(&(
        "wsl-document-v1",
        &environment.distribution,
        environment.uid,
        path,
    ))
    .map_err(io::Error::other)?;
    let digest = Sha256::digest(key);
    Ok(format!(
        "wsl:v1:{}",
        digest
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>()
    ))
}

pub async fn open(
    runtime: BackendRuntime,
    root: &str,
    path: &str,
    directories: Arc<Mutex<Vec<String>>>,
    notes: Option<PathBuf>,
) -> io::Result<Opened> {
    let file = runtime
        .read_wsl_file(
            root,
            path,
            marknoria::reader::Budgets::default().source_bytes,
            &shared(&directories)?,
        )
        .await?;
    let environment = file
        .environment
        .clone()
        .ok_or_else(|| invalid("WSL source lacks actual environment identity"))?;
    if file.unchanged {
        return Err(invalid("Opening a document requires original bytes"));
    }
    let name = file.path.rsplit('/').next().unwrap_or_default().to_owned();
    let format = DocumentFormat::for_name(&name)
        .ok_or_else(|| invalid("Unsupported WSL document format"))?;
    // This was explicitly unavailable in Studio's existing WSL transport.
    if format == DocumentFormat::Epub {
        return Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "WSL EPUB reading-state transport is unavailable",
        ));
    }
    let pinned = runtime.pinned_wsl(&environment);
    let identity = resource_identity(&environment, &file.path)?;
    let (owner_root, owner_source, document_root) = (
        file.root.clone(),
        file.path.clone(),
        file.document_root.clone(),
    );
    let handle = tokio::runtime::Handle::current();
    let mut options = ServiceOptions::default()
        .resource_id(identity)
        .diagrams(DiagramMode::Browser)
        .reference_image_loader(move |target, hint, limit, cancelled| {
            let path = resolve_reference(&owner_source, &document_root, target)?;
            let current = shared(&directories)?;
            let image = handle.block_on(pinned.read_wsl_file_cached(
                &owner_root,
                &path,
                limit,
                &current,
                hint,
                Some(cancelled),
            ))?;
            if image.root != owner_root || image.environment.as_ref() != Some(&environment) {
                return Err(io::Error::new(
                    io::ErrorKind::PermissionDenied,
                    "WSL source environment or project root changed",
                ));
            }
            let identity = image
                .fingerprint
                .ok_or_else(|| invalid("WSL image lacks source fingerprint"))?;
            Ok(HostImage {
                name: image.path.rsplit('/').next().unwrap_or_default().to_owned(),
                identity,
                bytes: if image.unchanged {
                    None
                } else {
                    Some(image.content)
                },
            })
        });
    if let Some(notes) = notes {
        options = options.notes_directory(notes);
    }
    let service = Service::from_bytes(name, file.content, options).await?;
    Ok(Opened {
        service,
        root: file.root,
        path: file.path,
        document_root: file.document_root,
        relative_path: file.relative_path,
        size: file.size,
        format,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn reference_candidates_preserve_unicode_root_relative_and_explicit_file_semantics() {
        let source = "/project/docs/notes.md";
        assert_eq!(
            resolve_reference(source, "/project", "../images/%E4%B8%AD%E6%96%87%20a.png#x")
                .unwrap(),
            "/project/images/\u{4e2d}\u{6587} a.png"
        );
        assert_eq!(
            resolve_reference(source, "/project", "/images/a.png").unwrap(),
            "/project/images/a.png"
        );
        assert_eq!(
            resolve_reference(source, "/project", "..\\images\\a.png").unwrap(),
            "/project/images/a.png"
        );
        assert_eq!(
            resolve_reference(source, "/project", "file:///shared/a.png").unwrap(),
            "/shared/a.png"
        );
        assert_eq!(
            resolve_reference("/project/literal%20dir/notes.md", "/project", "next.md").unwrap(),
            "/project/literal%20dir/next.md"
        );
        assert!(resolve_reference(source, "/project", "https://example.invalid/a.png").is_err());
        assert!(resolve_reference(source, "/project", "//untrusted/a.png").is_err());
        assert!(resolve_reference(source, "/project", "a%00.png").is_err());
    }
    #[test]
    fn logical_identity_uses_actual_environment_and_path_not_content_or_temporary_names() {
        let environment = WslEnvironment {
            distribution: "one".into(),
            uid: 1000,
            user: "reader".into(),
        };
        let identity = resource_identity(&environment, "/project/data.csv").unwrap();
        assert_eq!(
            identity,
            resource_identity(&environment, "/project/data.csv").unwrap()
        );
        assert_ne!(
            identity,
            resource_identity(
                &WslEnvironment {
                    distribution: "two".into(),
                    ..environment.clone()
                },
                "/project/data.csv"
            )
            .unwrap()
        );
        assert_ne!(
            identity,
            resource_identity(
                &WslEnvironment {
                    uid: 1001,
                    ..environment.clone()
                },
                "/project/data.csv"
            )
            .unwrap()
        );
        assert_ne!(
            identity,
            resource_identity(&environment, "/project/other.csv").unwrap()
        );
    }
}
