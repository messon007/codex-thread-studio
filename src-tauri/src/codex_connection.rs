use serde::Deserialize;

/// Configured providers stay isolated unless sharing is explicitly requested.
#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum ConnectionMode {
    Auto,
    Shared,
    #[default]
    Embedded,
}

impl ConnectionMode {
    pub fn builtin() -> Result<Self, String> {
        match std::env::var("CODEX_THREAD_STUDIO_CODEX_CONNECTION") {
            Err(std::env::VarError::NotPresent) => Ok(Self::Auto),
            Ok(value) => Self::parse(&value),
            Err(_) => Err("CODEX_THREAD_STUDIO_CODEX_CONNECTION must be UTF-8".into()),
        }
    }

    pub fn parse(value: &str) -> Result<Self, String> {
        match value {
            "auto" => Ok(Self::Auto),
            "shared" => Ok(Self::Shared),
            "embedded" => Ok(Self::Embedded),
            _ => Err("Codex connection mode must be auto, shared or embedded".into()),
        }
    }
}

pub fn unsupported_command(stderr: &str) -> bool {
    stderr.contains("unrecognized subcommand") || stderr.contains("unexpected argument")
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn modes_are_explicit_and_custom_providers_default_to_isolation() {
        assert_eq!(ConnectionMode::default(), ConnectionMode::Embedded);
        for value in ["auto", "shared", "embedded"] {
            assert!(ConnectionMode::parse(value).is_ok());
        }
        assert!(ConnectionMode::parse("sharedd").is_err());
    }
    #[test]
    fn only_unsupported_cli_commands_allow_auto_fallback() {
        assert!(unsupported_command(
            "error: unrecognized subcommand 'proxy'"
        ));
        assert!(!unsupported_command("permission denied"));
        assert!(!unsupported_command("daemon socket is unavailable"));
    }
}
