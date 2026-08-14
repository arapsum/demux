use std::path::{Path, PathBuf};

use iced::font::Weight;
use iced::widget::{button, column, container, row, rule, scrollable, space, stack, text};
use iced::{Element, Fill, FillPortion, Font, Padding, Task};

use crate::{
    app::{
        intake::{AcceptedInput, IntakeResult, RejectedInput},
        queue_runner::{QueueRunSummary, QueueRunner},
    },
    ffmpeg::{RipPhase, RipProgressEvent, RipRequest, RipTermination},
    model::{
        encoding::RipOptions,
        job::{JobId, JobStatus, RipJob, RipProgress},
        media::MediaInfo,
        source::DestinationPolicy,
    },
};

use super::{
    TaskResult, drop_zone, icon,
    presentation::{JobPresentation, format_size},
    style::{
        BUTTON_TEXT, DANGER, DANGER_TEXT, ICON_MUTED, TEXT_MUTED, destructive_action, error_panel,
        panel, primary_action, queue_footer, queue_header, queue_row, queue_row_action,
        secondary_action, selected_queue_row,
    },
};

const DROP_BATCH_DELAY: std::time::Duration = std::time::Duration::from_millis(100);
const VISIBLE_JOB_BATCH: usize = 50;

#[derive(Debug, Clone)]
pub enum Message {
    AddFiles,
    AddFolder,
    PathsSelected(Vec<PathBuf>),
    PathsDropped(Vec<PathBuf>),
    DropHoverChanged(bool),
    DropBatchReady(u64),
    IntakeCompleted(IntakeResult),
    ProbeCompleted {
        job_id: JobId,
        result: TaskResult<MediaInfo>,
    },
    OutputResolved {
        job_id: JobId,
        result: TaskResult<PathBuf>,
    },
    ShowMore,
    DismissRejected,
    Select(JobId),
    Remove(JobId),
    StartQueue,
    ProgressReceived {
        job_id: JobId,
        progress: RipProgressEvent,
    },
    RipCompleted {
        job_id: JobId,
        result: TaskResult<RipTermination>,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Action {
    None,
    FilePickerOpened,
    IntakeRequested(Vec<PathBuf>),
    IntakeAccepted(Vec<AcceptedInput>),
    ProbeRequested(Vec<(JobId, PathBuf)>),
    ProbeFailed(String),
    ResolveOutput {
        job_id: JobId,
        requested: PathBuf,
    },
    RipRequested {
        job_id: JobId,
        request: Box<RipRequest>,
        initial_progress: RipProgress,
    },
    QueueFinished {
        summary: QueueRunSummary,
        error: Option<String>,
    },
}

#[derive(Debug)]
pub(crate) struct Queue {
    jobs: Vec<RipJob>,
    selected_job: Option<JobId>,
    picking_file: bool,
    discovering: bool,
    dropped_paths: Vec<PathBuf>,
    drop_hovered: bool,
    drop_batch: u64,
    rejected: Vec<RejectedInput>,
    visible_jobs: usize,
    next_job_id: u64,
    runner: Option<QueueRunner>,
    run_error: Option<String>,
}

impl Queue {
    pub(crate) fn new() -> Self {
        Self {
            jobs: Vec::new(),
            selected_job: None,
            picking_file: false,
            discovering: false,
            dropped_paths: Vec::new(),
            drop_hovered: false,
            drop_batch: 0,
            rejected: Vec::new(),
            visible_jobs: VISIBLE_JOB_BATCH,
            next_job_id: 1,
            runner: None,
            run_error: None,
        }
    }

    pub(crate) fn update(&mut self, message: Message) -> (Action, Task<Message>) {
        match message {
            Message::AddFiles => {
                self.picking_file = true;
                (
                    Action::FilePickerOpened,
                    Task::perform(pick_video_files(), Message::PathsSelected),
                )
            }
            Message::AddFolder => {
                self.picking_file = true;
                (
                    Action::FilePickerOpened,
                    Task::perform(pick_video_folder(), |path| {
                        Message::PathsSelected(path.into_iter().collect())
                    }),
                )
            }
            Message::PathsSelected(paths) => {
                self.picking_file = false;
                let action = if paths.is_empty() || self.is_running() {
                    Action::None
                } else if self.discovering {
                    self.dropped_paths.extend(paths);
                    self.drop_batch += 1;
                    let batch = self.drop_batch;
                    return (
                        Action::None,
                        Task::perform(wait_for_drop_batch(batch), Message::DropBatchReady),
                    );
                } else {
                    self.discovering = true;
                    Action::IntakeRequested(paths)
                };
                (action, Task::none())
            }
            Message::PathsDropped(paths) => {
                self.drop_hovered = false;
                if self.is_running() {
                    return (Action::None, Task::none());
                }
                self.dropped_paths.extend(paths);
                self.drop_batch += 1;
                let batch = self.drop_batch;
                (
                    Action::None,
                    Task::perform(wait_for_drop_batch(batch), Message::DropBatchReady),
                )
            }
            Message::DropHoverChanged(hovered) => {
                self.drop_hovered = hovered;
                (Action::None, Task::none())
            }
            Message::DropBatchReady(batch) => {
                if batch != self.drop_batch || self.dropped_paths.is_empty() {
                    return (Action::None, Task::none());
                }
                if self.discovering {
                    return (
                        Action::None,
                        Task::perform(wait_for_drop_batch(batch), Message::DropBatchReady),
                    );
                }
                self.discovering = true;
                (
                    Action::IntakeRequested(std::mem::take(&mut self.dropped_paths)),
                    Task::none(),
                )
            }
            Message::IntakeCompleted(result) => {
                self.discovering = false;
                self.rejected.extend(result.rejected);
                (Action::IntakeAccepted(result.accepted), Task::none())
            }
            Message::ProbeCompleted { job_id, result } => {
                let Some(job) = self.job_mut(&job_id) else {
                    return (Action::None, Task::none());
                };
                if !matches!(job.status, JobStatus::Probing) {
                    return (Action::None, Task::none());
                }
                match result {
                    Ok(metadata) => {
                        job.record_metadata(metadata);
                        (Action::None, Task::none())
                    }
                    Err(error) => {
                        tracing::warn!(job_id = job_id.0, error = %error, "media probe failed");
                        let message = error.to_string();
                        job.fail(message.clone());
                        (Action::ProbeFailed(message), Task::none())
                    }
                }
            }
            Message::ShowMore => {
                self.visible_jobs = self.visible_jobs.saturating_add(VISIBLE_JOB_BATCH);
                (Action::None, Task::none())
            }
            Message::DismissRejected => {
                self.rejected.clear();
                (Action::None, Task::none())
            }
            Message::Select(job_id) => {
                if self.jobs.iter().any(|job| job.id == job_id) {
                    self.selected_job = Some(job_id);
                }
                (Action::None, Task::none())
            }
            Message::Remove(job_id) => {
                if self.is_running() {
                    return (Action::None, Task::none());
                }
                let removable = self
                    .jobs
                    .iter()
                    .find(|job| job.id == job_id)
                    .is_some_and(|job| !matches!(job.status, JobStatus::Ripping));
                if removable {
                    self.jobs.retain(|job| job.id != job_id);
                    if self.selected_job.as_ref() == Some(&job_id) {
                        self.selected_job = self.jobs.first().map(|job| job.id.clone());
                    }
                }
                (Action::None, Task::none())
            }
            Message::StartQueue => {
                let action = self.start_queue();
                (action, Task::none())
            }
            Message::OutputResolved { job_id, result } => {
                let action = self.output_resolved(&job_id, result);
                (action, Task::none())
            }
            Message::ProgressReceived { job_id, progress } => {
                self.record_progress(&job_id, progress);
                (Action::None, Task::none())
            }
            Message::RipCompleted { job_id, result } => {
                let action = self.finish_active(&job_id, result);
                (action, Task::none())
            }
        }
    }

    pub(crate) fn enqueue_many(
        &mut self,
        paths: Vec<(AcceptedInput, PathBuf)>,
        options: RipOptions,
        destination_policy: DestinationPolicy,
    ) -> Action {
        let mut probes = Vec::with_capacity(paths.len());
        for (input, output) in paths {
            if self
                .jobs
                .iter()
                .any(|job| Path::new(&job.input) == input.path)
            {
                self.rejected.push(RejectedInput {
                    path: input.path,
                    reason: "This file is already in the queue".into(),
                });
                continue;
            }
            let job_id = JobId::new(self.next_job_id);
            self.next_job_id += 1;
            let mut job = RipJob::with_options_and_destination(
                job_id.clone(),
                input.path.to_string_lossy().into_owned(),
                output.to_string_lossy().into_owned(),
                options,
                destination_policy,
                input.hierarchy,
            );
            job.input_size = Some(input.size);
            job.start_probing();
            probes.push((job_id.clone(), input.path));
            self.jobs.push(job);
            self.selected_job.get_or_insert(job_id);
        }
        Action::ProbeRequested(probes)
    }

    pub(crate) fn canonical_paths(&self) -> Vec<PathBuf> {
        self.jobs
            .iter()
            .map(|job| PathBuf::from(&job.input))
            .collect()
    }

    pub(crate) fn selected(&self) -> Option<&RipJob> {
        let selected = self.selected_job.as_ref()?;
        self.jobs.iter().find(|job| &job.id == selected)
    }

    fn job_mut(&mut self, id: &JobId) -> Option<&mut RipJob> {
        self.jobs.iter_mut().find(|job| &job.id == id)
    }

    pub(crate) fn selected_status(&self) -> Option<&JobStatus> {
        self.selected().map(|job| &job.status)
    }

    pub(crate) fn has_folder_hierarchy(&self) -> bool {
        self.jobs.iter().any(|job| job.source_hierarchy.is_some())
    }

    pub(crate) fn set_output_paths(&mut self, mut derive: impl FnMut(&RipJob) -> PathBuf) {
        for job in &mut self.jobs {
            if !matches!(
                job.status,
                JobStatus::Queued
                    | JobStatus::Analyzing
                    | JobStatus::Ripping
                    | JobStatus::Completed
            ) {
                job.output = derive(job).to_string_lossy().into_owned();
            }
        }
    }

    pub(crate) fn set_options(&mut self, options: RipOptions) {
        for job in &mut self.jobs {
            if matches!(
                job.status,
                JobStatus::Pending | JobStatus::Probing | JobStatus::Ready
            ) {
                job.set_options(options);
            }
        }
    }

    pub(crate) fn set_destination_policy(&mut self, policy: DestinationPolicy) {
        for job in &mut self.jobs {
            if matches!(
                job.status,
                JobStatus::Pending | JobStatus::Probing | JobStatus::Ready
            ) {
                job.destination_policy = policy;
            }
        }
    }

    fn start_queue(&mut self) -> Action {
        if !self.can_start() {
            return Action::None;
        }

        let eligible: Vec<_> = self
            .jobs
            .iter()
            .filter(|job| matches!(job.status, JobStatus::Ready))
            .map(|job| job.id.clone())
            .collect();
        let skipped = self
            .jobs
            .iter()
            .filter(|job| matches!(job.status, JobStatus::Failed(_)))
            .count();

        for job in &mut self.jobs {
            match &job.status {
                JobStatus::Ready => job.queue(),
                JobStatus::Failed(message) => job.skip(message.clone()),
                _ => {}
            }
        }

        self.runner = QueueRunner::new(eligible, skipped);
        self.run_error = None;
        self.advance_runner()
    }

    fn advance_runner(&mut self) -> Action {
        let Some(runner) = &mut self.runner else {
            return Action::None;
        };
        if let Some(job_id) = runner.start_next() {
            let Some(job) = self.jobs.iter_mut().find(|job| job.id == job_id) else {
                return Action::None;
            };
            if !matches!(job.status, JobStatus::Queued) {
                return Action::None;
            }
            self.selected_job = Some(job_id.clone());
            if job.options.normalize_audio {
                job.start_analyzing();
            } else {
                job.start_ripping();
            }
            return Action::ResolveOutput {
                job_id,
                requested: PathBuf::from(&job.output),
            };
        }

        if runner.is_finished() {
            let summary = runner.summary();
            self.runner = None;
            Action::QueueFinished {
                summary,
                error: self.run_error.take(),
            }
        } else {
            Action::None
        }
    }

    fn output_resolved(&mut self, job_id: &JobId, result: TaskResult<PathBuf>) -> Action {
        if self.runner.as_ref().and_then(QueueRunner::active) != Some(job_id) {
            return Action::None;
        }
        if self.runner.as_ref().is_some_and(QueueRunner::is_cancelling) {
            if let Some(job) = self.job_mut(job_id) {
                job.cancel();
            }
            if let Some(runner) = &mut self.runner {
                runner.finish_cancelled(job_id);
            }
            return self.advance_runner();
        }
        let Some(job) = self.job_mut(job_id) else {
            return Action::None;
        };
        if !matches!(job.status, JobStatus::Analyzing | JobStatus::Ripping) {
            return Action::None;
        }

        match result {
            Ok(output) => {
                job.output = output.to_string_lossy().into_owned();
                Action::RipRequested {
                    job_id: job_id.clone(),
                    request: Box::new(RipRequest::with_options_and_metadata(
                        job.input.clone(),
                        output,
                        job.options,
                        job.metadata.clone(),
                    )),
                    initial_progress: job.progress.clone(),
                }
            }
            Err(error) => {
                tracing::error!(job_id = job_id.0, error = %error, "output resolution failed");
                job.fail(error.to_string());
                if let Some(runner) = &mut self.runner {
                    runner.finish_active(job_id, false);
                }
                self.advance_runner()
            }
        }
    }

    fn finish_active(&mut self, job_id: &JobId, result: TaskResult<RipTermination>) -> Action {
        if self.runner.as_ref().and_then(QueueRunner::active) != Some(job_id) {
            return Action::None;
        }
        let Some(job) = self.job_mut(job_id) else {
            return Action::None;
        };
        if !matches!(
            job.status,
            JobStatus::Analyzing | JobStatus::Ripping | JobStatus::Cancelling
        ) {
            return Action::None;
        }

        enum Finish {
            Completed,
            Cancelled,
            Failed,
        }

        let finish = match result {
            Ok(RipTermination::Completed(_)) => {
                job.complete();
                Finish::Completed
            }
            Ok(RipTermination::Cancelled { .. }) => {
                job.cancel();
                Finish::Cancelled
            }
            Err(error) => {
                tracing::error!(job_id = job_id.0, error = %error, "audio extraction failed");
                let message = error.to_string();
                job.fail(message.clone());
                if matches!(&*error, crate::Error::PartialOutputCleanup { .. }) {
                    self.run_error = Some(message);
                }
                Finish::Failed
            }
        };
        if let Some(runner) = &mut self.runner {
            match finish {
                Finish::Completed => {
                    runner.finish_active(job_id, true);
                }
                Finish::Cancelled => {
                    runner.finish_cancelled(job_id);
                }
                Finish::Failed => {
                    runner.finish_active(job_id, false);
                }
            }
        }
        self.advance_runner()
    }

    pub(crate) fn request_cancel(&mut self, job_id: &JobId) -> Action {
        let Some(pending) = self
            .runner
            .as_mut()
            .and_then(|runner| runner.request_cancel(job_id))
        else {
            return Action::None;
        };

        if let Some(active) = self.job_mut(job_id) {
            active.start_cancelling();
        }
        for pending_id in pending {
            if let Some(job) = self.job_mut(&pending_id) {
                job.cancel();
            }
        }

        Action::None
    }

    fn record_progress(&mut self, job_id: &JobId, event: RipProgressEvent) {
        if !self.is_active_job(job_id) {
            return;
        }
        let Some(job) = self.job_mut(job_id) else {
            return;
        };
        match event.phase {
            RipPhase::Analyzing if !matches!(job.status, JobStatus::Analyzing) => {
                job.start_analyzing();
            }
            RipPhase::Encoding if !matches!(job.status, JobStatus::Ripping) => {
                job.reset_for_encoding();
            }
            _ => {}
        }

        job.record_progress(
            event.progress.elapsed,
            event.progress.speed,
            event.progress.bitrate_kbps,
            event.progress.output_size,
        );
    }

    pub(crate) fn can_start(&self) -> bool {
        !self.is_running()
            && !self.discovering
            && !self.picking_file
            && !self
                .jobs
                .iter()
                .any(|job| matches!(job.status, JobStatus::Pending | JobStatus::Probing))
            && self
                .jobs
                .iter()
                .any(|job| matches!(job.status, JobStatus::Ready))
    }

    pub(crate) fn is_busy(&self) -> bool {
        self.discovering
            || self.is_running()
            || self
                .jobs
                .iter()
                .any(|job| matches!(job.status, JobStatus::Pending | JobStatus::Probing))
    }

    pub(crate) fn is_running(&self) -> bool {
        self.runner.is_some()
    }

    pub(crate) fn is_active_job(&self, job_id: &JobId) -> bool {
        self.runner.as_ref().and_then(QueueRunner::active) == Some(job_id)
    }

    pub(crate) fn active_job_id(&self) -> Option<JobId> {
        self.runner.as_ref()?.active().cloned()
    }

    pub(crate) fn run_progress(&self) -> Option<(usize, usize)> {
        let runner = self.runner.as_ref()?;
        Some((runner.position()?, runner.total()))
    }

    pub(crate) fn view<'a>(&'a self, error: Option<&'a str>) -> Element<'a, Message> {
        let choose_copy = if self.drop_hovered {
            "Drop to add these videos"
        } else if self.picking_file {
            "Waiting for file selection…"
        } else if self.discovering {
            "Discovering supported media…"
        } else if self.is_running() {
            "Queue running — intake will reopen when it finishes"
        } else if self.is_busy() {
            "Demux is inspecting your videos"
        } else if self.jobs.is_empty() {
            "Add videos or drop a folder to begin"
        } else {
            "Add more videos"
        };

        let intake_enabled = !self.picking_file && !self.discovering && !self.is_running();

        let add_button = button(
            row![
                icon::add_files(if intake_enabled {
                    iced::Color::WHITE
                } else {
                    ICON_MUTED
                }),
                text("Add Files").size(14)
            ]
            .spacing(8)
            .align_y(iced::Alignment::Center),
        )
        .padding(Padding::from([10, 16]))
        .style(primary_action)
        .on_press_maybe(intake_enabled.then_some(Message::AddFiles));
        let folder_button = button(
            row![
                icon::add_folder(if intake_enabled {
                    BUTTON_TEXT
                } else {
                    ICON_MUTED
                }),
                text("Add Folder").size(14)
            ]
            .spacing(8)
            .align_y(iced::Alignment::Center),
        )
        .padding(Padding::from([10, 16]))
        .style(secondary_action)
        .on_press_maybe(intake_enabled.then_some(Message::AddFolder));

        let remove_message = self
            .selected()
            .filter(|_| !self.is_running())
            .map(|job| Message::Remove(job.id.clone()));
        let remove_enabled = remove_message.is_some();
        let remove_button = button(
            row![
                icon::remove(if remove_enabled {
                    DANGER_TEXT
                } else {
                    ICON_MUTED
                }),
                text("Remove").size(14)
            ]
            .spacing(8)
            .align_y(iced::Alignment::Center),
        )
        .padding(Padding::from([10, 16]))
        .style(destructive_action)
        .on_press_maybe(remove_message);

        let chooser_copy = container(
            column![
                icon::media_file(if self.drop_hovered {
                    super::style::ACCENT
                } else {
                    ICON_MUTED
                }),
                text(choose_copy).size(17).font(Font {
                    weight: Weight::Semibold,
                    ..Font::default()
                }),
                text("Supports MP4, MKV, MOV, AVI, WMV, FLV, MPEG, and more")
                    .size(13)
                    .color(TEXT_MUTED),
            ]
            .spacing(10)
            .align_x(iced::Alignment::Center),
        )
        .width(Fill)
        .height(Fill)
        .center_x(Fill)
        .center_y(Fill);

        let chooser = container(stack![drop_zone::chrome(self.drop_hovered), chooser_copy,])
            .width(Fill)
            .height(150);

        let intake = container(
            column![
                chooser,
                row![add_button, folder_button, remove_button].spacing(8)
            ]
            .spacing(10),
        )
        .width(Fill)
        .padding(10)
        .style(panel);

        let mut content = column![intake, self.queue_panel()].spacing(10);
        if !self.rejected.is_empty() {
            let hidden = self.rejected.len().saturating_sub(3);
            let rejected = self.rejected.iter().take(3).fold(
                column![
                    row![
                        text("Some items were not added").font(Font {
                            weight: Weight::Semibold,
                            ..Font::default()
                        }),
                        space::horizontal(),
                        button(text("Dismiss").size(12))
                            .padding(Padding::from([6, 10]))
                            .on_press(Message::DismissRejected),
                    ]
                    .align_y(iced::Alignment::Center)
                ]
                .spacing(4),
                |column, item| {
                    column.push(
                        text(format!("{} — {}", item.path.display(), item.reason))
                            .size(12)
                            .color(DANGER_TEXT),
                    )
                },
            );
            let rejected = if hidden > 0 {
                rejected.push(
                    text(format!(
                        "and {hidden} more item{}",
                        if hidden == 1 { "" } else { "s" }
                    ))
                    .size(12)
                    .color(DANGER_TEXT),
                )
            } else {
                rejected
            };
            content = content.push(
                container(rejected)
                    .width(Fill)
                    .padding(14)
                    .style(error_panel),
            );
        }
        if let Some(error) = error {
            content = content.push(
                container(
                    column![
                        text("The operation could not be completed")
                            .font(Font {
                                weight: Weight::Semibold,
                                ..Font::default()
                            })
                            .color(DANGER),
                        text(error).size(13).color(DANGER_TEXT),
                    ]
                    .spacing(5),
                )
                .width(Fill)
                .padding(14)
                .style(error_panel),
            );
        }

        container(content).width(Fill).height(Fill).into()
    }

    fn queue_panel(&self) -> Element<'_, Message> {
        let header = container(
            row![
                text("#").size(12).color(TEXT_MUTED).width(34),
                text("File Name")
                    .size(12)
                    .color(TEXT_MUTED)
                    .width(FillPortion(7)),
                text("Duration")
                    .size(12)
                    .color(TEXT_MUTED)
                    .width(FillPortion(2)),
                text("Status")
                    .size(12)
                    .color(TEXT_MUTED)
                    .width(FillPortion(3)),
                text("Output Format")
                    .size(12)
                    .color(TEXT_MUTED)
                    .width(FillPortion(3)),
                text("Size")
                    .size(12)
                    .color(TEXT_MUTED)
                    .width(FillPortion(2)),
            ]
            .align_y(iced::Alignment::Center),
        )
        .width(Fill)
        .padding(Padding::from([10, 12]))
        .style(queue_header);

        let queue: Element<'_, Message> = if self.jobs.is_empty() {
            container(
                column![
                    text("Your queue is empty")
                        .size(17)
                        .font(Font {
                            weight: Weight::Semibold,
                            ..Font::default()
                        }),
                    text("Add videos above or drop files and folders into Demux. Each supported file will be inspected before extraction.")
                        .size(14)
                        .color(TEXT_MUTED),
                ]
                .spacing(7)
                .max_width(520),
            )
            .height(180)
            .padding(22)
            .center_y(180)
            .into()
        } else {
            let mut jobs = self.jobs.iter().take(self.visible_jobs).enumerate().fold(
                column![],
                |column, (index, job)| {
                    let run_progress = self.runner.as_ref().and_then(|runner| {
                        (runner.active() == Some(&job.id)
                            && matches!(job.status, JobStatus::Analyzing | JobStatus::Ripping))
                        .then(|| (runner.position().unwrap_or(1), runner.total()))
                    });
                    column.push(job_row(
                        job,
                        index + 1,
                        self.selected_job.as_ref() == Some(&job.id),
                        run_progress,
                    ))
                },
            );
            if self.visible_jobs < self.jobs.len() {
                jobs = jobs.push(
                    button(text(format!(
                        "Show {} more",
                        (self.jobs.len() - self.visible_jobs).min(VISIBLE_JOB_BATCH)
                    )))
                    .width(Fill)
                    .padding(12)
                    .on_press(Message::ShowMore),
                );
            }
            jobs.into()
        };

        let source_bytes = self
            .jobs
            .iter()
            .filter_map(|job| job.input_size)
            .fold(0_u64, u64::saturating_add);
        let footer = container(
            row![
                text(format!(
                    "{} file{} in queue",
                    self.jobs.len(),
                    if self.jobs.len() == 1 { "" } else { "s" }
                ))
                .size(12)
                .color(TEXT_MUTED),
                space::horizontal(),
                text(if self.jobs.is_empty() {
                    String::new()
                } else {
                    format!("Total source size: {}", format_size(source_bytes))
                })
                .size(12)
                .color(TEXT_MUTED),
            ]
            .align_y(iced::Alignment::Center),
        )
        .width(Fill)
        .padding(Padding::from([8, 12]))
        .style(queue_footer);

        container(column![
            header,
            rule::horizontal(1),
            scrollable(queue),
            rule::horizontal(1),
            footer
        ])
        .width(Fill)
        .height(Fill)
        .style(panel)
        .into()
    }
}

