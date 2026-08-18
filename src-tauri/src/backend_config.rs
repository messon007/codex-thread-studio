use std::collections::BTreeSet;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

const MAX_CONFIG_BYTES: u64 = 256 * 1024;
const MAX_CONFIGURED_BACKENDS: usize = 16;

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BackendDescriptor {
    pub id: String,
    pub name: String,
    pub tag: String,
    pub kind: &'static str,
    pub adapter: &'static str,
    pub info_path: String,
    pub socket_path: String,
    pub protocol: &'static str,
    pub transport: &'static str,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ConfiguredCodexBackend {
    pub descriptor: BackendDescriptor,
    pub command: String,
    pub args: Vec<String>,
}

#[derive(Clone, Debug, Default)]
pub struct LoadedBackendConfiguration {
    pub backends: Vec<ConfiguredCodexBackend>,
    pub error: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct BackendConfigurationFile {
    #[serde(default = "configuration_version")]
    version: u8,
    #[serde(default)]
    backends: Vec<BackendEntry>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct BackendEntry {
    id: String,
    name: String,
    tag: String,
    adapter: String,
    command: String,
    #[serde(default)]
    args: Vec<String>,
    #[serde(default = "enabled_by_default")]
    enabled: bool,
}

fn configuration_version() -> u8 {
    1
}

fn enabled_by_default() -> bool {
    true
}

pub fn configuration_path(preferences_path: &Path) -> PathBuf {
    env::var_os("CODEX_THREAD_STUDIO_BACKENDS_CONFIG")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| preferences_path.with_file_name("backends.json"))
}

pub fn load(path: &Path) -> LoadedBackendConfiguration {
    match load_inner(path) {
        Ok(backends) => LoadedBackendConfiguration {
            backends,
            error: None,
        },
        Err(error) => LoadedBackendConfiguration {
            backends: Vec::new(),
            error: Some(error),
        },
    }
}

fn load_inner(path: &Path) -> Result<Vec<ConfiguredCodexBackend>, String> {
    let metadata = match fs::metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(format!("could not read {}: {error}", path.display())),
    };
    if !metadata.is_file() {
        return Err(format!("{} is not a regular file", path.display()));
    }
    if metadata.len() > MAX_CONFIG_BYTES {
        return Err(format!(
            "{} exceeds the {} byte limit",
            path.display(),
            MAX_CONFIG_BYTES
        ));
    }
    let source = fs::read_to_string(path)
        .map_err(|error| format!("could not read {}: {error}", path.display()))?;
    let config: BackendConfigurationFile = serde_json::from_str(&source)
        .map_err(|error| format!("invalid {}: {error}", path.display()))?;
    if config.version != 1 {
        return Err(format!(
            "{} uses unsupported backend configuration version {}",
            path.display(),
            config.version
        ));
    }
    if config.backends.len() > MAX_CONFIGURED_BACKENDS {
        return Err(format!(
            "{} contains more than {MAX_CONFIGURED_BACKENDS} backends",
            path.display()
        ));
    }

    let mut ids = BTreeSet::from(["codex".to_string(), "opencode".to_string()]);
    let mut backends = Vec::new();
    for entry in config.backends {
        if !entry.enabled {
            continue;
        }
        validate_entry(&entry)?;
        if !ids.insert(entry.id.clone()) {
            return Err(format!("duplicate or reserved backend id: {}", entry.id));
        }
        let id = entry.id;
        backends.push(ConfiguredCodexBackend {
            descriptor: BackendDescriptor {
                info_path: format!("/studio/backend/{id}"),
                socket_path: format!("/ws/codex/{id}"),
                id,
                name: entry.name.trim().to_string(),
                tag: entry.tag,
                kind: "codex",
                adapter: "codex-app-server",
                protocol: "Codex App Server v2",
                transport: "stdio JSONL via Studio WebSocket",
            },
            command: entry.command.trim().to_string(),
            args: entry.args,
        });
    }
    Ok(backends)
}

