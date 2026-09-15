use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::sync::{broadcast, mpsc, Mutex, Semaphore};
use tracing::{error, info, warn};

use crate::binaries::create_quiet_cmd;

static DOWNLOAD_COUNTER: AtomicU64 = AtomicU64::new(1);

/// Target format for a TikTok (or any supported URL) download.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DownloadFormat {
    Mp4,
    Webm,
    Mp3,
    M4a,
    Opus,
    Flac,
}

impl DownloadFormat {
    pub const ALL: &'static [DownloadFormat] = &[
        DownloadFormat::Mp4,
        DownloadFormat::Webm,
        DownloadFormat::Mp3,
        DownloadFormat::M4a,
        DownloadFormat::Opus,
        DownloadFormat::Flac,
    ];

    pub fn label(&self) -> &'static str {
        match self {
            DownloadFormat::Mp4 => "MP4",
            DownloadFormat::Webm => "WebM",
            DownloadFormat::Mp3 => "MP3",
            DownloadFormat::M4a => "M4A",
            DownloadFormat::Opus => "OPUS",
            DownloadFormat::Flac => "FLAC",
        }
    }

    pub fn extension(&self) -> &'static str {
        match self {
            DownloadFormat::Mp4 => "mp4",
            DownloadFormat::Webm => "webm",
            DownloadFormat::Mp3 => "mp3",
            DownloadFormat::M4a => "m4a",
            DownloadFormat::Opus => "opus",
            DownloadFormat::Flac => "flac",
        }
    }

    pub fn is_audio(&self) -> bool {
        matches!(self, DownloadFormat::Mp3 | DownloadFormat::M4a | DownloadFormat::Opus | DownloadFormat::Flac)
    }

    /// Extra yt-dlp arguments needed to produce this output format.
    pub fn extra_args(&self) -> Vec<String> {
        match self {
            DownloadFormat::Mp4 => vec![
                "-f".to_string(),
                "bv*+ba/b".to_string(),
                "-S".to_string(),
                "ext:mp4:m4a".to_string(),
            ],
            DownloadFormat::Webm => vec![
                "-f".to_string(),
                "bv*+ba/b".to_string(),
                "-S".to_string(),
                "ext:webm".to_string(),
                "--merge-output-format".to_string(),
                "webm".to_string(),
            ],
            DownloadFormat::Mp3 => vec![
                "-x".to_string(),
                "--audio-format".to_string(),
                "mp3".to_string(),
                "--audio-quality".to_string(),
                "0".to_string(),
            ],
            DownloadFormat::M4a => vec![
                "-x".to_string(),
                "--audio-format".to_string(),
                "m4a".to_string(),
                "--audio-quality".to_string(),
                "0".to_string(),
            ],
            DownloadFormat::Opus => vec![
                "-x".to_string(),
                "--audio-format".to_string(),
                "opus".to_string(),
                "--audio-quality".to_string(),
                "0".to_string(),
            ],
            DownloadFormat::Flac => vec![
                "-x".to_string(),
                "--audio-format".to_string(),
                "flac".to_string(),
                "--audio-quality".to_string(),
                "0".to_string(),
            ],
        }
    }
}

impl std::fmt::Display for DownloadFormat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.label())
    }
}

/// Unique identifier for a download job.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct DownloadId(pub u64);

impl DownloadId {
    pub fn new() -> Self {
        DownloadId(DOWNLOAD_COUNTER.fetch_add(1, Ordering::Relaxed))
    }
}

impl Default for DownloadId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for DownloadId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "dl#{}", self.0)
    }
}

/// Live progress for an in-flight download.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct DownloadProgress {
    pub percentage: f64,
    pub downloaded_bytes: Option<u64>,
    pub total_bytes: Option<u64>,
    pub speed: Option<String>,
    pub eta_seconds: Option<u64>,
}

/// Status lifecycle for a download job.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum DownloadStatus {
    Queued,
    Downloading(DownloadProgress),
    Completed { output_path: PathBuf },
    Failed(String),
    Cancelled,
}

/// Specification of a single download task.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DownloadJob {
    pub id: DownloadId,
    pub url: String,
    pub output_directory: PathBuf,
    pub format: DownloadFormat,
    pub status: DownloadStatus,
}