fn job_row(
    job: &RipJob,
    index: usize,
    selected: bool,
    run_progress: Option<(usize, usize)>,
) -> Element<'_, Message> {
    let duration_known = !job.progress.duration.is_zero();
    let percent = job.progress.percent;
    let analyzing = matches!(job.status, JobStatus::Analyzing);
    let job = JobPresentation::from(job);
    let id = job.id.clone();
    let status_label = run_progress.map_or_else(
        || job.status.label.to_owned(),
        |(position, total)| {
            let phase = if analyzing {
                "Analyzing loudness"
            } else {
                "Ripping audio"
            };
            if !duration_known {
                format!("{phase} ({position} of {total})")
            } else {
                format!("{phase} ({position} of {total}) · {percent:.0}%")
            }
        },
    );

    let filename: Element<'_, Message> = if let Some(detail) = job.terminal_detail {
        column![
            text(job.filename).size(13).font(Font {
                weight: Weight::Semibold,
                ..Font::default()
            }),
            text(detail).size(11).color(DANGER_TEXT),
        ]
        .spacing(3)
        .into()
    } else {
        text(job.filename)
            .size(13)
            .font(Font {
                weight: Weight::Semibold,
                ..Font::default()
            })
            .into()
    };

    let row = container(
        row![
            text(index).size(12).color(TEXT_MUTED).width(34),
            row![icon::queue_media(ICON_MUTED), filename]
                .spacing(8)
                .align_y(iced::Alignment::Center)
                .width(FillPortion(7)),
            text(job.duration).size(12).width(FillPortion(2)),
            text(status_label)
                .size(12)
                .font(Font {
                    weight: Weight::Semibold,
                    ..Font::default()
                })
                .color(job.status.color)
                .width(FillPortion(3)),
            text(job.output_details).size(12).width(FillPortion(3)),
            text(job.size).size(12).width(FillPortion(2)),
        ]
        .align_y(iced::Alignment::Center),
    )
    .width(Fill)
    .padding(Padding::from([10, 12]))
    .style(if selected {
        selected_queue_row
    } else {
        queue_row
    });

    column![
        button(row)
            .padding(0)
            .width(Fill)
            .style(queue_row_action)
            .on_press(Message::Select(id)),
        rule::horizontal(1),
    ]
    .into()
}

