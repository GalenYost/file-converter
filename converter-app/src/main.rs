#![windows_subsystem = "windows"]

mod config;
mod i18n;
mod updates;

use std::hash::{Hash, Hasher};
use std::path::PathBuf;
use std::sync::Arc;

use directories::UserDirs;
use iced::widget::{
    button, column, container, pick_list, progress_bar, radio, row, rule, scrollable, slider,
    text, text_input, Space,
};
use iced::{
    Alignment, Color, Element, Length, Subscription, Task, Theme,
};
use rfd::AsyncFileDialog;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

use converter_core::downloader::{
    DownloadEngine, DownloadEngineConfig, DownloadEvent, DownloadFormat, DownloadId,
    DownloadProgress, DownloadStatus,
};
use converter_core::engine::{ConversionEngine, EngineConfig};
use converter_core::format::MediaFormat;
use converter_core::job::{ConversionJob, JobEvent, JobId, JobStatus};
use converter_core::progress::ConversionProgress;

use config::{get_config_dir, load_config, save_config, AppConfig, StartupWindowMode};
use i18n::Language;

fn main() -> iced::Result {
    let _log_guard = init_logging();
    tracing::info!("Starting File converter v{}", updates::CURRENT_VERSION);

    updates::cleanup_stale_update_files();

    let app_config = load_config();

    let window_settings = iced::window::Settings {
        maximized: app_config.window_mode == StartupWindowMode::Maximized,
        ..Default::default()
    };

    iced::application(move || App::new(app_config.clone()), App::update, App::view)
        .title("File converter")
        .subscription(App::subscription)
        .theme(App::theme)
        .scale_factor(|state: &App| state.ui_scale as f32)
        .window(window_settings)
        .run()
}

fn init_logging() -> Option<tracing_appender::non_blocking::WorkerGuard> {
    if let Some(config_dir) = get_config_dir() {
        let _ = std::fs::create_dir_all(&config_dir);
        let file_appender = tracing_appender::rolling::never(&config_dir, "file-converter.log");
        let (non_blocking, guard) = tracing_appender::non_blocking(file_appender);

        let file_layer = tracing_subscriber::fmt::layer()
            .with_writer(non_blocking)
            .with_ansi(false);

        let stdout_layer = tracing_subscriber::fmt::layer();

        tracing_subscriber::registry()
            .with(file_layer)
            .with(stdout_layer)
            .init();

        Some(guard)
    } else {
        tracing_subscriber::fmt::init();
        None
    }
}

#[derive(Clone)]
struct EngineHandle(Arc<ConversionEngine>);

impl Hash for EngineHandle {
    fn hash<H: Hasher>(&self, state: &mut H) {
        state.write_u8(42);
    }
}
impl PartialEq for EngineHandle {
    fn eq(&self, _other: &Self) -> bool {
        true
    }
}
impl Eq for EngineHandle {}

#[derive(Clone)]
struct DownloadEngineHandle(Arc<DownloadEngine>);

