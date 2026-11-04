//! Scoped appearance settings shared with the other clients. File work stays off async lanes.
use crate::preferences_document::{read, revision, write};
use axum::{
    body::Body,
    extract::{rejection::JsonRejection, State},
    http::{Response, StatusCode},
    Json,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::path::Path;
type Error = (StatusCode, String);
fn bad(e: impl ToString) -> Error {
    (StatusCode::BAD_REQUEST, e.to_string())
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Patch {
    expected_revision: String,
    values: Value,
}
#[derive(Serialize)]
struct Snapshot {
    revision: String,
    values: Value,
    fonts: Value,
}
fn projection(raw: &Value) -> Value {
    let field = |name| {
        raw.get("mermaid")
            .and_then(|v| v.get(name))
            .cloned()
            .unwrap_or(Value::Null)
    };
    json!({"contentWidth":raw.get("contentWidth").cloned().unwrap_or(Value::Null),"typography":raw.get("typography").cloned().unwrap_or(Value::Null),"markdownMode":raw.get("markdown").and_then(|v|v.get("mode")).cloned().unwrap_or(Value::Null),"mermaidFontSize":field("fontSize"),"mermaidDensity":field("density"),"mermaidCurve":field("curve"),"mermaidStyle":field("style"),"mermaidLayout":field("layout")})
}
fn families(value: &str) -> Result<Vec<String>, Error> {
    if value.len() > 512 || value.chars().any(char::is_control) {
        return Err(bad("Invalid font family"));
    }
    let mut input = cssparser::ParserInput::new(value);
    let mut parser = cssparser::Parser::new(&mut input);
    let parsed: Result<Vec<String>, cssparser::ParseError<'_, ()>> =
        parser.parse_comma_separated(|p| {
            let first = p.next()?.clone();
            let name = match first {
                cssparser::Token::QuotedString(name) => {
                    p.expect_exhausted()?;
                    name.to_string()
                }
                cssparser::Token::Ident(name) => {
                    let mut words = vec![name.to_string()];
                    while !p.is_exhausted() {
                        words.push(p.expect_ident()?.to_string());
                    }
                    words.join(" ")
                }
                _ => return Err(p.new_custom_error(())),
            };
            if name.trim().is_empty() || name.chars().any(char::is_control) {
                return Err(p.new_custom_error(()));
            }
            Ok(if name == "system-ui" {
                "sans-serif".into()
            } else {
                name
            })
        });
    let parsed = parsed.map_err(|_| bad("Font family must be a comma-separated list of names"))?;
    if parsed.is_empty() || parsed.len() > 16 {
        return Err(bad("A font stack supports 1–16 families"));
    }
    Ok(parsed)
}
fn validate(values: &Value) -> Result<Value, Error> {
    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase", deny_unknown_fields)]
    struct Values {
        content_width: Option<String>,
        typography: Option<crate::TypographyPreferences>,
        markdown_mode: Option<String>,
        mermaid_font_size: Option<u8>,
        mermaid_density: Option<String>,
        mermaid_curve: Option<String>,
        mermaid_style: Option<String>,
        mermaid_layout: Option<String>,
    }
    let parsed: Values = serde_json::from_value(values.clone()).map_err(bad)?;
    let p = crate::StudioPreferences {
        content_width: parsed.content_width,
        typography: parsed.typography,
        markdown: crate::MarkdownPreferences {
            mode: parsed.markdown_mode.unwrap_or_else(|| "technical".into()),
        },
        mermaid: crate::MermaidPreferences {
            font_size: parsed.mermaid_font_size.unwrap_or(15),
            density: parsed.mermaid_density.unwrap_or_else(|| "standard".into()),
            curve: parsed.mermaid_curve.unwrap_or_else(|| "rounded".into()),
            style: parsed.mermaid_style.unwrap_or_else(|| "auto".into()),
            layout: parsed.mermaid_layout.unwrap_or_else(|| "auto".into()),
        },
        ..Default::default()
    };
    crate::validate_preferences(&p).map_err(bad)?;
    if let Some(t) = p.typography {
        Ok(
            json!({"ui":families(&t.ui_font_family)?,"content":families(t.content_font_family.as_deref().unwrap_or(&t.ui_font_family))?,"code":families(&t.code_font_family)?,"workspace":families(&t.workspace_font_family)?}),
        )
    } else {
        Ok(json!({"ui":[],"content":[],"code":[],"workspace":[]}))
    }
}
fn snapshot(raw: &Value) -> Result<Snapshot, Error> {
    let values = projection(raw);
    Ok(Snapshot {
        revision: revision(&values)?,
        fonts: validate(&values)?,
        values,
    })
}
fn save(path: &Path, request: Patch) -> Result<Snapshot, Error> {
    let mut raw = read(path)?;
    if request.expected_revision != revision(&projection(&raw))? {
        return Err((
            StatusCode::CONFLICT,
            "Appearance changed in another client. Reload before saving; your edits are retained."
                .into(),
        ));
    }
    let mut next = projection(&raw);
    if !request.values.is_object()
        || request.values.as_object().unwrap().keys().any(|k| {
            !matches!(
                k.as_str(),
                "contentWidth"
                    | "typography"
                    | "markdownMode"
                    | "mermaidFontSize"
                    | "mermaidDensity"
                    | "mermaidCurve"
                    | "mermaidStyle"
                    | "mermaidLayout"
            )
        })
    {
        return Err(bad("Invalid appearance fields"));
    }
    for (key, value) in request.values.as_object().unwrap() {
        next[key] = value.clone();
    }
    validate(&next)?;
    for key in ["contentWidth", "typography"] {
        match request.values.get(key) {
            Some(v) if !v.is_null() => {
                raw[key] = v.clone();
            }
            Some(_) => {
                raw.as_object_mut().unwrap().remove(key);
            }
            None => {}
        }
    }
    if let Some(mode) = request.values.get("markdownMode") {
        if raw.get("markdown").is_none_or(Value::is_null) {
            raw["markdown"] = json!({});
        }
        if !raw["markdown"].is_object() {
            return Err(bad("Invalid Markdown preferences"));
        }
        raw["markdown"]["mode"] = mode.as_str().map_or(json!("technical"), |m| json!(m));
    }
    for (field, key, default) in [
        ("mermaidFontSize", "fontSize", json!(15)),
        ("mermaidDensity", "density", json!("standard")),
        ("mermaidCurve", "curve", json!("rounded")),
        ("mermaidStyle", "style", json!("auto")),
        ("mermaidLayout", "layout", json!("auto")),
    ] {
        if let Some(value) = request.values.get(field) {
            if raw.get("mermaid").is_none_or(Value::is_null) {
                raw["mermaid"] =
                    serde_json::to_value(crate::MermaidPreferences::default()).map_err(bad)?;
            }
            if !raw["mermaid"].is_object() {
                return Err(bad("Invalid Mermaid preferences"));
            }
            raw["mermaid"][key] = if value.is_null() {
                default
            } else {
                value.clone()
            };
        }
    }
    let result = snapshot(&raw)?;
    write(path, &raw)?;
    Ok(result)
}
async fn operation(state: crate::GatewayState, request: Option<Patch>) -> Response<Body> {
    let work = tokio::task::spawn_blocking(move || {
        let _lock = state
            .preferences_lock
            .lock()
            .map_err(|_| bad("Settings lock is unavailable"))?;
        if let Some(request) = request {
            save(&state.preferences_path, request)
        } else {
            snapshot(&read(&state.preferences_path)?)
        }
    })
    .await;
    match work {
        Ok(Ok(value)) => crate::json_response(StatusCode::OK, &value),
        Ok(Err((status, error))) => crate::json_error(status, &error),
        Err(_) => crate::json_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "Appearance worker stopped",
        ),
    }
}
pub async fn get(State(state): State<crate::GatewayState>) -> Response<Body> {
    operation(state, None).await
}
pub async fn patch(
    State(state): State<crate::GatewayState>,
    body: Result<Json<Patch>, JsonRejection>,
) -> Response<Body> {
    match body {
        Ok(Json(body)) => operation(state, Some(body)).await,
        Err(_) => crate::json_error(StatusCode::BAD_REQUEST, "Invalid appearance request"),
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn native_style_layout_changes_share_revisions_and_preserve_other_preferences() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("settings.json");
        let original =
            json!({"mermaid":{"style":"auto","layout":"auto","future":{"keep":true}},"future":42});
        std::fs::write(&path, serde_json::to_vec(&original).unwrap()).unwrap();
        let before = snapshot(&original).unwrap();
        let saved = save(
            &path,
            Patch {
                expected_revision: before.revision.clone(),
                values: json!({"mermaidStyle":"handDrawn","mermaidLayout":"elk"}),
            },
        )
        .unwrap();
        assert_eq!(saved.values["mermaidStyle"], "handDrawn");
        assert_eq!(saved.values["mermaidLayout"], "elk");
        let raw = read(&path).unwrap();
        assert_eq!(raw["mermaid"]["future"], original["mermaid"]["future"]);
        assert_eq!(raw["future"], 42);
        assert_eq!(
            save(
                &path,
                Patch {
                    expected_revision: before.revision,
                    values: json!({"mermaidStyle":"neo"})
                }
            )
            .err()
            .unwrap()
            .0,
            StatusCode::CONFLICT
        );
        let reset = save(
            &path,
            Patch {
                expected_revision: saved.revision,
                values: json!({"mermaidStyle":null,"mermaidLayout":null}),
            },
        )
        .unwrap();
        assert_eq!(reset.values["mermaidStyle"], "auto");
        assert_eq!(reset.values["mermaidLayout"], "auto");
    }
    #[test]
    fn diagram_preferences_preserve_unedited_fields_and_future_metadata() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("settings.json");
        let initial = json!({"mermaid":{"style":"handDrawn","layout":"elk","density":"standard","curve":"rounded","fontSize":15,"future":{"keep":true}},"future":42});
        std::fs::write(&path, serde_json::to_vec(&initial).unwrap()).unwrap();
        let base = snapshot(&initial).unwrap();
        let changed=save(&path,Patch{expected_revision:base.revision.clone(),values:json!({"mermaidFontSize":20,"mermaidDensity":"loose","mermaidCurve":"step"})}).unwrap();
        let raw = read(&path).unwrap();
        for key in ["style", "layout", "future"] {
            assert_eq!(raw["mermaid"][key], initial["mermaid"][key]);
        }
        let bytes = std::fs::read(&path).unwrap();
        for values in [
            json!({"mermaidFontSize":21}),
            json!({"mermaidFontSize":15.5}),
            json!({"mermaidDensity":"huge"}),
            json!({"mermaidCurve":"invalid"}),
            json!({"mermaidStyle":"invalid"}),
            json!({"mermaidLayout":"invalid"}),
        ] {
            assert_eq!(
                save(
                    &path,
                    Patch {
                        expected_revision: changed.revision.clone(),
                        values
                    }
                )
                .err()
                .unwrap()
                .0,
                StatusCode::BAD_REQUEST
            );
            assert_eq!(std::fs::read(&path).unwrap(), bytes);
        }
        assert_eq!(
            save(
                &path,
                Patch {
                    expected_revision: base.revision,
                    values: json!({"mermaidFontSize":12})
                }
            )
            .err()
            .unwrap()
            .0,
            StatusCode::CONFLICT
        );
        save(
            &path,
            Patch {
                expected_revision: changed.revision,
                values: json!({"mermaidFontSize":null,"mermaidDensity":null,"mermaidCurve":null}),
            },
        )
        .unwrap();
        let final_value = read(&path).unwrap();
        assert_eq!(final_value["mermaid"]["fontSize"], 15);
        assert_eq!(final_value["mermaid"]["style"], "handDrawn");
    }
    #[test]
    fn markdown_mode_keeps_future_fields_and_atomic_conflict_guards() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("settings.json");
        let original = json!({"markdown":{"mode":"technical","future":{"keep":true}},"mermaid":{"density":"loose"},"future":42});
        std::fs::write(&path, serde_json::to_vec(&original).unwrap()).unwrap();
        let base = snapshot(&original).unwrap();
        let reading = save(
            &path,
            Patch {
                expected_revision: base.revision.clone(),
                values: json!({"markdownMode":"reading"}),
            },
        )
        .unwrap();
        let saved = read(&path).unwrap();
        assert_eq!(saved["markdown"]["mode"], "reading");
        assert_eq!(saved["markdown"]["future"], original["markdown"]["future"]);
        assert_eq!(saved["mermaid"], original["mermaid"]);
        let bytes = std::fs::read(&path).unwrap();
        assert_eq!(
            save(
                &path,
                Patch {
                    expected_revision: base.revision,
                    values: json!({"markdownMode":"compact"})
                }
            )
            .err()
            .unwrap()
            .0,
            StatusCode::CONFLICT
        );
        for invalid in [json!("unsupported"), json!(42), json!({"mode":"compact"})] {
            assert_eq!(
                save(
                    &path,
                    Patch {
                        expected_revision: reading.revision.clone(),
                        values: json!({"markdownMode":invalid})
                    }
                )
                .err()
                .unwrap()
                .0,
                StatusCode::BAD_REQUEST
            );
            assert_eq!(std::fs::read(&path).unwrap(), bytes);
        }
        save(
            &path,
            Patch {
                expected_revision: reading.revision,
                values: json!({"markdownMode":null}),
            },
        )
        .unwrap();
        assert_eq!(read(&path).unwrap()["markdown"], original["markdown"]);
    }
    #[test]
    fn scoped_save_preserves_other_preferences_and_rejects_stale_appearance() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        let initial = json!({"contentWidth":"comfortable","future":{"keep":42},"router":{"controllers":{"codex":"original"}}});
        std::fs::write(&path, serde_json::to_vec(&initial).unwrap()).unwrap();
        let base = snapshot(&initial).unwrap();
        let mut other = initial.clone();
        other["future"]["new"] = json!(true);
        std::fs::write(&path, serde_json::to_vec(&other).unwrap()).unwrap();
        let result = save(
            &path,
            Patch {
                expected_revision: base.revision.clone(),
                values: json!({"contentWidth":"wide","typography":null}),
            },
        )
        .unwrap();
        assert_eq!(read(&path).unwrap()["future"]["new"], true);
        assert_eq!(read(&path).unwrap()["router"], initial["router"]);
        assert_ne!(base.revision, result.revision);
        assert_eq!(
            save(
                &path,
                Patch {
                    expected_revision: base.revision,
                    values: json!({"contentWidth":"full"})
                }
            )
            .err()
            .unwrap()
            .0,
            StatusCode::CONFLICT
        );
        assert_eq!(
            families("\"Noto Sans CJK SC\", system-ui, sans-serif").unwrap(),
            vec!["Noto Sans CJK SC", "sans-serif", "sans-serif"]
        );
        assert!(families("sans-serif; } button { opacity: 0").is_err());
        assert!(families("\"bad\0font\"").is_err());
    }
    #[test]
    fn partial_updates_keep_legacy_typography_and_future_fields() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        let original = json!({"typography":{"uiFontFamily":"sans-serif","uiFontWeight":400,"codeFontFamily":"monospace","codeFontSize":14,"codeFontWeight":500,"highContrast":false,"futureTypography":true},"future":42});
        std::fs::write(&path, serde_json::to_vec(&original).unwrap()).unwrap();
        let base = snapshot(&original).unwrap();
        let changed = save(
            &path,
            Patch {
                expected_revision: base.revision,
                values: json!({"contentWidth":"wide"}),
            },
        )
        .unwrap();
        assert_eq!(read(&path).unwrap()["typography"], original["typography"]);
        assert_eq!(changed.fonts["workspace"][0], "Noto Sans CJK SC");
        assert_eq!(
            save(
                &path,
                Patch {
                    expected_revision: changed.revision.clone(),
                    values: json!({"theme":"dark"})
                }
            )
            .err()
            .unwrap()
            .0,
            StatusCode::BAD_REQUEST
        );
        let reset = save(
            &path,
            Patch {
                expected_revision: changed.revision,
                values: json!({"typography":null}),
            },
        )
        .unwrap();
        assert!(reset.values["typography"].is_null());
        assert_eq!(read(&path).unwrap()["future"], 42);
        for value in [
            "",
            ",sans-serif",
            "serif,",
            "var(--font)",
            "sans-serif !important",
            "url(file:///private)",
        ] {
            assert!(families(value).is_err(), "{value}");
        }
    }
}