fn validate_entry(entry: &BackendEntry) -> Result<(), String> {
    if !valid_backend_id(&entry.id) {
        return Err(format!("invalid backend id: {}", entry.id));
    }
    if entry.name.trim().is_empty()
        || entry.name.chars().count() > 64
        || entry.name.chars().any(char::is_control)
    {
        return Err(format!("backend {} has an invalid name", entry.id));
    }
    if entry.tag.is_empty()
        || entry.tag.len() > 4
        || !entry
            .tag
            .bytes()
            .all(|value| value.is_ascii_uppercase() || value.is_ascii_digit())
    {
        return Err(format!("backend {} has an invalid tag", entry.id));
    }
    if entry.adapter != "codex-app-server" {
        return Err(format!(
            "backend {} uses unsupported adapter {}",
            entry.id, entry.adapter
        ));
    }
    if entry.command.trim().is_empty()
        || entry.command.len() > 4096
        || entry.command.chars().any(char::is_control)
    {
        return Err(format!("backend {} has an invalid command", entry.id));
    }
    if entry.args.len() > 32
        || entry.args.iter().any(|argument| {
            argument.is_empty() || argument.len() > 4096 || argument.chars().any(char::is_control)
        })
    {
        return Err(format!("backend {} has invalid arguments", entry.id));
    }
    Ok(())
}

pub fn valid_backend_id(value: &str) -> bool {
    let mut bytes = value.bytes();
    let Some(first) = bytes.next() else {
        return false;
    };
    value.len() <= 64
        && first.is_ascii_lowercase()
        && bytes.all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'-' | b'_')
        })
}

pub fn builtin_descriptors() -> Vec<BackendDescriptor> {
    vec![
        BackendDescriptor {
            id: "codex".to_string(),
            name: "Codex".to_string(),
            tag: "CX".to_string(),
            kind: "codex",
            adapter: "codex-app-server",
            info_path: "/studio/backend/codex".to_string(),
            socket_path: "/ws/codex/codex".to_string(),
            protocol: "Codex App Server v2",
            transport: "stdio JSONL via Studio WebSocket",
        },
        BackendDescriptor {
            id: "opencode".to_string(),
            name: "OpenCode".to_string(),
            tag: "OC".to_string(),
            kind: "opencode",
            adapter: "opencode-server",
            info_path: "/studio/opencode".to_string(),
            socket_path: String::new(),
            protocol: "OpenCode Server API",
            transport: "HTTP + SSE",
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temporary_path(name: &str) -> PathBuf {
        env::temp_dir().join(format!(
            "codex-thread-studio-backends-{name}-{}.json",
            uuid::Uuid::new_v4()
        ))
    }

    #[test]
    fn missing_configuration_keeps_only_builtin_backends() {
        let path = temporary_path("missing");
        let loaded = load(&path);
        assert!(loaded.backends.is_empty());
        assert_eq!(loaded.error, None);
    }

    #[test]
    fn loads_an_independent_codex_compatible_instance() {
        let path = temporary_path("valid");
        fs::write(
            &path,
            r#"{
              "version": 1,
              "backends": [{
                "id": "work-codex",
                "name": "Work Codex",
                "tag": "WK",
                "adapter": "codex-app-server",
                "command": "ept",
                "args": ["codex"]
              }]
            }"#,
        )
        .unwrap();
        let loaded = load(&path);
        fs::remove_file(path).unwrap();
        assert_eq!(loaded.error, None);
        assert_eq!(loaded.backends.len(), 1);
        assert_eq!(loaded.backends[0].descriptor.id, "work-codex");
        assert_eq!(loaded.backends[0].command, "ept");
        assert_eq!(loaded.backends[0].args, ["codex"]);
    }

    #[test]
    fn rejects_reserved_and_unknown_adapter_entries() {
        let path = temporary_path("invalid");
        fs::write(
            &path,
            r#"{"backends":[{"id":"codex","name":"Other","tag":"OT","adapter":"shell","command":"other"}]}"#,
        )
        .unwrap();
        let loaded = load(&path);
        fs::remove_file(path).unwrap();
        assert!(loaded.backends.is_empty());
        assert!(loaded.error.unwrap().contains("unsupported adapter"));
    }

    #[test]
    fn validates_backend_namespace_ids() {
        assert!(valid_backend_id("work-codex"));
        assert!(valid_backend_id("deepseek_2"));
        assert!(!valid_backend_id("EPT"));
        assert!(!valid_backend_id("2codex"));
        assert!(!valid_backend_id("codex:other"));
    }
}