async fn wait_for_drop_batch(batch: u64) -> u64 {
    tokio::time::sleep(DROP_BATCH_DELAY).await;
    batch
}

async fn pick_video_files() -> Vec<PathBuf> {
    rfd::AsyncFileDialog::new()
        .set_title("Choose a video to rip")
        .add_filter(
            "Video files",
            &["mp4", "mkv", "mov", "avi", "wmv", "flv", "mpeg", "mpg"],
        )
        .pick_files()
        .await
        .unwrap_or_default()
        .into_iter()
        .map(|file| file.path().to_path_buf())
        .collect()
}

async fn pick_video_folder() -> Option<PathBuf> {
    rfd::AsyncFileDialog::new()
        .set_title("Choose a folder of videos")
        .pick_folder()
        .await
        .map(|folder| folder.path().to_path_buf())
}

#[cfg(test)]
mod tests {
    use std::{sync::Arc, time::Duration};

    use crate::ffmpeg::{ChannelMode, FfmpegProgress, Mp3Bitrate, RipOutcome, SampleRate};
    use crate::model::media::AudioMetadata;

    use super::*;

    fn task_error(message: &str) -> Arc<crate::Error> {
        Arc::new(std::io::Error::other(message).into())
    }

    fn metadata() -> MediaInfo {
        MediaInfo {
            duration: Duration::from_secs(75),
            container: "mp4".into(),
            bitrate: None,
            creation_time: None,
            tags: Default::default(),
            artwork: None,
            audio: AudioMetadata {
                stream_index: 1,
                codec: "aac".into(),
                sample_rate: Some(48_000),
                channels: Some(2),
                channel_layout: Some("stereo".into()),
                bitrate: None,
                language: None,
            },
        }
    }

