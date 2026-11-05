//! Platform configuration location; independent of any UI preferences.
use std::path::PathBuf;

pub fn directory() -> Option<PathBuf> {
    if let Some(path) = std::env::var_os("MARKNORIA_CONFIG_DIR").filter(|path| !path.is_empty()) {
        return Some(path.into());
    }
    #[cfg(windows)]
    {
        return std::env::var_os("APPDATA").map(|root| PathBuf::from(root).join("MarkNoria"));
    }
    #[cfg(target_os = "macos")]
    {
        return std::env::var_os("HOME")
            .map(|root| PathBuf::from(root).join("Library/Application Support/MarkNoria"));
    }
    #[cfg(not(any(windows, target_os = "macos")))]
    {
        std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .filter(|path| path.is_absolute())
            .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".config")))
            .map(|path| path.join("marknoria"))
    }
}
