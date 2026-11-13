#![windows_subsystem = "windows"]

mod adblock;
mod dark_mode;
mod downloader;
mod engine;
mod extensions;
mod storage;
mod ui;

use adblock::AdblockManager;
use dark_mode::DarkModeEngine;
use downloader::DownloadManager;
use engine::ipc::{resolve_user_input_to_url, IpcMessageFromUi};
use engine::tabs::TabManager;
use engine::{RUSTIUM_USER_AGENT, RUSTIUM_WINDOW_TITLE};
use extensions::ExtensionManager;
use storage::StorageManager;
use ui::{get_internal_page, CHROME_HTML, PAGE_NEWTAB, PAGE_SCAM_WARNING};

use serde_json::json;
use std::borrow::Cow;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tao::dpi::{LogicalPosition, LogicalSize};
use tao::event::{Event, WindowEvent};
use tao::event_loop::{ControlFlow, EventLoopBuilder, EventLoopProxy};
use tao::window::{Window, WindowBuilder};
use wry::http::{Request, Response};
use wry::{Rect, WebView, WebViewBuilder};

const CHROME_HEIGHT_COLLAPSED: u32 = 88;

#[derive(Debug)]
pub enum AppEvent {
    SyncState,
    Navigate {
        tab_id: String,
        url: String,
    },
    PageLoaded {
        tab_id: String,
        url: String,
        title: String,
    },
    GoBack {
        tab_id: String,
    },
    GoForward {
        tab_id: String,
    },
    Reload {
        tab_id: String,
    },
    AdBlocked {
        tab_id: String,
        url: String,
    },
    ExpandChrome {
        height: u32,
    },
    CollapseChrome,
    ApplyDarkMode {
        tab_id: String,
        enabled: bool,
        brightness: u32,
        contrast: u32,
    },
    DownloadTick,
    MinimizeWindow,
    MaximizeWindow,
    CloseWindow,
    DragWindow,
    OpenDevtools,
    OpenSettingsWindow,
    ImportPasswords {
        csv_content: String,
    },
    DeleteLogin {
        id: i64,
    },
}

struct BrowserApp {
    window: Window,
    storage: Arc<StorageManager>,
    adblock: Arc<AdblockManager>,
    dark_mode: Arc<DarkModeEngine>,
    downloader: Arc<DownloadManager>,
    extensions: Arc<ExtensionManager>,
    tabs: Arc<Mutex<TabManager>>,
    ui_webview: WebView,
    content_webviews: HashMap<String, WebView>,
    event_proxy: EventLoopProxy<AppEvent>,
    window_width: u32,
    window_height: u32,
    current_chrome_height: u32,
}

impl BrowserApp {
    fn new(
        window: Window,
        event_proxy: EventLoopProxy<AppEvent>,
        storage: Arc<StorageManager>,
        adblock: Arc<AdblockManager>,
        dark_mode: Arc<DarkModeEngine>,
        downloader: Arc<DownloadManager>,
        extensions: Arc<ExtensionManager>,
    ) -> Self {
        let size = window.inner_size();
        let window_width = size.width.max(800);
        let window_height = size.height.max(600);
        let current_chrome_height = CHROME_HEIGHT_COLLAPSED;

        let tabs = Arc::new(Mutex::new(TabManager::new()));

        let ui_bounds = Rect {
            position: LogicalPosition::new(0.0, 0.0).into(),
            size: LogicalSize::new(window_width as f64, current_chrome_height as f64).into(),
        };

        let proxy_c = event_proxy.clone();
        let tabs_c = Arc::clone(&tabs);
        let storage_c = Arc::clone(&storage);
        let adblock_c = Arc::clone(&adblock);
        let dark_mode_c = Arc::clone(&dark_mode);
        let downloader_c = Arc::clone(&downloader);
        let extensions_c = Arc::clone(&extensions);

        let ui_webview = match WebViewBuilder::new()
            .with_user_agent(RUSTIUM_USER_AGENT)
            .with_bounds(ui_bounds)
            .with_transparent(true)
            .with_devtools(true)
            .with_html(CHROME_HTML)
            .with_custom_protocol("rustium".into(), move |_id, req| {
                handle_rustium_protocol(&req)
            })
            .with_ipc_handler(move |req: Request<String>| {
                handle_ui_ipc(
                    req.body(),
                    &proxy_c,
                    &tabs_c,
                    &storage_c,
                    &adblock_c,
                    &dark_mode_c,
                    &downloader_c,
                    &extensions_c,
                );
            })
            .build_as_child(&window)
        {
            Ok(wv) => wv,
            Err(e) => {
                show_webview2_missing_dialog(&e.to_string());
                panic!("Не удалось инициализировать интерфейс браузера: {}", e);
            }
        };

        let mut app = Self {
            window,
            storage,
            adblock,
            dark_mode,
            downloader,
            extensions,
            tabs,
            ui_webview,
            content_webviews: HashMap::new(),
            event_proxy,
            window_width,
            window_height,
            current_chrome_height,
        };

        // Create initial active tab webview
        let active_id = {
            let lock = app.tabs.lock().unwrap();
            lock.active_tab_id.clone()
        };
        app.ensure_tab_webview(&active_id, "rustium://newtab");
        app.sync_state_to_ui();

        app
    }

