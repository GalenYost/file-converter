mod config;
mod i18n;

use std::hash::{Hash, Hasher};
use std::path::PathBuf;
use std::sync::Arc;

use directories::UserDirs;
use iced::widget::{
    button, column, container, pick_list, progress_bar, row, rule, scrollable, text,
    Space,
};
use iced::{
    Alignment, Color, Element, Length, Subscription, Task, Theme,
};
use rfd::AsyncFileDialog;

use converter_core::engine::{ConversionEngine, EngineConfig};
use converter_core::format::MediaFormat;
use converter_core::job::{ConversionJob, JobEvent, JobId, JobStatus};
use converter_core::progress::ConversionProgress;

use config::{load_config, save_config, AppConfig};
use i18n::Language;

fn main() -> iced::Result {
    tracing_subscriber::fmt::init();

    iced::application(App::new, App::update, App::view)
        .title("File converter")
        .subscription(App::subscription)
        .theme(App::theme)
        .run()
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ViewMode {
    Queue,
    Settings,
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

pub struct App {
    engine: Arc<ConversionEngine>,
    items: Vec<JobCardItem>,
    selected_target_format: MediaFormat,
    output_directory: PathBuf,
    view_mode: ViewMode,
    config: AppConfig,
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
    LanguageSelected(Language),
    ConcurrencyChanged(usize),
}

const CONCURRENCY_OPTIONS: [usize; 6] = [1, 2, 3, 4, 6, 8];

impl App {
    pub fn new() -> (Self, Task<Message>) {
        let loaded_config = load_config();

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

        (
            Self {
                engine,
                items: Vec::new(),
                selected_target_format: loaded_config.default_target_format,
                output_directory: default_dir,
                view_mode: ViewMode::Queue,
                config: loaded_config,
            },
            Task::none(),
        )
    }

    pub fn theme(&self) -> Theme {
        Theme::Dark
    }

    pub fn subscription(&self) -> Subscription<Message> {
        Subscription::run_with(
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
        )
    }

    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::ToggleSettings => {
                self.view_mode = match self.view_mode {
                    ViewMode::Queue => ViewMode::Settings,
                    ViewMode::Settings => ViewMode::Queue,
                };
                Task::none()
            }
            Message::LanguageSelected(lang) => {
                self.config.language = lang;
                let _ = save_config(&self.config);
                Task::none()
            }
            Message::ConcurrencyChanged(conc) => {
                self.config.max_concurrent_jobs = conc;
                let _ = save_config(&self.config);
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

        if self.view_mode == ViewMode::Settings {
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

        let main_layout = column![
            title_header,
            rule::horizontal(1),
            toolbar,
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

    fn view_settings(&self) -> Element<'_, Message> {
        let lang = self.config.language;

        let lang_picker = row![
            text(lang.language_label()).size(13).width(Length::Fixed(180.0)),
            pick_list(
                Language::ALL,
                Some(self.config.language),
                Message::LanguageSelected
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
                Some(self.config.max_concurrent_jobs),
                Message::ConcurrencyChanged
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
                Some(self.selected_target_format),
                Message::GlobalTargetFormatChanged
            )
            .text_size(13)
            .padding([5, 10]),
        ]
        .spacing(10)
        .align_y(Alignment::Center);

        let back_btn = button(text(lang.close_btn()).size(13))
            .on_press(Message::ToggleSettings)
            .padding([6, 16]);

        let settings_card = column![
            text(lang.settings_title()).size(16),
            rule::horizontal(1),
            lang_picker,
            concurrency_picker,
            format_picker,
            Space::new().height(Length::Fixed(12.0)),
            back_btn,
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