impl Hash for DownloadEngineHandle {
    fn hash<H: Hasher>(&self, state: &mut H) {
        state.write_u8(43);
    }
}
impl PartialEq for DownloadEngineHandle {
    fn eq(&self, _other: &Self) -> bool {
        true
    }
}
impl Eq for DownloadEngineHandle {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MainTab {
    Converter,
    Downloader,
}

#[derive(Debug, Clone)]
pub struct JobCardItem {
    pub id: Option<JobId>,
    pub input_path: PathBuf,
    pub source_format: Option<MediaFormat>,
    pub target_format: MediaFormat,
    pub output_path: PathBuf,
    pub status: JobStatus,
    pub progress: Option<ConversionProgress>,
}

#[derive(Debug, Clone)]
pub struct DownloadCardItem {
    pub id: Option<DownloadId>,
    pub url: String,
    pub format: DownloadFormat,
    pub status: DownloadStatus,
    pub progress: Option<DownloadProgress>,
}

pub struct App {
    engine: Arc<ConversionEngine>,
    items: Vec<JobCardItem>,
    selected_target_format: MediaFormat,
    output_directory: PathBuf,
    tab: MainTab,
    settings_open: bool,
    config: AppConfig,
    /// UI scale factor applied globally via iced's scale_factor.
    /// Defaults to 1.0 on standard DPI; automatically adjusted
    /// so the interface is comfortably sized on any monitor.
    ui_scale: f64,
    /// The OS / monitor DPI scale factor reported by the window system
    /// (e.g. 1.0 at 100%, 1.5 at 150%, 2.0 at 200%). Used only when the
    /// user has not manually set `ui_scale` in their config.
    monitor_scale: Option<f64>,
    download_engine: Arc<DownloadEngine>,
    download_items: Vec<DownloadCardItem>,
    download_url: String,
    download_format: DownloadFormat,
    /// The id of the main window, captured on the first window event so we can
    /// apply maximize/minimize actions to it later.
    window_id: Option<iced::window::Id>,
    /// True when the configured startup mode is Minimized; applies the
    /// minimize action as soon as the window id becomes available.
    startup_minimize_pending: bool,
    /// Pending (not yet saved) copies of the settings. The settings screen
    /// edits these and only applies them when "Save" is pressed.
    draft_language: Language,
    draft_max_concurrent_jobs: usize,
    draft_default_target_format: MediaFormat,
    draft_ui_scale: Option<f64>,
    draft_window_mode: StartupWindowMode,
    /// State of the automatic updater, driven from the settings screen.
    update_state: UpdateState,
}

/// The current state of the built-in updater.
#[derive(Debug, Clone)]
pub enum UpdateState {
    /// No check was run yet in this session.
    Idle,
    /// A "check for updates" request is in flight.
    Checking,
    /// A newer release is available and can be installed.
    Available(updates::UpdateInfo),
    /// The latest release matches the running version.
    UpToDate,
    /// The new release is being downloaded and installed.
    Downloading,
    /// The last check or update attempt failed.
    Failed(String),
}

#[derive(Debug, Clone)]
pub enum Message {
    AddFilesClicked,
    FilesSelected(Vec<PathBuf>),
    SelectOutputDirClicked,
    OutputDirSelected(Option<PathBuf>),
    GlobalTargetFormatChanged(MediaFormat),
    StartAllJobs,
    StartSingleJob(usize),
    CancelJob(JobId),
    RemoveItem(usize),
    ClearCompleted,
    EngineEvent(JobEvent),
    ToggleSettings,
    TabSelected(MainTab),
    SettingsLanguageChanged(Language),
    SettingsConcurrencyChanged(usize),
    SettingsTargetFormatChanged(MediaFormat),
    SettingsUiScaleChanged(f64),
    SettingsUiScaleReset,
    SettingsWindowModeChanged(StartupWindowMode),
    SaveSettings,
    CheckForUpdates,
    UpdateCheckResult(Result<Option<updates::UpdateInfo>, String>),
    PerformUpdate,
    UpdateResult(Result<(), String>),
    WindowEvent(iced::window::Id, iced::window::Event),
    DownloadUrlChanged(String),
    DownloadFormatChanged(DownloadFormat),
    StartDownload,
    CancelDownload(DownloadId),
    RemoveDownload(usize),
    DownloadEvent(DownloadEvent),
    Redraw,
}

const CONCURRENCY_OPTIONS: [usize; 6] = [1, 2, 3, 4, 6, 8];

const UI_SCALE_MIN: f64 = 0.75;
const UI_SCALE_MAX: f64 = 1.75;
const UI_SCALE_STEP: f64 = 0.05;

/// Compute a reasonable UI scale factor.
///
/// Iced already handles HiDPI (e.g. 200% Windows scaling) via winit internally,
/// so `scale_factor` here is an *additional* multiplier on top of the OS DPI.
///
/// We target a comfortable base size; on a monitor running at 100% OS scaling
/// this is ~1.15. When the window system reports a larger native DPI scale
/// (150%, 200%, ...), we compensate so the *total* effective size stays at the
/// comfortable target, clamped to sane bounds.
fn compute_ui_scale(monitor_scale: Option<f64>) -> f64 {
    match monitor_scale {
        Some(os_scale) if os_scale > 0.0 => (1.15 / os_scale).clamp(0.85, 1.5),
        _ => 1.15,
    }
}

impl App {
    pub fn new(loaded_config: AppConfig) -> (Self, Task<Message>) {
        let ui_scale = loaded_config
            .ui_scale
            .unwrap_or_else(|| compute_ui_scale(None));

        let default_dir = loaded_config.output_directory.clone().unwrap_or_else(|| {
            UserDirs::new()
                .and_then(|dirs| {
                    dirs.video_dir()
                        .or_else(|| dirs.download_dir())
                        .map(|p| p.to_path_buf())
                })
                .unwrap_or_else(|| PathBuf::from("."))
        });

        let engine = Arc::new(ConversionEngine::new(EngineConfig {
            max_concurrent_jobs: loaded_config.max_concurrent_jobs,
            ffmpeg_path: converter_core::find_ffmpeg(),
        }));

        let download_engine = Arc::new(DownloadEngine::new(DownloadEngineConfig {
            ytdlp_path: converter_core::find_ytdlp(),
            max_concurrent_jobs: loaded_config.max_concurrent_jobs.max(1),
        }));

        let startup_minimize_pending =
            loaded_config.window_mode == StartupWindowMode::Minimized;

        (
            Self {
                engine,
                items: Vec::new(),
                selected_target_format: loaded_config.default_target_format,
                output_directory: default_dir,
                tab: MainTab::Converter,
                settings_open: false,
                config: loaded_config.clone(),
                ui_scale,
                monitor_scale: None,
                download_engine,
                download_items: Vec::new(),
                download_url: String::new(),
                download_format: DownloadFormat::Mp4,
                window_id: None,
                startup_minimize_pending,
                draft_language: loaded_config.language,
                draft_max_concurrent_jobs: loaded_config.max_concurrent_jobs,
                draft_default_target_format: loaded_config.default_target_format,
                draft_ui_scale: loaded_config.ui_scale,
                draft_window_mode: loaded_config.window_mode,
                update_state: UpdateState::Idle,
            },
            Task::none(),
        )
    }

    pub fn theme(&self) -> Theme {
        Theme::Dark
    }

