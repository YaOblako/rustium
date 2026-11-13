use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateInfo {
    pub has_update: bool,
    pub current_version: String,
    pub latest_version: String,
    pub download_url: String,
    pub release_notes: String,
    pub html_url: String,
}

#[derive(Deserialize)]
struct GitHubRelease {
    tag_name: String,
    html_url: String,
    body: Option<String>,
    assets: Vec<GitHubAsset>,
}

#[derive(Deserialize)]
struct GitHubAsset {
    name: String,
    browser_download_url: String,
}

pub struct UpdaterManager;

impl UpdaterManager {
    pub async fn check_for_updates(current_version: &str) -> Option<UpdateInfo> {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(8))
            .build()
            .ok()?;

        let resp = client
            .get("https://api.github.com/repos/YaOblako/rustium/releases/latest")
            .header("User-Agent", "Rustium-Browser")
            .header("Accept", "application/vnd.github.v3+json")
            .send()
            .await
            .ok()?;

        if !resp.status().is_success() {
            return None;
        }

        let release: GitHubRelease = resp.json().await.ok()?;
        let latest_ver = release.tag_name.trim_start_matches('v').to_string();

        let has_update = is_newer_version(&latest_ver, current_version);

        let download_url = release
            .assets
            .iter()
            .find(|a| a.name.ends_with(".exe") && a.name.contains("Setup"))
            .map(|a| a.browser_download_url.clone())
            .or_else(|| {
                release
                    .assets
                    .iter()
                    .find(|a| a.name.ends_with(".exe"))
                    .map(|a| a.browser_download_url.clone())
            })
            .unwrap_or_else(|| release.html_url.clone());

        Some(UpdateInfo {
            has_update,
            current_version: current_version.to_string(),
            latest_version: latest_ver,
            download_url,
            release_notes: release.body.unwrap_or_else(|| "Обновление производительности и стабильности".to_string()),
            html_url: release.html_url,
        })
    }

    pub async fn download_and_run_installer(download_url: &str) -> Result<(), String> {
        let temp_installer = std::env::temp_dir().join("Rustium-Setup-Latest.exe");

        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(120))
            .build()
            .map_err(|e| e.to_string())?;

        let resp = client
            .get(download_url)
            .header("User-Agent", "Rustium-Browser")
            .send()
            .await
            .map_err(|e| e.to_string())?;

        if !resp.status().is_success() {
            return Err(format!("Ошибка загрузки обновления: HTTP {}", resp.status()));
        }

        let bytes = resp.bytes().await.map_err(|e| e.to_string())?;
        std::fs::write(&temp_installer, bytes).map_err(|e| e.to_string())?;

        // Launch installer
        #[cfg(windows)]
        {
            use windows_sys::Win32::UI::Shell::ShellExecuteW;
            use std::ptr::null_mut;

            let file_w: Vec<u16> = temp_installer.to_str().unwrap_or("").encode_utf16().chain(std::iter::once(0)).collect();
            let op_w: Vec<u16> = "open\0".encode_utf16().collect();
            unsafe {
                ShellExecuteW(0, op_w.as_ptr(), file_w.as_ptr(), null_mut(), null_mut(), 1);
            }
        }

        Ok(())
    }
}

fn is_newer_version(latest: &str, current: &str) -> bool {
    let parse_parts = |v: &str| -> Vec<u32> {
        v.split('.')
            .filter_map(|p| p.chars().take_while(|c| c.is_ascii_digit()).collect::<String>().parse().ok())
            .collect()
    };

    let lat_parts = parse_parts(latest);
    let cur_parts = parse_parts(current);

    for (l, c) in lat_parts.iter().zip(cur_parts.iter()) {
        if l > c {
            return true;
        } else if l < c {
            return false;
        }
    }

    lat_parts.len() > cur_parts.len()
}
