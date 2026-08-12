use std::path::Path;

use iced::Task;

use crate::{app::runtime, ffmpeg::DependencyState};

use super::{message::Message, output_settings, queue, state::Demux};

impl Demux {
    pub fn new() -> (Self, Task<Message>) {
        (
            Self::default(),
            Task::perform(runtime::check_dependencies(), Message::DependenciesChecked),
        )
    }

    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::DependenciesChecked(result) => {
                match result {
                    Ok(dependencies) => {
                        self.dependency_state = DependencyState::Ready(dependencies);
                    }
                    Err(message) => {
                        self.dependency_state = DependencyState::Failed {
                            program: "ffmpeg/ffprobe",
                            message: message.clone(),
                        };
                        self.error = Some(message);
                    }
                }
                Task::none()
            }
            Message::Queue(message) => self.update_queue(message),
            Message::OutputSettings(message) => {
                let (action, task) = self.output_settings.update(message);
                match action {
                    output_settings::Action::None => {}
                    output_settings::Action::OutputChanged => self.refresh_output_path(),
                    output_settings::Action::StartRipping => {
                        return self.update_queue(queue::Message::StartSelected);
                    }
                }
                task.map(Message::OutputSettings)
            }
            Message::RipCompleted { job_id, result } => {
                self.update_queue(queue::Message::RipCompleted { job_id, result })
            }
            Message::Notifications(message) => self
                .notifications
                .update(message)
                .map(Message::Notifications),
        }
    }

    fn update_queue(&mut self, message: queue::Message) -> Task<Message> {
        let (action, task) = self.queue.update(message);
        let local_task = task.map(Message::Queue);

        match action {
            queue::Action::None => local_task,
            queue::Action::FilePickerOpened => {
                self.error = None;
                local_task
            }
            queue::Action::FileSelected(input) => {
                self.error = None;
                self.output_settings.set_default_from_input(&input);
                let output = self.output_settings.output_path(&input);
                let action = self.queue.enqueue(input, output);
                self.handle_queue_action(action)
            }
            queue::Action::ProbeRequested { .. }
            | queue::Action::RipRequested { .. }
            | queue::Action::RipCompleted { .. }
            | queue::Action::RipFailed(_) => self.handle_queue_action(action),
            queue::Action::ProbeFailed(message) => {
                self.error = Some(message);
                local_task
            }
        }
    }

    fn handle_queue_action(&mut self, action: queue::Action) -> Task<Message> {
        match action {
            queue::Action::ProbeRequested { job_id, input } => {
                Task::perform(runtime::probe(job_id.clone(), input), move |result| {
                    Message::Queue(queue::Message::ProbeCompleted { job_id, result })
                })
            }
            queue::Action::RipRequested { job_id, request } => {
                if !matches!(self.dependency_state, DependencyState::Ready(_))
                    || !self.output_settings.has_folder()
                {
                    return Task::none();
                }
                self.error = None;
                Task::perform(runtime::rip(job_id.clone(), request), move |result| {
                    Message::RipCompleted { job_id, result }
                })
            }
            queue::Action::RipCompleted { output } => {
                let output_name = Path::new(&output)
                    .file_name()
                    .and_then(|name| name.to_str())
                    .unwrap_or("Your MP3")
                    .to_owned();
                self.notifications
                    .success(
                        "Ripping complete",
                        format!("{output_name} is ready in your output folder."),
                    )
                    .map(Message::Notifications)
            }
            queue::Action::RipFailed(message) => {
                self.error = Some(message);
                self.notifications
                    .failure(
                        "Ripping failed",
                        "Review the error message, then try the extraction again.",
                    )
                    .map(Message::Notifications)
            }
            queue::Action::None
            | queue::Action::FilePickerOpened
            | queue::Action::FileSelected(_)
            | queue::Action::ProbeFailed(_) => Task::none(),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{path::PathBuf, time::Duration};

    use crate::{
        ffmpeg::{Dependencies, RipOutcome},
        model::job::{JobId, JobStatus},
        model::media::{AudioMetadata, MediaInfo},
    };

    use super::*;

    fn metadata() -> MediaInfo {
        MediaInfo {
            duration: Duration::from_secs(65),
            container: "mov,mp4".into(),
            bitrate: Some(1_000_000),
            creation_time: None,
            audio: AudioMetadata {
                stream_index: 1,
                codec: "aac".into(),
                sample_rate: Some(48_000),
                channels: Some(2),
                channel_layout: Some("stereo".into()),
                bitrate: Some(192_000),
                language: None,
            },
        }
    }

    fn enqueue(state: &mut Demux, input: &str) -> JobId {
        let input = PathBuf::from(input);
        state.output_settings.set_default_from_input(&input);
        let output = state.output_settings.output_path(&input);
        let _ = state.queue.enqueue(input, output);
        state.queue.selected().unwrap().id.clone()
    }

    fn start(state: &mut Demux, job_id: &JobId) {
        let _ = state.queue.update(queue::Message::ProbeCompleted {
            job_id: job_id.clone(),
            result: Ok(metadata()),
        });
        let _ = state.queue.update(queue::Message::StartSelected);
    }

    #[test]
    fn probe_completion_makes_selected_job_ready() {
        let mut state = Demux {
            dependency_state: DependencyState::Ready(Dependencies {
                ffmpeg_version: "ffmpeg test".into(),
                ffprobe_version: "ffprobe test".into(),
            }),
            ..Demux::default()
        };
        let job_id = enqueue(&mut state, "/videos/example.mp4");

        let _ = state.update(Message::Queue(queue::Message::ProbeCompleted {
            job_id,
            result: Ok(metadata()),
        }));

        assert!(matches!(
            state.queue.selected_status(),
            Some(JobStatus::Ready)
        ));
        assert!(state.can_start());
    }

    #[test]
    fn probe_failure_is_visible_in_job_and_error_area() {
        let mut state = Demux::default();
        let job_id = enqueue(&mut state, "/videos/silent.mp4");

        let _ = state.update(Message::Queue(queue::Message::ProbeCompleted {
            job_id,
            result: Err("No audio stream was found".into()),
        }));

        assert!(matches!(
            state.queue.selected_status(),
            Some(JobStatus::Failed(_))
        ));
        assert_eq!(state.error.as_deref(), Some("No audio stream was found"));
    }

    #[test]
    fn successful_rip_adds_a_completion_toast() {
        let mut state = Demux::default();
        let job_id = enqueue(&mut state, "/videos/example.mp4");
        start(&mut state, &job_id);

        let _ = state.update(Message::RipCompleted {
            job_id,
            result: Ok(RipOutcome {
                status: "success".into(),
            }),
        });

        assert_eq!(state.notifications.len(), 1);
    }

    #[test]
    fn failed_rip_adds_a_danger_toast_and_keeps_the_error() {
        let mut state = Demux::default();
        let job_id = enqueue(&mut state, "/videos/example.mp4");
        start(&mut state, &job_id);

        let _ = state.update(Message::RipCompleted {
            job_id,
            result: Err("FFmpeg exited with status 1".into()),
        });

        assert_eq!(state.notifications.len(), 1);
        assert_eq!(state.error.as_deref(), Some("FFmpeg exited with status 1"));
    }

    #[test]
    fn selected_file_action_wires_output_settings_into_the_queue() {
        let mut state = Demux::default();

        let _ = state.update(Message::Queue(queue::Message::FileSelected(Some(
            PathBuf::from("/videos/example.mp4"),
        ))));

        let job = state.queue.selected().unwrap();
        assert_eq!(job.input, "/videos/example.mp4");
        assert_eq!(job.output, "/videos/example.mp3");
        assert!(matches!(job.status, JobStatus::Probing));
    }

    #[test]
    fn stale_rip_completion_does_not_create_a_notification() {
        let mut state = Demux::default();

        let _ = state.update(Message::RipCompleted {
            job_id: JobId::new(99),
            result: Ok(RipOutcome {
                status: "success".into(),
            }),
        });

        assert_eq!(state.notifications.len(), 0);
    }

    #[test]
    fn duplicate_rip_completion_does_not_create_another_notification() {
        let mut state = Demux::default();
        let job_id = enqueue(&mut state, "/videos/example.mp4");
        start(&mut state, &job_id);
        let completion = || Message::RipCompleted {
            job_id: job_id.clone(),
            result: Ok(RipOutcome {
                status: "success".into(),
            }),
        };

        let _ = state.update(completion());
        let _ = state.update(completion());

        assert_eq!(state.notifications.len(), 1);
    }
}
