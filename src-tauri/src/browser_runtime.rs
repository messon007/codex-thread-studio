#[cfg(any(all(feature = "desktop", windows), test))]
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

use url::Url;

// Shared with the GTK4 frontend; keep navigation policy identical across shells.
#[allow(unused_imports)]
pub use crate::browser_policy::*;
#[cfg(any(all(feature = "desktop", windows), test))]
pub const DEFAULT_BROWSER_URL: &str = "https://example.com";
#[cfg(all(feature = "desktop", windows))]
pub const MIN_BROWSER_WIDTH: u32 = 480;
#[cfg(all(feature = "desktop", windows))]
pub const MIN_STUDIO_WIDTH: u32 = 520;

#[cfg(any(all(feature = "desktop", windows), test))]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BrowserLifecycle {
    Cold,
    Hidden,
    Visible,
}

#[cfg(any(all(feature = "desktop", windows), test))]
impl BrowserLifecycle {
    pub fn runtime_loaded(self) -> bool {
        !matches!(self, Self::Cold)
    }

    pub fn visible(self) -> bool {
        matches!(self, Self::Visible)
    }

    pub fn hide(self) -> Self {
        match self {
            Self::Cold => Self::Cold,
            Self::Hidden | Self::Visible => Self::Hidden,
        }
    }

    pub fn show(self) -> Self {
        Self::Visible
    }

    pub fn exit(self) -> Self {
        Self::Cold
    }
}

#[cfg(any(all(feature = "desktop", windows), test))]
pub const fn embedded_browser_platform_available() -> bool {
    cfg!(any(target_os = "linux", target_os = "windows"))
}

#[cfg(any(all(feature = "desktop", any(target_os = "linux", windows)), test))]
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct BrowserTypography {
    pub font_family: String,
    pub font_size: f64,
    pub font_weight: u16,
}

#[cfg(any(all(feature = "desktop", any(target_os = "linux", windows)), test))]
pub fn parse_browser_typography(raw: &str) -> Option<BrowserTypography> {
    if raw.len() > 4096 {
        return None;
    }
    let value: BrowserTypography = serde_json::from_str(raw).ok()?;
    if value.font_family.trim().is_empty()
        || value.font_family.len() > 2048
        || !(11.0..=20.0).contains(&value.font_size)
        || ![400, 500, 600].contains(&value.font_weight)
    {
        return None;
    }
    Some(value)
}

#[cfg(any(all(feature = "desktop", windows), test))]
#[derive(Clone, Debug, PartialEq)]
pub enum ToolbarAction {
    Toggle,
    Show,
    Open(String),
    Exit,
    Navigate(String),
    NewTab(Option<String>),
    ActivateTab(u64),
    CloseTab(u64),
    NextTab,
    PreviousTab,
    Back,
    Forward,
    Reload,
    ZoomIn,
    ZoomOut,
    ZoomReset,
    FitWidth,
    CommentSelection,
    CopyUrl,
    ShowMenu,
    ShowInfo,
    ShowDownloads,
    OpenDownloadsDirectory,
    SetBrowserWidth(u32),
    SetTranslations(HashMap<String, String>),
    SetTypography(BrowserTypography),
}

