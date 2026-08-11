use serde::{Deserialize, Serialize};
use url::{Host, Url};

pub const MAX_BROWSER_URL_BYTES: usize = 8 * 1024;

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(default, rename_all = "camelCase")]
pub struct BrowserAgentPreferences {
    pub enabled: bool,
    pub provider: String,
    pub profile: String,
    pub approval: String,
    #[serde(default)]
    pub allowed_origins: Vec<String>,
}

impl Default for BrowserAgentPreferences {
    fn default() -> Self {
        Self {
            enabled: false,
            provider: "playwright-mcp".to_string(),
            profile: "persistent".to_string(),
            approval: "interactive".to_string(),
            allowed_origins: Vec::new(),
        }
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(default, rename_all = "camelCase")]
pub struct BrowserPreferences {
    pub enabled: bool,
    pub restore_tabs: bool,
    pub workspace_profiles: bool,
    pub allow_http: bool,
    pub allow_private_network: bool,
    pub allow_localhost: bool,
    pub preview_javascript: bool,
    pub external_open_fallback: bool,
    #[serde(default)]
    pub agent: BrowserAgentPreferences,
}

impl Default for BrowserPreferences {
    fn default() -> Self {
        Self {
            enabled: false,
            restore_tabs: true,
            workspace_profiles: true,
            allow_http: true,
            allow_private_network: false,
            allow_localhost: true,
            preview_javascript: true,
            external_open_fallback: true,
            agent: BrowserAgentPreferences::default(),
        }
    }
}

impl BrowserPreferences {
    pub fn validate(&self) -> Result<(), String> {
        if self.agent.provider != "playwright-mcp" {
            return Err("browser agent provider must be playwright-mcp".to_string());
        }
        if !matches!(self.agent.profile.as_str(), "persistent" | "isolated") {
            return Err("browser agent profile must be persistent or isolated".to_string());
        }
        if !matches!(self.agent.approval.as_str(), "interactive" | "read-only") {
            return Err("browser agent approval must be interactive or read-only".to_string());
        }
        if self.agent.allowed_origins.len() > 128
            || self.agent.allowed_origins.iter().any(|origin| {
                origin.len() > MAX_BROWSER_URL_BYTES || validate_http_origin(origin).is_err()
            })
        {
            return Err("browser agent allowed origins are invalid".to_string());
        }
        Ok(())
    }
}

#[derive(Clone, Debug)]
pub struct BrowserController {
    preferences: BrowserPreferences,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BrowserNavigation {
    pub url: Url,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum BrowserPolicyError {
    Disabled,
    EmptyUrl,
    UrlTooLong,
    InvalidUrl,
    SchemeNotAllowed(String),
    HttpNotAllowed,
    LocalhostNotAllowed,
    PrivateNetworkNotAllowed,
}

impl std::fmt::Display for BrowserPolicyError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Disabled => write!(formatter, "Browser Workspace is disabled"),
            Self::EmptyUrl => write!(formatter, "browser URL is empty"),
            Self::UrlTooLong => write!(formatter, "browser URL is too long"),
            Self::InvalidUrl => write!(formatter, "browser URL is invalid"),
            Self::SchemeNotAllowed(scheme) => {
                write!(formatter, "browser URL scheme '{scheme}' is not allowed")
            }
            Self::HttpNotAllowed => write!(formatter, "HTTP browser navigation is disabled"),
            Self::LocalhostNotAllowed => {
                write!(formatter, "localhost browser navigation is disabled")
            }
            Self::PrivateNetworkNotAllowed => {
                write!(formatter, "private-network browser navigation is disabled")
            }
        }
    }
}

impl BrowserController {
    pub fn new(preferences: BrowserPreferences) -> Self {
        Self { preferences }
    }

    pub fn is_enabled(&self) -> bool {
        self.preferences.enabled
    }

    pub fn prepare_navigation(&self, input: &str) -> Result<BrowserNavigation, BrowserPolicyError> {
        if !self.is_enabled() {
            return Err(BrowserPolicyError::Disabled);
        }
        let url = validate_browser_url(input, &self.preferences)?;
        Ok(BrowserNavigation { url })
    }
}