impl DownloadJob {
    pub fn new(url: String, output_directory: PathBuf, format: DownloadFormat) -> Self {
        Self {
            id: DownloadId::new(),
            url,
            output_directory,
            format,
            status: DownloadStatus::Queued,
        }
    }
}

/// Real-time events broadcast by the download engine.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum DownloadEvent {
    Added(DownloadJob),
    StatusUpdated {
        id: DownloadId,
        status: DownloadStatus,
    },
    ProgressUpdated {
        id: DownloadId,
        progress: DownloadProgress,
    },
    Completed {
        id: DownloadId,
        output_path: PathBuf,
    },
    Failed { id: DownloadId, error: String },
    Cancelled { id: DownloadId },
}

/// Configuration for the download engine.
#[derive(Debug, Clone)]
pub struct DownloadEngineConfig {
    /// Path to the yt-dlp binary (defaults to "yt-dlp").
    pub ytdlp_path: PathBuf,
    /// Maximum number of downloads executed concurrently.
    pub max_concurrent_jobs: usize,
}

impl Default for DownloadEngineConfig {
    fn default() -> Self {
        Self {
            ytdlp_path: crate::binaries::find_ytdlp(),
            max_concurrent_jobs: 2,
        }
    }
}

/// Thread-safe download engine managing concurrent yt-dlp downloads.
pub struct DownloadEngine {
    config: DownloadEngineConfig,
    jobs: Arc<Mutex<HashMap<DownloadId, DownloadJob>>>,
    cancellation_senders: Arc<Mutex<HashMap<DownloadId, mpsc::Sender<()>>>>,
    event_sender: broadcast::Sender<DownloadEvent>,
    semaphore: Arc<Semaphore>,
}

impl DownloadEngine {
    pub fn new(config: DownloadEngineConfig) -> Self {
        let (event_sender, _) = broadcast::channel(1024);
        let max_concurrent = config.max_concurrent_jobs.max(1);

        Self {
            config,
            jobs: Arc::new(Mutex::new(HashMap::new())),
            cancellation_senders: Arc::new(Mutex::new(HashMap::new())),
            event_sender,
            semaphore: Arc::new(Semaphore::new(max_concurrent)),
        }
    }

    /// Subscribe to live download events (progress, completion, errors).
    pub fn subscribe(&self) -> broadcast::Receiver<DownloadEvent> {
        self.event_sender.subscribe()
    }

    /// Retrieve a snapshot of all download jobs in the engine.
    pub async fn get_all_jobs(&self) -> Vec<DownloadJob> {
        let jobs = self.jobs.lock().await;
        let mut list: Vec<DownloadJob> = jobs.values().cloned().collect();
        list.sort_by_key(|j| j.id.0);
        list
    }

    /// Retrieve a specific download job.
    pub async fn get_job(&self, id: DownloadId) -> Option<DownloadJob> {
        let jobs = self.jobs.lock().await;
        jobs.get(&id).cloned()
    }

    /// Enqueue a download and schedule it for execution.
    pub async fn submit(
        &self,
        url: String,
        output_directory: PathBuf,
        format: DownloadFormat,
    ) -> DownloadId {
        let job = DownloadJob::new(url, output_directory, format);
        let id = job.id;

        {
            let mut jobs = self.jobs.lock().await;
            jobs.insert(id, job.clone());
        }

        let _ = self.event_sender.send(DownloadEvent::Added(job.clone()));

        let (cancel_tx, cancel_rx) = mpsc::channel(1);
        {
            let mut cancels = self.cancellation_senders.lock().await;
            cancels.insert(id, cancel_tx);
        }

        let jobs_map = Arc::clone(&self.jobs);
        let cancel_map = Arc::clone(&self.cancellation_senders);
        let event_sender = self.event_sender.clone();
        let semaphore = Arc::clone(&self.semaphore);
        let ytdlp_bin = self.config.ytdlp_path.clone();

        tokio::spawn(async move {
            run_download_lifecycle(
                job,
                jobs_map,
                cancel_map,
                event_sender,
                semaphore,
                cancel_rx,
                ytdlp_bin,
            )
            .await;
        });

        id
    }

    /// Request cancellation of an active or queued download.
    pub async fn cancel(&self, id: DownloadId) -> bool {
        let cancels = self.cancellation_senders.lock().await;
        if let Some(tx) = cancels.get(&id) {
            let _ = tx.send(()).await;
            true
        } else {
            false
        }
    }
}

