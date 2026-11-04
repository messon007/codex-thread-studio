use std::collections::{hash_map::DefaultHasher, BTreeMap};
use std::fs;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

const MAX_ENTRIES: usize = 128;
const MAX_VALUE_BYTES: usize = 16 * 1024;

#[derive(Clone, Debug, Default, Deserialize, Hash, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EnvironmentProfile {
    pub root: String,
    #[serde(default)]
    pub variables: BTreeMap<String, String>,
    #[serde(default)]
    pub secrets: BTreeMap<String, String>,
    #[serde(default)]
    pub network_policy: NetworkPolicy,
    #[serde(default)]
    pub allowed_hosts: Vec<String>,
    #[serde(default)]
    pub cache_variables: BTreeMap<String, String>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PublicEnvironmentProfile {
    pub root: String,
    pub configured: bool,
    pub revision: String,
    pub variables: BTreeMap<String, String>,
    pub secret_names: Vec<String>,
    pub network_policy: NetworkPolicy,
    pub allowed_hosts: Vec<String>,
    pub cache_variables: BTreeMap<String, String>,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Hash, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum NetworkPolicy {
    #[default]
    Restricted,
    Enabled,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateEnvironmentProfile {
    pub root: String,
    #[serde(default)]
    pub expected_revision: Option<String>,
    #[serde(default)]
    pub variables: BTreeMap<String, String>,
    #[serde(default)]
    pub secrets: BTreeMap<String, String>,
    #[serde(default)]
    pub remove_secrets: Vec<String>,
    #[serde(default)]
    pub network_policy: NetworkPolicy,
    #[serde(default)]
    pub allowed_hosts: Vec<String>,
    #[serde(default)]
    pub cache_variables: BTreeMap<String, String>,
}

#[derive(Default, Deserialize, Serialize)]
struct Store {
    #[serde(default)]
    profiles: BTreeMap<String, EnvironmentProfile>,
}

pub fn read_public(path: &Path, root: &str) -> Result<PublicEnvironmentProfile, String> {
    let canonical = canonical_root(root)?;
    let profile = load(path)?.profiles.remove(&canonical);
    Ok(match profile {
        Some(profile) => to_public(profile, true),
        None => to_public(
            EnvironmentProfile {
                root: canonical,
                ..Default::default()
            },
            false,
        ),
    })
}

pub fn update(
    path: &Path,
    request: UpdateEnvironmentProfile,
) -> Result<PublicEnvironmentProfile, String> {
    let root = canonical_root(&request.root)?;
    validate_map(&request.variables)?;
    validate_map(&request.secrets)?;
    validate_map(&request.cache_variables)?;
    let allowed_hosts = normalize_hosts(request.allowed_hosts)?;
    let mut store = load(path)?;
    if let Some(expected) = request.expected_revision.as_deref() {
        let current = store.profiles.get(&root).cloned();
        let configured = current.is_some();
        let current = to_public(
            current.unwrap_or_else(|| EnvironmentProfile {
                root: root.clone(),
                ..Default::default()
            }),
            configured,
        );
        if current.revision != expected {
            return Err("Environment profile changed elsewhere. Reload before saving.".into());
        }
    }
    let profile = store
        .profiles
        .entry(root.clone())
        .or_insert_with(|| EnvironmentProfile {
            root: root.clone(),
            ..Default::default()
        });
    profile.variables = request.variables;
    profile.network_policy = request.network_policy;
    profile.allowed_hosts = allowed_hosts;
    profile.cache_variables = request.cache_variables;
    for name in request.remove_secrets {
        profile.secrets.remove(&name);
    }
    for (name, value) in request.secrets {
        profile.secrets.insert(name, value);
    }
    let result = to_public(profile.clone(), true);
    save(path, &store)?;
    Ok(result)
}

pub fn process_environment(path: &Path, root: &Path) -> BTreeMap<String, String> {
    let canonical = root
        .canonicalize()
        .unwrap_or_else(|_| root.to_path_buf())
        .to_string_lossy()
        .into_owned();
    let Ok(mut store) = load(path) else {
        return BTreeMap::new();
    };
    let Some(profile) = store.profiles.remove(&canonical) else {
        return BTreeMap::new();
    };
    let mut values = profile.variables;
    values.extend(profile.cache_variables);
    values.extend(profile.secrets);
    values.insert(
        "CODEX_THREAD_STUDIO_NETWORK_POLICY".into(),
        match profile.network_policy {
            NetworkPolicy::Restricted => "restricted",
            NetworkPolicy::Enabled => "enabled",
        }
        .into(),
    );
    if !profile.allowed_hosts.is_empty() {
        values.insert(
            "CODEX_THREAD_STUDIO_ALLOWED_HOSTS".into(),
            profile.allowed_hosts.join(","),
        );
    }
    values
}

pub fn app_server_environment(path: &Path, root: &str) -> Result<BTreeMap<String, String>, String> {
    let canonical = PathBuf::from(canonical_root(root)?);
    Ok(process_environment(path, &canonical))
}

fn to_public(profile: EnvironmentProfile, configured: bool) -> PublicEnvironmentProfile {
    let mut hasher = DefaultHasher::new();
    profile.hash(&mut hasher);
    let revision = format!("{:016x}", hasher.finish());
    PublicEnvironmentProfile {
        root: profile.root,
        configured,
        revision,
        variables: profile.variables,
        secret_names: profile.secrets.keys().cloned().collect(),
        network_policy: profile.network_policy,
        allowed_hosts: profile.allowed_hosts,
        cache_variables: profile.cache_variables,
    }
}

fn canonical_root(root: &str) -> Result<String, String> {
    let path =
        fs::canonicalize(root).map_err(|_| "project directory is unavailable".to_string())?;
    if !path.is_dir() {
        return Err("project root is not a directory".into());
    }
    Ok(path.to_string_lossy().into_owned())
}

fn validate_map(values: &BTreeMap<String, String>) -> Result<(), String> {
    if values.len() > MAX_ENTRIES {
        return Err(format!("environment profile exceeds {MAX_ENTRIES} entries"));
    }
    for (name, value) in values {
        let mut chars = name.chars();
        if name.len() > 128
            || !chars
                .next()
                .is_some_and(|value| value == '_' || value.is_ascii_alphabetic())
            || !chars.all(|value| value == '_' || value.is_ascii_alphanumeric())
        {
            return Err(format!("invalid environment variable name: {name}"));
        }
        if value.len() > MAX_VALUE_BYTES || value.contains('\0') {
            return Err(format!("invalid value for {name}"));
        }
    }
    Ok(())
}

fn normalize_hosts(hosts: Vec<String>) -> Result<Vec<String>, String> {
    let mut output = Vec::new();
    for value in hosts.into_iter().take(MAX_ENTRIES) {
        let host = value.trim().to_ascii_lowercase();
        if host.is_empty() {
            continue;
        }
        if host.len() > 253
            || !host.chars().all(|value| {
                value.is_ascii_alphanumeric() || matches!(value, '.' | '-' | '*' | ':')
            })
        {
            return Err(format!("invalid allowed host: {host}"));
        }
        if !output.contains(&host) {
            output.push(host);
        }
    }
    Ok(output)
}

fn load(path: &Path) -> Result<Store, String> {
    match fs::read(path) {
        Ok(bytes) => serde_json::from_slice(&bytes)
            .map_err(|error| format!("invalid environment store: {error}")),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(Store::default()),
        Err(error) => Err(format!("unable to read environment store: {error}")),
    }
}

fn save(path: &Path, store: &Store) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    // NamedTempFile is private before secret bytes are written, and persist
    // replaces atomically on all supported platforms without an absent-file gap.
    use std::io::Write;
    let mut temporary = tempfile::NamedTempFile::new_in(path.parent().unwrap_or(Path::new(".")))
        .map_err(|error| error.to_string())?;
    serde_json::to_writer_pretty(&mut temporary, store).map_err(|error| error.to_string())?;
    temporary.flush().map_err(|error| error.to_string())?;
    temporary
        .as_file()
        .sync_all()
        .map_err(|error| error.to_string())?;
    temporary.persist(path).map_err(|error| error.to_string())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stale_revision_cannot_overwrite_secrets_or_variables() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("environments.json");
        let root = directory.path().to_string_lossy().into_owned();
        let initial = read_public(&path, &root).unwrap();
        let saved = update(
            &path,
            UpdateEnvironmentProfile {
                root: root.clone(),
                expected_revision: Some(initial.revision.clone()),
                variables: BTreeMap::from([("MULTILINE".into(), " 保留\n😀 ".into())]),
                secrets: BTreeMap::from([("TOKEN".into(), "private-value".into())]),
                ..Default::default()
            },
        )
        .unwrap();
        let before = fs::read(&path).unwrap();
        let error = update(
            &path,
            UpdateEnvironmentProfile {
                root: root.clone(),
                expected_revision: Some(initial.revision),
                remove_secrets: vec!["TOKEN".into()],
                ..Default::default()
            },
        )
        .unwrap_err();
        assert!(error.contains("changed elsewhere"));
        assert_eq!(fs::read(&path).unwrap(), before);
        let preserved = update(
            &path,
            UpdateEnvironmentProfile {
                root: root.clone(),
                expected_revision: Some(saved.revision),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(preserved.secret_names, vec!["TOKEN"]);
        assert!(!serde_json::to_string(&preserved)
            .unwrap()
            .contains("private-value"));
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                fs::metadata(&path).unwrap().permissions().mode() & 0o777,
                0o600
            );
        }
        update(
            &path,
            UpdateEnvironmentProfile {
                root,
                expected_revision: Some(preserved.revision),
                remove_secrets: vec!["TOKEN".into()],
                ..Default::default()
            },
        )
        .unwrap();
        assert!(!fs::read_to_string(&path).unwrap().contains("private-value"));
        assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 1);
    }

    #[test]
    fn secrets_are_redacted_but_injected_into_process_environment() {
        let base =
            std::env::temp_dir().join(format!("studio-environment-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&base).unwrap();
        let store = base.join("environments.json");
        let result = update(
            &store,
            UpdateEnvironmentProfile {
                root: base.to_string_lossy().into_owned(),
                variables: BTreeMap::from([("MODE".into(), "test".into())]),
                secrets: BTreeMap::from([("TOKEN".into(), "hidden".into())]),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(result.secret_names, vec!["TOKEN"]);
        let first_revision = result.revision.clone();
        let serialized = serde_json::to_string(&result).unwrap();
        assert!(!serialized.contains("hidden"));
        let environment = process_environment(&store, &base);
        assert_eq!(environment.get("TOKEN").map(String::as_str), Some("hidden"));

        let changed = update(
            &store,
            UpdateEnvironmentProfile {
                root: base.to_string_lossy().into_owned(),
                variables: BTreeMap::from([("MODE".into(), "test".into())]),
                secrets: BTreeMap::from([("TOKEN".into(), "changed".into())]),
                ..Default::default()
            },
        )
        .unwrap();
        assert_ne!(changed.revision, first_revision);
        assert!(!serde_json::to_string(&changed).unwrap().contains("changed"));
        let _ = fs::remove_dir_all(base);
    }
}
