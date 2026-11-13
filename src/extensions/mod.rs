use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::RwLock;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Extension {
    pub id: String,
    pub name: String,
    pub version: String,
    pub description: String,
    pub enabled: bool,
    pub icon: Option<String>,
    pub content_scripts: Vec<ContentScript>,
    pub path: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContentScript {
    pub matches: Vec<String>,
    pub js: Vec<String>,
    pub css: Vec<String>,
    pub run_at: String, // "document_start", "document_end", "document_idle"
}

// Manifest representation for JSON deserialization
#[allow(dead_code)]
#[derive(Debug, Deserialize)]
struct ManifestJson {
    pub name: String,
    pub version: String,
    pub description: Option<String>,
    pub manifest_version: Option<u32>,
    pub icons: Option<HashMap<String, String>>,
    pub content_scripts: Option<Vec<ManifestContentScript>>,
}

#[derive(Debug, Deserialize)]
struct ManifestContentScript {
    pub matches: Vec<String>,
    pub js: Option<Vec<String>>,
    pub css: Option<Vec<String>>,
    pub run_at: Option<String>,
}

pub struct ExtensionManager {
    extensions: RwLock<HashMap<String, Extension>>,
    extensions_dir: PathBuf,
}

impl ExtensionManager {
    pub fn new(dir: Option<PathBuf>) -> Self {
        let extensions_dir = dir.unwrap_or_else(|| {
            let mut p = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
            p.push("extensions");
            p
        });

        let manager = Self {
            extensions: RwLock::new(HashMap::new()),
            extensions_dir,
        };

        // Create sample extension if not exists
        manager.ensure_sample_extension();
        manager.scan_extensions();

        manager
    }

    fn ensure_sample_extension(&self) {
        let sample_dir = self.extensions_dir.join("sample_helper");
        if !sample_dir.exists() {
            let _ = fs::create_dir_all(&sample_dir);

            let manifest = r#"{
  "manifest_version": 3,
  "name": "Rustium Assistant",
  "version": "1.0.0",
  "description": "Официальный помощник Rustium: отображает индикатор безопасности и горячие клавиши",
  "content_scripts": [
    {
      "matches": ["<all_urls>"],
      "css": ["content.css"],
      "js": ["content.js"],
      "run_at": "document_idle"
    }
  ]
}"#;
            let css = r#"/* Rustium Assistant Badge */
#rustium-page-assistant-badge {
    position: fixed;
    bottom: 16px;
    right: 16px;
    background: #1b2123;
    color: #a8c7fa;
    font-family: system-ui, -apple-system, sans-serif;
    font-size: 11px;
    font-weight: 500;
    padding: 6px 14px;
    border-radius: 9999px;
    box-shadow: 0 4px 12px rgba(0, 0, 0, 0.35);
    z-index: 2147483647;
    pointer-events: none;
    opacity: 0.85;
    transition: opacity 0.2s ease;
}
"#;
            let js = r#"// Rustium Assistant Content Script
(function() {
    console.log("[Rustium Extension] Rustium Assistant loaded for: " + window.location.href);

    // Test storage API
    if (window.chrome && window.chrome.storage && window.chrome.storage.local) {
        window.chrome.storage.local.get("visit_count", function(res) {
            const count = (res.visit_count || 0) + 1;
            window.chrome.storage.local.set({ visit_count: count });
        });
    }

    // Floating non-intrusive badge showing Rustium is protecting the page
    if (!document.getElementById("rustium-page-assistant-badge")) {
        const badge = document.createElement("div");
        badge.id = "rustium-page-assistant-badge";
        badge.textContent = "🛡️ Rustium Protected";
        (document.body || document.documentElement).appendChild(badge);

        setTimeout(() => {
            if (badge.parentNode) {
                badge.style.opacity = "0";
                setTimeout(() => badge.remove(), 400);
            }
        }, 3000);
    }
})();
"#;

            let _ = fs::write(sample_dir.join("manifest.json"), manifest);
            let _ = fs::write(sample_dir.join("content.css"), css);
            let _ = fs::write(sample_dir.join("content.js"), js);
        }
    }

    pub fn scan_extensions(&self) {
        if !self.extensions_dir.exists() {
            return;
        }

        if let Ok(entries) = fs::read_dir(&self.extensions_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() && path.join("manifest.json").exists() {
                    let _ = self.load_unpacked_extension(&path);
                }
            }
        }
    }

    pub fn load_unpacked_extension(&self, folder_path: &Path) -> Result<Extension, String> {
        let manifest_path = folder_path.join("manifest.json");
        if !manifest_path.exists() {
            return Err("Файл manifest.json не найден".to_string());
        }

        let content = fs::read_to_string(&manifest_path)
            .map_err(|e| format!("Не удалось прочитать manifest.json: {}", e))?;

        let manifest: ManifestJson = serde_json::from_str(&content)
            .map_err(|e| format!("Ошибка парсинга manifest.json: {}", e))?;

        let folder_name = folder_path
            .file_name()
            .and_then(|f| f.to_str())
            .unwrap_or("ext")
            .to_string();

        let id = format!("ext_{}", folder_name);

        let mut content_scripts = Vec::new();
        if let Some(scripts) = manifest.content_scripts {
            for s in scripts {
                content_scripts.push(ContentScript {
                    matches: s.matches,
                    js: s.js.unwrap_or_default(),
                    css: s.css.unwrap_or_default(),
                    run_at: s.run_at.unwrap_or_else(|| "document_idle".to_string()),
                });
            }
        }

        let ext = Extension {
            id: id.clone(),
            name: manifest.name,
            version: manifest.version,
            description: manifest
                .description
                .unwrap_or_else(|| "Пользовательское расширение".to_string()),
            enabled: true,
            icon: None,
            content_scripts,
            path: folder_path.to_path_buf(),
        };

        let mut lock = self.extensions.write().unwrap();
        lock.insert(id, ext.clone());

        Ok(ext)
    }

    pub fn set_extension_enabled(&self, id: &str, enabled: bool) -> bool {
        let mut lock = self.extensions.write().unwrap();
        if let Some(ext) = lock.get_mut(id) {
            ext.enabled = enabled;
            return true;
        }
        false
    }

    pub fn list_extensions(&self) -> Vec<Extension> {
        let lock = self.extensions.read().unwrap();
        lock.values().cloned().collect()
    }

    pub fn matches_pattern(pattern: &str, url: &str) -> bool {
        if pattern == "<all_urls>" {
            return !url.starts_with("rustium://");
        }

        if let Ok(target) = url::Url::parse(url) {
            if pattern.starts_with("*://*/*") || pattern == "*://*" {
                return target.scheme() == "http" || target.scheme() == "https";
            }

            // Simple wildcard host/path matching
            if let Some(host_part) = pattern.strip_prefix("*://") {
                let parts: Vec<&str> = host_part.splitn(2, '/').collect();
                let host_pattern = parts[0];

                if host_pattern == "*" {
                    return true;
                }

                if let Some(domain_suffix) = host_pattern.strip_prefix("*.") {
                    if let Some(host) = target.host_str() {
                        return host == domain_suffix || host.ends_with(&format!(".{}", domain_suffix));
                    }
                } else if let Some(host) = target.host_str() {
                    return host == host_pattern;
                }
            }
        }

        false
    }

    pub fn get_injected_scripts_for_url(&self, url: &str) -> String {
        if url.starts_with("rustium://") || url == "about:blank" || url.is_empty() {
            return String::new();
        }

        let lock = self.extensions.read().unwrap();
        let mut combined_js = String::new();

        for ext in lock.values() {
            if !ext.enabled {
                continue;
            }

            for script in &ext.content_scripts {
                let mut matches = false;
                for pat in &script.matches {
                    if Self::matches_pattern(pat, url) {
                        matches = true;
                        break;
                    }
                }

                if !matches {
                    continue;
                }

                // Collect CSS
                let mut css_payload = String::new();
                for css_file in &script.css {
                    let css_path = ext.path.join(css_file);
                    if let Ok(c) = fs::read_to_string(css_path) {
                        css_payload.push_str(&c);
                        css_payload.push('\n');
                    }
                }

                // Collect JS
                let mut js_payload = String::new();
                for js_file in &script.js {
                    let js_path = ext.path.join(js_file);
                    if let Ok(j) = fs::read_to_string(js_path) {
                        js_payload.push_str(&j);
                        js_payload.push('\n');
                    }
                }

                let ext_id = &ext.id;
                let css_escaped = css_payload.replace('\\', "\\\\").replace('`', "\\`");

                let wrapper = format!(
                    r#"(function() {{
                        // WebExtensions API Shim for extension: {ext_id}
                        if (!window.chrome) window.chrome = {{}};
                        if (!window.browser) window.browser = window.chrome;
                        if (!window.chrome.runtime) {{
                            window.chrome.runtime = {{
                                id: "{ext_id}",
                                getURL: function(path) {{ return "rustium-extension://{ext_id}/" + path; }},
                                sendMessage: function(msg, cb) {{
                                    if (window.ipc) {{
                                        window.ipc.postMessage(JSON.stringify({{ action: "extension_msg", id: "{ext_id}", data: msg }}));
                                    }}
                                    if (cb) cb({{ status: "ok" }});
                                }}
                            }};
                        }}
                        if (!window.chrome.storage) {{
                            window.chrome.storage = {{
                                local: {{
                                    get: function(key, cb) {{
                                        let res = {{}};
                                        try {{
                                            let val = localStorage.getItem("__rustium_ext_{ext_id}__");
                                            res = val ? JSON.parse(val) : {{}};
                                        }} catch(e) {{}}
                                        if (cb) cb(res);
                                        return Promise.resolve(res);
                                    }},
                                    set: function(obj, cb) {{
                                        try {{
                                            let current = localStorage.getItem("__rustium_ext_{ext_id}__");
                                            let map = current ? JSON.parse(current) : {{}};
                                            Object.assign(map, obj);
                                            localStorage.setItem("__rustium_ext_{ext_id}__", JSON.stringify(map));
                                        }} catch(e) {{}}
                                        if (cb) cb();
                                        return Promise.resolve();
                                    }}
                                }}
                            }};
                        }}

                        // Inject CSS
                        if (`{css_escaped}`.trim().length > 0) {{
                            const st = document.createElement("style");
                            st.id = "ext-css-{ext_id}";
                            st.textContent = `{css_escaped}`;
                            (document.head || document.documentElement).appendChild(st);
                        }}

                        // Inject JS payload
                        try {{
                            {js_payload}
                        }} catch(err) {{
                            console.error("[Rustium Extension Error: {ext_id}]", err);
                        }}
                    }})();
                    "#
                );

                combined_js.push_str(&wrapper);
                combined_js.push('\n');
            }
        }

        combined_js
    }
}