async fn run_download_lifecycle(
    job: DownloadJob,
    jobs_map: Arc<Mutex<HashMap<DownloadId, DownloadJob>>>,
    cancel_map: Arc<Mutex<HashMap<DownloadId, mpsc::Sender<()>>>>,
    event_sender: broadcast::Sender<DownloadEvent>,
    semaphore: Arc<Semaphore>,
    mut cancel_rx: mpsc::Receiver<()>,
    ytdlp_bin: PathBuf,
) {
    let id = job.id;

    // Acquire concurrency permit
    let permit = tokio::select! {
        _ = cancel_rx.recv() => {
            update_download_status(&jobs_map, &event_sender, id, DownloadStatus::Cancelled).await;
            cleanup_cancel_tx(&cancel_map, id).await;
            return;
        }
        res = semaphore.acquire_owned() => {
            match res {
                Ok(p) => p,
                Err(_) => return,
            }
        }
    };

    let ffmpeg_loc = ffmpeg_location().await;

    info!(
        "Starting download {}: {} -> {:?}",
        id, job.url, job.output_directory
    );

    let mut cmd = create_quiet_cmd(&ytdlp_bin);
    cmd.args(build_ytdlp_args(&job, &ffmpeg_loc))
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);

    let mut child = match cmd.spawn() {
        Ok(c) => c,
        Err(e) => {
            let err_msg = format!("Failed to spawn yt-dlp: {}", e);
            error!("Download {} error: {}", id, err_msg);
            update_download_status(&jobs_map, &event_sender, id, DownloadStatus::Failed(err_msg))
                .await;
            cleanup_cancel_tx(&cancel_map, id).await;
            drop(permit);
            return;
        }
    };

    let stdout = child.stdout.take().expect("Child stdout piped");
    let stderr = child.stderr.take().expect("Child stderr piped");

    let mut stdout_reader = BufReader::new(stdout).lines();

    update_download_status(
        &jobs_map,
        &event_sender,
        id,
        DownloadStatus::Downloading(DownloadProgress::default()),
    )
    .await;

    // Background stderr collector for error reporting
    let stderr_handle = tokio::spawn(async move {
        let mut reader = BufReader::new(stderr).lines();
        let mut lines = Vec::new();
        while let Ok(Some(line)) = reader.next_line().await {
            lines.push(line);
            if lines.len() > 50 {
                lines.remove(0); // keep last 50 error lines
            }
        }
        lines.join("\n")
    });

    let mut destination_path: Option<PathBuf> = None;
    let mut cancelled = false;

    loop {
        tokio::select! {
            _ = cancel_rx.recv() => {
                info!("Download {} received cancellation request", id);
                let _ = child.kill().await;
                cancelled = true;
                break;
            }
            line_res = stdout_reader.next_line() => {
                match line_res {
                    Ok(Some(line)) => {
                        if let Some(dest) = extract_destination(&line) {
                            destination_path = Some(dest);
                        }
                        if let Some(progress) = parse_download_progress(&line) {
                            let _ = event_sender.send(DownloadEvent::ProgressUpdated {
                                id,
                                progress: progress.clone(),
                            });
                            let mut map = jobs_map.lock().await;
                            if let Some(j) = map.get_mut(&id) {
                                j.status = DownloadStatus::Downloading(progress);
                            }
                        }
                    }
                    Ok(None) => break, // EOF
                    Err(_) => break,
                }
            }
        }
    }

    if cancelled {
        update_download_status(&jobs_map, &event_sender, id, DownloadStatus::Cancelled).await;
        cleanup_cancel_tx(&cancel_map, id).await;
        drop(permit);
        return;
    }

    let status = child.wait().await;
    let stderr_output = stderr_handle.await.unwrap_or_default();

    match status {
        Ok(s) if s.success() => {
            let output_path = resolve_output_path(&job, destination_path);
            info!("Download {} completed", id);
            update_download_status(
                &jobs_map,
                &event_sender,
                id,
                DownloadStatus::Completed {
                    output_path: output_path.clone(),
                },
            )
            .await;
            let _ = event_sender.send(DownloadEvent::Completed { id, output_path });
        }
        Ok(s) => {
            let code = s.code().unwrap_or(-1);
            let msg = if !stderr_output.is_empty() {
                format!("yt-dlp failed (exit code {}): {}", code, stderr_output)
            } else {
                format!("yt-dlp exited with error code {}", code)
            };
            warn!("Download {} failed: {}", id, msg);
            update_download_status(&jobs_map, &event_sender, id, DownloadStatus::Failed(msg.clone()))
                .await;
            let _ = event_sender.send(DownloadEvent::Failed { id, error: msg });
        }
        Err(e) => {
            let msg = format!("Failed waiting for yt-dlp: {}", e);
            update_download_status(&jobs_map, &event_sender, id, DownloadStatus::Failed(msg.clone()))
                .await;
            let _ = event_sender.send(DownloadEvent::Failed { id, error: msg });
        }
    }

    cleanup_cancel_tx(&cancel_map, id).await;
    drop(permit);
}

