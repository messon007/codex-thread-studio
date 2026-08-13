use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

const MAX_ENTRIES: usize = 128;
const MAX_VALUE_BYTES: usize = 16 * 1024;

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
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
    pub variables: BTreeMap<String, String>,
    pub secret_names: Vec<String>,
    pub network_policy: NetworkPolicy,
    pub allowed_hosts: Vec<String>,
    pub cache_variables: BTreeMap<String, String>,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize)]
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
    PublicEnvironmentProfile {
        root: profile.root,
        configured,
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
    let temporary = PathBuf::from(format!("{}.tmp", path.display()));
    fs::write(
        &temporary,
        serde_json::to_vec_pretty(store).map_err(|error| error.to_string())?,
    )
    .map_err(|error| error.to_string())?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&temporary, fs::Permissions::from_mode(0o600))
            .map_err(|error| error.to_string())?;
    }
    #[cfg(windows)]
    if path.exists() {
        fs::remove_file(path).map_err(|error| error.to_string())?;
    }
    fs::rename(temporary, path).map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

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
        let environment = process_environment(&store, &base);
        assert_eq!(environment.get("TOKEN").map(String::as_str), Some("hidden"));
        let _ = fs::remove_dir_all(base);
    }
}
