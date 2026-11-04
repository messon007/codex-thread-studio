//! Scoped shared-folder settings, decoded and validated away from GTK.
use serde::Deserialize;
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    pub revision: String,
    pub directories: Vec<String>,
}
pub fn validate(paths: &[String]) -> Result<(), String> {
    if paths.len() > 256
        || paths.iter().any(|p| {
            p.is_empty()
                || p.len() > 4096
                || p.chars().any(char::is_control)
                || !std::path::Path::new(p).is_absolute()
        })
    {
        return Err("Choose at most 256 absolute folder paths (maximum 4096 bytes each)".into());
    }
    Ok(())
}
#[derive(Debug)]
pub enum Request {
    Read,
    Save {
        revision: String,
        directories: Vec<String>,
    },
}
pub async fn execute(api: &crate::gateway::Api, request: Request) -> Result<Snapshot, String> {
    let (method, body) = match request {
        Request::Read => (reqwest::Method::GET, None),
        Request::Save {
            revision,
            directories,
        } => {
            validate(&directories)?;
            (
                reqwest::Method::PATCH,
                Some(serde_json::json!({"expectedRevision":revision,"directories":directories})),
            )
        }
    };
    let value = api
        .json(method, "/studio/native/shared-directories", body.as_ref())
        .await?;
    crate::worker::compute(move || {
        let snapshot: Snapshot =
            serde_json::from_value(value).map_err(|_| "Invalid shared-folder response")?;
        if snapshot.revision.len() != 64
            || !snapshot.revision.bytes().all(|b| b.is_ascii_hexdigit())
        {
            return Err("Invalid shared-folder revision".into());
        }
        validate(&snapshot.directories)?;
        Ok(snapshot)
    })
    .await
}
