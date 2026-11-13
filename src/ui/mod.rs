pub const CHROME_HTML: &str = include_str!("chrome.html");
pub const PAGE_NEWTAB: &str = include_str!("pages/newtab.html");
pub const PAGE_SETTINGS: &str = include_str!("pages/settings.html");
pub const PAGE_DOWNLOADS: &str = include_str!("pages/downloads.html");
pub const PAGE_HISTORY: &str = include_str!("pages/history.html");
pub const PAGE_BOOKMARKS: &str = include_str!("pages/bookmarks.html");
pub const PAGE_EXTENSIONS: &str = include_str!("pages/extensions.html");
pub const PAGE_PASSWORDS: &str = include_str!("pages/passwords.html");
pub const PAGE_SCAM_WARNING: &str = include_str!("pages/scam_warning.html");

pub fn get_internal_page(path: &str) -> Option<&'static str> {
    match path {
        "newtab" | "home" | "" => Some(PAGE_NEWTAB),
        "settings" => Some(PAGE_SETTINGS),
        "downloads" => Some(PAGE_DOWNLOADS),
        "history" => Some(PAGE_HISTORY),
        "bookmarks" => Some(PAGE_BOOKMARKS),
        "extensions" => Some(PAGE_EXTENSIONS),
        "passwords" | "logins" => Some(PAGE_PASSWORDS),
        "warning" | "scam" => Some(PAGE_SCAM_WARNING),
        _ => None,
    }
}