    fn enqueue(queue: &mut Queue, name: &str) -> JobId {
        let input = PathBuf::from(format!("/videos/{name}.mp4"));
        let output = PathBuf::from(format!("/music/{name}.mp3"));
        let _ = queue.enqueue_many(
            vec![(
                AcceptedInput {
                    path: input,
                    size: 42,
                    hierarchy: None,
                },
                output,
            )],
            RipOptions::default(),
            DestinationPolicy::default(),
        );
        queue.jobs.last().unwrap().id.clone()
    }

    #[test]
    fn out_of_order_probe_results_do_not_reorder_jobs() {
        let mut queue = Queue::new();
        let first = enqueue(&mut queue, "first");
        let second = enqueue(&mut queue, "second");

        let _ = queue.update(Message::ProbeCompleted {
            job_id: second.clone(),
            result: Ok(metadata()),
        });
        let (action, _) = queue.update(Message::ProbeCompleted {
            job_id: first.clone(),
            result: Ok(metadata()),
        });

        assert_eq!(action, Action::None);
        assert_eq!(queue.jobs[0].id, first);
        assert_eq!(queue.jobs[1].id, second);
        assert!(
            queue
                .jobs
                .iter()
                .all(|job| matches!(job.status, JobStatus::Ready))
        );
    }