    fn ensure_tab_webview(&mut self, tab_id: &str, url: &str) {
        if self.content_webviews.contains_key(tab_id) {
            return;
        }

        let top_y = self.current_chrome_height as f64;
        let content_h = (self.window_height.saturating_sub(self.current_chrome_height)).max(10) as f64;
        let bounds = Rect {
            position: LogicalPosition::new(0.0, top_y).into(),
            size: LogicalSize::new(self.window_width as f64, content_h).into(),
        };

        let tid_c = tab_id.to_string();
        let proxy_c = self.event_proxy.clone();
        let storage_c = Arc::clone(&self.storage);
        let downloader_c = Arc::clone(&self.downloader);
        let tabs_c = Arc::clone(&self.tabs);
        let adblock_c = Arc::clone(&self.adblock);
        let dark_mode_c = Arc::clone(&self.dark_mode);
        let extensions_c = Arc::clone(&self.extensions);

        // Advanced Content Injected Script:
        // 1. Report loaded & title changes
        // 2. Intercept Ctrl+Click and Middle-Click on links to open background tabs
        // 3. Firefox keyboard shortcuts (Ctrl+T, Ctrl+W, Ctrl+Tab, Ctrl+Shift+Tab, F12, F5)
        // 4. Custom MD3 Context Menu on right click
        let init_script = format!(
            r#"(function() {{
                const tid = "{tid_c}";

                function sendIpc(msg) {{
                    if (window.ipc) window.ipc.postMessage(JSON.stringify(msg));
                }}

                function reportLoaded() {{
                    const cur = window.location.href;
                    if (cur === "about:blank") return;
                    sendIpc({{ action: "content_loaded", tab_id: tid, url: cur, title: document.title }});
                }}

                function observeTitle() {{
                    const titleEl = document.querySelector('title');
                    if (titleEl) {{
                        let lastTitle = document.title;
                        const obs = new MutationObserver(() => {{
                            if (document.title !== lastTitle) {{
                                lastTitle = document.title;
                                sendIpc({{ action: "title_changed", tab_id: tid, title: document.title }});
                            }}
                        }});
                        obs.observe(titleEl, {{ childList: true, characterData: true, subtree: true }});
                    }}
                }}

                if (document.readyState === 'complete') {{
                    reportLoaded();
                    observeTitle();
                }} else {{
                    window.addEventListener('DOMContentLoaded', () => {{
                        reportLoaded();
                        observeTitle();
                    }});
                }}

                // Requirement 13: Ctrl + Click or Middle-Click opens in background tab
                window.addEventListener('click', (e) => {{
                    const link = e.target.closest('a');
                    if (link && link.href) {{
                        if (e.ctrlKey || e.button === 1) {{
                            e.preventDefault();
                            e.stopPropagation();
                            sendIpc({{ action: "create_tab", url: link.href, background: true }});
                        }}
                    }}
                }}, true);

                // Requirement 14: Firefox Keybinds inside web content
                window.addEventListener('keydown', (e) => {{
                    if (e.key === 'F12') {{
                        e.preventDefault();
                        sendIpc({{ action: "open_devtools" }});
                    }} else if (e.ctrlKey && e.key.toLowerCase() === 't') {{
                        e.preventDefault();
                        sendIpc({{ action: "create_tab" }});
                    }} else if (e.ctrlKey && e.key.toLowerCase() === 'w') {{
                        e.preventDefault();
                        sendIpc({{ action: "close_tab", tab_id: tid }});
                    }} else if (e.ctrlKey && e.key === 'Tab') {{
                        e.preventDefault();
                        sendIpc({{ action: "cycle_tab", backward: e.shiftKey }});
                    }} else if (e.ctrlKey && e.key.toLowerCase() === 'r' || e.key === 'F5') {{
                        e.preventDefault();
                        sendIpc({{ action: "reload", tab_id: tid }});
                    }} else if (e.ctrlKey && e.key.toLowerCase() === 'h') {{
                        e.preventDefault();
                        sendIpc({{ action: "navigate", url: "rustium://history" }});
                    }} else if (e.ctrlKey && e.key.toLowerCase() === 'j') {{
                        e.preventDefault();
                        sendIpc({{ action: "navigate", url: "rustium://downloads" }});
                    }} else if (e.ctrlKey && e.key.toLowerCase() === 'b') {{
                        e.preventDefault();
                        sendIpc({{ action: "navigate", url: "rustium://bookmarks" }});
                    }}
                }}, true);

                // Requirement 12: Custom MD3 Context Menu with Inspect (DevTools)
                window.addEventListener('contextmenu', (e) => {{
                    // Remove existing menu if any
                    const existing = document.getElementById('rustium-ctx-menu');
                    if (existing) existing.remove();

                    const link = e.target.closest('a');
                    const linkUrl = link ? link.href : '';

                    const menu = document.createElement('div');
                    menu.id = 'rustium-ctx-menu';
                    menu.style.cssText = 'position:fixed; z-index:2147483647; background:#1e2428; color:#e1e3e5; border-radius:12px; padding:6px; box-shadow:0 8px 32px rgba(0,0,0,0.5); font-family:sans-serif; font-size:13px; min-width:180px; display:flex; flex-direction:column; gap:2px;';
                    menu.style.left = Math.min(e.clientX, window.innerWidth - 200) + 'px';
                    menu.style.top = Math.min(e.clientY, window.innerHeight - 220) + 'px';

                    function createItem(title, shortcut, onClick) {{
                        const it = document.createElement('div');
                        it.style.cssText = 'padding:8px 12px; border-radius:8px; cursor:pointer; display:flex; justify-content:space-between; align-items:center; transition:background 0.15s;';
                        it.onmouseenter = () => it.style.background = '#2c353b';
                        it.onmouseleave = () => it.style.background = 'transparent';
                        it.innerHTML = `<span>${{title}}</span><span style="font-size:11px; color:#a0a7af;">${{shortcut}}</span>`;
                        it.onclick = (evt) => {{ evt.stopPropagation(); onClick(); menu.remove(); }};
                        return it;
                    }}

                    menu.appendChild(createItem('Назад', 'Alt+Left', () => sendIpc({{ action: "go_back", tab_id: tid }})));
                    menu.appendChild(createItem('Вперед', 'Alt+Right', () => sendIpc({{ action: "go_forward", tab_id: tid }})));
                    menu.appendChild(createItem('Обновить', 'F5', () => sendIpc({{ action: "reload", tab_id: tid }})));
                    if (linkUrl) {{
                        menu.appendChild(createItem('Копировать ссылку', '', () => navigator.clipboard.writeText(linkUrl)));
                    }}
                    menu.appendChild(createItem('Исследовать элемент', 'F12', () => sendIpc({{ action: "open_devtools" }})));

                    document.body.appendChild(menu);
                    const closeHandler = () => {{ menu.remove(); window.removeEventListener('click', closeHandler); }};
                    setTimeout(() => window.addEventListener('click', closeHandler), 10);
                }}, true);
            }})();"#
        );

        let adblock_script = self.adblock.get_injected_script(url, tab_id);
        let dark_mode_script = self.dark_mode.get_injected_script(true);
        let extension_scripts = self.extensions.get_injected_scripts_for_url(url);

        let mut builder = WebViewBuilder::new()
            .with_user_agent(RUSTIUM_USER_AGENT)
            .with_bounds(bounds)
            .with_devtools(true)
            .with_initialization_script(&init_script)
            .with_initialization_script(&adblock_script)
            .with_initialization_script(&dark_mode_script)
            .with_initialization_script(&extension_scripts)
            .with_custom_protocol("rustium".into(), move |_id, req| {
                handle_rustium_protocol(&req)
            });

        let tid_ipc = tab_id.to_string();
        let proxy_ipc = proxy_c.clone();
        let dl_c = downloader_c.clone();
        let st_c = storage_c.clone();
        let prx_dl = proxy_c.clone();

        builder = builder.with_ipc_handler(move |req: Request<String>| {
            let body = req.body();
            if let Ok(msg) = serde_json::from_str::<serde_json::Value>(body) {
                let action = msg.get("action").and_then(|v| v.as_str()).unwrap_or("");
                match action {
                    "content_loaded" | "title_changed" => {
                        let title = msg.get("title").and_then(|v| v.as_str()).unwrap_or("").to_string();
                        let cur_url = msg.get("url").and_then(|v| v.as_str()).unwrap_or("").to_string();
                        let _ = proxy_ipc.send_event(AppEvent::PageLoaded {
                            tab_id: tid_ipc.clone(),
                            url: cur_url,
                            title,
                        });
                    }
                    "ad_blocked" => {
                        let blocked_url = msg.get("url").and_then(|v| v.as_str()).unwrap_or("").to_string();
                        let _ = proxy_ipc.send_event(AppEvent::AdBlocked {
                            tab_id: tid_ipc.clone(),
                            url: blocked_url,
                        });
                    }
                    "open_devtools" => {
                        let _ = proxy_ipc.send_event(AppEvent::OpenDevtools);
                    }
                    "cycle_tab" => {
                        // Forward tab cycling request
                        let _ = proxy_ipc.send_event(AppEvent::SyncState);
                    }
                    _ => {
                        handle_ui_ipc(
                            body,
                            &proxy_ipc,
                            &tabs_c,
                            &storage_c,
                            &adblock_c,
                            &dark_mode_c,
                            &downloader_c,
                            &extensions_c,
                        );
                    }
                }
            }
        });

        // Download interceptor
        builder = builder.with_download_started_handler(
            move |url: String, suggested_path: &mut std::path::PathBuf| -> bool {
                let s = st_c.get_settings();
                let fname = suggested_path
                    .file_name()
                    .and_then(|f| f.to_str())
                    .unwrap_or("");
                let threads = if s.multithread_downloader_enabled {
                    s.download_threads
                } else {
                    1
                };
                dl_c.start_download(&url, Some(fname), threads);
                let _ = prx_dl.send_event(AppEvent::SyncState);
                false
            },
        );

        // Internal pages (rustium://) are loaded via with_html for guaranteed instantaneous, non-network rendering
        if url.starts_with("rustium://") {
            let target = url.trim_start_matches("rustium://").trim_matches('/');
            let page_content = get_internal_page(target).unwrap_or(PAGE_NEWTAB);
            builder = builder.with_html(page_content);
        } else {
            builder = builder.with_url(url);
        }

        let webview = builder
            .build_as_child(&self.window)
            .expect("Не удалось создать WebView для вкладки");

        self.content_webviews.insert(tab_id.to_string(), webview);

        // Keep ui_webview on top
        #[cfg(target_os = "windows")]
        {
            use wry::WebViewExtWindows;
            use windows_sys::Win32::UI::WindowsAndMessaging::{SetWindowPos, HWND_TOP, SWP_NOMOVE, SWP_NOSIZE, SWP_SHOWWINDOW};
            let ui_hwnd = self.ui_webview.hwnd().0 as _;
            unsafe {
                SetWindowPos(ui_hwnd, HWND_TOP, 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_SHOWWINDOW);
            }
        }
    }

    fn update_active_tab_visibility(&mut self) {
        let active_id = {
            let lock = self.tabs.lock().unwrap();
            lock.active_tab_id.clone()
        };

        for (id, wv) in &self.content_webviews {
            let is_active = id == &active_id;
            let _ = wv.set_visible(is_active);
        }
    }

    fn update_layout(&mut self) {
        let w = self.window_width as f64;
        let chrome_h = self.current_chrome_height as f64;
        let base_chrome_h = CHROME_HEIGHT_COLLAPSED as f64;
        let content_h = (self.window_height.saturating_sub(CHROME_HEIGHT_COLLAPSED)).max(10) as f64;

        let ui_bounds = Rect {
            position: LogicalPosition::new(0.0, 0.0).into(),
            size: LogicalSize::new(w, chrome_h).into(),
        };
        let _ = self.ui_webview.set_bounds(ui_bounds);

        let content_bounds = Rect {
            position: LogicalPosition::new(0.0, base_chrome_h).into(),
            size: LogicalSize::new(w, content_h).into(),
        };

        let active_id = {
            let lock = self.tabs.lock().unwrap();
            lock.active_tab_id.clone()
        };

        if let Some(wv) = self.content_webviews.get(&active_id) {
            let _ = wv.set_bounds(content_bounds);
        }

        #[cfg(target_os = "windows")]
        {
            use wry::WebViewExtWindows;
            use windows_sys::Win32::UI::WindowsAndMessaging::{SetWindowPos, HWND_TOP, SWP_NOMOVE, SWP_NOSIZE, SWP_SHOWWINDOW};
            let ui_hwnd = self.ui_webview.hwnd().0 as _;
            unsafe {
                SetWindowPos(ui_hwnd, HWND_TOP, 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_SHOWWINDOW);
            }
        }
    }

    fn sync_state_to_ui(&self) {
        let tabs_lock = self.tabs.lock().unwrap();
        let settings = self.storage.get_settings();
        let adblock_total = self.adblock.get_total_blocked_count();
        let downloads = self.downloader.list_downloads();
        let extensions = self.extensions.list_extensions();
        let logins = self.storage.get_logins();

        let state = json!({
            "tabs": tabs_lock.tabs,
            "active_tab_id": tabs_lock.active_tab_id,
            "settings": settings,
            "adblock_total": adblock_total,
            "downloads": downloads,
            "extensions": extensions,
            "logins": logins,
        });

        let script = format!("window.__rustium_update_state__ && window.__rustium_update_state__({});", state);
        let _ = self.ui_webview.evaluate_script(&script);

        // Update internal pages if active
        if let Some(active_tab) = tabs_lock.get_active_tab() {
            if let Some(active_wv) = self.content_webviews.get(&active_tab.id) {
                if active_tab.url.starts_with("rustium://downloads") {
                    let dl_script = format!(
                        "window.__rustium_render_downloads__ && window.__rustium_render_downloads__({});",
                        json!(downloads)
                    );
                    let _ = active_wv.evaluate_script(&dl_script);
                } else if active_tab.url.starts_with("rustium://history") {
                    let hist = self.storage.get_history(100);
                    let hist_script = format!(
                        "window.__rustium_render_history__ && window.__rustium_render_history__({});",
                        json!(hist)
                    );
                    let _ = active_wv.evaluate_script(&hist_script);
                } else if active_tab.url.starts_with("rustium://bookmarks") {
                    let bms = self.storage.get_bookmarks();
                    let bm_script = format!(
                        "window.__rustium_render_bookmarks__ && window.__rustium_render_bookmarks__({});",
                        json!(bms)
                    );
                    let _ = active_wv.evaluate_script(&bm_script);
                } else if active_tab.url.starts_with("rustium://extensions") {
                    let ext_script = format!(
                        "window.__rustium_render_extensions__ && window.__rustium_render_extensions__({});",
                        json!(extensions)
                    );
                    let _ = active_wv.evaluate_script(&ext_script);
                } else if active_tab.url.starts_with("rustium://settings") {
                    let set_script = format!(
                        "window.__rustium_render_settings__ && window.__rustium_render_settings__({}, {});",
                        json!(settings),
                        json!(logins)
                    );
                    let _ = active_wv.evaluate_script(&set_script);
                } else if active_tab.url.starts_with("rustium://passwords") {
                    let pwd_script = format!(
                        "window.__rustium_render_passwords__ && window.__rustium_render_passwords__({});",
                        json!(logins)
                    );
                    let _ = active_wv.evaluate_script(&pwd_script);
                } else if active_tab.url.starts_with("rustium://newtab") || active_tab.url.is_empty() {
                    let nt_script = format!(
                        "window.__rustium_render_newtab__ && window.__rustium_render_newtab__({});",
                        json!(settings)
                    );
                    let _ = active_wv.evaluate_script(&nt_script);
                }
            }
        }
    }

    fn navigate_tab(&mut self, tab_id: &str, raw_target: &str) {
        let search_engine = {
            let s = self.storage.get_settings();
            s.search_engine
        };

        let target_url = resolve_user_input_to_url(raw_target, &search_engine);

        // Check uBlock Origin scam & phishing safety warning!
        if !target_url.starts_with("rustium://") {
            if let Some(_warning) = self.adblock.check_scam_or_malware(&target_url) {
                let warning_target = format!("rustium://warning?target={}", url::form_urlencoded::byte_serialize(target_url.as_bytes()).collect::<String>());
                {
                    let mut lock = self.tabs.lock().unwrap();
                    if let Some(tab) = lock.get_tab_mut(tab_id) {
                        tab.navigate(warning_target.clone(), Some("Предупреждение о безопасности".to_string()));
                    }
                }
                self.ensure_tab_webview(tab_id, &warning_target);
                if let Some(wv) = self.content_webviews.get(tab_id) {
                    let _ = wv.load_html(PAGE_SCAM_WARNING);
                }
                self.update_active_tab_visibility();
                self.update_layout();
                self.sync_state_to_ui();
                return;
            }
        }

        {
            let mut lock = self.tabs.lock().unwrap();
            if let Some(tab) = lock.get_tab_mut(tab_id) {
                tab.navigate(target_url.clone(), None);
            }
        }

        if !target_url.starts_with("rustium://") {
            let _ = self.storage.add_history(&target_url, &target_url);
        }

        let already_existed = self.content_webviews.contains_key(tab_id);
        self.ensure_tab_webview(tab_id, &target_url);

        if let Some(wv) = self.content_webviews.get(tab_id) {
            if already_existed {
                if target_url.starts_with("rustium://") {
                    let target = target_url.trim_start_matches("rustium://").trim_matches('/');
                    let page_content = get_internal_page(target).unwrap_or(PAGE_NEWTAB);
                    let _ = wv.load_html(page_content);
                } else {
                    let _ = wv.load_url(&target_url);
                }
            }
        }

        self.update_active_tab_visibility();
        self.update_layout();
        self.sync_state_to_ui();
    }
}

