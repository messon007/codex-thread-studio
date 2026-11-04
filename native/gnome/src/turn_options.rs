//! Shared per-session options and capability interpretation, independent of GTK.
use serde::{Deserialize, Serialize};
use serde_json::Value;
#[derive(Default, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Options {
    pub model: String,
    pub effort: String,
    pub service_tier: String,
}
pub fn supports_fast(model: &Value) -> bool {
    model["serviceTiers"]
        .as_array()
        .into_iter()
        .flatten()
        .any(|tier| {
            tier["id"]
                .as_str()
                .is_some_and(|id| id.eq_ignore_ascii_case("fast"))
        })
        || model["additionalSpeedTiers"]
            .as_array()
            .into_iter()
            .flatten()
            .any(|tier| {
                tier.as_str()
                    .is_some_and(|tier| tier.eq_ignore_ascii_case("fast"))
            })
}
impl Options {
    pub fn effective_tier<'a>(&'a self, session_tier: &'a str) -> &'a str {
        if self.service_tier.is_empty() {
            if session_tier.is_empty() {
                "default"
            } else {
                session_tier
            }
        } else {
            &self.service_tier
        }
    }
    pub fn apply(&self, params: &mut Value, codex: bool) {
        if !self.model.is_empty() {
            params["model"] = self.model.clone().into();
        }
        if codex && !self.effort.is_empty() {
            params["effort"] = self.effort.clone().into();
        }
        if codex && !self.service_tier.is_empty() {
            params["serviceTier"] = self.service_tier.clone().into();
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn inheritance_explicit_default_and_backend_capabilities_are_preserved() {
        let mut options = Options::default();
        assert_eq!(options.effective_tier("fast"), "fast");
        options.service_tier = "default".into();
        assert_eq!(options.effective_tier("fast"), "default");
        let mut params = json!({});
        options.apply(&mut params, false);
        assert!(params.get("serviceTier").is_none());
        options.apply(&mut params, true);
        assert_eq!(params["serviceTier"], "default");
        assert!(supports_fast(&json!({"serviceTiers":[{"id":"fast"}]})));
        assert!(supports_fast(&json!({"additionalSpeedTiers":["fast"]})));
        assert!(!supports_fast(&json!({})));
    }
}