    #[test]
    fn add_file_reports_that_the_picker_was_opened() {
        let mut queue = Queue::new();

        let (action, _) = queue.update(Message::AddFiles);

        assert_eq!(action, Action::FilePickerOpened);
        assert!(queue.picking_file);
    }

    #[test]
    fn probe_failure_is_reported_as_a_surface_action() {
        let mut queue = Queue::new();
        let job_id = enqueue(&mut queue, "silent");

        let (action, _) = queue.update(Message::ProbeCompleted {
            job_id,
            result: Err(task_error("No audio stream was found")),
        });

        assert_eq!(
            action,
            Action::ProbeFailed("No audio stream was found".into())
        );
        assert!(matches!(
            queue.selected_status(),
            Some(JobStatus::Failed(_))
        ));
    }

    #[test]
    fn ignores_a_duplicate_probe_result_after_readiness() {
        let mut queue = Queue::new();
        let job_id = enqueue(&mut queue, "example");
        let _ = queue.update(Message::ProbeCompleted {
            job_id: job_id.clone(),
            result: Ok(metadata()),
        });

        let (action, _) = queue.update(Message::ProbeCompleted {
            job_id,
            result: Err(task_error("late failure")),
        });

        assert_eq!(action, Action::None);
        assert!(matches!(queue.selected_status(), Some(JobStatus::Ready)));
    }