fn handle_rustium_protocol(req: &Request<Vec<u8>>) -> Response<Cow<'static, [u8]>> {
    let uri = req.uri();
    let host = uri.host().unwrap_or("");
    let path = uri.path().trim_start_matches('/');

    let target = if !host.is_empty() { host } else { path };
    let page_content = get_internal_page(target).unwrap_or(PAGE_NEWTAB);

    Response::builder()
        .header("Content-Type", "text/html; charset=utf-8")
        .header("Access-Control-Allow-Origin", "*")
        .body(Cow::Borrowed(page_content.as_bytes()))
        .unwrap()
}

fn handle_ui_ipc(
    body: &str,
    event_proxy: &EventLoopProxy<AppEvent>,
    tabs: &Arc<Mutex<TabManager>>,
    storage: &Arc<StorageManager>,
    adblock: &Arc<AdblockManager>,
    dark_mode: &Arc<DarkModeEngine>,
    downloader: &Arc<DownloadManager>,
    extensions: &Arc<ExtensionManager>,
) {
    if let Ok(msg) = serde_json::from_str::<IpcMessageFromUi>(body) {
        match msg {
            IpcMessageFromUi::Navigate { tab_id, url } => {
                let target_tab_id = tab_id.unwrap_or_else(|| {
                    let lock = tabs.lock().unwrap();
                    lock.active_tab_id.clone()
                });
                let _ = event_proxy.send_event(AppEvent::Navigate {
                    tab_id: target_tab_id,
                    url,
                });
            }
            IpcMessageFromUi::GoBack { tab_id } => {
                let target_tab_id = tab_id.unwrap_or_else(|| {
                    let lock = tabs.lock().unwrap();
                    lock.active_tab_id.clone()
                });
                let _ = event_proxy.send_event(AppEvent::GoBack {
                    tab_id: target_tab_id,
                });
            }
            IpcMessageFromUi::GoForward { tab_id } => {
                let target_tab_id = tab_id.unwrap_or_else(|| {
                    let lock = tabs.lock().unwrap();
                    lock.active_tab_id.clone()
                });
                let _ = event_proxy.send_event(AppEvent::GoForward {
                    tab_id: target_tab_id,
                });
            }
            IpcMessageFromUi::Reload { tab_id } => {
                let target_tab_id = tab_id.unwrap_or_else(|| {
                    let lock = tabs.lock().unwrap();
                    lock.active_tab_id.clone()
                });
                let _ = event_proxy.send_event(AppEvent::Reload {
                    tab_id: target_tab_id,
                });
            }
            IpcMessageFromUi::CreateTab { url, title, background } => {
                let u = url.unwrap_or_else(|| "rustium://newtab".to_string());
                let t = title.unwrap_or_else(|| "Новая вкладка".to_string());
                let mut lock = tabs.lock().unwrap();
                let is_bg = background.unwrap_or(false);
                let current_active = lock.active_tab_id.clone();
                let new_id = lock.create_tab(&u, &t);
                if is_bg {
                    lock.switch_tab(&current_active);
                }
                let _ = event_proxy.send_event(AppEvent::Navigate {
                    tab_id: new_id,
                    url: u,
                });
            }
            IpcMessageFromUi::CloseTab { tab_id } => {
                let mut lock = tabs.lock().unwrap();
                lock.close_tab(&tab_id);
                let _ = event_proxy.send_event(AppEvent::SyncState);
            }
            IpcMessageFromUi::SwitchTab { tab_id } => {
                let mut lock = tabs.lock().unwrap();
                lock.switch_tab(&tab_id);
                let _ = event_proxy.send_event(AppEvent::SyncState);
            }
            IpcMessageFromUi::PinTab { tab_id, is_pinned } => {
                let mut lock = tabs.lock().unwrap();
                lock.pin_tab(&tab_id, is_pinned);
                let _ = event_proxy.send_event(AppEvent::SyncState);
            }
            IpcMessageFromUi::ToggleAdblock { domain, enabled } => {
                let _ = storage.set_adblock_whitelisted(&domain, !enabled);
                let _ = event_proxy.send_event(AppEvent::SyncState);
            }
            IpcMessageFromUi::ToggleDarkMode {
                domain,
                mode,
                brightness,
                contrast,
            } => {
                let m = mode.as_deref().unwrap_or("system");
                dark_mode.set_mode(m);
                let b = brightness.unwrap_or_else(|| dark_mode.get_brightness());
                let c = contrast.unwrap_or_else(|| dark_mode.get_contrast());
                dark_mode.set_brightness(b);
                dark_mode.set_contrast(c);
                let is_on = m != "always_off";
                let _ = storage.set_dark_mode_whitelisted(&domain, !is_on);

                let target_tab_id = {
                    let lock = tabs.lock().unwrap();
                    lock.active_tab_id.clone()
                };
                let _ = event_proxy.send_event(AppEvent::ApplyDarkMode {
                    tab_id: target_tab_id,
                    enabled: is_on,
                    brightness: b,
                    contrast: c,
                });
                let _ = event_proxy.send_event(AppEvent::SyncState);
            }
            IpcMessageFromUi::StartDownload { url, filename } => {
                let s = storage.get_settings();
                let threads = if s.multithread_downloader_enabled {
                    s.download_threads
                } else {
                    1
                };
                downloader.start_download(&url, filename.as_deref(), threads);
                let _ = event_proxy.send_event(AppEvent::SyncState);
            }
            IpcMessageFromUi::PauseDownload { id } => {
                downloader.pause_download(&id);
                let _ = event_proxy.send_event(AppEvent::SyncState);
            }
            IpcMessageFromUi::ResumeDownload { id } => {
                let s = storage.get_settings();
                downloader.resume_download(&id, s.download_threads);
                let _ = event_proxy.send_event(AppEvent::SyncState);
            }
            IpcMessageFromUi::CancelDownload { id } => {
                downloader.cancel_download(&id);
                let _ = event_proxy.send_event(AppEvent::SyncState);
            }
            IpcMessageFromUi::OpenDownloadFile { id } => {
                downloader.open_downloaded_file(&id);
            }
            IpcMessageFromUi::ShowInFolder { id } => {
                downloader.show_in_folder(&id);
            }
            IpcMessageFromUi::OpenDownloadsFolder => {
                downloader.open_downloads_folder();
            }
            IpcMessageFromUi::LoadUnpackedExtension { path } => {
                let _ = extensions.load_unpacked_extension(std::path::Path::new(&path));
                let _ = event_proxy.send_event(AppEvent::SyncState);
            }
            IpcMessageFromUi::ToggleExtension { id, enabled } => {
                extensions.set_extension_enabled(&id, enabled);
                let _ = event_proxy.send_event(AppEvent::SyncState);
            }
            IpcMessageFromUi::UpdateSettings { settings: new_s } => {
                let _ = storage.save_settings(&new_s);
                adblock.set_enabled(new_s.adblock_enabled);
                dark_mode.set_mode(&new_s.dark_mode_mode);
                dark_mode.set_brightness(new_s.dark_mode_brightness);
                dark_mode.set_contrast(new_s.dark_mode_contrast);
                let _ = event_proxy.send_event(AppEvent::SyncState);
            }
            IpcMessageFromUi::AddBookmark { title, url } => {
                let _ = storage.add_bookmark(&title, &url);
                let _ = event_proxy.send_event(AppEvent::SyncState);
            }
            IpcMessageFromUi::RemoveBookmark { url } => {
                let _ = storage.remove_bookmark(&url);
                let _ = event_proxy.send_event(AppEvent::SyncState);
            }
            IpcMessageFromUi::ClearHistory => {
                let _ = storage.clear_history();
                let _ = event_proxy.send_event(AppEvent::SyncState);
            }
            IpcMessageFromUi::ImportPasswords { csv_content } => {
                let _ = storage.import_passwords_csv(&csv_content);
                let _ = event_proxy.send_event(AppEvent::SyncState);
            }
            IpcMessageFromUi::AddLogin { url, username, password } => {
                let _ = storage.add_login(&url, &username, &password);
                let _ = event_proxy.send_event(AppEvent::SyncState);
            }
            IpcMessageFromUi::DeleteLogin { id } => {
                let _ = storage.delete_login(id);
                let _ = event_proxy.send_event(AppEvent::SyncState);
            }
            IpcMessageFromUi::GetLogins => {
                let _ = event_proxy.send_event(AppEvent::SyncState);
            }
            IpcMessageFromUi::OpenDevtools => {
                let _ = event_proxy.send_event(AppEvent::OpenDevtools);
            }
            IpcMessageFromUi::OpenSettingsWindow => {
                let _ = event_proxy.send_event(AppEvent::OpenSettingsWindow);
            }
            IpcMessageFromUi::MinimizeWindow => {
                let _ = event_proxy.send_event(AppEvent::MinimizeWindow);
            }
            IpcMessageFromUi::MaximizeWindow => {
                let _ = event_proxy.send_event(AppEvent::MaximizeWindow);
            }
            IpcMessageFromUi::CloseWindow => {
                let _ = event_proxy.send_event(AppEvent::CloseWindow);
            }
            IpcMessageFromUi::DragWindow => {
                let _ = event_proxy.send_event(AppEvent::DragWindow);
            }
            IpcMessageFromUi::ExpandChrome { height } => {
                let _ = event_proxy.send_event(AppEvent::ExpandChrome { height });
            }
            IpcMessageFromUi::CollapseChrome => {
                let _ = event_proxy.send_event(AppEvent::CollapseChrome);
            }
            IpcMessageFromUi::GetState => {
                let _ = event_proxy.send_event(AppEvent::SyncState);
            }
        }
    }
}

