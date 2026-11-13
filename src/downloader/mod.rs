use futures_util::StreamExt;
use reqwest::header::{ACCEPT_RANGES, CONTENT_LENGTH, RANGE};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::OpenOptions;
use std::io::{Seek, SeekFrom, Write};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum DownloadStatus {
    Pending,
    Downloading,
    Paused,
    Completed,
    Failed(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DownloadItemDto {
    pub id: String,
    pub url: String,
    pub filename: String,
    pub file_path: String,
    pub total_bytes: u64,
    pub downloaded_bytes: u64,
    pub speed_bytes_per_sec: u64,
    pub progress_percent: f32,
    pub status: DownloadStatus,
    pub threads_used: usize,
    pub supports_ranges: bool,
}

pub struct ActiveDownload {
    pub id: String,
    pub url: String,
    pub filename: String,
    pub file_path: PathBuf,
    pub total_bytes: Arc<AtomicU64>,
    pub downloaded_bytes: Arc<AtomicU64>,
    pub speed_bps: Arc<AtomicU64>,
    pub threads_used: usize,
    pub supports_ranges: bool,
    pub status: Arc<RwLock<DownloadStatus>>,
    pub cancel_token: Arc<RwLock<CancellationToken>>,
}

pub struct DownloadManager {
    client: Client,
    downloads: Arc<RwLock<HashMap<String, Arc<ActiveDownload>>>>,
    downloads_dir: PathBuf,
}

impl DownloadManager {
    pub fn new(dir: Option<PathBuf>) -> Self {
        let downloads_dir = dir.unwrap_or_else(|| {
            // Default to user's standard Downloads folder or ./downloads
            if let Some(user_profile) = std::env::var_os("USERPROFILE") {
                let p = PathBuf::from(user_profile).join("Downloads");
                if p.exists() {
                    return p;
                }
            }
            let mut p = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
            p.push("downloads");
            let _ = std::fs::create_dir_all(&p);
            p
        });

        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .unwrap_or_default();

        Self {
            client,
            downloads: Arc::new(RwLock::new(HashMap::new())),
            downloads_dir,
        }
    }

    pub fn list_downloads(&self) -> Vec<DownloadItemDto> {
        let lock = self.downloads.read().unwrap();
        let mut list = Vec::new();

        for item in lock.values() {
            let total = item.total_bytes.load(Ordering::Relaxed);
            let downloaded = item.downloaded_bytes.load(Ordering::Relaxed);
            let speed = item.speed_bps.load(Ordering::Relaxed);
            let status = item.status.read().unwrap().clone();

            let progress = if total > 0 {
                (downloaded as f32 / total as f32) * 100.0
            } else if status == DownloadStatus::Completed {
                100.0
            } else {
                0.0
            };

            list.push(DownloadItemDto {
                id: item.id.clone(),
                url: item.url.clone(),
                filename: item.filename.clone(),
                file_path: item.file_path.to_string_lossy().to_string(),
                total_bytes: total,
                downloaded_bytes: downloaded,
                speed_bytes_per_sec: speed,
                progress_percent: progress.min(100.0),
                status,
                threads_used: item.threads_used,
                supports_ranges: item.supports_ranges,
            });
        }

        list.sort_by(|a, b| b.id.cmp(&a.id));
        list
    }

    pub fn start_download(
        &self,
        url: &str,
        suggested_filename: Option<&str>,
        max_threads: u32,
    ) -> String {
        let id = Uuid::new_v4().to_string();

        let filename = suggested_filename
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string())
            .unwrap_or_else(|| {
                url.split('/')
                    .last()
                    .and_then(|seg| seg.split('?').next())
                    .filter(|s| !s.is_empty())
                    .unwrap_or("downloaded_file.bin")
                    .to_string()
            });

        let file_path = self.downloads_dir.join(&filename);

        let active = Arc::new(ActiveDownload {
            id: id.clone(),
            url: url.to_string(),
            filename: filename.clone(),
            file_path: file_path.clone(),
            total_bytes: Arc::new(AtomicU64::new(0)),
            downloaded_bytes: Arc::new(AtomicU64::new(0)),
            speed_bps: Arc::new(AtomicU64::new(0)),
            threads_used: 1,
            supports_ranges: false,
            status: Arc::new(RwLock::new(DownloadStatus::Downloading)),
            cancel_token: Arc::new(RwLock::new(CancellationToken::new())),
        });

        {
            let mut lock = self.downloads.write().unwrap();
            lock.insert(id.clone(), Arc::clone(&active));
        }

        let client = self.client.clone();
        let active_clone = Arc::clone(&active);

        tokio::spawn(async move {
            Self::execute_download(client, active_clone, max_threads).await;
        });

        id
    }

    async fn execute_download(
        client: Client,
        item: Arc<ActiveDownload>,
        max_threads: u32,
    ) {
        let cancel_token = item.cancel_token.read().unwrap().clone();

        // 1. Send HEAD or initial probe to check range support & length
        let mut total_length: Option<u64> = None;
        let mut accepts_ranges = false;

        if let Ok(resp) = client.head(&item.url).send().await {
            if resp.status().is_success() {
                if let Some(cl) = resp.headers().get(CONTENT_LENGTH) {
                    if let Ok(cl_str) = cl.to_str() {
                        total_length = cl_str.parse().ok();
                    }
                }
                if let Some(ar) = resp.headers().get(ACCEPT_RANGES) {
                    if let Ok(ar_str) = ar.to_str() {
                        accepts_ranges = ar_str.eq_ignore_ascii_case("bytes");
                    }
                }
            }
        }

        // Additional probe for range if HEAD omitted Accept-Ranges
        if !accepts_ranges && total_length.is_some() {
            if let Ok(test_resp) = client
                .get(&item.url)
                .header(RANGE, "bytes=0-0")
                .send()
                .await
            {
                if test_resp.status() == reqwest::StatusCode::PARTIAL_CONTENT {
                    accepts_ranges = true;
                }
            }
        }

        let total_bytes = total_length.unwrap_or(0);
        item.total_bytes.store(total_bytes, Ordering::Relaxed);

        // Decide multi-threaded vs single-threaded
        let threads = if accepts_ranges && total_bytes > 1024 * 512 && max_threads > 1 {
            (max_threads as usize).clamp(2, 16)
        } else {
            1
        };

        unsafe {
            // Update threads_used and supports_ranges in item
            let mutable_ptr = Arc::as_ptr(&item) as *mut ActiveDownload;
            (*mutable_ptr).threads_used = threads;
            (*mutable_ptr).supports_ranges = accepts_ranges;
        }

        // Start speed monitor task
        let monitor_token = cancel_token.clone();
        let monitor_downloaded = Arc::clone(&item.downloaded_bytes);
        let monitor_speed = Arc::clone(&item.speed_bps);
        let monitor_status = Arc::clone(&item.status);

        let monitor_handle = tokio::spawn(async move {
            let mut last_bytes = 0u64;
            let mut last_time = Instant::now();

            while !monitor_token.is_cancelled() {
                tokio::time::sleep(Duration::from_millis(500)).await;
                let current_bytes = monitor_downloaded.load(Ordering::Relaxed);
                let elapsed = last_time.elapsed().as_secs_f64();

                if elapsed > 0.0 {
                    let bytes_diff = current_bytes.saturating_sub(last_bytes);
                    let bps = (bytes_diff as f64 / elapsed) as u64;
                    monitor_speed.store(bps, Ordering::Relaxed);
                }

                last_bytes = current_bytes;
                last_time = Instant::now();

                let st = monitor_status.read().unwrap().clone();
                if st != DownloadStatus::Downloading {
                    break;
                }
            }
            monitor_speed.store(0, Ordering::Relaxed);
        });

        // 2. Perform Download
        let res = if threads > 1 {
            Self::download_multithreaded(
                client,
                &item.url,
                &item.file_path,
                total_bytes,
                threads,
                Arc::clone(&item.downloaded_bytes),
                cancel_token.clone(),
            )
            .await
        } else {
            Self::download_singlethreaded(
                client,
                &item.url,
                &item.file_path,
                Arc::clone(&item.downloaded_bytes),
                cancel_token.clone(),
            )
            .await
        };

        monitor_handle.abort();
        item.speed_bps.store(0, Ordering::Relaxed);

        let mut st_lock = item.status.write().unwrap();
        if cancel_token.is_cancelled() {
            *st_lock = DownloadStatus::Paused;
        } else {
            match res {
                Ok(_) => {
                    *st_lock = DownloadStatus::Completed;
                    let downloaded = item.downloaded_bytes.load(Ordering::Relaxed);
                    if total_bytes == 0 {
                        item.total_bytes.store(downloaded, Ordering::Relaxed);
                    }
                }
                Err(err) => {
                    *st_lock = DownloadStatus::Failed(err);
                }
            }
        }
    }

    async fn download_multithreaded(
        client: Client,
        url: &str,
        dest_path: &PathBuf,
        total_bytes: u64,
        threads: usize,
        downloaded_counter: Arc<AtomicU64>,
        cancel_token: CancellationToken,
    ) -> Result<(), String> {
        // Pre-allocate file on disk
        {
            let file = OpenOptions::new()
                .create(true)
                .write(true)
                .truncate(true)
                .open(dest_path)
                .map_err(|e| format!("Не удалось создать файл: {}", e))?;
            file.set_len(total_bytes)
                .map_err(|e| format!("Не удалось выделить размер файла: {}", e))?;
        }

        let part_size = total_bytes / (threads as u64);
        let mut tasks = Vec::new();

        for i in 0..threads {
            let start = (i as u64) * part_size;
            let end = if i == threads - 1 {
                total_bytes - 1
            } else {
                ((i + 1) as u64) * part_size - 1
            };

            let client_c = client.clone();
            let url_c = url.to_string();
            let path_c = dest_path.clone();
            let counter_c = Arc::clone(&downloaded_counter);
            let token_c = cancel_token.clone();

            tasks.push(tokio::spawn(async move {
                Self::download_chunk(client_c, url_c, path_c, start, end, counter_c, token_c).await
            }));
        }

        for task in tasks {
            match task.await {
                Ok(Ok(_)) => {}
                Ok(Err(e)) => return Err(e),
                Err(join_err) => return Err(format!("Ошибка потока загрузки: {}", join_err)),
            }
        }

        Ok(())
    }

    async fn download_chunk(
        client: Client,
        url: String,
        dest_path: PathBuf,
        start: u64,
        end: u64,
        downloaded_counter: Arc<AtomicU64>,
        cancel_token: CancellationToken,
    ) -> Result<(), String> {
        let range_header = format!("bytes={}-{}", start, end);
        let resp = client
            .get(&url)
            .header(RANGE, range_header)
            .send()
            .await
            .map_err(|e| format!("Ошибка HTTP запроса: {}", e))?;

        if !resp.status().is_success() {
            return Err(format!("Сервер вернул статус {}", resp.status()));
        }

        // Open local file for seeking and writing
        let mut file = OpenOptions::new()
            .write(true)
            .open(&dest_path)
            .map_err(|e| format!("Ошибка открытия файла для записи: {}", e))?;

        file.seek(SeekFrom::Start(start))
            .map_err(|e| format!("Ошибка Seek: {}", e))?;

        let mut stream = resp.bytes_stream();
        while let Some(chunk_result) = stream.next().await {
            if cancel_token.is_cancelled() {
                return Ok(());
            }

            let chunk = chunk_result.map_err(|e| format!("Ошибка чтения потока: {}", e))?;
            file.write_all(&chunk)
                .map_err(|e| format!("Ошибка записи в файл: {}", e))?;
            downloaded_counter.fetch_add(chunk.len() as u64, Ordering::Relaxed);
        }

        file.flush()
            .map_err(|e| format!("Ошибка сброса буфера: {}", e))?;
        Ok(())
    }

    async fn download_singlethreaded(
        client: Client,
        url: &str,
        dest_path: &PathBuf,
        downloaded_counter: Arc<AtomicU64>,
        cancel_token: CancellationToken,
    ) -> Result<(), String> {
        let resp = client
            .get(url)
            .send()
            .await
            .map_err(|e| format!("Ошибка сети: {}", e))?;

        if !resp.status().is_success() {
            return Err(format!("Сервер вернул статус {}", resp.status()));
        }

        let mut file = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(dest_path)
            .map_err(|e| format!("Ошибка создания файла: {}", e))?;

        let mut stream = resp.bytes_stream();
        while let Some(chunk_result) = stream.next().await {
            if cancel_token.is_cancelled() {
                return Ok(());
            }

            let chunk = chunk_result.map_err(|e| format!("Ошибка потока: {}", e))?;
            file.write_all(&chunk)
                .map_err(|e| format!("Ошибка записи: {}", e))?;
            downloaded_counter.fetch_add(chunk.len() as u64, Ordering::Relaxed);
        }

        file.flush()
            .map_err(|e| format!("Ошибка сброса файла: {}", e))?;
        Ok(())
    }

    pub fn pause_download(&self, id: &str) -> bool {
        let lock = self.downloads.read().unwrap();
        if let Some(item) = lock.get(id) {
            item.cancel_token.read().unwrap().cancel();
            let mut st = item.status.write().unwrap();
            *st = DownloadStatus::Paused;
            return true;
        }
        false
    }

    pub fn resume_download(&self, id: &str, max_threads: u32) -> bool {
        let lock = self.downloads.read().unwrap();
        if let Some(item) = lock.get(id) {
            let mut st = item.status.write().unwrap();
            if *st == DownloadStatus::Paused {
                *st = DownloadStatus::Downloading;
                *item.cancel_token.write().unwrap() = CancellationToken::new();

                let client = self.client.clone();
                let item_clone = Arc::clone(item);

                tokio::spawn(async move {
                    Self::execute_download(client, item_clone, max_threads).await;
                });
                return true;
            }
        }
        false
    }

    pub fn cancel_download(&self, id: &str) -> bool {
        let lock = self.downloads.read().unwrap();
        if let Some(item) = lock.get(id) {
            item.cancel_token.read().unwrap().cancel();
            let mut st = item.status.write().unwrap();
            *st = DownloadStatus::Failed("Отменено пользователем".to_string());
            let _ = std::fs::remove_file(&item.file_path);
            return true;
        }
        false
    }

    pub fn has_active_downloads(&self) -> bool {
        let lock = self.downloads.read().unwrap();
        lock.values().any(|item| *item.status.read().unwrap() == DownloadStatus::Downloading)
    }

    pub fn open_downloaded_file(&self, id: &str) -> bool {
        let lock = self.downloads.read().unwrap();
        if let Some(item) = lock.get(id) {
            if item.file_path.exists() {
                #[cfg(target_os = "windows")]
                {
                    let _ = std::process::Command::new("cmd")
                        .args(["/c", "start", "", &item.file_path.to_string_lossy()])
                        .spawn();
                    return true;
                }
                #[cfg(not(target_os = "windows"))]
                {
                    let _ = std::process::Command::new("xdg-open")
                        .arg(&item.file_path)
                        .spawn();
                    return true;
                }
            }
        }
        false
    }

    pub fn show_in_folder(&self, id: &str) -> bool {
        let lock = self.downloads.read().unwrap();
        if let Some(item) = lock.get(id) {
            if item.file_path.exists() {
                #[cfg(target_os = "windows")]
                {
                    let _ = std::process::Command::new("explorer")
                        .arg(format!("/select,\"{}\"", item.file_path.to_string_lossy()))
                        .spawn();
                    return true;
                }
                #[cfg(not(target_os = "windows"))]
                {
                    let _ = std::process::Command::new("xdg-open")
                        .arg(&self.downloads_dir)
                        .spawn();
                    return true;
                }
            }
        }
        false
    }

    pub fn open_downloads_folder(&self) {
        let _ = std::process::Command::new("explorer.exe")
            .arg(&self.downloads_dir)
            .spawn();
    }
}