    #[test]
    fn terminal_results_only_apply_to_a_running_job_once() {
        let mut queue = Queue::new();
        let job_id = enqueue(&mut queue, "example");
        let _ = queue.update(Message::ProbeCompleted {
            job_id: job_id.clone(),
            result: Ok(metadata()),
        });
        assert!(matches!(queue.start_queue(), Action::ResolveOutput { .. }));
        assert!(matches!(
            queue.output_resolved(&job_id, Ok(PathBuf::from("/music/example.mp3"))),
            Action::RipRequested { .. }
        ));
        assert!(matches!(
            queue.finish_active(
                &job_id,
                Ok(RipTermination::Completed(RipOutcome {
                    status: "success".into()
                }))
            ),
            Action::QueueFinished {
                summary: QueueRunSummary {
                    completed: 1,
                    failed: 0,
                    skipped: 0,
                    cancelled: 0,
                },
                error: None,
            }
        ));
        assert_eq!(
            queue.finish_active(&job_id, Err(task_error("late failure"))),
            Action::None
        );
        assert!(matches!(
            queue.selected_status(),
            Some(JobStatus::Completed)
        ));
    }

    #[test]
    fn queue_runs_in_order_and_continues_after_failure() {
        let mut queue = Queue::new();
        let first = enqueue(&mut queue, "first");
        let second = enqueue(&mut queue, "second");
        for job_id in [&first, &second] {
            let _ = queue.update(Message::ProbeCompleted {
                job_id: job_id.clone(),
                result: Ok(metadata()),
            });
        }

        let first_action = queue.start_queue();
        assert!(matches!(
            first_action,
            Action::ResolveOutput { job_id, .. } if job_id == first
        ));
        assert_eq!(queue.run_progress(), Some((1, 2)));
        assert_eq!(queue.start_queue(), Action::None);

        let _ = queue.output_resolved(&first, Ok(PathBuf::from("/music/first.mp3")));
        let second_action = queue.finish_active(&first, Err(task_error("first failed")));
        assert!(matches!(
            second_action,
            Action::ResolveOutput { job_id, .. } if job_id == second
        ));
        assert_eq!(queue.run_progress(), Some((2, 2)));

        let _ = queue.output_resolved(&second, Ok(PathBuf::from("/music/second.mp3")));
        let finished = queue.finish_active(
            &second,
            Ok(RipTermination::Completed(RipOutcome {
                status: "success".into(),
            })),
        );
        assert_eq!(
            finished,
            Action::QueueFinished {
                summary: QueueRunSummary {
                    completed: 1,
                    failed: 1,
                    skipped: 0,
                    cancelled: 0,
                },
                error: None,
            }
        );
        assert!(!queue.is_running());
        assert!(matches!(queue.jobs[0].status, JobStatus::Failed(_)));
        assert!(matches!(queue.jobs[1].status, JobStatus::Completed));
    }