#[tokio::main]
async fn main() {
    println!("Запуск Rustium 1.1.1...");

    // 1. Initialize Subsystems
    let storage = Arc::new(StorageManager::new(None).expect("Ошибка инициализации базы данных SQLite"));
    let settings = storage.get_settings();

    let adblock = Arc::new(AdblockManager::new(settings.adblock_enabled));
    let dark_mode = Arc::new(DarkModeEngine::new(
        &settings.dark_mode_mode,
        settings.dark_mode_brightness,
        settings.dark_mode_contrast,
    ));
    let downloader = Arc::new(DownloadManager::new(None));
    let extensions = Arc::new(ExtensionManager::new(None));

    // 2. Initialize Tao Event Loop with AppEvent
    let event_loop = EventLoopBuilder::<AppEvent>::with_user_event().build();
    let event_proxy = event_loop.create_proxy();

    // 3. Create Main Window (Frameless Custom Tabs titlebar like Chrome/Firefox)
    let mut window_builder = WindowBuilder::new()
        .with_title(RUSTIUM_WINDOW_TITLE)
        .with_inner_size(LogicalSize::new(1280.0, 800.0))
        .with_min_inner_size(LogicalSize::new(600.0, 400.0))
        .with_decorations(false); // Seamless custom MD3 tabs in titlebar!

    const ICON_RGBA: &[u8] = include_bytes!("../assets/icon_32.rgba");
    if let Ok(icon) = tao::window::Icon::from_rgba(ICON_RGBA.to_vec(), 32, 32) {
        window_builder = window_builder.with_window_icon(Some(icon));
    }

    let window = window_builder
        .build(&event_loop)
        .expect("Не удалось создать главное окно");

    // 4. Create Browser Application State
    let mut browser = BrowserApp::new(
        window,
        event_proxy.clone(),
        Arc::clone(&storage),
        Arc::clone(&adblock),
        Arc::clone(&dark_mode),
        Arc::clone(&downloader),
        Arc::clone(&extensions),
    );

    // 5. Periodic Download Progress Reporter
    let proxy_for_timer = event_proxy.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_millis(500));
        loop {
            interval.tick().await;
            let _ = proxy_for_timer.send_event(AppEvent::DownloadTick);
        }
    });

    println!("Rustium готов к работе!");

    // Helper for confirm exit dialog
    let can_close = |downloader: &DownloadManager| -> bool {
        if downloader.has_active_downloads() {
            #[cfg(target_os = "windows")]
            {
                use windows_sys::Win32::UI::WindowsAndMessaging::{MessageBoxW, MB_ICONWARNING, MB_YESNO, IDYES};
                let text: Vec<u16> = "В браузере выполняются активные загрузки файлов.\nВы уверены, что хотите прервать загрузки и закрыть Rustium?\0".encode_utf16().collect();
                let caption: Vec<u16> = "Rustium 1.1.1\0".encode_utf16().collect();
                let res = unsafe {
                    MessageBoxW(0 as _, text.as_ptr(), caption.as_ptr(), MB_YESNO | MB_ICONWARNING)
                };
                return res == IDYES;
            }
        }
        true
    };

    // 6. Tao Event Loop Run
    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Wait;

        match event {
            Event::WindowEvent { event, .. } => match event {
                WindowEvent::CloseRequested => {
                    if can_close(&browser.downloader) {
                        let tabs_lock = browser.tabs.lock().unwrap();
                        let session_tabs: Vec<storage::SessionTab> = tabs_lock
                            .tabs
                            .iter()
                            .map(|t| storage::SessionTab {
                                id: t.id.clone(),
                                url: t.url.clone(),
                                title: t.title.clone(),
                                is_pinned: t.is_pinned,
                                is_active: t.id == tabs_lock.active_tab_id,
                            })
                            .collect();
                        let _ = browser.storage.save_session_tabs(&session_tabs);
                        *control_flow = ControlFlow::Exit;
                    }
                }
                WindowEvent::Resized(new_size) => {
                    browser.window_width = new_size.width;
                    browser.window_height = new_size.height;
                    browser.update_layout();
                }
                _ => {}
            },

            Event::UserEvent(app_event) => match app_event {
                AppEvent::SyncState | AppEvent::DownloadTick => {
                    browser.update_active_tab_visibility();
                    browser.sync_state_to_ui();
                }
                AppEvent::Navigate { tab_id, url } => {
                    browser.navigate_tab(&tab_id, &url);
                }
                AppEvent::PageLoaded { tab_id, url, title } => {
                    {
                        let mut lock = browser.tabs.lock().unwrap();
                        if let Some(tab) = lock.get_tab_mut(&tab_id) {
                            if !url.is_empty() && url != "about:blank" && !tab.url.starts_with("rustium://") {
                                tab.url = url.clone();
                            }
                            if !title.is_empty() && title != "about:blank" {
                                tab.title = title.clone();
                            }
                        }
                    }
                    if !url.is_empty() && url != "about:blank" && !url.starts_with("rustium://") {
                        let _ = browser.storage.add_history(&url, &title);
                    }
                    browser.update_active_tab_visibility();
                    browser.sync_state_to_ui();
                }
                AppEvent::GoBack { tab_id } => {
                    if let Some(wv) = browser.content_webviews.get(&tab_id) {
                        let _ = wv.evaluate_script("window.history.back()");
                    }
                }
                AppEvent::GoForward { tab_id } => {
                    if let Some(wv) = browser.content_webviews.get(&tab_id) {
                        let _ = wv.evaluate_script("window.history.forward()");
                    }
                }
                AppEvent::Reload { tab_id } => {
                    if let Some(wv) = browser.content_webviews.get(&tab_id) {
                        let _ = wv.evaluate_script("window.location.reload()");
                    }
                }
                AppEvent::AdBlocked { tab_id, url: _ } => {
                    {
                        let mut lock = browser.tabs.lock().unwrap();
                        if let Some(tab) = lock.get_tab_mut(&tab_id) {
                            tab.blocked_count += 1;
                        }
                    }
                    browser.sync_state_to_ui();
                }
                AppEvent::ExpandChrome { height } => {
                    browser.current_chrome_height = height.max(browser.window_height).max(750);
                    browser.update_layout();
                }
                AppEvent::CollapseChrome => {
                    browser.current_chrome_height = CHROME_HEIGHT_COLLAPSED;
                    browser.update_layout();
                }
                AppEvent::ApplyDarkMode { tab_id, enabled, brightness, contrast } => {
                    if let Some(wv) = browser.content_webviews.get(&tab_id) {
                        let script = format!(
                            "window.__rustium_dark_mode__ && window.__rustium_dark_mode__.updateConfig({{ enabled: {}, brightness: {}, contrast: {} }});",
                            enabled, brightness, contrast
                        );
                        let _ = wv.evaluate_script(&script);
                    }
                }
                AppEvent::MinimizeWindow => {
                    browser.window.set_minimized(true);
                }
                AppEvent::MaximizeWindow => {
                    let is_max = browser.window.is_maximized();
                    browser.window.set_maximized(!is_max);
                }
                AppEvent::CloseWindow => {
                    if can_close(&browser.downloader) {
                        let tabs_lock = browser.tabs.lock().unwrap();
                        let session_tabs: Vec<storage::SessionTab> = tabs_lock
                            .tabs
                            .iter()
                            .map(|t| storage::SessionTab {
                                id: t.id.clone(),
                                url: t.url.clone(),
                                title: t.title.clone(),
                                is_pinned: t.is_pinned,
                                is_active: t.id == tabs_lock.active_tab_id,
                            })
                            .collect();
                        let _ = browser.storage.save_session_tabs(&session_tabs);
                        *control_flow = ControlFlow::Exit;
                    }
                }
                AppEvent::DragWindow => {
                    let _ = browser.window.drag_window();
                }
                AppEvent::OpenDevtools => {
                    let active_id = {
                        let lock = browser.tabs.lock().unwrap();
                        lock.active_tab_id.clone()
                    };
                    if let Some(wv) = browser.content_webviews.get(&active_id) {
                        #[cfg(target_os = "windows")]
                        {
                            wv.open_devtools();
                        }
                    }
                }
                AppEvent::OpenSettingsWindow => {
                    let active_id = {
                        let lock = browser.tabs.lock().unwrap();
                        lock.active_tab_id.clone()
                    };
                    browser.navigate_tab(&active_id, "rustium://settings");
                }
                AppEvent::ImportPasswords { csv_content } => {
                    let _ = browser.storage.import_passwords_csv(&csv_content);
                    browser.sync_state_to_ui();
                }
                AppEvent::DeleteLogin { id } => {
                    let _ = browser.storage.delete_login(id);
                    browser.sync_state_to_ui();
                }
            },

            _ => {}
        }
    });
}

