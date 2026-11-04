//! Shared language preference and pure UI-string lookup. No document translation.
use serde::Deserialize;
use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicBool, Ordering},
        OnceLock,
    },
};
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Snapshot {
    pub revision: String,
    pub language: String,
}
#[derive(Debug)]
pub enum Request {
    Read,
    Save { revision: String, language: String },
}
pub async fn execute(api: &crate::gateway::Api, request: Request) -> Result<Snapshot, String> {
    crate::worker::compute(|| {
        let _ = catalog();
        Ok(())
    })
    .await?;
    let value = match request {
        Request::Read => {
            api.json(reqwest::Method::GET, "/studio/native/language", None)
                .await?
        }
        Request::Save { revision, language } => {
            api.json(
                reqwest::Method::PATCH,
                "/studio/native/language",
                Some(&serde_json::json!({"expectedRevision":revision,"language":language})),
            )
            .await?
        }
    };
    let value: Snapshot =
        serde_json::from_value(value).map_err(|_| "Invalid language settings response")?;
    resolve(&value.language, "en-US")?;
    if value.revision.len() != 64 || !value.revision.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err("Invalid language revision".into());
    }
    Ok(value)
}
static CHINESE: AtomicBool = AtomicBool::new(false);
fn catalog() -> &'static HashMap<String, String> {
    static CATALOG: OnceLock<HashMap<String, String>> = OnceLock::new();
    CATALOG.get_or_init(|| {
        serde_json::from_str(include_str!("../vendor/i18n/zh-CN.json"))
            .expect("checked native language catalog")
    })
}
pub fn resolve<'a>(preference: &str, system: &'a str) -> Result<&'static str, String> {
    match preference {
        "zh-CN" => Ok("zh-CN"),
        "en-US" => Ok("en-US"),
        "system" => Ok(
            if system
                .get(..2)
                .is_some_and(|p| p.eq_ignore_ascii_case("zh"))
            {
                "zh-CN"
            } else {
                "en-US"
            },
        ),
        _ => Err("Invalid interface language".into()),
    }
}
pub fn set(preference: &str, system: &str) -> Result<(), String> {
    let language = resolve(preference, system)?;
    let _ = catalog();
    CHINESE.store(language == "zh-CN", Ordering::Relaxed);
    Ok(())
}
pub fn tr(source: &str) -> String {
    format(source, &[])
}
pub fn format(source: &str, variables: &[(&str, &str)]) -> String {
    // gettext-style context separates equal English words with distinct UI
    // meanings; e.g. Markdown Source is code, not a reference's origin.
    let display = source
        .split_once('\u{4}')
        .map_or(source, |(_, label)| label);
    let translated = if CHINESE.load(Ordering::Relaxed) {
        catalog().get(source).map_or(display, String::as_str)
    } else {
        display
    };
    if variables.is_empty() {
        translated.to_owned()
    } else {
        substitute(translated, variables)
    }
}
fn substitute(source: &str, variables: &[(&str, &str)]) -> String {
    let mut output = String::new();
    let mut rest = source;
    while let Some(start) = rest.find('{') {
        output.push_str(&rest[..start]);
        rest = &rest[start..];
        let Some(end) = rest.find('}') else {
            break;
        };
        let name = &rest[1..end];
        output.push_str(
            variables
                .iter()
                .find(|(k, _)| *k == name)
                .map_or(&rest[..end + 1], |(_, v)| *v),
        );
        rest = &rest[end + 1..];
    }
    output.push_str(rest);
    output
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn locale_selection_and_literal_substitution_preserve_user_values() {
        assert_eq!(resolve("system", "zh_TW.UTF-8").unwrap(), "zh-CN");
        assert_eq!(resolve("system", "en_GB.UTF-8").unwrap(), "en-US");
        assert_eq!(resolve("en-US", "zh-CN").unwrap(), "en-US");
        assert!(resolve("invalid", "en-US").is_err());
        assert_eq!(format("markdown-mode\u{4}Source", &[]), "Source");
        assert_eq!(
            substitute(
                "File {path}: {error}",
                &[("path", "中文/{error}.md"), ("error", "<bad>😀")]
            ),
            "File 中文/{error}.md: <bad>😀"
        );
        assert_eq!(substitute("trailing {", &[]), "trailing {");
        assert_eq!(catalog().get("Send").map(String::as_str), Some("发送"));
    }
}
