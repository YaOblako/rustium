use rustium::adblock::AdblockManager;
use rustium::dark_mode::DarkModeEngine;
use rustium::downloader::DownloadManager;
use rustium::engine::ipc::resolve_user_input_to_url;
use rustium::extensions::ExtensionManager;
use rustium::storage::{BrowserSettings, StorageManager};
use std::fs;
use std::path::PathBuf;
use std::time::Duration;

#[test]
fn test_url_resolution() {
    assert_eq!(resolve_user_input_to_url("rustium://settings", "google"), "rustium://settings");
    assert_eq!(resolve_user_input_to_url("https://rust-lang.org", "google"), "https://rust-lang.org");
    assert_eq!(resolve_user_input_to_url("habr.com", "google"), "https://habr.com");
    assert_eq!(resolve_user_input_to_url("localhost:8080", "google"), "http://localhost:8080");

    let query_ddg = resolve_user_input_to_url("новости rust", "duckduckgo");
    assert!(query_ddg.starts_with("https://duckduckgo.com/?q="));

    let query_google = resolve_user_input_to_url("rust tutorial", "google");
    assert!(query_google.starts_with("https://www.google.com/search?q="));
}

#[test]
fn test_adblock_engine() {
    let adblock = AdblockManager::new(true);

    // Check request blocking
    let blocked_ad = adblock.check_request(
        "https://doubleclick.net/pagead/ads?client=ca-pub",
        "https://example.com",
        "script",
        "example.com",
        Some("tab_1"),
    );
    assert!(blocked_ad, "Рекламный скрипт DoubleClick должен быть заблокирован");

    // Check normal content
    let allowed_content = adblock.check_request(
        "https://example.com/main.js",
        "https://example.com",
        "script",
        "example.com",
        Some("tab_1"),
    );
    assert!(!allowed_content, "Легитимный скрипт не должен блокироваться");

    // Check cosmetic CSS generation
    let cosmetic_css = adblock.get_cosmetic_css("https://example.com");
    assert!(!cosmetic_css.is_empty(), "Косметические правила должны генерироваться");
    assert!(cosmetic_css.contains("display: none !important"));
}

#[test]
fn test_storage_manager() {
    let temp_db = PathBuf::from("test_rustium_db.sqlite");
    let _ = fs::remove_file(&temp_db);

    let storage = StorageManager::new(Some(temp_db.clone())).expect("База данных должна открываться");

    // 1. Settings
    let mut s = BrowserSettings::default();
    s.theme = "win11".to_string();
    s.download_threads = 8;
    s.gradient = "sunset".to_string();
    s.bg_type = "gif".to_string();
    s.gif_preset = "cyberpunk".to_string();
    s.bg_opacity = 50;
    s.bg_blur = 10;
    s.frosted_glass_enabled = true;
    s.frosted_glass_blur = 20;
    s.site_exclusions = vec!["example.com".to_string()];
    storage.save_settings(&s).expect("Настройки должны сохраняться");

    let loaded_s = storage.get_settings();
    assert_eq!(loaded_s.theme, "win11");
    assert_eq!(loaded_s.download_threads, 8);
    assert_eq!(loaded_s.gradient, "sunset");
    assert_eq!(loaded_s.bg_type, "gif");
    assert_eq!(loaded_s.gif_preset, "cyberpunk");
    assert_eq!(loaded_s.bg_opacity, 50);
    assert_eq!(loaded_s.bg_blur, 10);
    assert!(loaded_s.frosted_glass_enabled);
    assert_eq!(loaded_s.frosted_glass_blur, 20);
    assert_eq!(loaded_s.site_exclusions, vec!["example.com".to_string()]);

    // 2. Bookmarks
    storage.add_bookmark("Хабр", "https://habr.com").expect("Закладка должна добавляться");
    assert!(storage.is_bookmarked("https://habr.com"));

    let bms = storage.get_bookmarks();
    assert_eq!(bms.len(), 1);
    assert_eq!(bms[0].title, "Хабр");

    // 3. History
    storage.add_history("https://rust-lang.org", "Rust Programming Language").expect("История должна сохраняться");
    let hist = storage.get_history(10);
    assert_eq!(hist.len(), 1);
    assert_eq!(hist[0].url, "https://rust-lang.org");

    let _ = fs::remove_file(&temp_db);
}

#[test]
fn test_dark_mode_engine() {
    let engine = DarkModeEngine::new("always_on", 95, 110);
    assert_eq!(engine.get_brightness(), 95);
    assert_eq!(engine.get_contrast(), 110);
    assert!(engine.should_apply_to("example.com", false));
    assert!(!engine.should_apply_to("example.com", true)); // whitelisted

    let script = engine.get_injected_script(true);
    assert!(script.contains("data-rustium-dark"));
    assert!(script.contains("invert(90%)"));
    assert!(script.contains("brightness(0.95)"));
    assert!(script.contains("contrast(1.1)"));
}

#[test]
fn test_extension_manager() {
    let test_dir = PathBuf::from("test_extensions_dir");
    let _ = fs::create_dir_all(&test_dir);

    let ext_manager = ExtensionManager::new(Some(test_dir.clone()));
    let exts = ext_manager.list_extensions();
    assert!(!exts.is_empty(), "Образец расширения должен быть создан и загружен");

    assert!(ExtensionManager::matches_pattern("<all_urls>", "https://example.com/test"));
    assert!(ExtensionManager::matches_pattern("*://*.google.com/*", "https://mail.google.com/inbox"));
    assert!(!ExtensionManager::matches_pattern("*://*.google.com/*", "https://yandex.ru"));

    let _ = fs::remove_dir_all(&test_dir);
}

#[tokio::test]
async fn test_downloader_multithreaded_execution() {
    let temp_dl = PathBuf::from("test_downloads_dir");
    let _ = fs::create_dir_all(&temp_dl);

    let dl_manager = DownloadManager::new(Some(temp_dl.clone()));

    // Test with real public file supporting Range
    let url = "https://proof.ovh.net/files/10Mb.dat";
    let dl_id = dl_manager.start_download(url, Some("test_10mb.dat"), 4);

    // Wait a couple of seconds to observe progress
    tokio::time::sleep(Duration::from_millis(1500)).await;

    let items = dl_manager.list_downloads();
    let item = items.iter().find(|i| i.id == dl_id);
    assert!(item.is_some(), "Загрузка должна быть зарегистрирована");

    let it = item.unwrap();
    println!("Тестовая загрузка: {} байт получено, скорость: {} байт/с, статус: {:?}", it.downloaded_bytes, it.speed_bytes_per_sec, it.status);

    // Pause download test
    let paused = dl_manager.pause_download(&dl_id);
    assert!(paused, "Загрузка должна успешно ставиться на паузу");

    // Clean up
    let _ = fs::remove_dir_all(&temp_dl);
}