fn show_webview2_missing_dialog(err: &str) {
    #[cfg(windows)]
    unsafe {
        use windows_sys::Win32::UI::WindowsAndMessaging::{MessageBoxW, MB_ICONERROR, MB_OKCANCEL};
        use windows_sys::Win32::UI::Shell::ShellExecuteW;
        use std::ptr::null_mut;

        let title: Vec<u16> = "Rustium 1.1.1 — Ошибка запуска\0".encode_utf16().collect();
        let message = format!(
            "Для запуска браузера Rustium на вашем компьютере требуется компонент Microsoft Edge WebView2 Runtime.\n\n\
            Детали: {}\n\n\
            Нажмите «ОК», чтобы скачать официальный установщик WebView2 с сайта Microsoft, или установите Rustium через файл установщика Rustium-Setup-v1.1.1.exe (в него уже включена автоматическая установка всех нужных компонентов).",
            err
        );
        let msg_w: Vec<u16> = message.encode_utf16().chain(std::iter::once(0)).collect();

        let ret = MessageBoxW(0, msg_w.as_ptr(), title.as_ptr(), MB_OKCANCEL | MB_ICONERROR);
        if ret == 1 {
            let url: Vec<u16> = "https://go.microsoft.com/fwlink/p/?LinkId=2124703\0".encode_utf16().collect();
            let op: Vec<u16> = "open\0".encode_utf16().collect();
            ShellExecuteW(0, op.as_ptr(), url.as_ptr(), null_mut(), null_mut(), 1);
        }
    }
}