#[cfg(any(all(feature = "desktop", windows), test))]
pub fn parse_toolbar_action(raw: &str) -> Option<ToolbarAction> {
    let url = Url::parse(raw).ok()?;
    if url.scheme() != "studio-action" {
        return None;
    }
    match url.host_str()? {
        "toggle-browser" => Some(ToolbarAction::Toggle),
        "show-browser" => Some(ToolbarAction::Show),
        "open-browser" => query_value(&url, "url").map(ToolbarAction::Open),
        "exit-browser" => Some(ToolbarAction::Exit),
        "navigate" => query_value(&url, "url").map(ToolbarAction::Navigate),
        "new-tab" => Some(ToolbarAction::NewTab(query_value(&url, "url"))),
        "activate-tab" => query_u64(&url, "id").map(ToolbarAction::ActivateTab),
        "close-tab" => query_u64(&url, "id").map(ToolbarAction::CloseTab),
        "next-tab" => Some(ToolbarAction::NextTab),
        "previous-tab" => Some(ToolbarAction::PreviousTab),
        "back" => Some(ToolbarAction::Back),
        "forward" => Some(ToolbarAction::Forward),
        "reload" => Some(ToolbarAction::Reload),
        "zoom-in" => Some(ToolbarAction::ZoomIn),
        "zoom-out" => Some(ToolbarAction::ZoomOut),
        "zoom-reset" => Some(ToolbarAction::ZoomReset),
        "fit-width" => Some(ToolbarAction::FitWidth),
        "comment-selection" => Some(ToolbarAction::CommentSelection),
        "copy-url" => Some(ToolbarAction::CopyUrl),
        "browser-menu" => Some(ToolbarAction::ShowMenu),
        "browser-info" => Some(ToolbarAction::ShowInfo),
        "browser-downloads" => Some(ToolbarAction::ShowDownloads),
        "open-downloads-directory" => Some(ToolbarAction::OpenDownloadsDirectory),
        "set-browser-width" => query_u32(&url, "width").map(ToolbarAction::SetBrowserWidth),
        "set-browser-translations" => query_value(&url, "messages")
            .and_then(|value| serde_json::from_str(&value).ok())
            .map(ToolbarAction::SetTranslations),
        "set-browser-typography" => query_value(&url, "profile")
            .and_then(|value| parse_browser_typography(&value))
            .map(ToolbarAction::SetTypography),
        _ => None,
    }
}

#[cfg_attr(not(test), allow(dead_code))]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BrowserTabState {
    pub id: u64,
    pub url: String,
    pub title: String,
}

#[cfg_attr(not(test), allow(dead_code))]
#[derive(Clone, Debug)]
pub struct BrowserTabRegistry {
    tabs: Vec<BrowserTabState>,
    active_tab_id: u64,
    next_tab_id: u64,
}

#[cfg_attr(not(test), allow(dead_code))]
impl BrowserTabRegistry {
    pub fn new(default_url: &str) -> Self {
        Self {
            tabs: vec![BrowserTabState {
                id: 1,
                url: default_url.to_owned(),
                title: display_title("", default_url),
            }],
            active_tab_id: 1,
            next_tab_id: 2,
        }
    }

    pub fn tabs(&self) -> &[BrowserTabState] {
        &self.tabs
    }

    pub fn active_tab_id(&self) -> u64 {
        self.active_tab_id
    }

    pub fn active_tab(&self) -> Option<&BrowserTabState> {
        self.tabs.iter().find(|tab| tab.id == self.active_tab_id)
    }

    pub fn add(&mut self, url: String) -> u64 {
        let id = self.next_tab_id;
        self.next_tab_id = self.next_tab_id.wrapping_add(1).max(1);
        self.tabs.push(BrowserTabState {
            id,
            title: display_title("", &url),
            url,
        });
        self.active_tab_id = id;
        id
    }

    pub fn activate(&mut self, id: u64) -> bool {
        if self.tabs.iter().any(|tab| tab.id == id) {
            self.active_tab_id = id;
            true
        } else {
            false
        }
    }

    pub fn cycle(&mut self, delta: isize) -> Option<u64> {
        let current = self
            .tabs
            .iter()
            .position(|tab| tab.id == self.active_tab_id)? as isize;
        let next = (current + delta).rem_euclid(self.tabs.len() as isize) as usize;
        self.active_tab_id = self.tabs[next].id;
        Some(self.active_tab_id)
    }

    pub fn close(&mut self, id: u64, default_url: &str) -> Option<BrowserTabState> {
        let index = self.tabs.iter().position(|tab| tab.id == id)?;
        if self.tabs.len() == 1 {
            let tab = &mut self.tabs[0];
            tab.url = default_url.to_owned();
            tab.title = display_title("", default_url);
            self.active_tab_id = tab.id;
            return None;
        }
        let removed = self.tabs.remove(index);
        if self.active_tab_id == id {
            self.active_tab_id = self.tabs[index.min(self.tabs.len() - 1)].id;
        }
        Some(removed)
    }
}

#[cfg(any(all(feature = "desktop", windows), test))]
fn query_value(url: &Url, name: &str) -> Option<String> {
    url.query_pairs()
        .find_map(|(key, value)| (key == name).then(|| value.into_owned()))
}

