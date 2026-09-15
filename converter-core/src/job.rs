use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use serde::{Deserialize, Serialize};

use crate::format::MediaFormat;
use crate::progress::ConversionProgress;

static JOB_COUNTER: AtomicU64 = AtomicU64::new(1);

/// Unique identifier for a conversion job.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct JobId(pub u64);

impl JobId {
    pub fn new() -> Self {
        JobId(JOB_COUNTER.fetch_add(1, Ordering::Relaxed))
    }
}

impl Default for JobId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for JobId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "#{}", self.0)
    }
}

/// Status lifecycle for a job.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum JobStatus {
    Queued,
    Probing,
    Converting(ConversionProgress),
    Completed {
        output_path: PathBuf,
        duration_seconds: f64,
    },
    Failed(String),
    Cancelled,
}

impl JobStatus {
    pub fn is_terminal(&self) -> bool {
        matches!(
            self,
            JobStatus::Completed { .. } | JobStatus::Failed(_) | JobStatus::Cancelled
        )
    }

    pub fn is_active(&self) -> bool {
        matches!(self, JobStatus::Probing | JobStatus::Converting(_))
    }

    pub fn display_label(&self) -> String {
        match self {
            JobStatus::Queued => "Queued".to_string(),
            JobStatus::Probing => "Probing metadata...".to_string(),
            JobStatus::Converting(p) => format!("{:.1}%", p.percentage),
            JobStatus::Completed { .. } => "Completed".to_string(),
            JobStatus::Failed(err) => format!("Failed: {}", err),
            JobStatus::Cancelled => "Cancelled".to_string(),
        }
    }
}

/// Encoding / transcoding options for a job.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct JobOptions {
    /// Constant Rate Factor (CRF) for video quality (e.g. 18-28).
    pub crf: Option<u8>,
    /// Encoder speed preset (e.g. "ultrafast", "medium", "veryslow").
    pub speed_preset: Option<String>,
    /// Audio bitrate in kbps (e.g. 128, 192, 320).
    pub audio_bitrate_kbps: Option<u32>,
    /// Target video resolution scaling (width, height), preserving aspect ratio if one is -1.
    pub resolution: Option<(i32, i32)>,
    /// Custom FFmpeg flags if needed.
    pub extra_args: Vec<String>,
}

/// Specification of a single conversion task.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ConversionJob {
    pub id: JobId,
    pub input_path: PathBuf,
    pub output_path: PathBuf,
    pub target_format: MediaFormat,
    pub options: JobOptions,
    pub status: JobStatus,
}

impl ConversionJob {
    pub fn new(input_path: PathBuf, output_path: PathBuf, target_format: MediaFormat) -> Self {
        Self {
            id: JobId::new(),
            input_path,
            output_path,
            target_format,
            options: JobOptions::default(),
            status: JobStatus::Queued,
        }
    }

    pub fn with_options(mut self, options: JobOptions) -> Self {
        self.options = options;
        self
    }
}

/// Real-time events broadcasted by the conversion engine.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum JobEvent {
    Added(ConversionJob),
    StatusUpdated { id: JobId, status: JobStatus },
    ProgressUpdated { id: JobId, progress: ConversionProgress },
    Completed { id: JobId, output_path: PathBuf },
    Failed { id: JobId, error: String },
    Cancelled { id: JobId },
}
