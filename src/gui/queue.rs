use std::path::{Path, PathBuf};

use iced::font::Weight;
use iced::widget::{button, column, container, row, rule, scrollable, space, text};
use iced::{Color, Element, Fill, FillPortion, Font, Padding, Task};

use crate::{
    ffmpeg::{RipOutcome, RipRequest},
    model::{
        job::{JobId, JobStatus, RipJob},
        media::MediaInfo,
    },
};

use super::{
    presentation::JobPresentation,
    style::{DANGER, TEXT_MUTED, error_panel, inset_panel, panel, selected_row},
};

#[derive(Debug, Clone)]
pub enum Message {
    AddFile,
    FileSelected(Option<PathBuf>),
    ProbeCompleted {
        job_id: JobId,
        result: Result<MediaInfo, String>,
    },
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
    FileSelected(PathBuf),
    ProbeRequested { job_id: JobId, input: PathBuf },
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
    next_job_id: u64,
}

impl Queue {
    pub(crate) fn new() -> Self {
        Self {
            jobs: Vec::new(),
            selected_job: None,
            picking_file: false,
            next_job_id: 1,
        }
    }

    pub(crate) fn update(&mut self, message: Message) -> (Action, Task<Message>) {
        match message {
            Message::AddFile => {
                self.picking_file = true;
                (
                    Action::FilePickerOpened,
                    Task::perform(pick_video_file(), Message::FileSelected),
                )
            }
            Message::FileSelected(path) => {
                self.picking_file = false;
                (
                    path.map_or(Action::None, Action::FileSelected),
                    Task::none(),
                )
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

    pub(crate) fn enqueue(&mut self, input: PathBuf, output: PathBuf) -> Action {
        let job_id = JobId::new(self.next_job_id);
        self.next_job_id += 1;

        let mut job = RipJob::new(
            job_id.clone(),
            input.to_string_lossy().into_owned(),
            output.to_string_lossy().into_owned(),
        );
        job.start_probing();

        // Milestone 0 preserves the existing single-file behavior. Milestone 1
        // will stop replacing the collection when multi-file intake lands.
        self.jobs.clear();
        self.jobs.push(job);
        self.selected_job = Some(job_id.clone());

        Action::ProbeRequested { job_id, input }
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

    pub(crate) fn selected_input(&self) -> Option<&Path> {
        self.selected().map(|job| Path::new(&job.input))
    }

    pub(crate) fn set_selected_output(&mut self, output: PathBuf) {
        if let Some(job) = self.selected_mut()
            && !matches!(job.status, JobStatus::Ripping | JobStatus::Completed)
        {
            job.output = output.to_string_lossy().into_owned();
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
        } else if self.is_busy() {
            "Demux is working on the selected video"
        } else if self.jobs.is_empty() {
            "Choose a video to begin"
        } else {
            "Replace the selected video"
        };

        let add_button = button(text("Add File").size(15))
            .padding(Padding::from([10, 16]))
            .style(button::primary)
            .on_press_maybe((!self.picking_file && !self.is_busy()).then_some(Message::AddFile));

        let chooser = container(
            column![
                text(choose_copy).size(18).font(Font {
                    weight: Weight::Semibold,
                    ..Font::default()
                }),
                text("MP4, MKV, MOV, AVI, WMV, FLV, MPEG")
                    .size(13)
                    .color(TEXT_MUTED),
                add_button,
            ]
            .spacing(10)
            .align_x(iced::Alignment::Center),
        )
        .width(Fill)
        .padding(Padding::from([24, 18]))
        .center_x(Fill)
        .style(inset_panel);

        let mut content = column![chooser, self.queue_panel()].spacing(14);
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
                        text(error)
                            .size(13)
                            .color(Color::from_rgb(0.95, 0.76, 0.77)),
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
            text(if self.jobs.is_empty() {
                "No files"
            } else {
                "1 file"
            })
            .size(13)
            .color(TEXT_MUTED),
        ]
        .align_y(iced::Alignment::Center);

        let queue: Element<'_, Message> = match self.selected() {
            Some(job) => job_row(job),
            None => container(
                column![
                    text("Your queue is empty")
                        .size(17)
                        .font(Font {
                            weight: Weight::Semibold,
                            ..Font::default()
                        }),
                    text("Add a video above. Demux will inspect its audio stream before you can start ripping.")
                        .size(14)
                        .color(TEXT_MUTED),
                ]
                .spacing(7)
                .max_width(520),
            )
            .height(180)
            .padding(22)
            .center_y(180)
            .into(),
        };

        container(column![heading, rule::horizontal(1), scrollable(queue)].spacing(12))
            .width(Fill)
            .height(Fill)
            .padding(18)
            .style(panel)
            .into()
    }
}

fn job_row(job: &RipJob) -> Element<'_, Message> {
    let job = JobPresentation::from(job);

    container(
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
            ]
            .spacing(18),
        ]
        .spacing(14),
    )
    .width(Fill)
    .padding(18)
    .style(selected_row)
    .into()
}

async fn pick_video_file() -> Option<PathBuf> {
    rfd::AsyncFileDialog::new()
        .set_title("Choose a video to rip")
        .add_filter(
            "Video files",
            &["mp4", "mkv", "mov", "avi", "wmv", "flv", "mpeg", "mpg"],
        )
        .pick_file()
        .await
        .map(|file| file.path().to_path_buf())
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
        let _ = queue.enqueue(input, output);
        queue.selected().unwrap().id.clone()
    }

    #[test]
    fn ignores_a_probe_result_for_a_replaced_job() {
        let mut queue = Queue::new();
        let replaced = enqueue(&mut queue, "first");
        let current = enqueue(&mut queue, "second");

        let (action, _) = queue.update(Message::ProbeCompleted {
            job_id: replaced,
            result: Ok(metadata()),
        });

        assert_eq!(action, Action::None);
        assert_eq!(queue.selected().map(|job| &job.id), Some(&current));
        assert!(matches!(queue.selected_status(), Some(JobStatus::Probing)));
    }

    #[test]
    fn add_file_reports_that_the_picker_was_opened() {
        let mut queue = Queue::new();

        let (action, _) = queue.update(Message::AddFile);

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
}
