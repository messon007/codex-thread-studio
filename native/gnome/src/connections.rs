//! Explicit, bounded diagnostics over existing gateway metadata endpoints.
use crate::gateway::Api;
use futures_util::{stream, StreamExt};
use reqwest::{Method, StatusCode};
use serde::Deserialize;
use serde_json::Value;
use std::{
    collections::HashSet,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};

const BACKENDS: usize = 18;
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Descriptor {
    pub id: String,
    pub name: String,
    pub kind: String,
    pub protocol: String,
    pub transport: String,
}
impl Descriptor {
    fn path(&self) -> String {
        if self.kind == "opencode" {
            "/studio/opencode".into()
        } else {
            format!("/studio/backend/{}", self.id)
        }
    }
}
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Registry {
    pub config_path: String,
    pub configuration_error: Option<String>,
    pub backends: Vec<Descriptor>,
}
impl Registry {
    fn parse(value: Value) -> Result<Self, String> {
        let mut registry: Self =
            serde_json::from_value(value).map_err(|_| "Invalid connection registry")?;
        let mut ids = HashSet::new();
        if registry.config_path.len() > 16 * 1024
            || registry
                .configuration_error
                .as_ref()
                .is_some_and(|s| s.len() > 16 * 1024)
            || registry.backends.len() > BACKENDS
            || registry.backends.iter().any(|b| {
                b.id.is_empty()
                    || b.id.len() > 64
                    || !b
                        .id
                        .bytes()
                        .all(|c| c.is_ascii_alphanumeric() || matches!(c, b'-' | b'_'))
                    || !ids.insert(b.id.clone())
                    || b.name.len() > 1024
                    || b.protocol.len() > 1024
                    || b.transport.len() > 1024
                    || !matches!(b.kind.as_str(), "codex" | "opencode")
                    || (b.kind == "opencode" && b.id != "opencode")
            })
        {
            return Err("Invalid connection registry bounds or identity".into());
        }
        for backend in &mut registry.backends {
            for text in [
                &mut backend.name,
                &mut backend.protocol,
                &mut backend.transport,
            ] {
                *text = text.replace('\0', "�");
            }
        }
        Ok(registry)
    }
}
#[derive(Clone, Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Info {
    #[serde(default)]
    pub binary: String,
    #[serde(default)]
    pub protocol: String,
    #[serde(default)]
    pub transport: String,
    #[serde(default)]
    pub execution_environment: String,
    pub wsl_distribution: Option<String>,
    #[serde(default)]
    pub host_platform: String,
    pub backend_version: Option<String>,
    pub reachable: Option<bool>,
    pub error: Option<String>,
}
#[derive(Clone, Debug)]
pub struct Backend {
    pub descriptor: Descriptor,
    pub info: Info,
    pub error: Option<String>,
}
fn bounded(value: &str, limit: usize) -> String {
    let mut end = value.len().min(limit);
    while !value.is_char_boundary(end) {
        end -= 1;
    }
    let mut result = value[..end].replace('\0', "�");
    let mut display_end = result.len().min(limit);
    while !result.is_char_boundary(display_end) {
        display_end -= 1;
    }
    let truncated = end < value.len() || display_end < result.len();
    result.truncate(display_end);
    if truncated {
        result.push('…');
    }
    result
}
fn metadata(descriptor: Descriptor, status: StatusCode, value: Value) -> Backend {
    let rejected = (!status.is_success()).then(|| crate::gateway::response_error(status, &value));
    let parsed = serde_json::from_value::<Info>(value);
    let (mut info, mut error) = match parsed {
        Ok(info) => (info, None),
        Err(_) => (
            Info::default(),
            Some("Invalid backend information response".into()),
        ),
    };
    if let Some(message) = rejected {
        error = Some(message);
    } else if let Some(message) = info.error.as_ref().filter(|s| !s.is_empty()) {
        error = Some(message.clone());
    }
    for text in [
        &mut info.binary,
        &mut info.protocol,
        &mut info.transport,
        &mut info.execution_environment,
        &mut info.host_platform,
    ] {
        *text = bounded(text, 8192);
    }
    for text in [
        &mut info.wsl_distribution,
        &mut info.backend_version,
        &mut info.error,
    ] {
        if let Some(value) = text {
            *value = bounded(value, 8192);
        }
    }
    Backend {
        descriptor,
        info,
        error: error.map(|e| bounded(&e, 8192)),
    }
}
#[derive(Debug)]
pub struct Snapshot {
    pub config_path: String,
    pub configuration_error: Option<String>,
    pub gateway_version: String,
    pub gateway_error: Option<String>,
    pub backends: Vec<Backend>,
}
fn cancelled(cancel: &AtomicBool) -> Result<(), String> {
    if cancel.load(Ordering::Acquire) {
        Err("Connection diagnostics cancelled".into())
    } else {
        Ok(())
    }
}
pub async fn read(api: &Api, cancel: Arc<AtomicBool>) -> Result<Snapshot, String> {
    cancelled(&cancel)?;
    let (registry, gateway) = tokio::join!(
        api.json(Method::GET, "/studio/backends", None),
        api.json(Method::GET, "/studio/native/info", None)
    );
    let registry = registry?;
    let registry = crate::worker::compute(move || Registry::parse(registry)).await?;
    cancelled(&cancel)?;
    let (gateway_version, gateway_error) = crate::worker::compute(move || {
        Ok(match gateway {
            Ok(value) => match value["appVersion"]
                .as_str()
                .filter(|s| !s.is_empty() && s.len() <= 128)
            {
                Some(version) => (version.replace('\0', "�"), None),
                None => (
                    String::new(),
                    Some("Invalid gateway version response".into()),
                ),
            },
            Err(error) => (String::new(), Some(bounded(&error, 8192))),
        })
    })
    .await?;
    let mut rows = stream::iter(registry.backends.into_iter().enumerate().map(
        |(index, descriptor)| {
            let cancel = cancel.clone();
            async move {
                cancelled(&cancel)?;
                let result = api
                    .json_status_optional(Method::GET, &descriptor.path(), None)
                    .await;
                cancelled(&cancel)?;
                let row = crate::worker::compute(move || {
                    Ok(match result {
                        Ok((status, value)) => metadata(descriptor, status, value),
                        Err(error) => Backend {
                            descriptor,
                            info: Info::default(),
                            error: Some(bounded(&error, 8192)),
                        },
                    })
                })
                .await?;
                Ok::<_, String>((index, row))
            }
        },
    ))
    .buffer_unordered(4)
    .collect::<Vec<_>>()
    .await
    .into_iter()
    .collect::<Result<Vec<_>, _>>()?;
    cancelled(&cancel)?;
    rows.sort_by_key(|(i, _)| *i);
    Ok(Snapshot {
        config_path: registry.config_path.replace('\0', "�"),
        configuration_error: registry.configuration_error.map(|s| s.replace('\0', "�")),
        gateway_version,
        gateway_error,
        backends: rows.into_iter().map(|(_, b)| b).collect(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    fn descriptor() -> Descriptor {
        serde_json::from_value(json!({"id":"custom","name":"<literal> 中文","kind":"codex","protocol":"App Server","transport":"stdio"})).unwrap()
    }
    #[tokio::test]
    async fn diagnostics_only_get_known_endpoints_and_bound_concurrency_preserving_order() {
        use std::sync::atomic::AtomicUsize;
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let api = Api::from_access_url(&format!(
            "http://{}/#token=0123456789abcdef0123456789abcdef",
            listener.local_addr().unwrap()
        ))
        .unwrap();
        let cancelled = read(&api, Arc::new(AtomicBool::new(true)))
            .await
            .unwrap_err();
        assert!(cancelled.contains("cancelled"));
        let active = Arc::new(AtomicUsize::new(0));
        let peak = Arc::new(AtomicUsize::new(0));
        let observed = peak.clone();
        let server = tokio::spawn(async move {
            let mut tasks = Vec::new();
            for _ in 0..8 {
                let (mut socket, _) =
                    tokio::time::timeout(std::time::Duration::from_secs(5), listener.accept())
                        .await
                        .unwrap()
                        .unwrap();
                let active = active.clone();
                let peak = peak.clone();
                tasks.push(tokio::spawn(async move {
                    let mut request=Vec::new();
                    while !request.windows(4).any(|b|b==b"\r\n\r\n") {
                        let mut bytes=[0;2048];let n=socket.read(&mut bytes).await.unwrap();assert!(n>0);request.extend_from_slice(&bytes[..n]);
                    }
                    let request=String::from_utf8(request).unwrap();
                    assert!(request.to_ascii_lowercase().contains("authorization: bearer 0123456789abcdef0123456789abcdef"));
                    let parts=request.lines().next().unwrap().split_whitespace().collect::<Vec<_>>();assert_eq!(parts[0],"GET");
                    let path=parts[1];let mut status=200;
                    let value=if path=="/studio/backends" {
                        let mut backends=(0..5).map(|i|json!({"id":format!("custom-{i}"),"name":format!("<Literal {i}> 中文"),"kind":"codex","protocol":"App Server","transport":"stdio","infoPath":"https://untrusted.invalid"})).collect::<Vec<_>>();
                        backends.push(json!({"id":"opencode","name":"OpenCode","kind":"opencode","protocol":"HTTP","transport":"SSE"}));
                        json!({"configPath":"/literal/config","configurationError":"Literal <configuration> error","backends":backends})
                    } else if path=="/studio/native/info" {json!({"appVersion":"gateway-v"})}
                    else {
                        assert!(path=="/studio/opencode" || path.starts_with("/studio/backend/custom-"),"{path}");
                        let current=active.fetch_add(1,Ordering::SeqCst)+1;peak.fetch_max(current,Ordering::SeqCst);
                        tokio::time::sleep(std::time::Duration::from_millis(if path.ends_with("0"){70}else{15})).await;
                        active.fetch_sub(1,Ordering::SeqCst);
                        if path=="/studio/opencode" {status=502;json!({"binary":"/bin/opencode","reachable":false,"error":"Health check unavailable 中文"})}
                        else {json!({"binary":format!("literal {path}"),"executionEnvironment":"local"})}
                    };
                    let body=value.to_string();socket.write_all(format!("HTTP/1.1 {status} Fixture\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",body.len()).as_bytes()).await.unwrap();
                }));
            }
            for task in tasks {
                task.await.unwrap();
            }
        });
        let snapshot = read(&api, Arc::new(AtomicBool::new(false))).await.unwrap();
        server.await.unwrap();
        assert_eq!(snapshot.gateway_version, "gateway-v");
        assert_eq!(
            snapshot.configuration_error.as_deref(),
            Some("Literal <configuration> error")
        );
        assert_eq!(
            snapshot
                .backends
                .iter()
                .map(|b| b.descriptor.id.as_str())
                .collect::<Vec<_>>(),
            vec!["custom-0", "custom-1", "custom-2", "custom-3", "custom-4", "opencode"]
        );
        assert!(snapshot.backends[0].info.binary.ends_with("custom-0"));
        assert_eq!(
            snapshot.backends[5].error.as_deref(),
            Some("Health check unavailable 中文")
        );
        assert!((2..=4).contains(&observed.load(Ordering::SeqCst)));
    }
    #[test]
    fn registry_rejects_unsafe_paths_duplicates_and_excessive_backend_counts() {
        let entry = json!({"id":"custom","name":"<literal> 中文","kind":"codex","protocol":"App Server","transport":"stdio","infoPath":"https://untrusted.invalid/"});
        let registry = json!({"configPath":"/literal/config","backends":[entry.clone()]});
        assert_eq!(
            Registry::parse(registry.clone()).unwrap().backends[0].path(),
            "/studio/backend/custom"
        );
        for id in ["..", "slash/id", "bad?query", "https://host", ""] {
            let mut bad = registry.clone();
            bad["backends"][0]["id"] = id.into();
            assert!(Registry::parse(bad).is_err());
        }
        assert!(Registry::parse(
            json!({"configPath":"/config","backends":[entry.clone(),entry.clone()]})
        )
        .is_err());
        assert!(
            Registry::parse(json!({"configPath":"/config","backends":vec![entry;19]})).is_err()
        );
    }
    #[test]
    fn rejected_health_retains_useful_metadata_and_caps_literal_diagnostics() {
        let row = metadata(
            descriptor(),
            StatusCode::BAD_GATEWAY,
            json!({"binary":"<literal> 中文","protocol":"HTTP","reachable":false,"error":"😀".repeat(4000)}),
        );
        assert_eq!(row.info.binary, "<literal> 中文");
        assert_eq!(row.info.reachable, Some(false));
        assert!(row.error.unwrap().len() <= 8195);
        let nul = bounded(&"\0".repeat(9000), 8192);
        assert!(nul.len() <= 8195 && !nul.contains('\0') && nul.ends_with('…'));
        let row = metadata(descriptor(), StatusCode::OK, json!({"binary":"/bin/codex"}));
        assert!(row.error.is_none());
        assert_eq!(row.info.reachable, None);
        assert!(metadata(descriptor(), StatusCode::OK, json!([]))
            .error
            .is_some());
    }
}
