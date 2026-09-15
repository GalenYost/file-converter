use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::Arc;
use std::time::Instant;

use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tokio::sync::{broadcast, mpsc, Mutex, Semaphore};
use tracing::{error, info, warn};

use crate::format::{MediaCategory, MediaFormat};
use crate::job::{ConversionJob, JobEvent, JobId, JobOptions, JobStatus};
use crate::probe::probe_file;
use crate::progress::{ConversionProgress, FfmpegProgressParser};

/// Configuration for the conversion engine.
#[derive(Debug, Clone)]
pub struct EngineConfig {
    /// Maximum number of conversion tasks executed concurrently.
    pub max_concurrent_jobs: usize,
    /// Path to ffmpeg binary (defaults to "ffmpeg").
    pub ffmpeg_path: PathBuf,
}

impl Default for EngineConfig {
    fn default() -> Self {
        Self {
            max_concurrent_jobs: 2,
            ffmpeg_path: PathBuf::from("ffmpeg"),
        }
    }
}

/// Thread-safe media conversion engine managing parallel transcoding tasks.
pub struct ConversionEngine {
    config: EngineConfig,
    jobs: Arc<Mutex<HashMap<JobId, ConversionJob>>>,
    cancellation_senders: Arc<Mutex<HashMap<JobId, mpsc::Sender<()>>>>,
    event_sender: broadcast::Sender<JobEvent>,
    semaphore: Arc<Semaphore>,
}

