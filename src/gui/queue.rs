use std::path::{Path, PathBuf};

use iced::font::Weight;
use iced::widget::{button, column, container, row, rule, scrollable, space, text};
use iced::{Element, Fill, FillPortion, Font, Padding, Task};

use crate::{
    app::intake::{AcceptedInput, IntakeResult, RejectedInput},
    ffmpeg::{RipOutcome, RipRequest},
    model::{
        job::{JobId, JobStatus, RipJob},
        media::MediaInfo,
    },
};

use super::{
    presentation::JobPresentation,
    style::{DANGER, DANGER_TEXT, TEXT_MUTED, error_panel, inset_panel, panel, selected_row},
};

const DROP_BATCH_DELAY: std::time::Duration = std::time::Duration::from_millis(100);
const VISIBLE_JOB_BATCH: usize = 50;

#[derive(Debug, Clone)]
pub enum Message {
    AddFiles,
    AddFolder,
    PathsSelected(Vec<PathBuf>),
    PathsDropped(Vec<PathBuf>),
    DropBatchReady(u64),
    IntakeCompleted(IntakeResult),
    ProbeCompleted {
        job_id: JobId,
        result: Result<MediaInfo, String>,
    },
    ShowMore,
    DismissRejected,
    Select(JobId),
    Remove(JobId),
    StartSelected,
    RipCompleted {
        job_id: JobId,
        result: Result<RipOutcome, String>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Action {
    None,
    FilePickerOpened,
    IntakeRequested(Vec<PathBuf>),
    IntakeAccepted(Vec<AcceptedInput>),
    ProbeRequested(Vec<(JobId, PathBuf)>),
    ProbeFailed(String),
    RipRequested { job_id: JobId, request: RipRequest },
    RipCompleted { output: String },
    RipFailed(String),
}

#[derive(Debug)]
pub(crate) struct Queue {
    jobs: Vec<RipJob>,
    selected_job: Option<JobId>,
    picking_file: bool,
    discovering: bool,
    dropped_paths: Vec<PathBuf>,
    drop_batch: u64,
    rejected: Vec<RejectedInput>,
    visible_jobs: usize,
    next_job_id: u64,
}

impl Queue {
    pub(crate) fn new() -> Self {
        Self {
            jobs: Vec::new(),
            selected_job: None,
            picking_file: false,
            discovering: false,
            dropped_paths: Vec::new(),
            drop_batch: 0,
            rejected: Vec::new(),
            visible_jobs: VISIBLE_JOB_BATCH,
            next_job_id: 1,
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
                let action = if paths.is_empty() {
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
                self.dropped_paths.extend(paths);
                self.drop_batch += 1;
                let batch = self.drop_batch;
                (
                    Action::None,
                    Task::perform(wait_for_drop_batch(batch), Message::DropBatchReady),
                )
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
                    Err(message) => {
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
            Message::StartSelected => {
                let action = self
                    .start_selected()
                    .map_or(Action::None, |(job_id, request)| Action::RipRequested {
                        job_id,
                        request,
                    });
                (action, Task::none())
            }
            Message::RipCompleted { job_id, result } => {
                let action = match result {
                    Ok(_) => self
                        .complete(&job_id)
                        .map_or(Action::None, |output| Action::RipCompleted { output }),
                    Err(message) if self.fail(&job_id, message.clone()) => {
                        Action::RipFailed(message)
                    }
                    Err(_) => Action::None,
                };
                (action, Task::none())
            }
        }
    }

    pub(crate) fn enqueue_many(&mut self, paths: Vec<(AcceptedInput, PathBuf)>) -> Action {
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
            let mut job = RipJob::new(
                job_id.clone(),
                input.path.to_string_lossy().into_owned(),
                output.to_string_lossy().into_owned(),
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

    fn selected_mut(&mut self) -> Option<&mut RipJob> {
        let selected = self.selected_job.as_ref()?;
        self.jobs.iter_mut().find(|job| &job.id == selected)
    }

    fn job_mut(&mut self, id: &JobId) -> Option<&mut RipJob> {
        self.jobs.iter_mut().find(|job| &job.id == id)
    }

    pub(crate) fn selected_status(&self) -> Option<&JobStatus> {
        self.selected().map(|job| &job.status)
    }

    pub(crate) fn set_output_paths(&mut self, mut derive: impl FnMut(&Path) -> PathBuf) {
        for job in &mut self.jobs {
            if !matches!(job.status, JobStatus::Ripping | JobStatus::Completed) {
                job.output = derive(Path::new(&job.input)).to_string_lossy().into_owned();
            }
        }
    }

    fn start_selected(&mut self) -> Option<(JobId, RipRequest)> {
        let job = self.selected_mut()?;
        if !matches!(job.status, JobStatus::Ready) {
            return None;
        }
        job.start_ripping();
        Some((
            job.id.clone(),
            RipRequest::new(job.input.clone(), job.output.clone()),
        ))
    }

    fn complete(&mut self, id: &JobId) -> Option<String> {
        let job = self.job_mut(id)?;
        if !matches!(job.status, JobStatus::Ripping) {
            return None;
        }
        job.complete();
        Some(job.output.clone())
    }

    fn fail(&mut self, id: &JobId, message: String) -> bool {
        let Some(job) = self.job_mut(id) else {
            return false;
        };
        if !matches!(job.status, JobStatus::Ripping) {
            return false;
        }
        job.fail(message);
        true
    }

    pub(crate) fn is_ready(&self) -> bool {
        matches!(self.selected_status(), Some(JobStatus::Ready))
    }

    pub(crate) fn is_busy(&self) -> bool {
        matches!(
            self.selected_status(),
            Some(JobStatus::Probing | JobStatus::Ripping)
        )
    }

    pub(crate) fn view<'a>(&'a self, error: Option<&'a str>) -> Element<'a, Message> {
        let choose_copy = if self.picking_file {
            "Waiting for file selection…"
        } else if self.discovering {
            "Discovering supported media…"
        } else if self.is_busy() {
            "Demux is inspecting your videos"
        } else if self.jobs.is_empty() {
            "Add videos or drop a folder to begin"
        } else {
            "Add more videos"
        };

        let intake_enabled = !self.picking_file && !self.discovering;

        let add_button = button(text("Add Files").size(15))
            .padding(Padding::from([10, 16]))
            .style(button::primary)
            .on_press_maybe(intake_enabled.then_some(Message::AddFiles));
        let folder_button = button(text("Add Folder").size(15))
            .padding(Padding::from([10, 16]))
            .on_press_maybe(intake_enabled.then_some(Message::AddFolder));

        let chooser = container(
            column![
                text(choose_copy).size(18).font(Font {
                    weight: Weight::Semibold,
                    ..Font::default()
                }),
                text("MP4, MKV, MOV, AVI, WMV, FLV, MPEG")
                    .size(13)
                    .color(TEXT_MUTED),
                row![add_button, folder_button].spacing(8),
            ]
            .spacing(10)
            .align_x(iced::Alignment::Center),
        )
        .width(Fill)
        .padding(Padding::from([24, 18]))
        .center_x(Fill)
        .style(inset_panel);

        let mut content = column![chooser, self.queue_panel()].spacing(14);
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

        container(content).width(FillPortion(7)).height(Fill).into()
    }

    fn queue_panel(&self) -> Element<'_, Message> {
        let heading = row![
            text("Queue").size(17).font(Font {
                weight: Weight::Semibold,
                ..Font::default()
            }),
            space::horizontal(),
            button(text("Remove").size(13))
                .padding(Padding::from([7, 11]))
                .on_press_maybe(
                    self.selected()
                        .filter(|job| !matches!(job.status, JobStatus::Ripping))
                        .map(|job| Message::Remove(job.id.clone())),
                ),
            text(match self.jobs.len() {
                0 => "No files".into(),
                1 => "1 file".into(),
                count => format!("{count} files"),
            })
            .size(13)
            .color(TEXT_MUTED),
        ]
        .align_y(iced::Alignment::Center);

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
            let mut jobs = self.jobs.iter().take(self.visible_jobs).fold(
                column![].spacing(8),
                |column, job| {
                    column.push(job_row(job, self.selected_job.as_ref() == Some(&job.id)))
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

        container(column![heading, rule::horizontal(1), scrollable(queue)].spacing(12))
            .width(Fill)
            .height(Fill)
            .padding(18)
            .style(panel)
            .into()
    }
}

fn job_row(job: &RipJob, selected: bool) -> Element<'_, Message> {
    let job = JobPresentation::from(job);
    let id = job.id.clone();

    let row = container(
        column![
            row![
                column![
                    text(job.filename).size(16).font(Font {
                        weight: Weight::Semibold,
                        ..Font::default()
                    }),
                    text(job.input).size(12).color(TEXT_MUTED),
                ]
                .spacing(4)
                .width(Fill),
                text(job.status.label)
                    .size(13)
                    .font(Font {
                        weight: Weight::Semibold,
                        ..Font::default()
                    })
                    .color(job.status.color),
                text(if selected { "Selected" } else { "" })
                    .size(12)
                    .color(TEXT_MUTED),
            ]
            .spacing(14)
            .align_y(iced::Alignment::Start),
            rule::horizontal(1),
            row![
                column![
                    text("Duration").size(12).color(TEXT_MUTED),
                    text(job.duration)
                ]
                .spacing(4)
                .width(FillPortion(1)),
                column![
                    text("Audio stream").size(12).color(TEXT_MUTED),
                    text(job.audio_details)
                ]
                .spacing(4)
                .width(FillPortion(2)),
                column![
                    text("Output").size(12).color(TEXT_MUTED),
                    text(job.output_details)
                ]
                .spacing(4)
                .width(FillPortion(1)),
                column![text("Size").size(12).color(TEXT_MUTED), text(job.size)]
                    .spacing(4)
                    .width(FillPortion(1)),
            ]
            .spacing(18),
        ]
        .spacing(14),
    )
    .width(Fill)
    .padding(18)
    .style(if selected { selected_row } else { inset_panel });

    button(row)
        .padding(0)
        .width(Fill)
        .style(button::text)
        .on_press(Message::Select(id))
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
    use std::time::Duration;

    use crate::model::media::AudioMetadata;

    use super::*;

    fn metadata() -> MediaInfo {
        MediaInfo {
            duration: Duration::from_secs(75),
            container: "mp4".into(),
            bitrate: None,
            creation_time: None,
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
        let _ = queue.enqueue_many(vec![(
            AcceptedInput {
                path: input,
                size: 42,
            },
            output,
        )]);
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
            result: Err("No audio stream was found".into()),
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
            result: Err("late failure".into()),
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
        let _ = queue.start_selected();

        assert_eq!(queue.complete(&job_id), Some("/music/example.mp3".into()));
        assert_eq!(queue.complete(&job_id), None);
        assert!(!queue.fail(&job_id, "late failure".into()));
        assert!(matches!(
            queue.selected_status(),
            Some(JobStatus::Completed)
        ));
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
        };

        let action = queue.enqueue_many(vec![(input, PathBuf::from("/music/example.mp3"))]);

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
    fn output_folder_changes_apply_to_every_editable_job() {
        let mut queue = Queue::new();
        let _ = enqueue(&mut queue, "first");
        let _ = enqueue(&mut queue, "second");

        queue.set_output_paths(|input| {
            Path::new("/new-output")
                .join(input.file_name().unwrap())
                .with_extension("mp3")
        });

        assert_eq!(queue.jobs[0].output, "/new-output/first.mp3");
        assert_eq!(queue.jobs[1].output, "/new-output/second.mp3");
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