    #[test]
    fn cancelling_stops_the_entire_queue_and_is_idempotent() {
        let mut queue = Queue::new();
        let first = enqueue(&mut queue, "first");
        let second = enqueue(&mut queue, "second");
        for job_id in [&first, &second] {
            let _ = queue.update(Message::ProbeCompleted {
                job_id: job_id.clone(),
                result: Ok(metadata()),
            });
        }
        let _ = queue.start_queue();
        let _ = queue.output_resolved(&first, Ok(PathBuf::from("/music/first.mp3")));

        assert_eq!(queue.request_cancel(&first), Action::None);
        assert!(matches!(queue.jobs[0].status, JobStatus::Cancelling));
        assert!(matches!(queue.jobs[1].status, JobStatus::Cancelled));
        assert_eq!(queue.request_cancel(&first), Action::None);

        let action = queue.finish_active(&first, Ok(RipTermination::Cancelled { forced: false }));
        assert_eq!(
            action,
            Action::QueueFinished {
                summary: QueueRunSummary {
                    completed: 0,
                    failed: 0,
                    skipped: 0,
                    cancelled: 2,
                },
                error: None,
            }
        );
        assert!(matches!(queue.jobs[0].status, JobStatus::Cancelled));
        assert!(!queue.is_running());
    }

    #[test]
    fn queue_start_freezes_each_jobs_encoding_snapshot() {
        let mut queue = Queue::new();
        let job_id = enqueue(&mut queue, "example");
        let options = RipOptions {
            bitrate: Mp3Bitrate::Kbps320,
            sample_rate: SampleRate::Hz48000,
            channels: ChannelMode::Mono,
            ..RipOptions::default()
        };
        queue.set_options(options);
        let _ = queue.update(Message::ProbeCompleted {
            job_id: job_id.clone(),
            result: Ok(metadata()),
        });
        let _ = queue.start_queue();

        queue.set_options(RipOptions::default());
        let action = queue.output_resolved(&job_id, Ok(PathBuf::from("/music/example.mp3")));

        assert!(matches!(
            action,
            Action::RipRequested { request, .. }
                if request.options == options && request.metadata == Some(metadata())
        ));
        assert_eq!(queue.jobs[0].options, options);
    }

    #[test]
    fn cleanup_failure_is_preserved_in_the_terminal_queue_action() {
        let mut queue = Queue::new();
        let job_id = enqueue(&mut queue, "example");
        let _ = queue.update(Message::ProbeCompleted {
            job_id: job_id.clone(),
            result: Ok(metadata()),
        });
        let _ = queue.start_queue();
        let _ = queue.output_resolved(&job_id, Ok(PathBuf::from("/music/example.mp3")));
        let _ = queue.request_cancel(&job_id);
        let error = Arc::new(crate::Error::PartialOutputCleanup {
            path: PathBuf::from("/music/example.mp3"),
            source: std::io::Error::new(std::io::ErrorKind::PermissionDenied, "denied"),
        });

        let action = queue.finish_active(&job_id, Err(error));

        assert!(matches!(
            action,
            Action::QueueFinished {
                summary: QueueRunSummary { failed: 1, .. },
                error: Some(message),
            } if message.contains("partial output `/music/example.mp3`")
        ));
        assert!(matches!(queue.jobs[0].status, JobStatus::Failed(_)));
    }

    #[test]
    fn progress_updates_only_the_active_job_and_remains_monotonic() {
        let mut queue = Queue::new();
        let active = enqueue(&mut queue, "active");
        let waiting = enqueue(&mut queue, "waiting");
        for job_id in [&active, &waiting] {
            let _ = queue.update(Message::ProbeCompleted {
                job_id: job_id.clone(),
                result: Ok(metadata()),
            });
        }
        let _ = queue.start_queue();

        let _ = queue.update(Message::ProgressReceived {
            job_id: waiting,
            progress: RipProgressEvent {
                phase: RipPhase::Encoding,
                progress: FfmpegProgress {
                    elapsed: Some(Duration::from_secs(60)),
                    ..FfmpegProgress::default()
                },
            },
        });
        let _ = queue.update(Message::ProgressReceived {
            job_id: active.clone(),
            progress: RipProgressEvent {
                phase: RipPhase::Encoding,
                progress: FfmpegProgress {
                    elapsed: Some(Duration::from_secs(40)),
                    speed: Some(2.0),
                    ..FfmpegProgress::default()
                },
            },
        });
        let _ = queue.update(Message::ProgressReceived {
            job_id: active,
            progress: RipProgressEvent {
                phase: RipPhase::Encoding,
                progress: FfmpegProgress {
                    elapsed: Some(Duration::from_secs(30)),
                    ..FfmpegProgress::default()
                },
            },
        });

        assert_eq!(queue.jobs[0].progress.elapsed, Duration::from_secs(40));
        assert!(queue.jobs[0].progress.percent > 53.0);
        assert_eq!(queue.jobs[0].progress.speed, Some(2.0));
        assert_eq!(queue.jobs[1].progress.elapsed, Duration::ZERO);
    }