impl ConversionEngine {
    pub fn new(config: EngineConfig) -> Self {
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

    /// Subscribe to live job events (progress, completion, errors).
    pub fn subscribe(&self) -> broadcast::Receiver<JobEvent> {
        self.event_sender.subscribe()
    }

    /// Retrieve a snapshot of all jobs in the engine.
    pub async fn get_all_jobs(&self) -> Vec<ConversionJob> {
        let jobs = self.jobs.lock().await;
        let mut list: Vec<ConversionJob> = jobs.values().cloned().collect();
        list.sort_by_key(|j| j.id.0);
        list
    }

    /// Retrieve a specific job's status.
    pub async fn get_job(&self, id: JobId) -> Option<ConversionJob> {
        let jobs = self.jobs.lock().await;
        jobs.get(&id).cloned()
    }

    /// Add a single conversion job and schedule it for execution.
    pub async fn submit_job(&self, job: ConversionJob) -> JobId {
        let id = job.id;

        {
            let mut jobs = self.jobs.lock().await;
            jobs.insert(id, job.clone());
        }

        let _ = self.event_sender.send(JobEvent::Added(job.clone()));

        // Spawn background worker for this job
        let (cancel_tx, cancel_rx) = mpsc::channel(1);
        {
            let mut cancels = self.cancellation_senders.lock().await;
            cancels.insert(id, cancel_tx);
        }

        let jobs_map = Arc::clone(&self.jobs);
        let cancel_map = Arc::clone(&self.cancellation_senders);
        let event_sender = self.event_sender.clone();
        let semaphore = Arc::clone(&self.semaphore);
        let ffmpeg_bin = self.config.ffmpeg_path.clone();

        tokio::spawn(async move {
            run_job_lifecycle(
                job,
                jobs_map,
                cancel_map,
                event_sender,
                semaphore,
                cancel_rx,
                ffmpeg_bin,
            )
            .await;
        });

        id
    }

    /// Helper to submit a batch of input files converting to the same target format.
    pub async fn submit_batch(
        &self,
        inputs: Vec<PathBuf>,
        output_directory: &Path,
        target_format: MediaFormat,
        options: Option<JobOptions>,
    ) -> Vec<JobId> {
        let mut submitted_ids = Vec::new();

        for input in inputs {
            let stem = input
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("converted");
            let output_filename = format!("{}.{}", stem, target_format.extension());
            let output_path = output_directory.join(output_filename);

            let mut job = ConversionJob::new(input, output_path, target_format);
            if let Some(ref opts) = options {
                job = job.with_options(opts.clone());
            }

            let id = self.submit_job(job).await;
            submitted_ids.push(id);
        }

        submitted_ids
    }

    /// Request cancellation of an active or queued job.
    pub async fn cancel_job(&self, id: JobId) -> bool {
        let cancels = self.cancellation_senders.lock().await;
        if let Some(tx) = cancels.get(&id) {
            let _ = tx.send(()).await;
            true
        } else {
            false
        }
    }
}

async fn run_job_lifecycle(
    job: ConversionJob,
    jobs_map: Arc<Mutex<HashMap<JobId, ConversionJob>>>,
    cancel_map: Arc<Mutex<HashMap<JobId, mpsc::Sender<()>>>>,
    event_sender: broadcast::Sender<JobEvent>,
    semaphore: Arc<Semaphore>,
    mut cancel_rx: mpsc::Receiver<()>,
    ffmpeg_bin: PathBuf,
) {
    let id = job.id;

    // Acquire concurrency permit
    let permit = tokio::select! {
        _ = cancel_rx.recv() => {
            update_job_status(&jobs_map, &event_sender, id, JobStatus::Cancelled).await;
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

    // Stage 1: Probing
    update_job_status(&jobs_map, &event_sender, id, JobStatus::Probing).await;

    let media_info = probe_file(&job.input_path).await.ok();
    let total_duration = media_info.as_ref().and_then(|i| i.duration_seconds);

    // Stage 2: Execution
    let start_time = Instant::now();
    let args = build_ffmpeg_args(&job);

    info!(
        "Starting job {} transcoding: {:?} -> {:?}",
        id, job.input_path, job.output_path
    );

    let mut cmd = Command::new(&ffmpeg_bin);
    cmd.args(&args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);

    let mut child = match cmd.spawn() {
        Ok(c) => c,
        Err(e) => {
            let err_msg = format!("Failed to spawn ffmpeg: {}", e);
            error!("Job {} error: {}", id, err_msg);
            update_job_status(&jobs_map, &event_sender, id, JobStatus::Failed(err_msg)).await;
            cleanup_cancel_tx(&cancel_map, id).await;
            drop(permit);
            return;
        }
    };

    let stdout = child.stdout.take().expect("Child stdout piped");
    let stderr = child.stderr.take().expect("Child stderr piped");

    let mut stdout_reader = BufReader::new(stdout).lines();
    let mut parser = FfmpegProgressParser::new(total_duration);
    let mut initial_progress = ConversionProgress::default();
    initial_progress.total_duration_seconds = total_duration;
    update_job_status(
        &jobs_map,
        &event_sender,
        id,
        JobStatus::Converting(initial_progress),
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

    let mut cancelled = false;

    loop {
        tokio::select! {
            _ = cancel_rx.recv() => {
                info!("Job {} received cancellation request", id);
                let _ = child.kill().await;
                cancelled = true;
                break;
            }
            line_res = stdout_reader.next_line() => {
                match line_res {
                    Ok(Some(line)) => {
                        if let Some(progress) = parser.parse_line(&line) {
                            let _ = event_sender.send(JobEvent::ProgressUpdated {
                                id,
                                progress: progress.clone(),
                            });
                            let mut map = jobs_map.lock().await;
                            if let Some(j) = map.get_mut(&id) {
                                j.status = JobStatus::Converting(progress);
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
        update_job_status(&jobs_map, &event_sender, id, JobStatus::Cancelled).await;
        cleanup_cancel_tx(&cancel_map, id).await;
        drop(permit);
        return;
    }

    let status = child.wait().await;
    let stderr_output = stderr_handle.await.unwrap_or_default();

    match status {
        Ok(s) if s.success() => {
            let duration = start_time.elapsed().as_secs_f64();
            info!("Job {} completed in {:.2}s", id, duration);
            update_job_status(
                &jobs_map,
                &event_sender,
                id,
                JobStatus::Completed {
                    output_path: job.output_path.clone(),
                    duration_seconds: duration,
                },
            )
            .await;
            let _ = event_sender.send(JobEvent::Completed {
                id,
                output_path: job.output_path,
            });
        }
        Ok(s) => {
            let code = s.code().unwrap_or(-1);
            let msg = if !stderr_output.is_empty() {
                format!("FFmpeg failed (exit code {}): {}", code, stderr_output)
            } else {
                format!("FFmpeg exited with error code {}", code)
            };
            warn!("Job {} failed: {}", id, msg);
            update_job_status(&jobs_map, &event_sender, id, JobStatus::Failed(msg.clone())).await;
            let _ = event_sender.send(JobEvent::Failed { id, error: msg });
        }
        Err(e) => {
            let msg = format!("Failed waiting for ffmpeg: {}", e);
            update_job_status(&jobs_map, &event_sender, id, JobStatus::Failed(msg.clone())).await;
            let _ = event_sender.send(JobEvent::Failed { id, error: msg });
        }
    }

    cleanup_cancel_tx(&cancel_map, id).await;
    drop(permit);
}

fn build_ffmpeg_args(job: &ConversionJob) -> Vec<String> {
    let mut args = vec![
        "-y".to_string(), // overwrite output
        "-nostats".to_string(),
        "-progress".to_string(),
        "pipe:1".to_string(),
        "-i".to_string(),
        job.input_path.to_string_lossy().to_string(),
    ];

    // Format & Category specific defaults
    match job.target_format.category() {
        MediaCategory::Audio => {
            args.push("-vn".to_string()); // Strip video
            match job.target_format {
                MediaFormat::Mp3 => {
                    args.push("-c:a".to_string());
                    args.push("libmp3lame".to_string());
                    let br = job.options.audio_bitrate_kbps.unwrap_or(320);
                    args.push("-b:a".to_string());
                    args.push(format!("{}k", br));
                }
                MediaFormat::Wav => {
                    args.push("-c:a".to_string());
                    args.push("pcm_s16le".to_string());
                }
                MediaFormat::Flac => {
                    args.push("-c:a".to_string());
                    args.push("flac".to_string());
                }
                MediaFormat::Aac | MediaFormat::M4a => {
                    args.push("-c:a".to_string());
                    args.push("aac".to_string());
                    let br = job.options.audio_bitrate_kbps.unwrap_or(256);
                    args.push("-b:a".to_string());
                    args.push(format!("{}k", br));
                }
                MediaFormat::Ogg => {
                    args.push("-c:a".to_string());
                    args.push("libvorbis".to_string());
                    let br = job.options.audio_bitrate_kbps.unwrap_or(192);
                    args.push("-b:a".to_string());
                    args.push(format!("{}k", br));
                }
                MediaFormat::Opus => {
                    args.push("-c:a".to_string());
                    args.push("libopus".to_string());
                    let br = job.options.audio_bitrate_kbps.unwrap_or(128);
                    args.push("-b:a".to_string());
                    args.push(format!("{}k", br));
                }
                _ => {}
            }
        }
        MediaCategory::Video => {
            match job.target_format {
                MediaFormat::Mp4 => {
                    args.push("-c:v".to_string());
                    args.push("libx264".to_string());
                    args.push("-c:a".to_string());
                    args.push("aac".to_string());
                    args.push("-pix_fmt".to_string());
                    args.push("yuv420p".to_string());
                }
                MediaFormat::Webm => {
                    args.push("-c:v".to_string());
                    args.push("libvpx-vp9".to_string());
                    args.push("-c:a".to_string());
                    args.push("libopus".to_string());
                }
                MediaFormat::Mkv => {
                    args.push("-c:v".to_string());
                    args.push("libx264".to_string());
                    args.push("-c:a".to_string());
                    args.push("aac".to_string());
                }
                _ => {}
            }

            if let Some(crf) = job.options.crf {
                args.push("-crf".to_string());
                args.push(crf.to_string());
            }

            if let Some(ref preset) = job.options.speed_preset {
                args.push("-preset".to_string());
                args.push(preset.clone());
            }

            if let Some((w, h)) = job.options.resolution {
                args.push("-vf".to_string());
                args.push(format!("scale={}:{}", w, h));
            }
        }
        MediaCategory::Image => match job.target_format {
            MediaFormat::Gif => {
                args.push("-vf".to_string());
                args.push("fps=15,scale=480:-1:flags=lanczos".to_string());
            }
            _ => {}
        },
    }

    // Append extra user args
    for extra in &job.options.extra_args {
        args.push(extra.clone());
    }

    // Target output path
    args.push(job.output_path.to_string_lossy().to_string());

    args
}

async fn update_job_status(
    jobs_map: &Arc<Mutex<HashMap<JobId, ConversionJob>>>,
    event_sender: &broadcast::Sender<JobEvent>,
    id: JobId,
    status: JobStatus,
) {
    let mut map = jobs_map.lock().await;
    if let Some(j) = map.get_mut(&id) {
        j.status = status.clone();
    }
    let _ = event_sender.send(JobEvent::StatusUpdated { id, status });
}

async fn cleanup_cancel_tx(
    cancel_map: &Arc<Mutex<HashMap<JobId, mpsc::Sender<()>>>>,
    id: JobId,
) {
    let mut map = cancel_map.lock().await;
    map.remove(&id);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_ffmpeg_args_audio() {
        let job = ConversionJob::new(
            PathBuf::from("/input/audio.wav"),
            PathBuf::from("/output/audio.mp3"),
            MediaFormat::Mp3,
        );
        let args = build_ffmpeg_args(&job);
        assert!(args.contains(&"-vn".to_string()));
        assert!(args.contains(&"libmp3lame".to_string()));
        assert!(args.contains(&"/output/audio.mp3".to_string()));
    }

    #[test]
    fn test_build_ffmpeg_args_video() {
        let job = ConversionJob::new(
            PathBuf::from("/input/video.mov"),
            PathBuf::from("/output/video.mp4"),
            MediaFormat::Mp4,
        );
        let args = build_ffmpeg_args(&job);
        assert!(args.contains(&"libx264".to_string()));
        assert!(args.contains(&"aac".to_string()));
    }
}
