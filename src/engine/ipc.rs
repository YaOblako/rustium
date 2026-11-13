use crate::storage::BrowserSettings;
use serde::Deserialize;

#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "action")]
pub enum IpcMessageFromUi {
    #[serde(rename = "navigate")]
    Navigate { tab_id: Option<String>, url: String },

    #[serde(rename = "go_back")]
    GoBack { tab_id: Option<String> },

    #[serde(rename = "go_forward")]
    GoForward { tab_id: Option<String> },

    #[serde(rename = "reload")]
    Reload { tab_id: Option<String> },

    #[serde(rename = "create_tab")]
    CreateTab {
        url: Option<String>,
        title: Option<String>,
        background: Option<bool>,
    },

    #[serde(rename = "close_tab")]
    CloseTab { tab_id: String },

    #[serde(rename = "switch_tab")]
    SwitchTab { tab_id: String },

    #[serde(rename = "pin_tab")]
    PinTab { tab_id: String, is_pinned: bool },

    #[serde(rename = "toggle_adblock")]
    ToggleAdblock { domain: String, enabled: bool },

    #[serde(rename = "toggle_dark_mode")]
    ToggleDarkMode {
        domain: String,
        mode: Option<String>,
        brightness: Option<u32>,
        contrast: Option<u32>,
    },

    #[serde(rename = "start_download")]
    StartDownload { url: String, filename: Option<String> },

    #[serde(rename = "pause_download")]
    PauseDownload { id: String },

    #[serde(rename = "resume_download")]
    ResumeDownload { id: String },

    #[serde(rename = "cancel_download")]
    CancelDownload { id: String },

    #[serde(rename = "open_download_file")]
    OpenDownloadFile { id: String },

    #[serde(rename = "show_in_folder")]
    ShowInFolder { id: String },

    #[serde(rename = "open_downloads_folder")]
    OpenDownloadsFolder,

    #[serde(rename = "load_unpacked_extension")]
    LoadUnpackedExtension { path: String },

    #[serde(rename = "toggle_extension")]
    ToggleExtension { id: String, enabled: bool },

    #[serde(rename = "update_settings")]
    UpdateSettings { settings: BrowserSettings },

    #[serde(rename = "add_bookmark")]
    AddBookmark { title: String, url: String },

    #[serde(rename = "remove_bookmark")]
    RemoveBookmark { url: String },

    #[serde(rename = "clear_history")]
    ClearHistory,

    #[serde(rename = "import_passwords")]
    ImportPasswords { csv_content: String },

    #[serde(rename = "add_login")]
    AddLogin { url: String, username: String, password: String },

    #[serde(rename = "delete_login")]
    DeleteLogin { id: i64 },

    #[serde(rename = "get_logins")]
    GetLogins,

    #[serde(rename = "open_devtools")]
    OpenDevtools,

    #[serde(rename = "open_settings_window")]
    OpenSettingsWindow,

    #[serde(rename = "minimize_window")]
    MinimizeWindow,

    #[serde(rename = "maximize_window")]
    MaximizeWindow,

    #[serde(rename = "close_window")]
    CloseWindow,

    #[serde(rename = "drag_window")]
    DragWindow,

    #[serde(rename = "expand_chrome")]
    ExpandChrome { height: u32 },

    #[serde(rename = "collapse_chrome")]
    CollapseChrome,

    #[serde(rename = "get_state")]
    GetState,

    #[serde(rename = "check_for_updates")]
    CheckForUpdates,

    #[serde(rename = "install_update")]
    InstallUpdate { download_url: String },

    #[serde(rename = "add_site_exclusion")]
    AddSiteExclusion { domain: String },

    #[serde(rename = "remove_site_exclusion")]
    RemoveSiteExclusion { domain: String },
}

pub fn resolve_user_input_to_url(input: &str, search_engine: &str) -> String {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return "rustium://newtab".to_string();
    }

    if trimmed.starts_with("rustium://") || trimmed == "about:blank" {
        return trimmed.to_string();
    }

    if trimmed.starts_with("http://") || trimmed.starts_with("https://") {
        return trimmed.to_string();
    }

    if trimmed.starts_with("localhost:")
        || trimmed.starts_with("127.0.0.1")
        || trimmed.starts_with("0.0.0.0")
    {
        return format!("http://{}", trimmed);
    }

    // Check if input looks like a valid hostname or URL without protocol
    let is_likely_domain = !trimmed.contains(' ')
        && (trimmed.contains('.') || trimmed.contains(':'))
        && trimmed
            .split('.')
            .last()
            .map(|tld| tld.len() >= 2 && tld.chars().all(|c| c.is_alphabetic()))
            .unwrap_or(false);

    if is_likely_domain {
        return format!("https://{}", trimmed);
    }

    // Default search engine lookup (Google or DuckDuckGo)
    let encoded = url::form_urlencoded::byte_serialize(trimmed.as_bytes()).collect::<String>();
    match search_engine {
        "duckduckgo" => format!("https://duckduckgo.com/?q={}", encoded),
        "google" | _ => format!("https://www.google.com/search?q={}", encoded),
    }
}
