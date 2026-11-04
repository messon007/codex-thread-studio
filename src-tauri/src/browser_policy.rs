//! Portable browser preferences, navigation policy and download names.
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
    pub allow_http: bool,
    pub allow_private_network: bool,
    pub allow_localhost: bool,
    pub preview_javascript: bool,
    pub embedded_width: u32,
    #[serde(default)]
    pub agent: BrowserAgentPreferences,
}

/// Navigation controls shared by the native editor and browser runtime.
/// Other browser/agent settings remain owned by their existing clients.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NavigationPreferences {
    pub enabled: bool,
    pub allow_http: bool,
    pub allow_private_network: bool,
    pub allow_localhost: bool,
    pub preview_javascript: bool,
}
impl NavigationPreferences {
    pub fn from_browser(browser: &BrowserPreferences) -> Self {
        Self {
            enabled: browser.enabled,
            allow_http: browser.allow_http,
            allow_private_network: browser.allow_private_network,
            allow_localhost: browser.allow_localhost,
            preview_javascript: browser.preview_javascript,
        }
    }
    pub fn apply(&self, browser: &mut BrowserPreferences) {
        browser.enabled = self.enabled;
        browser.allow_http = self.allow_http;
        browser.allow_private_network = self.allow_private_network;
        browser.allow_localhost = self.allow_localhost;
        browser.preview_javascript = self.preview_javascript;
    }
}

impl Default for BrowserPreferences {
    fn default() -> Self {
        Self {
            enabled: true,
            restore_tabs: false,
            allow_http: true,
            allow_private_network: false,
            allow_localhost: true,
            preview_javascript: true,
            embedded_width: 720,
            agent: BrowserAgentPreferences::default(),
        }
    }
}

impl BrowserPreferences {
    pub fn validate(&self) -> Result<(), String> {
        if !(480..=2400).contains(&self.embedded_width) {
            return Err("embedded browser width must be between 480 and 2400".to_string());
        }
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

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum BrowserPolicyError {
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
    if url.host_str().is_none() || !url.username().is_empty() || url.password().is_some() {
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

pub fn sanitize_download_filename(value: &str) -> String {
    let mut sanitized = String::with_capacity(value.len().min(180));
    for character in value.chars().take(180) {
        if character.is_control()
            || matches!(
                character,
                '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|'
            )
        {
            sanitized.push('_');
        } else {
            sanitized.push(character);
        }
    }
    let sanitized =
        sanitized.trim_matches(|character: char| character == '.' || character.is_whitespace());
    if sanitized.is_empty() {
        "download".to_owned()
    } else {
        sanitized.to_owned()
    }
}
