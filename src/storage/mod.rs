use rusqlite::{params, Connection, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrowserSettings {
    pub theme: String, // "md3_dark", "md3_light", "amoled", "catppuccin", "nord"
    pub accent_color: String, // Hex seed color, e.g. "#6750A4"
    pub search_engine: String, // "google", "duckduckgo"
    pub adblock_enabled: bool,
    pub dark_mode_mode: String, // "system", "always_on", "always_off"
    pub dark_mode_brightness: u32, // 50..=150 (default 90)
    pub dark_mode_contrast: u32,   // 50..=150 (default 100)
    pub multithread_downloader_enabled: bool,
    pub download_threads: u32, // 2..=16 (default 4)
    #[serde(default = "default_first_run")]
    pub first_run: bool,
    #[serde(default = "default_gradient")]
    pub gradient: String,
    #[serde(default = "default_bg_type")]
    pub bg_type: String, // "gradient", "gif", "custom"
    #[serde(default = "default_gif_preset")]
    pub gif_preset: String, // "cyberpunk", "nebula", "retro_sunset", "rain_window", "lofi_room", "minimal_waves"
    #[serde(default = "default_custom_bg_url")]
    pub custom_bg_url: String,
    #[serde(default = "default_bg_opacity")]
    pub bg_opacity: u32, // 0..=100 (overlay tint opacity %, default 40)
    #[serde(default = "default_bg_blur")]
    pub bg_blur: u32, // 0..=30 (blur px, default 0)
    #[serde(default = "default_frosted_glass_enabled")]
    pub frosted_glass_enabled: bool,
    #[serde(default = "default_frosted_glass_blur")]
    pub frosted_glass_blur: u32,
    #[serde(default)]
    pub site_exclusions: Vec<String>,
}

fn default_first_run() -> bool {
    true
}

fn default_gradient() -> String {
    "aurora".to_string()
}

fn default_bg_type() -> String {
    "gradient".to_string()
}

fn default_gif_preset() -> String {
    "cyberpunk".to_string()
}

fn default_custom_bg_url() -> String {
    String::new()
}

fn default_bg_opacity() -> u32 {
    40
}

fn default_bg_blur() -> u32 {
    0
}

fn default_frosted_glass_enabled() -> bool {
    true
}

fn default_frosted_glass_blur() -> u32 {
    16
}

impl Default for BrowserSettings {
    fn default() -> Self {
        Self {
            theme: "md3_dark".to_string(),
            accent_color: "#6750A4".to_string(),
            search_engine: "google".to_string(),
            adblock_enabled: true,
            dark_mode_mode: "system".to_string(),
            dark_mode_brightness: 90,
            dark_mode_contrast: 100,
            multithread_downloader_enabled: true,
            download_threads: 4,
            first_run: true,
            gradient: "aurora".to_string(),
            bg_type: "gradient".to_string(),
            gif_preset: "cyberpunk".to_string(),
            custom_bg_url: String::new(),
            bg_opacity: 40,
            bg_blur: 0,
            frosted_glass_enabled: true,
            frosted_glass_blur: 16,
            site_exclusions: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryEntry {
    pub id: i64,
    pub url: String,
    pub title: String,
    pub visit_time: String,
    pub visit_count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BookmarkEntry {
    pub id: i64,
    pub title: String,
    pub url: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionTab {
    pub id: String,
    pub url: String,
    pub title: String,
    pub is_pinned: bool,
    pub is_active: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoginEntry {
    pub id: i64,
    pub url: String,
    pub username: String,
    pub password: String,
    pub created_at: String,
}

pub struct StorageManager {
    conn: Arc<Mutex<Connection>>,
}

#[allow(dead_code)]
impl StorageManager {
    pub fn new(db_path: Option<PathBuf>) -> Result<Self> {
        let path = db_path.unwrap_or_else(|| {
            let mut p = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
            p.push("rustium_data.db");
            p
        });

        let conn = Connection::open(path)?;
        let manager = Self {
            conn: Arc::new(Mutex::new(conn)),
        };
        manager.init_tables()?;
        Ok(manager)
    }

    fn init_tables(&self) -> Result<()> {
        let conn = self.conn.lock().unwrap();

        conn.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS settings (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS history (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                url TEXT NOT NULL,
                title TEXT NOT NULL,
                visit_time DATETIME DEFAULT CURRENT_TIMESTAMP,
                visit_count INTEGER DEFAULT 1
            );

            CREATE TABLE IF NOT EXISTS bookmarks (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                title TEXT NOT NULL,
                url TEXT NOT NULL UNIQUE,
                created_at DATETIME DEFAULT CURRENT_TIMESTAMP
            );

            CREATE TABLE IF NOT EXISTS sessions (
                id TEXT PRIMARY KEY,
                url TEXT NOT NULL,
                title TEXT NOT NULL,
                is_pinned INTEGER DEFAULT 0,
                is_active INTEGER DEFAULT 0,
                order_idx INTEGER DEFAULT 0
            );

            CREATE TABLE IF NOT EXISTS adblock_whitelist (
                domain TEXT PRIMARY KEY
            );

            CREATE TABLE IF NOT EXISTS dark_mode_whitelist (
                domain TEXT PRIMARY KEY
            );

            CREATE TABLE IF NOT EXISTS logins (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                url TEXT NOT NULL,
                username TEXT NOT NULL,
                password TEXT NOT NULL,
                created_at DATETIME DEFAULT CURRENT_TIMESTAMP
            );
            ",
        )?;

        Ok(())
    }

    pub fn get_settings(&self) -> BrowserSettings {
        let conn = self.conn.lock().unwrap();
        let mut settings = BrowserSettings::default();

        let mut stmt = conn
            .prepare("SELECT key, value FROM settings")
            .unwrap_or_else(|_| conn.prepare("SELECT '' as key, '' as value").unwrap());

        if let Ok(rows) = stmt.query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        }) {
            for item in rows.flatten() {
                match item.0.as_str() {
                    "theme" => settings.theme = item.1,
                    "accent_color" => settings.accent_color = item.1,
                    "search_engine" => settings.search_engine = item.1,
                    "adblock_enabled" => settings.adblock_enabled = item.1 == "1",
                    "dark_mode_mode" => settings.dark_mode_mode = item.1,
                    "dark_mode_brightness" => {
                        if let Ok(v) = item.1.parse() {
                            settings.dark_mode_brightness = v;
                        }
                    }
                    "dark_mode_contrast" => {
                        if let Ok(v) = item.1.parse() {
                            settings.dark_mode_contrast = v;
                        }
                    }
                    "multithread_downloader_enabled" => {
                        settings.multithread_downloader_enabled = item.1 == "1"
                    }
                    "download_threads" => {
                        if let Ok(v) = item.1.parse() {
                            settings.download_threads = v;
                        }
                    }
                    "first_run" => {
                        settings.first_run = item.1 == "1";
                    }
                    "gradient" => {
                        settings.gradient = item.1;
                    }
                    "bg_type" => {
                        settings.bg_type = item.1;
                    }
                    "gif_preset" => {
                        settings.gif_preset = item.1;
                    }
                    "custom_bg_url" => {
                        settings.custom_bg_url = item.1;
                    }
                    "bg_opacity" => {
                        if let Ok(v) = item.1.parse() {
                            settings.bg_opacity = v;
                        }
                    }
                    "bg_blur" => {
                        if let Ok(v) = item.1.parse() {
                            settings.bg_blur = v;
                        }
                    }
                    "frosted_glass_enabled" => {
                        settings.frosted_glass_enabled = item.1 == "1";
                    }
                    "frosted_glass_blur" => {
                        if let Ok(v) = item.1.parse() {
                            settings.frosted_glass_blur = v;
                        }
                    }
                    "site_exclusions" => {
                        if let Ok(list) = serde_json::from_str(&item.1) {
                            settings.site_exclusions = list;
                        }
                    }
                    _ => {}
                }
            }
        }

        settings
    }

    pub fn save_settings(&self, settings: &BrowserSettings) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        let pairs = [
            ("theme", settings.theme.clone()),
            ("accent_color", settings.accent_color.clone()),
            ("search_engine", settings.search_engine.clone()),
            (
                "adblock_enabled",
                if settings.adblock_enabled {
                    "1".to_string()
                } else {
                    "0".to_string()
                },
            ),
            ("dark_mode_mode", settings.dark_mode_mode.clone()),
            (
                "dark_mode_brightness",
                settings.dark_mode_brightness.to_string(),
            ),
            ("dark_mode_contrast", settings.dark_mode_contrast.to_string()),
            (
                "multithread_downloader_enabled",
                if settings.multithread_downloader_enabled {
                    "1".to_string()
                } else {
                    "0".to_string()
                },
            ),
            ("download_threads", settings.download_threads.to_string()),
            (
                "first_run",
                if settings.first_run {
                    "1".to_string()
                } else {
                    "0".to_string()
                },
            ),
            ("gradient", settings.gradient.clone()),
            ("bg_type", settings.bg_type.clone()),
            ("gif_preset", settings.gif_preset.clone()),
            ("custom_bg_url", settings.custom_bg_url.clone()),
            ("bg_opacity", settings.bg_opacity.to_string()),
            ("bg_blur", settings.bg_blur.to_string()),
            (
                "frosted_glass_enabled",
                if settings.frosted_glass_enabled {
                    "1".to_string()
                } else {
                    "0".to_string()
                },
            ),
            ("frosted_glass_blur", settings.frosted_glass_blur.to_string()),
            (
                "site_exclusions",
                serde_json::to_string(&settings.site_exclusions).unwrap_or_else(|_| "[]".to_string()),
            ),
        ];

        for (k, v) in pairs {
            conn.execute(
                "INSERT INTO settings (key, value) VALUES (?1, ?2)
                 ON CONFLICT(key) DO UPDATE SET value = ?2",
                params![k, v],
            )?;
        }

        Ok(())
    }

    pub fn add_history(&self, url: &str, title: &str) -> Result<()> {
        if url.starts_with("rustium://") || url == "about:blank" || url.is_empty() {
            return Ok(());
        }
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO history (url, title, visit_time, visit_count)
             VALUES (?1, ?2, CURRENT_TIMESTAMP, 1)",
            params![url, title],
        )?;
        Ok(())
    }

    pub fn get_history(&self, limit: usize) -> Vec<HistoryEntry> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn
            .prepare(
                "SELECT id, url, title, datetime(visit_time, 'localtime'), visit_count
                 FROM history ORDER BY id DESC LIMIT ?1",
            )
            .unwrap();

        let rows = stmt
            .query_map(params![limit as i64], |row| {
                Ok(HistoryEntry {
                    id: row.get(0)?,
                    url: row.get(1)?,
                    title: row.get(2)?,
                    visit_time: row.get(3)?,
                    visit_count: row.get(4)?,
                })
            })
            .unwrap();

        rows.flatten().collect()
    }

    pub fn clear_history(&self) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM history", [])?;
        Ok(())
    }

    pub fn add_bookmark(&self, title: &str, url: &str) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO bookmarks (title, url, created_at)
             VALUES (?1, ?2, CURRENT_TIMESTAMP)
             ON CONFLICT(url) DO UPDATE SET title = ?1",
            params![title, url],
        )?;
        Ok(())
    }

    pub fn remove_bookmark(&self, url: &str) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM bookmarks WHERE url = ?1", params![url])?;
        Ok(())
    }

    pub fn get_bookmarks(&self) -> Vec<BookmarkEntry> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn
            .prepare("SELECT id, title, url, datetime(created_at, 'localtime') FROM bookmarks ORDER BY id DESC")
            .unwrap();

        let rows = stmt
            .query_map([], |row| {
                Ok(BookmarkEntry {
                    id: row.get(0)?,
                    title: row.get(1)?,
                    url: row.get(2)?,
                    created_at: row.get(3)?,
                })
            })
            .unwrap();

        rows.flatten().collect()
    }

    pub fn is_bookmarked(&self, url: &str) -> bool {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn
            .prepare("SELECT 1 FROM bookmarks WHERE url = ?1 LIMIT 1")
            .unwrap();
        stmt.exists(params![url]).unwrap_or(false)
    }

    pub fn is_adblock_whitelisted(&self, domain: &str) -> bool {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn
            .prepare("SELECT 1 FROM adblock_whitelist WHERE domain = ?1 LIMIT 1")
            .unwrap();
        stmt.exists(params![domain]).unwrap_or(false)
    }

    pub fn set_adblock_whitelisted(&self, domain: &str, whitelisted: bool) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        if whitelisted {
            conn.execute(
                "INSERT OR IGNORE INTO adblock_whitelist (domain) VALUES (?1)",
                params![domain],
            )?;
        } else {
            conn.execute(
                "DELETE FROM adblock_whitelist WHERE domain = ?1",
                params![domain],
            )?;
        }
        Ok(())
    }

    pub fn is_dark_mode_whitelisted(&self, domain: &str) -> bool {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn
            .prepare("SELECT 1 FROM dark_mode_whitelist WHERE domain = ?1 LIMIT 1")
            .unwrap();
        stmt.exists(params![domain]).unwrap_or(false)
    }

    pub fn set_dark_mode_whitelisted(&self, domain: &str, whitelisted: bool) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        if whitelisted {
            conn.execute(
                "INSERT OR IGNORE INTO dark_mode_whitelist (domain) VALUES (?1)",
                params![domain],
            )?;
        } else {
            conn.execute(
                "DELETE FROM dark_mode_whitelist WHERE domain = ?1",
                params![domain],
            )?;
        }
        Ok(())
    }

    pub fn save_session_tabs(&self, tabs: &[SessionTab]) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM sessions", [])?;
        for (idx, tab) in tabs.iter().enumerate() {
            conn.execute(
                "INSERT INTO sessions (id, url, title, is_pinned, is_active, order_idx)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![
                    tab.id,
                    tab.url,
                    tab.title,
                    if tab.is_pinned { 1 } else { 0 },
                    if tab.is_active { 1 } else { 0 },
                    idx as i64
                ],
            )?;
        }
        Ok(())
    }

    pub fn load_session_tabs(&self) -> Vec<SessionTab> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn
            .prepare(
                "SELECT id, url, title, is_pinned, is_active FROM sessions ORDER BY order_idx ASC",
            )
            .unwrap();

        let rows = stmt
            .query_map([], |row| {
                Ok(SessionTab {
                    id: row.get(0)?,
                    url: row.get(1)?,
                    title: row.get(2)?,
                    is_pinned: row.get::<_, i64>(3)? == 1,
                    is_active: row.get::<_, i64>(4)? == 1,
                })
            })
            .unwrap();

        rows.flatten().collect()
    }

    pub fn add_login(&self, url: &str, username: &str, password: &str) -> Result<i64> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO logins (url, username, password) VALUES (?1, ?2, ?3)",
            params![url, username, password],
        )?;
        Ok(conn.last_insert_rowid())
    }

    pub fn get_logins(&self) -> Vec<LoginEntry> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn
            .prepare("SELECT id, url, username, password, created_at FROM logins ORDER BY id DESC")
            .unwrap();

        let rows = stmt
            .query_map([], |row| {
                Ok(LoginEntry {
                    id: row.get(0)?,
                    url: row.get(1)?,
                    username: row.get(2)?,
                    password: row.get(3)?,
                    created_at: row.get(4)?,
                })
            })
            .unwrap();

        rows.flatten().collect()
    }

    pub fn delete_login(&self, id: i64) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM logins WHERE id = ?1", params![id])?;
        Ok(())
    }

    pub fn import_passwords_csv(&self, csv_data: &str) -> Result<usize> {
        let mut count = 0;
        let conn = self.conn.lock().unwrap();

        for line in csv_data.lines() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }

            let parts: Vec<&str> = line.split(',').map(|s| s.trim().trim_matches('"')).collect();
            // Detect header line
            if parts.iter().any(|&p| p.eq_ignore_ascii_case("username") || p.eq_ignore_ascii_case("password")) {
                continue;
            }

            // Chrome CSV format: name,url,username,password or url,username,password
            let (url, username, password) = if parts.len() >= 4 {
                (parts[1], parts[2], parts[3])
            } else if parts.len() >= 3 {
                (parts[0], parts[1], parts[2])
            } else {
                continue;
            };

            if !url.is_empty() && (!username.is_empty() || !password.is_empty()) {
                let res = conn.execute(
                    "INSERT INTO logins (url, username, password) VALUES (?1, ?2, ?3)",
                    params![url, username, password],
                );
                if res.is_ok() {
                    count += 1;
                }
            }
        }

        Ok(count)
    }

    pub fn add_site_exclusion(&self, domain: &str) -> Result<()> {
        let mut settings = self.get_settings();
        let dom = domain.trim().to_lowercase();
        if !dom.is_empty() && !settings.site_exclusions.contains(&dom) {
            settings.site_exclusions.push(dom);
            self.save_settings(&settings)?;
        }
        Ok(())
    }

    pub fn remove_site_exclusion(&self, domain: &str) -> Result<()> {
        let mut settings = self.get_settings();
        let dom = domain.trim().to_lowercase();
        settings.site_exclusions.retain(|d| d != &dom);
        self.save_settings(&settings)?;
        Ok(())
    }
}