    #[test]
    fn start_waits_for_all_probes_and_marks_probe_failures_skipped() {
        let mut queue = Queue::new();
        let ready = enqueue(&mut queue, "ready");
        let failed = enqueue(&mut queue, "silent");
        let _ = queue.update(Message::ProbeCompleted {
            job_id: ready,
            result: Ok(metadata()),
        });
        assert!(!queue.can_start());

        let _ = queue.update(Message::ProbeCompleted {
            job_id: failed,
            result: Err(task_error("No audio stream was found")),
        });
        assert!(queue.can_start());
        assert!(matches!(queue.start_queue(), Action::ResolveOutput { .. }));
        assert!(matches!(queue.jobs[1].status, JobStatus::Skipped(_)));
    }

    #[test]
    fn selection_and_removal_ignore_unknown_or_late_results() {
        let mut queue = Queue::new();
        let job_id = enqueue(&mut queue, "example");

        let _ = queue.update(Message::Select(JobId::new(99)));
        assert_eq!(queue.selected().map(|job| &job.id), Some(&job_id));

        let _ = queue.update(Message::Remove(job_id.clone()));
        let (action, _) = queue.update(Message::ProbeCompleted {
            job_id,
            result: Ok(metadata()),
        });

        assert_eq!(action, Action::None);
        assert!(queue.selected().is_none());
    }

    #[test]
    fn duplicate_intake_does_not_create_a_second_job() {
        let mut queue = Queue::new();
        let _ = enqueue(&mut queue, "example");
        let input = AcceptedInput {
            path: PathBuf::from("/videos/example.mp4"),
            size: 42,
            hierarchy: None,
        };

        let action = queue.enqueue_many(
            vec![(input, PathBuf::from("/music/example.mp3"))],
            RipOptions::default(),
            DestinationPolicy::default(),
        );

        assert_eq!(action, Action::ProbeRequested(Vec::new()));
        assert_eq!(queue.jobs.len(), 1);
        assert_eq!(
            queue.rejected.last().map(|item| item.reason.as_str()),
            Some("This file is already in the queue")
        );
    }

    #[test]
    fn dropped_paths_are_emitted_once_in_event_order() {
        let mut queue = Queue::new();
        let (first_action, _) = queue.update(Message::PathsDropped(vec![PathBuf::from("b.mp4")]));
        let (second_action, _) = queue.update(Message::PathsDropped(vec![PathBuf::from("a.mp4")]));

        let (stale_action, _) = queue.update(Message::DropBatchReady(1));
        let (ready_action, _) = queue.update(Message::DropBatchReady(2));

        assert_eq!(first_action, Action::None);
        assert_eq!(second_action, Action::None);
        assert_eq!(stale_action, Action::None);
        assert_eq!(
            ready_action,
            Action::IntakeRequested(vec![PathBuf::from("b.mp4"), PathBuf::from("a.mp4")])
        );
    }

    #[test]
    fn intake_requests_are_serialized_while_discovery_is_running() {
        let mut queue = Queue::new();
        queue.discovering = true;

        let (action, _) = queue.update(Message::PathsSelected(vec![PathBuf::from("later.mp4")]));

        assert_eq!(action, Action::None);
        assert_eq!(queue.dropped_paths, [PathBuf::from("later.mp4")]);
        assert_eq!(queue.drop_batch, 1);
    }

    #[test]
    fn drop_hover_state_tracks_window_events_and_clears_on_drop() {
        let mut queue = Queue::new();

        let _ = queue.update(Message::DropHoverChanged(true));
        assert!(queue.drop_hovered);

        let _ = queue.update(Message::PathsDropped(vec![PathBuf::from("video.mp4")]));
        assert!(!queue.drop_hovered);
    }

    #[test]
    fn output_folder_changes_apply_to_every_editable_job() {
        let mut queue = Queue::new();
        let _ = enqueue(&mut queue, "first");
        let _ = enqueue(&mut queue, "second");

        queue.set_output_paths(|job| {
            Path::new("/new-output")
                .join(Path::new(&job.input).file_name().unwrap())
                .with_extension("mp3")
        });

        assert_eq!(queue.jobs[0].output, "/new-output/first.mp3");
        assert_eq!(queue.jobs[1].output, "/new-output/second.mp3");
    }

    #[test]
    fn destination_policy_changes_apply_to_editable_job_snapshots() {
        let mut queue = Queue::new();
        let _ = enqueue(&mut queue, "first");
        queue.set_destination_policy(DestinationPolicy {
            preserve_folder_structure: false,
        });

        assert!(!queue.jobs[0].destination_policy.preserve_folder_structure);
    }

    #[test]
    fn long_queues_expand_in_bounded_batches() {
        let mut queue = Queue::new();
        for index in 0..125 {
            let _ = enqueue(&mut queue, &format!("video-{index}"));
        }

        assert_eq!(queue.visible_jobs, 50);
        let _ = queue.update(Message::ShowMore);
        assert_eq!(queue.visible_jobs, 100);
        let _ = queue.update(Message::ShowMore);
        assert_eq!(queue.visible_jobs, 150);
    }
}