    pub fn subscription(&self) -> Subscription<Message> {
        let engine_sub = Subscription::run_with(
            EngineHandle(Arc::clone(&self.engine)),
            |handle: &EngineHandle| {
                let engine = Arc::clone(&handle.0);
                iced::stream::channel(1024, move |mut output: iced::futures::channel::mpsc::Sender<Message>| async move {
                    use iced::futures::SinkExt;
                    let mut rx = engine.subscribe();
                    while let Ok(event) = rx.recv().await {
                        let _ = output.send(Message::EngineEvent(event)).await;
                    }
                })
            },
        );

        let window_sub = iced::window::events()
            .map(|(id, event)| Message::WindowEvent(id, event));

        let download_sub = Subscription::run_with(
            DownloadEngineHandle(Arc::clone(&self.download_engine)),
            |handle: &DownloadEngineHandle| {
                let engine = Arc::clone(&handle.0);
                iced::stream::channel(1024, move |mut output: iced::futures::channel::mpsc::Sender<Message>| async move {
                    use iced::futures::SinkExt;
                    let mut rx = engine.subscribe();
                    while let Ok(event) = rx.recv().await {
                        let _ = output.send(Message::DownloadEvent(event)).await;
                    }
                })
            },
        );

        Subscription::batch([engine_sub, window_sub, download_sub])
    }

    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::ToggleSettings => {
                self.settings_open = !self.settings_open;
                if self.settings_open {
                    // Load current values into the pending drafts
                    self.draft_language = self.config.language;
                    self.draft_max_concurrent_jobs = self.config.max_concurrent_jobs;
                    self.draft_default_target_format = self.config.default_target_format;
                    self.draft_ui_scale = self.config.ui_scale;
                    self.draft_window_mode = self.config.window_mode;
                }
                Task::none()
            }
            Message::TabSelected(tab) => {
                self.tab = tab;
                self.settings_open = false;
                Task::none()
            }
            Message::SettingsLanguageChanged(lang) => {
                self.draft_language = lang;
                Task::none()
            }
            Message::SettingsConcurrencyChanged(conc) => {
                self.draft_max_concurrent_jobs = conc;
                Task::none()
            }
            Message::SettingsTargetFormatChanged(format) => {
                self.draft_default_target_format = format;
                Task::none()
            }
            Message::SettingsUiScaleChanged(scale) => {
                self.draft_ui_scale = Some(scale.clamp(UI_SCALE_MIN, UI_SCALE_MAX));
                Task::none()
            }
            Message::SettingsUiScaleReset => {
                self.draft_ui_scale = None;
                Task::none()
            }
            Message::SettingsWindowModeChanged(mode) => {
                self.draft_window_mode = mode;
                Task::none()
            }
            Message::SaveSettings => {
                self.config.language = self.draft_language;
                self.config.max_concurrent_jobs = self.draft_max_concurrent_jobs;
                self.selected_target_format = self.draft_default_target_format;
                self.config.default_target_format = self.draft_default_target_format;
                self.config.ui_scale = self.draft_ui_scale;
                self.ui_scale = match self.draft_ui_scale {
                    Some(scale) => scale.clamp(UI_SCALE_MIN, UI_SCALE_MAX),
                    None => compute_ui_scale(self.monitor_scale),
                };
                self.config.window_mode = self.draft_window_mode;
                let _ = save_config(&self.config);

                for item in &mut self.items {
                    if item.id.is_none() {
                        item.target_format = self.config.default_target_format;
                        let stem = item
                            .input_path
                            .file_stem()
                            .and_then(|s| s.to_str())
                            .unwrap_or("converted");
                        item.output_path = self
                            .output_directory
                            .join(format!("{}.{}", stem, self.config.default_target_format.extension()));
                    }
                }

                self.settings_open = false;

                match (self.config.window_mode, self.window_id) {
                    (StartupWindowMode::Minimized, Some(id)) => {
                        iced::window::minimize(id, true)
                    }
                    (StartupWindowMode::Maximized, Some(id)) => {
                        iced::window::maximize(id, true)
                    }
                    (StartupWindowMode::Normal, Some(id)) => {
                        iced::window::maximize(id, false)
                    }
                    _ => Task::none(),
                }
            }
            Message::CheckForUpdates => {
                self.update_state = UpdateState::Checking;
                Task::perform(
                    async move {
                        tokio::task::spawn_blocking(updates::check_for_updates)
                            .await
                            .map_err(|e| e.to_string())?
                    },
                    Message::UpdateCheckResult,
                )
            }
            Message::UpdateCheckResult(Ok(Some(info))) => {
                if info.latest_version
                    <= semver::Version::parse(updates::CURRENT_VERSION)
                        .unwrap_or(semver::Version::new(0, 0, 0))
                {
                    self.update_state = UpdateState::UpToDate;
                } else {
                    self.update_state = UpdateState::Available(info);
                }
                Task::none()
            }
            Message::UpdateCheckResult(Ok(None)) => {
                self.update_state = UpdateState::UpToDate;
                Task::none()
            }
            Message::UpdateCheckResult(Err(err)) => {
                self.update_state = UpdateState::Failed(err);
                Task::none()
            }
            Message::PerformUpdate => {
                let info = match &self.update_state {
                    UpdateState::Available(info) => info.clone(),
                    _ => return Task::none(),
                };
                self.update_state = UpdateState::Downloading;
                Task::perform(
                    async move {
                        tokio::task::spawn_blocking(move || updates::apply_update(&info))
                            .await
                            .map_err(|e| e.to_string())?
                    },
                    Message::UpdateResult,
                )
            }
            Message::UpdateResult(Ok(())) => {
                // The new version has been relaunched (or its start is queued).
                self.update_state = UpdateState::Downloading;
                std::process::exit(0);
            }
            Message::UpdateResult(Err(err)) => {
                self.update_state = UpdateState::Failed(err);
                Task::none()
            }
            Message::WindowEvent(id, event) => {
                let mut task: Task<Message> = Task::none();

                if self.window_id.is_none() {
                    self.window_id = Some(id);
                    if self.startup_minimize_pending {
                        self.startup_minimize_pending = false;
                        if self.config.window_mode == StartupWindowMode::Minimized {
                            task = iced::window::minimize(id, true);
                        }
                    }
                }

                if let iced::window::Event::Rescaled(scale) = event {
                    let os_scale = scale as f64;
                    self.monitor_scale = Some(os_scale);
                    if self.config.ui_scale.is_none() {
                        self.ui_scale = compute_ui_scale(Some(os_scale));
                    }
                }

                task
            }
            Message::DownloadUrlChanged(url) => {
                self.download_url = url;
                Task::none()
            }
            Message::DownloadFormatChanged(format) => {
                self.download_format = format;
                Task::none()
            }
            Message::StartDownload => {
                let url = self.download_url.trim().to_string();
                if url.is_empty() {
                    return Task::none();
                }
                self.download_url.clear();
                self.download_items.push(DownloadCardItem {
                    id: None,
                    url: url.clone(),
                    format: self.download_format,
                    status: DownloadStatus::Queued,
                    progress: None,
                });
                let output_dir = self.output_directory.clone();
                let format = self.download_format;
                let engine = Arc::clone(&self.download_engine);
                Task::perform(
                    async move { engine.submit(url, output_dir, format).await },
                    |_| Message::Redraw,
                )
            }
            Message::CancelDownload(id) => {
                let engine = Arc::clone(&self.download_engine);
                Task::perform(
                    async move {
                        engine.cancel(id).await;
                    },
                    |_| Message::Redraw,
                )
            }
            Message::RemoveDownload(idx) => {
                if idx < self.download_items.len() {
                    if let Some(id) = self.download_items[idx].id {
                        let engine = Arc::clone(&self.download_engine);
                        tokio::spawn(async move {
                            engine.cancel(id).await;
                        });
                    }
                    self.download_items.remove(idx);
                }
                Task::none()
            }
            Message::Redraw => Task::none(),
            Message::DownloadEvent(event) => {
                match event {
                    DownloadEvent::Added(job) => {
                        for item in &mut self.download_items {
                            if item.id.is_none() && item.url == job.url {
                                item.id = Some(job.id);
                                item.status = job.status;
                                break;
                            }
                        }
                    }
                    DownloadEvent::StatusUpdated { id, status } => {
                        for item in &mut self.download_items {
                            if item.id == Some(id) {
                                item.status = status;
                                break;
                            }
                        }
                    }
                    DownloadEvent::ProgressUpdated { id, progress } => {
                        for item in &mut self.download_items {
                            if item.id == Some(id) {
                                item.status = DownloadStatus::Downloading(progress.clone());
                                item.progress = Some(progress);
                                break;
                            }
                        }
                    }
                    DownloadEvent::Completed { id, output_path } => {
                        for item in &mut self.download_items {
                            if item.id == Some(id) {
                                item.status = DownloadStatus::Completed { output_path };
                                break;
                            }
                        }
                    }
                    DownloadEvent::Failed { id, error } => {
                        for item in &mut self.download_items {
                            if item.id == Some(id) {
                                item.status = DownloadStatus::Failed(error);
                                break;
                            }
                        }
                    }
                    DownloadEvent::Cancelled { id } => {
                        for item in &mut self.download_items {
                            if item.id == Some(id) {
                                item.status = DownloadStatus::Cancelled;
                                break;
                            }
                        }
                    }
                }
                Task::none()
            }
            Message::AddFilesClicked => {
                Task::perform(
                    async {
                        let dialog = AsyncFileDialog::new().set_title("Select Media Files");
                        if let Some(handles) = dialog.pick_files().await {
                            handles.into_iter().map(|h| h.path().to_path_buf()).collect()
                        } else {
                            Vec::new()
                        }
                    },
                    Message::FilesSelected,
                )
            }
            Message::FilesSelected(paths) => {
                for path in paths {
                    let source_fmt = MediaFormat::from_path(&path);
                    let target_fmt = self.selected_target_format;
                    let stem = path
                        .file_stem()
                        .and_then(|s| s.to_str())
                        .unwrap_or("converted");
                    let output_path = self
                        .output_directory
                        .join(format!("{}.{}", stem, target_fmt.extension()));

                    self.items.push(JobCardItem {
                        id: None,
                        input_path: path,
                        source_format: source_fmt,
                        target_format: target_fmt,
                        output_path,
                        status: JobStatus::Queued,
                        progress: None,
                    });
                }
                Task::none()
            }
            Message::SelectOutputDirClicked => {
                let current = self.output_directory.clone();
                Task::perform(
                    async move {
                        let dialog = AsyncFileDialog::new()
                            .set_directory(&current)
                            .set_title("Select Output Folder");
                        dialog.pick_folder().await.map(|h| h.path().to_path_buf())
                    },
                    Message::OutputDirSelected,
                )
            }
            Message::OutputDirSelected(Some(dir)) => {
                self.output_directory = dir.clone();
                self.config.output_directory = Some(dir);
                let _ = save_config(&self.config);

                for item in &mut self.items {
                    if item.id.is_none() {
                        let stem = item
                            .input_path
                            .file_stem()
                            .and_then(|s| s.to_str())
                            .unwrap_or("converted");
                        item.output_path = self
                            .output_directory
                            .join(format!("{}.{}", stem, item.target_format.extension()));
                    }
                }
                Task::none()
            }
            Message::OutputDirSelected(None) => Task::none(),
            Message::GlobalTargetFormatChanged(format) => {
                self.selected_target_format = format;
                self.config.default_target_format = format;
                let _ = save_config(&self.config);

                for item in &mut self.items {
                    if item.id.is_none() {
                        item.target_format = format;
                        let stem = item
                            .input_path
                            .file_stem()
                            .and_then(|s| s.to_str())
                            .unwrap_or("converted");
                        item.output_path = self
                            .output_directory
                            .join(format!("{}.{}", stem, format.extension()));
                    }
                }
                Task::none()
            }
            Message::StartAllJobs => {
                let mut tasks = Vec::new();
                for item in &self.items {
                    if item.id.is_none() {
                        let job = ConversionJob::new(
                            item.input_path.clone(),
                            item.output_path.clone(),
                            item.target_format,
                        );
                        let engine = Arc::clone(&self.engine);
                        tasks.push(Task::perform(
                            async move {
                                engine.submit_job(job).await
                            },
                            |_| Message::ClearCompleted, // Redraw trigger
                        ));
                    }
                }
                Task::batch(tasks)
            }
            Message::StartSingleJob(idx) => {
                if let Some(item) = self.items.get(idx) {
                    if item.id.is_none() {
                        let job = ConversionJob::new(
                            item.input_path.clone(),
                            item.output_path.clone(),
                            item.target_format,
                        );
                        let engine = Arc::clone(&self.engine);
                        return Task::perform(
                            async move {
                                engine.submit_job(job).await
                            },
                            |_| Message::ClearCompleted,
                        );
                    }
                }
                Task::none()
            }
            Message::CancelJob(job_id) => {
                let engine = Arc::clone(&self.engine);
                Task::perform(
                    async move {
                        engine.cancel_job(job_id).await;
                    },
                    |_| Message::ClearCompleted,
                )
            }
            Message::RemoveItem(idx) => {
                if idx < self.items.len() {
                    if let Some(job_id) = self.items[idx].id {
                        let engine = Arc::clone(&self.engine);
                        tokio::spawn(async move {
                            engine.cancel_job(job_id).await;
                        });
                    }
                    self.items.remove(idx);
                }
                Task::none()
            }
            Message::ClearCompleted => {
                self.items.retain(|item| {
                    !matches!(
                        item.status,
                        JobStatus::Completed { .. } | JobStatus::Cancelled
                    )
                });
                Task::none()
            }
            Message::EngineEvent(event) => {
                match event {
                    JobEvent::Added(job) => {
                        for item in &mut self.items {
                            if item.id.is_none()
                                && (item.input_path == job.input_path || job.input_path.as_os_str().is_empty())
                            {
                                item.id = Some(job.id);
                                item.status = job.status;
                                break;
                            }
                        }
                    }
                    JobEvent::StatusUpdated { id, status } => {
                        for item in &mut self.items {
                            if item.id == Some(id) {
                                item.status = status;
                                break;
                            }
                        }
                    }
                    JobEvent::ProgressUpdated { id, progress } => {
                        for item in &mut self.items {
                            if item.id == Some(id) {
                                item.status = JobStatus::Converting(progress.clone());
                                item.progress = Some(progress);
                                break;
                            }
                        }
                    }
                    JobEvent::Completed { id, output_path } => {
                        for item in &mut self.items {
                            if item.id == Some(id) {
                                item.status = JobStatus::Completed {
                                    output_path,
                                    duration_seconds: 0.0,
                                };
                                break;
                            }
                        }
                    }
                    JobEvent::Failed { id, error } => {
                        for item in &mut self.items {
                            if item.id == Some(id) {
                                item.status = JobStatus::Failed(error);
                                break;
                            }
                        }
                    }
                    JobEvent::Cancelled { id } => {
                        for item in &mut self.items {
                            if item.id == Some(id) {
                                item.status = JobStatus::Cancelled;
                                break;
                            }
                        }
                    }
                }
                Task::none()
            }
        }
    }

    pub fn view(&self) -> Element<'_, Message> {
        let lang = self.config.language;

        let title_header = row![
            text(lang.app_title())
                .size(18)
                .color(Color::from_rgb(0.95, 0.95, 1.0)),
            Space::new().width(Length::Fill),
            text(lang.files_in_queue(self.items.len()))
                .size(12)
                .color(Color::from_rgb(0.65, 0.65, 0.75)),
            Space::new().width(Length::Fixed(10.0)),
            button(text(lang.settings_btn()).size(13))
                .on_press(Message::ToggleSettings)
                .padding([4, 8]),
        ]
        .align_y(Alignment::Center);

        if self.settings_open {
            return container(
                column![
                    title_header,
                    rule::horizontal(1),
                    self.view_settings(),
                ]
                .spacing(12)
                .padding(14),
            )
            .width(Length::Fill)
            .height(Length::Fill)
            .into();
        }

        let tab_bar = row![
            self.view_tab_button(MainTab::Converter, lang.tab_converter()),
            self.view_tab_button(MainTab::Downloader, lang.tab_downloader()),
        ]
        .spacing(8);

        let content = match self.tab {
            MainTab::Converter => self.view_converter(),
            MainTab::Downloader => self.view_downloader(),
        };

        let main_layout = column![
            title_header,
            rule::horizontal(1),
            tab_bar,
            rule::horizontal(1),
            content,
        ]
        .spacing(10)
        .padding(12);

        container(main_layout)
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    }

    fn view_tab_button<'a>(&self, tab: MainTab, label: &'a str) -> Element<'a, Message> {
        let mut btn = button(text(label).size(13));
        if self.tab == tab {
            btn = btn.style(iced::widget::button::primary);
        }
        btn.on_press(Message::TabSelected(tab)).padding([5, 14]).into()
    }

    fn view_converter(&self) -> Element<'_, Message> {
        let lang = self.config.language;

        // Compact Action Toolbar
        let add_btn = button(text(lang.add_files()).size(12))
            .on_press(Message::AddFilesClicked)
            .padding([5, 10]);

        let format_picker = row![
            text(lang.convert_to()).size(12),
            pick_list(
                MediaFormat::all(),
                Some(self.selected_target_format),
                Message::GlobalTargetFormatChanged
            )
            .text_size(12)
            .padding([4, 8]),
        ]
        .spacing(6)
        .align_y(Alignment::Center);

        let output_folder = row![
            text(lang.output_folder()).size(12),
            button(
                text(
                    self.output_directory
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or(self.output_directory.to_str().unwrap_or("/"))
                )
                .size(11)
            )
            .on_press(Message::SelectOutputDirClicked)
            .padding([4, 8]),
        ]
        .spacing(6)
        .align_y(Alignment::Center);

        let clear_btn = button(text(lang.clear_finished()).size(12))
            .on_press(Message::ClearCompleted)
            .padding([5, 10]);

        let start_all_btn = button(text(lang.convert_all()).size(12))
            .on_press(Message::StartAllJobs)
            .padding([5, 12]);

        let toolbar = row![
            add_btn,
            format_picker,
            output_folder,
            Space::new().width(Length::Fill),
            clear_btn,
            start_all_btn,
        ]
        .spacing(8)
        .align_y(Alignment::Center);

        // List of jobs
        let content: Element<'_, Message> = if self.items.is_empty() {
            container(
                column![
                    text(lang.no_files_title()).size(15),
                    text(lang.no_files_subtitle()).size(12).color(Color::from_rgb(0.6, 0.6, 0.7)),
                ]
                .spacing(6)
                .align_x(Alignment::Center),
            )
            .width(Length::Fill)
            .height(Length::Fill)
            .center_x(Length::Fill)
            .center_y(Length::Fill)
            .into()
        } else {
            let mut list_col = column![].spacing(8);

            for (idx, item) in self.items.iter().enumerate() {
                list_col = list_col.push(self.view_job_card(idx, item));
            }

            scrollable(list_col.padding([0, 4])).into()
        };

        column![
            toolbar,
            rule::horizontal(1),
            content,
        ]
        .spacing(10)
        .into()
    }

    fn view_downloader(&self) -> Element<'_, Message> {
        let lang = self.config.language;

        let url_input = text_input(lang.dl_url_placeholder(), &self.download_url)
            .on_input(Message::DownloadUrlChanged)
            .padding([6, 10])
            .width(Length::Fill);

        let format_row = row![
            text(lang.dl_format_label()).size(12),
            pick_list(
                DownloadFormat::ALL,
                Some(self.download_format),
                Message::DownloadFormatChanged
            )
            .text_size(12)
            .padding([4, 8]),
        ]
        .spacing(6)
        .align_y(Alignment::Center);

        let output_row = row![
            text(lang.output_folder()).size(12),
            button(
                text(
                    self.output_directory
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or(self.output_directory.to_str().unwrap_or("/"))
                )
                .size(11)
            )
            .on_press(Message::SelectOutputDirClicked)
            .padding([4, 8]),
        ]
        .spacing(6)
        .align_y(Alignment::Center);

        let download_btn = if self.download_url.trim().is_empty() {
            button(text(lang.dl_download_btn()).size(13)).padding([5, 12])
        } else {
            button(text(lang.dl_download_btn()).size(13))
                .on_press(Message::StartDownload)
                .padding([5, 12])
        };

        let toolbar = row![
            url_input,
            format_row,
            output_row,
            download_btn,
        ]
        .spacing(8)
        .align_y(Alignment::Center);

        // List of downloads
        let content: Element<'_, Message> = if self.download_items.is_empty() {
            container(
                column![
                    text(lang.dl_no_jobs_title()).size(15),
                    text(lang.dl_no_jobs_subtitle()).size(12).color(Color::from_rgb(0.6, 0.6, 0.7)),
                ]
                .spacing(6)
                .align_x(Alignment::Center),
            )
            .width(Length::Fill)
            .height(Length::Fill)
            .center_x(Length::Fill)
            .center_y(Length::Fill)
            .into()
        } else {
            let mut list_col = column![].spacing(8);

            for (idx, item) in self.download_items.iter().enumerate() {
                list_col = list_col.push(self.view_download_card(idx, item));
            }

            scrollable(list_col.padding([0, 4])).into()
        };

        column![
            text(lang.dl_title()).size(16),
            toolbar,
            rule::horizontal(1),
            content,
        ]
        .spacing(10)
        .into()
    }

    fn view_download_card<'a>(&self, idx: usize, item: &'a DownloadCardItem) -> Element<'a, Message> {
        let lang = self.config.language;

        let (pct, status_text, status_color) = match &item.status {
            DownloadStatus::Queued => (
                0.0,
                lang.dl_status_queued().to_string(),
                Color::from_rgb(0.6, 0.6, 0.6),
            ),
            DownloadStatus::Downloading(p) => {
                let speed_str = p.speed.clone().unwrap_or_default();
                let eta_str = p
                    .eta_seconds
                    .map(|e| lang.eta_label(e))
                    .unwrap_or_default();
                (
                    p.percentage.clamp(0.0, 100.0),
                    format!("{:.1}%  {}  {}", p.percentage, speed_str, eta_str),
                    Color::from_rgb(0.4, 0.8, 1.0),
                )
            }
            DownloadStatus::Completed { .. } => (
                100.0,
                lang.dl_status_completed().to_string(),
                Color::from_rgb(0.4, 0.9, 0.4),
            ),
            DownloadStatus::Failed(e) => (
                0.0,
                lang.dl_status_failed(e),
                Color::from_rgb(1.0, 0.4, 0.4),
            ),
            DownloadStatus::Cancelled => (
                0.0,
                lang.dl_status_cancelled().to_string(),
                Color::from_rgb(0.6, 0.6, 0.6),
            ),
        };

        let header_row = row![
            text(&item.url).size(13),
            Space::new().width(Length::Fixed(6.0)),
            text(format!("[{}]", item.format.label()))
                .size(11)
                .color(Color::from_rgb(0.4, 0.8, 1.0)),
            Space::new().width(Length::Fill),
            self.view_download_action_button(idx, item),
        ]
        .align_y(Alignment::Center);

        let progress_bar_widget = progress_bar(0.0..=100.0, pct as f32);

        let output_hint = match &item.status {
            DownloadStatus::Completed { output_path } => output_path.to_string_lossy().to_string(),
            _ => String::new(),
        };

        let status_row = row![
            text(status_text).size(11).color(status_color),
            Space::new().width(Length::Fill),
            text(output_hint)
                .size(10)
                .color(Color::from_rgb(0.5, 0.5, 0.5)),
        ]
        .align_y(Alignment::Center);

        let card_content = column![header_row, progress_bar_widget, status_row].spacing(4);

        container(card_content)
            .padding(8)
            .width(Length::Fill)
            .into()
    }

    fn view_download_action_button<'a>(&self, idx: usize, item: &'a DownloadCardItem) -> Element<'a, Message> {
        let lang = self.config.language;

        match &item.status {
            DownloadStatus::Queued => {
                if let Some(id) = item.id {
                    button(text(lang.btn_cancel()).size(11))
                        .on_press(Message::CancelDownload(id))
                        .padding([3, 8])
                        .into()
                } else {
                    button(text("✕").size(11))
                        .on_press(Message::RemoveDownload(idx))
                        .padding([3, 6])
                        .into()
                }
            }
            DownloadStatus::Downloading(_) => {
                if let Some(id) = item.id {
                    button(text(lang.btn_cancel()).size(11))
                        .on_press(Message::CancelDownload(id))
                        .padding([3, 8])
                        .into()
                } else {
                    Space::new().width(Length::Fixed(0.0)).into()
                }
            }
            DownloadStatus::Completed { .. } | DownloadStatus::Failed(_) | DownloadStatus::Cancelled => {
                button(text(lang.btn_remove()).size(11))
                    .on_press(Message::RemoveDownload(idx))
                    .padding([3, 8])
                    .into()
            }
        }
    }

    fn view_settings(&self) -> Element<'_, Message> {
        let lang = self.config.language;

        let lang_picker = row![
            text(lang.language_label()).size(13).width(Length::Fixed(180.0)),
            pick_list(
                Language::ALL,
                Some(self.draft_language),
                Message::SettingsLanguageChanged
            )
            .text_size(13)
            .padding([5, 10]),
        ]
        .spacing(10)
        .align_y(Alignment::Center);

        let concurrency_picker = row![
            text(lang.concurrency_label()).size(13).width(Length::Fixed(180.0)),
            pick_list(
                &CONCURRENCY_OPTIONS[..],
                Some(self.draft_max_concurrent_jobs),
                Message::SettingsConcurrencyChanged
            )
            .text_size(13)
            .padding([5, 10]),
        ]
        .spacing(10)
        .align_y(Alignment::Center);

        let format_picker = row![
            text(lang.default_format_label()).size(13).width(Length::Fixed(180.0)),
            pick_list(
                MediaFormat::all(),
                Some(self.draft_default_target_format),
                Message::SettingsTargetFormatChanged
            )
            .text_size(13)
            .padding([5, 10]),
        ]
        .spacing(10)
        .align_y(Alignment::Center);

        let is_manual_scale = self.draft_ui_scale.is_some();
        let draft_scale =
            self.draft_ui_scale.unwrap_or_else(|| compute_ui_scale(self.monitor_scale));

        let auto_scale_btn = if is_manual_scale {
            button(text(lang.ui_scale_auto_btn()).size(11))
                .on_press(Message::SettingsUiScaleReset)
                .padding([3, 8])
        } else {
            button(
                text(lang.ui_scale_auto_btn())
                    .size(11)
                    .color(Color::from_rgb(0.4, 0.8, 1.0)),
            )
            .padding([3, 8])
        };

        let scale_slider = row![
            text(lang.ui_scale_label()).size(13).width(Length::Fixed(180.0)),
            slider(
                UI_SCALE_MIN..=UI_SCALE_MAX,
                draft_scale,
                Message::SettingsUiScaleChanged
            )
            .step(UI_SCALE_STEP)
            .width(Length::Fixed(220.0)),
            text(format!("{:.0}%", draft_scale * 100.0))
                .size(13)
                .width(Length::Fixed(40.0)),
            auto_scale_btn,
        ]
        .spacing(10)
        .align_y(Alignment::Center);

        let window_mode_picker = row![
            text(lang.window_state_label()).size(13).width(Length::Fixed(180.0)),
            radio(
                lang.window_mode_name(StartupWindowMode::Normal),
                StartupWindowMode::Normal,
                Some(self.draft_window_mode),
                Message::SettingsWindowModeChanged
            )
            .text_size(13),
            radio(
                lang.window_mode_name(StartupWindowMode::Maximized),
                StartupWindowMode::Maximized,
                Some(self.draft_window_mode),
                Message::SettingsWindowModeChanged
            )
            .text_size(13),
            radio(
                lang.window_mode_name(StartupWindowMode::Minimized),
                StartupWindowMode::Minimized,
                Some(self.draft_window_mode),
                Message::SettingsWindowModeChanged
            )
            .text_size(13),
        ]
        .spacing(14)
        .align_y(Alignment::Center);

        let busy = matches!(
            self.update_state,
            UpdateState::Checking | UpdateState::Downloading
        );

        let version_row = row![
            text(lang.version_label()).size(13).width(Length::Fixed(180.0)),
            text(updates::CURRENT_VERSION).size(13),
        ]
        .spacing(10)
        .align_y(Alignment::Center);

        let check_btn = if busy {
            button(text(lang.checking_updates()).size(13)).padding([6, 16])
        } else {
            button(text(lang.check_updates_btn()).size(13))
                .on_press(Message::CheckForUpdates)
                .padding([6, 16])
        };

        let updates_row = row![
            text(lang.updates_label()).size(13).width(Length::Fixed(180.0)),
            check_btn,
        ]
        .spacing(10)
        .align_y(Alignment::Center);

        let update_status_row: Element<'_, Message> = match &self.update_state {
            UpdateState::Available(info) => row![
                text(lang.update_available(info.latest_version.to_string()))
                    .size(13),
                button(text(lang.update_btn()).size(13))
                    .on_press(Message::PerformUpdate)
                    .padding([3, 10]),
            ]
            .spacing(10)
            .align_y(Alignment::Center)
            .into(),
            UpdateState::Downloading => row![text(lang.updating()).size(13)].into(),
            UpdateState::UpToDate => row![text(lang.up_to_date()).size(13)].into(),
            UpdateState::Failed(err) => row![
                text(err.as_str())
                    .size(12)
                    .color(Color::from_rgb(1.0, 0.45, 0.45)),
            ]
            .into(),
            UpdateState::Idle | UpdateState::Checking => {
                row![Space::new().height(Length::Fixed(0.0))].into()
            }
        };

        let save_btn = button(text(lang.btn_save()).size(13))
            .on_press(Message::SaveSettings)
            .padding([6, 16]);
        let cancel_btn = button(text(lang.btn_cancel()).size(13))
            .on_press(Message::ToggleSettings)
            .padding([6, 16]);
        let settings_actions = row![save_btn, cancel_btn].spacing(10);

        let settings_card = column![
            text(lang.settings_title()).size(16),
            rule::horizontal(1),
            lang_picker,
            concurrency_picker,
            format_picker,
            scale_slider,
            window_mode_picker,
            rule::horizontal(1),
            version_row,
            updates_row,
            update_status_row,
            Space::new().height(Length::Fixed(12.0)),
            settings_actions,
        ]
        .spacing(14);

        container(settings_card)
            .padding(16)
            .width(Length::Fill)
            .into()
    }

    fn view_job_card<'a>(&self, idx: usize, item: &'a JobCardItem) -> Element<'a, Message> {
        let lang = self.config.language;

        let filename = item
            .input_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("Unknown file");

        let source_badge = item
            .source_format
            .map(|f| f.to_string())
            .unwrap_or_else(|| "?".to_string());

        let target_badge = item.target_format.to_string();

        let header_row = row![
            text(filename).size(13),
            Space::new().width(Length::Fixed(6.0)),
            text(format!("[{} → {}]", source_badge, target_badge))
                .size(11)
                .color(Color::from_rgb(0.4, 0.8, 1.0)),
            Space::new().width(Length::Fill),
            self.view_job_action_button(idx, item),
        ]
        .align_y(Alignment::Center);

        // Progress row
        let (pct, progress_text) = match &item.status {
            JobStatus::Queued => (0.0, lang.status_queued().to_string()),
            JobStatus::Probing => (0.0, lang.status_probing().to_string()),
            JobStatus::Converting(p) => {
                let speed_str = p.speed.map(|s| format!("{:.1}x", s)).unwrap_or_default();
                let eta_str = p
                    .eta_seconds
                    .map(|e| lang.eta_label(e))
                    .unwrap_or_default();
                (
                    p.percentage,
                    format!("{:.1}%  {}  {}", p.percentage, speed_str, eta_str),
                )
            }
            JobStatus::Completed { .. } => (100.0, lang.status_completed().to_string()),
            JobStatus::Failed(e) => (0.0, lang.status_failed(e)),
            JobStatus::Cancelled => (0.0, lang.status_cancelled().to_string()),
        };

        let progress_bar_widget = progress_bar(0.0..=100.0, pct);

        let status_row = row![
            text(progress_text).size(11).color(match &item.status {
                JobStatus::Completed { .. } => Color::from_rgb(0.4, 0.9, 0.4),
                JobStatus::Failed(_) => Color::from_rgb(1.0, 0.4, 0.4),
                JobStatus::Converting(_) => Color::from_rgb(0.4, 0.8, 1.0),
                _ => Color::from_rgb(0.6, 0.6, 0.6),
            }),
            Space::new().width(Length::Fill),
            text(item.output_path.to_string_lossy().to_string())
                .size(10)
                .color(Color::from_rgb(0.5, 0.5, 0.5)),
        ]
        .align_y(Alignment::Center);

        let card_content = column![header_row, progress_bar_widget, status_row].spacing(4);

        container(card_content)
            .padding(8)
            .width(Length::Fill)
            .into()
    }

    fn view_job_action_button<'a>(&self, idx: usize, item: &'a JobCardItem) -> Element<'a, Message> {
        let lang = self.config.language;

        match &item.status {
            JobStatus::Queued => {
                if item.id.is_none() {
                    row![
                        button(text(lang.btn_start()).size(11))
                            .on_press(Message::StartSingleJob(idx))
                            .padding([3, 8]),
                        button(text("✕").size(11))
                            .on_press(Message::RemoveItem(idx))
                            .padding([3, 6]),
                    ]
                    .spacing(4)
                    .into()
                } else {
                    button(text(lang.btn_cancel()).size(11))
                        .on_press(Message::CancelJob(item.id.unwrap()))
                        .padding([3, 8])
                        .into()
                }
            }
            JobStatus::Probing | JobStatus::Converting(_) => {
                if let Some(id) = item.id {
                    button(text(lang.btn_cancel()).size(11))
                        .on_press(Message::CancelJob(id))
                        .padding([3, 8])
                        .into()
                } else {
                    Space::new().width(Length::Fixed(0.0)).into()
                }
            }
            JobStatus::Completed { .. } | JobStatus::Failed(_) | JobStatus::Cancelled => {
                button(text(lang.btn_remove()).size(11))
                    .on_press(Message::RemoveItem(idx))
                    .padding([3, 8])
                    .into()
            }
        }
    }
}