pub fn validate_browser_url(
    input: &str,
    preferences: &BrowserPreferences,
) -> Result<Url, BrowserPolicyError> {
    let input = input.trim();
    if input.is_empty() {
        return Err(BrowserPolicyError::EmptyUrl);
    }
    if input.len() > MAX_BROWSER_URL_BYTES {
        return Err(BrowserPolicyError::UrlTooLong);
    }
    let url = Url::parse(input).map_err(|_| BrowserPolicyError::InvalidUrl)?;
    match url.scheme() {
        "https" => {}
        "http" if preferences.allow_http => {}
        "http" => return Err(BrowserPolicyError::HttpNotAllowed),
        scheme => return Err(BrowserPolicyError::SchemeNotAllowed(scheme.to_string())),
    }
    if is_localhost(&url) && !preferences.allow_localhost {
        return Err(BrowserPolicyError::LocalhostNotAllowed);
    }
    if is_private_network(&url) && !is_localhost(&url) && !preferences.allow_private_network {
        return Err(BrowserPolicyError::PrivateNetworkNotAllowed);
    }
    if url.host_str().is_none() {
        return Err(BrowserPolicyError::InvalidUrl);
    }
    Ok(url)
}

fn validate_http_origin(input: &str) -> Result<(), ()> {
    let url = Url::parse(input).map_err(|_| ())?;
    if !matches!(url.scheme(), "http" | "https")
        || url.host_str().is_none()
        || url.path() != "/"
        || url.query().is_some()
        || url.fragment().is_some()
        || !url.username().is_empty()
        || url.password().is_some()
    {
        return Err(());
    }
    Ok(())
}

fn is_localhost(url: &Url) -> bool {
    match url.host() {
        Some(Host::Domain(host)) => host.eq_ignore_ascii_case("localhost"),
        Some(Host::Ipv4(address)) => address.is_loopback(),
        Some(Host::Ipv6(address)) => address.is_loopback(),
        None => false,
    }
}

fn is_private_network(url: &Url) -> bool {
    match url.host() {
        Some(Host::Domain(host)) => host.ends_with(".local") || host.ends_with(".localhost"),
        Some(Host::Ipv4(address)) => {
            address.is_private()
                || address.is_link_local()
                || address.is_loopback()
                || address.is_unspecified()
        }
        Some(Host::Ipv6(address)) => {
            let first = address.segments()[0];
            address.is_loopback()
                || address.is_unspecified()
                || first & 0xfe00 == 0xfc00
                || first & 0xffc0 == 0xfe80
        }
        None => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn feature_flag_is_disabled_by_default() {
        let preferences = BrowserPreferences::default();
        assert!(!preferences.enabled);
        assert_eq!(
            BrowserController::new(preferences).prepare_navigation("https://example.com"),
            Err(BrowserPolicyError::Disabled)
        );
    }

    #[test]
    fn allows_http_and_https_with_explicitly_enabled_defaults() {
        let preferences = BrowserPreferences {
            enabled: true,
            ..BrowserPreferences::default()
        };
        let controller = BrowserController::new(preferences);
        assert!(controller
            .prepare_navigation("https://example.com/docs")
            .is_ok());
        assert!(controller
            .prepare_navigation("http://localhost:3000/")
            .is_ok());
    }

    #[test]
    fn rejects_active_and_local_file_schemes_with_a_reason() {
        let preferences = BrowserPreferences {
            enabled: true,
            ..BrowserPreferences::default()
        };
        for (input, scheme) in [
            ("file:///tmp/index.html", "file"),
            ("javascript:alert(1)", "javascript"),
            ("data:text/html,hello", "data"),
            ("custom://example/path", "custom"),
        ] {
            assert_eq!(
                validate_browser_url(input, &preferences),
                Err(BrowserPolicyError::SchemeNotAllowed(scheme.to_string()))
            );
        }
    }

    #[test]
    fn separates_localhost_from_other_private_networks() {
        let mut preferences = BrowserPreferences {
            enabled: true,
            ..BrowserPreferences::default()
        };
        assert!(validate_browser_url("http://127.0.0.2:3000", &preferences).is_ok());
        assert_eq!(
            validate_browser_url("http://192.168.1.20", &preferences),
            Err(BrowserPolicyError::PrivateNetworkNotAllowed)
        );
        preferences.allow_private_network = true;
        assert!(validate_browser_url("http://192.168.1.20", &preferences).is_ok());
    }

    #[test]
    fn validates_future_agent_origins_as_origins_not_urls() {
        let mut preferences = BrowserPreferences::default();
        preferences.agent.allowed_origins = vec!["https://example.com/path".to_string()];
        assert!(preferences.validate().is_err());
        preferences.agent.allowed_origins = vec!["https://example.com".to_string()];
        assert!(preferences.validate().is_ok());
    }
}