fn build_ytdlp_args(job: &DownloadJob, ffmpeg_loc: &str) -> Vec<String> {
    let mut args = vec![
        "--newline".to_string(),
        "--no-playlist".to_string(),
        "--ffmpeg-location".to_string(),
        ffmpeg_loc.to_string(),
        "-o".to_string(),
        job.output_directory
            .join("%(title)s [%(id)s].%(ext)s")
            .to_string_lossy()
            .to_string(),
    ];

    args.extend(job.format.extra_args());
    args.push(job.url.clone());

    args
}

/// Directory containing the ffmpeg binary for yt-dlp to use when merging or
/// post-processing. Resolves ffmpeg with execution verification, preferring
/// the system binary, then falling back to the bundled copy next to the app.
async fn ffmpeg_location() -> String {
    let resolved = crate::binaries::find_ffmpeg_resolved().await;
    resolved
        .parent()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|| ".".to_string())
}

fn extract_destination(line: &str) -> Option<PathBuf> {
    let marker = "[download] Destination: ";
    if let Some(idx) = line.find(marker) {
        let path_str = line[idx + marker.len()..].trim();
        if !path_str.is_empty() {
            return Some(PathBuf::from(path_str));
        }
    }
    None
}

/// Resolve the final output path: for audio conversions yt-dlp changes the
/// extension during post-processing, otherwise keep the reported destination.
fn resolve_output_path(job: &DownloadJob, destination: Option<PathBuf>) -> PathBuf {
    if let Some(dest) = destination {
        let final_path = dest.with_extension(job.format.extension());
        if final_path.exists() || job.format.is_audio() {
            return final_path;
        }
        if dest.exists() {
            return dest;
        }
        return final_path;
    }
    job.output_directory.clone()
}

/// Parse a yt-dlp progress line like:
/// `[download]  12.3% of    2.00MiB at 819.5KiB/s ETA 00:01`
pub fn parse_download_progress(line: &str) -> Option<DownloadProgress> {
    if !line.contains("[download]") || line.contains("Destination:") {
        return None;
    }

    let pct_idx = line.find('%')?;
    let pct_str = line[..pct_idx].rsplit(' ').next()?.trim();
    let percentage: f64 = pct_str.parse().ok()?;
    if !(-0.01..=101.0).contains(&percentage) {
        return None;
    }

    let rest = &line[pct_idx + 1..];
    let tokens: Vec<&str> = rest.split_whitespace().collect();

    let mut total_bytes = None;
    let mut speed = None;
    let mut eta_seconds = None;

    for (i, tok) in tokens.iter().enumerate() {
        match *tok {
            "of" => {
                if let Some(size) = tokens.get(i + 1) {
                    total_bytes = parse_byte_size(size);
                }
            }
            "at" => {
                if let Some(speed_tok) = tokens.get(i + 1) {
                    speed = Some(speed_tok.to_string());
                }
            }
            "ETA" => {
                if let Some(eta_tok) = tokens.get(i + 1) {
                    eta_seconds = parse_eta(eta_tok);
                }
            }
            _ => {}
        }
    }

    let downloaded_bytes = total_bytes
        .map(|total| (total as f64 * percentage / 100.0) as u64);

    Some(DownloadProgress {
        percentage,
        downloaded_bytes,
        total_bytes,
        speed,
        eta_seconds,
    })
}