#[cfg(any(all(feature = "desktop", windows), test))]
fn query_u64(url: &Url, name: &str) -> Option<u64> {
    query_value(url, name)?.parse().ok()
}

#[cfg(any(all(feature = "desktop", windows), test))]
fn query_u32(url: &Url, name: &str) -> Option<u32> {
    query_value(url, name)?.parse().ok()
}

pub fn display_title(title: &str, url: &str) -> String {
    let title = title.trim();
    if !title.is_empty() {
        return title.to_owned();
    }
    Url::parse(url)
        .ok()
        .and_then(|url| url.host_str().map(str::to_owned))
        .filter(|host| !host.is_empty())
        .unwrap_or_else(|| "New Tab".to_owned())
}

/// Returns a filesystem-safe, user-presentable filename for a browser download.
///
/// The URL path is percent-decoded first so ordinary names such as `Guide%20.pdf`
/// do not stay escaped in the Downloads folder.  It deliberately does not trust a
/// server supplied path: path separators, Windows-reserved characters and control
/// characters are converted to underscores.
#[cfg(any(all(feature = "desktop", windows), test))]
pub fn download_filename(raw_url: &str) -> String {
    let candidate = Url::parse(raw_url)
        .ok()
        .and_then(|url| {
            url.path_segments()
                .and_then(|mut segments| segments.rfind(|segment| !segment.is_empty()))
                .map(percent_decode_filename)
        })
        .unwrap_or_else(|| "download".to_owned());
    sanitize_download_filename(&candidate)
}

#[cfg(any(all(feature = "desktop", windows), test))]
pub fn unique_download_path(directory: &Path, filename: &str) -> PathBuf {
    let initial = directory.join(filename);
    if !initial.exists() {
        return initial;
    }
    let path = Path::new(filename);
    let stem = path
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or("download");
    let extension = path.extension().and_then(|value| value.to_str());
    for suffix in 1..=10_000 {
        let name = match extension {
            Some(extension) => format!("{stem} ({suffix}).{extension}"),
            None => format!("{stem} ({suffix})"),
        };
        let candidate = directory.join(name);
        if !candidate.exists() {
            return candidate;
        }
    }
    directory.join(format!("download-{}", std::process::id()))
}

#[cfg(any(all(feature = "desktop", windows), test))]
fn percent_decode_filename(value: &str) -> String {
    let bytes = value.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%' && index + 2 < bytes.len() {
            let high = (bytes[index + 1] as char).to_digit(16);
            let low = (bytes[index + 2] as char).to_digit(16);
            if let (Some(high), Some(low)) = (high, low) {
                decoded.push(((high << 4) | low) as u8);
                index += 3;
                continue;
            }
        }
        decoded.push(bytes[index]);
        index += 1;
    }
    String::from_utf8(decoded).unwrap_or_else(|_| value.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn toolbar_typography_validates_and_routes_the_ui_profile() {
        let profile = r#"{"fontFamily":"Test Sans, sans-serif","fontSize":20,"fontWeight":600}"#;
        assert_eq!(parse_browser_typography(profile).unwrap().font_size, 20.0);
        assert_eq!(
            parse_browser_typography(&profile.replace("20", "14.5"))
                .unwrap()
                .font_size,
            14.5
        );
        let mut url = Url::parse("studio-action://set-browser-typography").unwrap();
        url.query_pairs_mut().append_pair("profile", profile);
        assert!(matches!(
            parse_toolbar_action(url.as_str()),
            Some(ToolbarAction::SetTypography(_))
        ));
        for invalid in [
            profile.replace("20", "21"),
            profile.replace("600", "800"),
            profile.replace("Test Sans, sans-serif", ""),
            "{}".to_string(),
        ] {
            assert!(parse_browser_typography(&invalid).is_none());
        }
    }

    #[test]
    fn browser_platform_availability_is_explicit_and_safe() {
        #[cfg(any(target_os = "linux", target_os = "windows"))]
        assert!(embedded_browser_platform_available());
        #[cfg(not(any(target_os = "linux", target_os = "windows")))]
        assert!(!embedded_browser_platform_available());
    }

    #[test]
    fn toolbar_parser_only_accepts_known_local_actions() {
        assert_eq!(
            parse_toolbar_action("studio-action://navigate?url=https%3A%2F%2Fexample.com"),
            Some(ToolbarAction::Navigate("https://example.com".to_owned()))
        );
        assert_eq!(
            parse_toolbar_action("studio-action://set-browser-width?width=720"),
            Some(ToolbarAction::SetBrowserWidth(720))
        );
        assert_eq!(
            parse_toolbar_action("studio-action://copy-url"),
            Some(ToolbarAction::CopyUrl)
        );
        assert_eq!(
            parse_toolbar_action("studio-action://run-shell?command=whoami"),
            None
        );
        assert_eq!(parse_toolbar_action("https://example.com"), None);
    }

    #[test]
    fn hide_and_exit_have_different_runtime_lifecycles() {
        assert_eq!(BrowserLifecycle::Cold.show(), BrowserLifecycle::Visible);
        assert_eq!(BrowserLifecycle::Visible.hide(), BrowserLifecycle::Hidden);
        assert!(BrowserLifecycle::Hidden.runtime_loaded());
        assert!(!BrowserLifecycle::Hidden.visible());
        assert_eq!(BrowserLifecycle::Hidden.exit(), BrowserLifecycle::Cold);
        assert!(!BrowserLifecycle::Cold.runtime_loaded());
    }

    #[test]
    fn tab_registry_reuses_the_last_tab_and_preserves_background_tabs() {
        let mut tabs = BrowserTabRegistry::new(DEFAULT_BROWSER_URL);
        let second = tabs.add("https://example.org".to_owned());
        let third = tabs.add("https://example.net".to_owned());
        assert_eq!(tabs.tabs().len(), 3);
        assert_eq!(tabs.active_tab().map(|tab| tab.id), Some(third));
        assert_eq!(tabs.cycle(-1), Some(second));
        assert!(tabs.activate(second));
        let removed = tabs
            .close(third, DEFAULT_BROWSER_URL)
            .expect("background tab closes");
        assert_eq!(removed.id, third);
        assert_eq!(tabs.active_tab_id(), second);
        assert!(tabs.close(second, DEFAULT_BROWSER_URL).is_some());
        assert_eq!(tabs.tabs().len(), 1);
        assert!(tabs.close(1, DEFAULT_BROWSER_URL).is_none());
        assert_eq!(tabs.tabs()[0].url, DEFAULT_BROWSER_URL);
    }

    #[test]
    fn embedded_browser_is_enabled_without_tab_restore_by_default() {
        let preferences = BrowserPreferences::default();
        assert!(preferences.enabled);
        assert!(!preferences.restore_tabs);
        assert!(validate_browser_url("https://example.com", &preferences).is_ok());
    }

    #[test]
    fn allows_http_and_https_with_safe_defaults() {
        let preferences = BrowserPreferences::default();
        assert!(validate_browser_url("https://example.com/docs", &preferences).is_ok());
        assert!(validate_browser_url("http://localhost:3000/", &preferences).is_ok());
    }

    #[test]
    fn rejects_active_local_and_credentialed_urls() {
        let preferences = BrowserPreferences::default();
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
        assert_eq!(
            validate_browser_url("https://user:password@example.com", &preferences),
            Err(BrowserPolicyError::InvalidUrl)
        );
    }

    #[test]
    fn separates_localhost_from_other_private_networks() {
        let mut preferences = BrowserPreferences::default();
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

    #[test]
    fn download_names_are_decoded_sanitized_and_do_not_overwrite() {
        assert_eq!(
            download_filename("https://example.com/files/Studio%20Guide.pdf?token=1"),
            "Studio Guide.pdf"
        );
        assert_eq!(
            sanitize_download_filename("../../unsafe\\name?.txt"),
            "_.._unsafe_name_.txt"
        );
        let directory =
            std::env::temp_dir().join(format!("cts-download-test-{}", std::process::id()));
        std::fs::create_dir_all(&directory).expect("temporary download directory");
        let first = unique_download_path(&directory, "report.pdf");
        std::fs::write(&first, b"first").expect("first file");
        assert_eq!(
            unique_download_path(&directory, "report.pdf"),
            directory.join("report (1).pdf")
        );
        let _ = std::fs::remove_dir_all(directory);
    }
}