fn parse_byte_size(tok: &str) -> Option<u64> {
    let tok = tok.trim();
    if tok.is_empty() {
        return None;
    }
    let num_end = tok
        .char_indices()
        .find(|(_, c)| !c.is_ascii_digit() && *c != '.')
        .map(|(i, _)| i)
        .unwrap_or(tok.len());
    let num_str = &tok[..num_end];
    let unit = &tok[num_end..];
    let val: f64 = num_str.parse().ok()?;
    let mult: f64 = match unit.to_lowercase().as_str() {
        "b" => 1.0,
        "kib" => 1024.0,
        "mib" => 1024.0 * 1024.0,
        "gib" => 1024.0 * 1024.0 * 1024.0,
        "kb" => 1000.0,
        "mb" => 1000.0 * 1000.0,
        "gb" => 1000.0 * 1000.0 * 1000.0,
        _ => return None,
    };
    Some((val * mult) as u64)
}

fn parse_eta(tok: &str) -> Option<u64> {
    let parts: Vec<&str> = tok.split(':').collect();
    match parts.len() {
        3 => {
            let hours: u64 = parts[0].parse().ok()?;
            let minutes: u64 = parts[1].parse().ok()?;
            let seconds: u64 = parts[2].parse().ok()?;
            Some(hours * 3600 + minutes * 60 + seconds)
        }
        2 => {
            let minutes: u64 = parts[0].parse().ok()?;
            let seconds: u64 = parts[1].parse().ok()?;
            Some(minutes * 60 + seconds)
        }
        _ => None,
    }
}

async fn update_download_status(
    jobs_map: &Arc<Mutex<HashMap<DownloadId, DownloadJob>>>,
    event_sender: &broadcast::Sender<DownloadEvent>,
    id: DownloadId,
    status: DownloadStatus,
) {
    let mut map = jobs_map.lock().await;
    if let Some(j) = map.get_mut(&id) {
        j.status = status.clone();
    }
    let _ = event_sender.send(DownloadEvent::StatusUpdated { id, status });
}

async fn cleanup_cancel_tx(
    cancel_map: &Arc<Mutex<HashMap<DownloadId, mpsc::Sender<()>>>>,
    id: DownloadId,
) {
    let mut map = cancel_map.lock().await;
    map.remove(&id);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_progress_typical() {
        let line = "[download]  12.3% of    2.00MiB at 819.5KiB/s ETA 00:01";
        let p = parse_download_progress(line).expect("should parse");
        assert!((p.percentage - 12.3).abs() < 0.001);
        assert_eq!(p.total_bytes, Some(2 * 1024 * 1024));
        assert_eq!(p.speed.as_deref(), Some("819.5KiB/s"));
        assert_eq!(p.eta_seconds, Some(1));
    }

    #[test]
    fn test_parse_progress_completed() {
        let line = "[download]  100% of 1.00MiB in 00:00:02 at 500.00KiB/s";
        let p = parse_download_progress(line).expect("should parse");
        assert!((p.percentage - 100.0).abs() < 0.001);
        assert_eq!(p.total_bytes, Some(1024 * 1024));
        assert_eq!(p.eta_seconds, None);
    }

    #[test]
    fn test_parse_progress_ignores_other_lines() {
        assert!(parse_download_progress("[download] Destination: /tmp/video.mp4").is_none());
        assert!(parse_download_progress("[info] something else").is_none());
    }

    #[test]
    fn test_format_args() {
        assert!(DownloadFormat::Mp3.extra_args().contains(&"--audio-format".to_string()));
        assert!(DownloadFormat::Mp3.extra_args().contains(&"mp3".to_string()));
        assert!(DownloadFormat::Mp4.extra_args().contains(&"-f".to_string()));
    }

    #[test]
    fn test_build_ytdlp_args_includes_url() {
        let job = DownloadJob::new(
            "https://www.tiktok.com/@user/video/123".to_string(),
            PathBuf::from("/out"),
            DownloadFormat::Mp4,
        );
let args = build_ytdlp_args(&job, "/usr/bin");
        assert!(args.contains(&"--newline".to_string()));
        assert!(args.contains(&"https://www.tiktok.com/@user/video/123".to_string()));
    }

    #[test]
    fn test_parse_eta() {
        assert_eq!(parse_eta("00:01"), Some(1));
        assert_eq!(parse_eta("01:02:03"), Some(3723));
        assert_eq!(parse_eta("bogus"), None);
    }
}