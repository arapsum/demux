use iced::Task;
use iced::futures::SinkExt;

use crate::{
    app::runtime,
    ffmpeg::{DependencyState, RipTermination, cancellation_pair},
    model::job::JobId,
};

use super::{message::Message, output_settings, progress, queue, share_error, state::Demux};

impl Demux {
    pub fn new() -> (Self, Task<Message>) {
        (
            Self::default(),
            Task::batch([
                Task::perform(runtime::check_dependencies(), |result| {
                    Message::DependenciesChecked(share_error(result))
                }),
                Task::perform(runtime::load_preferences(), |result| {
                    Message::PreferencesLoaded(share_error(result))
                }),
            ]),
        )
    }

    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::DependenciesChecked(result) => {
                match result {
                    Ok(dependencies) => {
                        self.dependency_state = DependencyState::Ready(dependencies);
                    }
                    Err(error) => {
                        tracing::error!(error = %error, "dependency check failed");
                        let message = error.to_string();
                        self.dependency_state = DependencyState::Failed {
                            program: "ffmpeg/ffprobe",
                            message: message.clone(),
                        };
                        self.error = Some(message);
                    }
                }
                Task::none()
            }
            Message::PreferencesLoaded(result) => {
                match result {
                    Ok(options) => {
                        if self.output_settings.apply_loaded_defaults(options) {
                            self.refresh_encoding_options();
                        }
                    }
                    Err(error) => {
                        tracing::warn!(error = %error, "could not restore encoding defaults");
                        let message = error.to_string();
                        self.error = Some(message.clone());
                        return self
                            .notifications
                            .failure("Settings were not restored", message)
                            .map(Message::Notifications);
                    }
                }
                Task::none()
            }
            Message::PreferencesSaved(result) => {
                if let Err(error) = result {
                    tracing::error!(error = %error, "could not persist encoding defaults");
                    let message = error.to_string();
                    self.error = Some(message.clone());
                    return self
                        .notifications
                        .failure("Settings were not saved", message)
                        .map(Message::Notifications);
                }
                Task::none()
            }
            Message::Queue(message) => self.update_queue(message),
            Message::OutputSettings(message) => {
                let (action, task) = self
                    .output_settings
                    .update(message, self.queue.is_running());
                match action {
                    output_settings::Action::None => {}
                    output_settings::Action::OutputChanged => self.refresh_output_path(),
                    output_settings::Action::EncodingChanged(options) => {
                        self.refresh_encoding_options();
                        let revision = runtime::next_preferences_revision();
                        return Task::batch([
                            task.map(Message::OutputSettings),
                            Task::perform(runtime::save_preferences(options, revision), |result| {
                                Message::PreferencesSaved(share_error(result))
                            }),
                        ]);
                    }
                    output_settings::Action::StartRipping => {
                        return self.update_queue(queue::Message::StartQueue);
                    }
                }
                task.map(Message::OutputSettings)
            }
            Message::Progress(message) => match self.progress.update(message) {
                progress::Action::None => Task::none(),
                progress::Action::CancelRequested(job_id) => self.request_cancellation(&job_id),
            },
            Message::RipProgress { job_id, progress } => {
                if self.queue.is_active_job(&job_id) {
                    let _ = self.progress.update(progress::Message::Advanced {
                        job_id: job_id.clone(),
                        progress: progress.clone(),
                    });
                }
                self.update_queue(queue::Message::ProgressReceived { job_id, progress })
            }
            Message::RipCompleted { job_id, result } => {
                if self.queue.is_active_job(&job_id) {
                    let status = match &result {
                        Ok(RipTermination::Completed(_)) => progress::TerminalStatus::Completed,
                        Ok(RipTermination::Cancelled { .. }) => progress::TerminalStatus::Cancelled,
                        Err(_) => progress::TerminalStatus::Failed,
                    };
                    let _ = self.progress.update(progress::Message::Finished {
                        job_id: job_id.clone(),
                        status,
                    });
                }
                if self
                    .active_cancellation
                    .as_ref()
                    .is_some_and(|(active, _)| active == &job_id)
                {
                    self.active_cancellation = None;
                }
                self.update_queue(queue::Message::RipCompleted { job_id, result })
            }
            Message::CloseRequested => {
                let Some(job_id) = self.queue.active_job_id() else {
                    return iced::exit();
                };
                self.exit_after_queue = true;
                self.progress.mark_cancelling(&job_id);
                self.request_cancellation(&job_id)
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
            queue::Action::IntakeRequested(paths) => {
                self.error = None;
                let existing = self.queue.canonical_paths();
                Task::perform(crate::app::intake::discover(paths, existing), |result| {
                    Message::Queue(queue::Message::IntakeCompleted(result))
                })
            }
            queue::Action::IntakeAccepted(inputs) => {
                if let Some(input) = inputs.first() {
                    self.output_settings.set_default_from_input(&input.path);
                }
                let paths = inputs
                    .into_iter()
                    .map(|input| {
                        let output = self.output_settings.output_path(&input.path);
                        (input, output)
                    })
                    .collect();
                let action = self
                    .queue
                    .enqueue_many(paths, self.output_settings.options());
                self.handle_queue_action(action)
            }
            queue::Action::ProbeRequested(_)
            | queue::Action::RipRequested { .. }
            | queue::Action::QueueFinished { .. } => self.handle_queue_action(action),
            queue::Action::ResolveOutput { .. } => {
                self.error = None;
                self.handle_queue_action(action)
            }
            queue::Action::ProbeFailed(message) => {
                self.error = Some(message);
                local_task
            }
        }
    }

    fn handle_queue_action(&mut self, action: queue::Action) -> Task<Message> {
        match action {
            queue::Action::ProbeRequested(requests) => {
                Task::batch(requests.into_iter().map(|(job_id, input)| {
                    Task::perform(
                        runtime::probe_bounded(job_id.clone(), input),
                        move |result| {
                            Message::Queue(queue::Message::ProbeCompleted {
                                job_id,
                                result: share_error(result),
                            })
                        },
                    )
                }))
            }
            queue::Action::ResolveOutput { job_id, requested } => Task::perform(
                runtime::resolve_output(job_id.clone(), requested),
                move |result| {
                    Message::Queue(queue::Message::OutputResolved {
                        job_id,
                        result: share_error(result),
                    })
                },
            ),
            queue::Action::RipRequested {
                job_id,
                request,
                initial_progress,
            } => {
                if !matches!(self.dependency_state, DependencyState::Ready(_))
                    || !self.output_settings.has_folder()
                {
                    return Task::none();
                }
                self.error = None;
                let _ = self.progress.update(progress::Message::Started {
                    job_id: job_id.clone(),
                    request: request.clone(),
                    progress: initial_progress,
                });
                let (handle, signal) = cancellation_pair();
                self.active_cancellation = Some((job_id.clone(), handle));
                rip_task(job_id, request, signal)
            }
            queue::Action::QueueFinished { summary, error } => {
                if self.exit_after_queue {
                    self.exit_after_queue = false;
                    return iced::exit();
                }
                let body = format!(
                    "{} completed, {} failed, {} skipped, {} cancelled.",
                    summary.completed, summary.failed, summary.skipped, summary.cancelled
                );
                if let Some(error) = error {
                    self.error = Some(error);
                    self.notifications
                        .failure("Queue finished with errors", body)
                        .map(Message::Notifications)
                } else if summary.failed > 0 {
                    self.error = Some(format!(
                        "{} queue job{} failed. Select a failed job for details.",
                        summary.failed,
                        if summary.failed == 1 { "" } else { "s" }
                    ));
                    self.notifications
                        .failure("Queue finished with errors", body)
                        .map(Message::Notifications)
                } else if summary.cancelled > 0 {
                    self.error = None;
                    self.notifications
                        .warning("Queue cancelled", body)
                        .map(Message::Notifications)
                } else {
                    self.notifications
                        .success("Queue complete", body)
                        .map(Message::Notifications)
                }
            }
            queue::Action::None
            | queue::Action::FilePickerOpened
            | queue::Action::IntakeRequested(_)
            | queue::Action::IntakeAccepted(_)
            | queue::Action::ProbeFailed(_) => Task::none(),
        }
    }

    fn request_cancellation(&mut self, job_id: &JobId) -> Task<Message> {
        let action = self.queue.request_cancel(job_id);
        if let Some((active, handle)) = &self.active_cancellation
            && active == job_id
        {
            let first_request = handle.cancel();
            tracing::info!(
                job_id = job_id.0,
                first_request,
                "audio extraction cancellation requested"
            );
        }
        self.handle_queue_action(action)
    }
}

fn rip_task(
    job_id: crate::model::job::JobId,
    request: crate::ffmpeg::RipRequest,
    cancellation: crate::ffmpeg::CancellationSignal,
) -> Task<Message> {
    let stream = iced::stream::channel(32, async move |mut output| {
        let (progress_sender, mut progress_receiver) = tokio::sync::mpsc::channel(16);
        let rip =
            runtime::rip_with_progress(job_id.clone(), request, progress_sender, cancellation);
        tokio::pin!(rip);
        let mut progress_open = true;

        loop {
            tokio::select! {
                progress = progress_receiver.recv(), if progress_open => {
                    match progress {
                        Some(progress) => {
                            let _ = output.try_send(Message::RipProgress {
                                job_id: job_id.clone(),
                                progress,
                            });
                        }
                        None => progress_open = false,
                    }
                }
                result = &mut rip => {
                    let _ = output
                        .send(Message::RipCompleted {
                            job_id,
                            result: share_error(result),
                        })
                        .await;
                    break;
                }
            }
        }
    });

    Task::run(stream, std::convert::identity)
}

#[cfg(test)]
mod tests {
    use std::{path::PathBuf, sync::Arc, time::Duration};

    use crate::{
        ffmpeg::{Dependencies, RipOutcome, RipTermination},
        model::media::{AudioMetadata, MediaInfo},
        model::{
            encoding::{ChannelMode, Mp3Bitrate, SampleRate},
            job::{JobId, JobStatus},
        },
    };

    use super::*;

    fn task_error(message: &str) -> Arc<crate::Error> {
        Arc::new(std::io::Error::other(message).into())
    }

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

    fn completed() -> RipTermination {
        RipTermination::Completed(RipOutcome {
            status: "success".into(),
        })
    }

    fn enqueue(state: &mut Demux, input: &str) -> JobId {
        let input = PathBuf::from(input);
        state.output_settings.set_default_from_input(&input);
        let output = state.output_settings.output_path(&input);
        let _ = state.queue.enqueue_many(
            vec![(
                crate::app::intake::AcceptedInput {
                    path: input,
                    size: 42,
                },
                output,
            )],
            state.output_settings.options(),
        );
        state.queue.selected().unwrap().id.clone()
    }

    fn start(state: &mut Demux, job_id: &JobId) {
        let _ = state.queue.update(queue::Message::ProbeCompleted {
            job_id: job_id.clone(),
            result: Ok(metadata()),
        });
        let _ = state.queue.update(queue::Message::StartQueue);
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
            result: Err(task_error("No audio stream was found")),
        }));

        assert!(matches!(
            state.queue.selected_status(),
            Some(JobStatus::Failed(_))
        ));
        assert_eq!(state.error.as_deref(), Some("No audio stream was found"));
    }

    #[test]
    fn starting_an_eligible_queue_clears_an_earlier_probe_error() {
        let mut state = Demux::default();
        let ready = enqueue(&mut state, "/videos/ready.mp4");
        let _ = enqueue(&mut state, "/videos/silent.mp4");
        let failed = JobId::new(2);
        let _ = state.update(Message::Queue(queue::Message::ProbeCompleted {
            job_id: ready,
            result: Ok(metadata()),
        }));
        let _ = state.update(Message::Queue(queue::Message::ProbeCompleted {
            job_id: failed,
            result: Err(task_error("No audio stream was found")),
        }));
        assert!(state.error.is_some());

        let _ = state.update_queue(queue::Message::StartQueue);

        assert!(state.error.is_none());
    }

    #[test]
    fn successful_queue_adds_a_completion_toast() {
        let mut state = Demux::default();
        let job_id = enqueue(&mut state, "/videos/example.mp4");
        start(&mut state, &job_id);

        let _ = state.update(Message::RipCompleted {
            job_id,
            result: Ok(completed()),
        });

        assert_eq!(state.notifications.len(), 1);
    }

    #[test]
    fn failed_queue_adds_a_danger_toast_and_keeps_a_summary_error() {
        let mut state = Demux::default();
        let job_id = enqueue(&mut state, "/videos/example.mp4");
        start(&mut state, &job_id);

        let _ = state.update(Message::RipCompleted {
            job_id,
            result: Err(task_error("FFmpeg exited with status 1")),
        });

        assert_eq!(state.notifications.len(), 1);
        assert_eq!(
            state.error.as_deref(),
            Some("1 queue job failed. Select a failed job for details.")
        );
    }

    #[test]
    fn selected_file_action_wires_output_settings_into_the_queue() {
        let mut state = Demux::default();

        let _ = state.update(Message::Queue(queue::Message::IntakeCompleted(
            crate::app::intake::IntakeResult {
                accepted: vec![crate::app::intake::AcceptedInput {
                    path: PathBuf::from("/videos/example.mp4"),
                    size: 42,
                }],
                rejected: Vec::new(),
            },
        )));

        let job = state.queue.selected().unwrap();
        assert_eq!(job.input, "/videos/example.mp4");
        assert_eq!(job.output, "/videos/example.mp3");
        assert!(matches!(job.status, JobStatus::Probing));
    }

    #[test]
    fn encoding_changes_update_every_editable_job_snapshot() {
        let mut state = Demux::default();
        let job_id = enqueue(&mut state, "/videos/example.mp4");
        let _ = state.update(Message::Queue(queue::Message::ProbeCompleted {
            job_id,
            result: Ok(metadata()),
        }));

        let _ = state.update(Message::OutputSettings(
            output_settings::Message::BitrateChanged(Mp3Bitrate::Kbps320),
        ));
        let _ = state.update(Message::OutputSettings(
            output_settings::Message::SampleRateChanged(SampleRate::Hz48000),
        ));
        let _ = state.update(Message::OutputSettings(
            output_settings::Message::ChannelsChanged(ChannelMode::Mono),
        ));

        let options = state.queue.selected().unwrap().options;
        assert_eq!(options.bitrate, Mp3Bitrate::Kbps320);
        assert_eq!(options.sample_rate, SampleRate::Hz48000);
        assert_eq!(options.channels, ChannelMode::Mono);
    }

    #[test]
    fn stale_rip_completion_does_not_create_a_notification() {
        let mut state = Demux::default();

        let _ = state.update(Message::RipCompleted {
            job_id: JobId::new(99),
            result: Ok(completed()),
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
            result: Ok(completed()),
        };

        let _ = state.update(completion());
        let _ = state.update(completion());

        assert_eq!(state.notifications.len(), 1);
    }

    #[test]
    fn close_during_extraction_requests_the_same_managed_cancellation_path() {
        let mut state = Demux::default();
        let job_id = enqueue(&mut state, "/videos/example.mp4");
        start(&mut state, &job_id);
        let (handle, _signal) = cancellation_pair();
        state.active_cancellation = Some((job_id, handle.clone()));

        let _ = state.update(Message::CloseRequested);

        assert!(state.exit_after_queue);
        assert!(handle.is_cancelled());
        assert!(matches!(
            state.queue.selected_status(),
            Some(JobStatus::Cancelling)
        ));
    }

    #[test]
    fn clean_cancellation_adds_a_warning_toast_without_a_persistent_error() {
        let mut state = Demux::default();
        let job_id = enqueue(&mut state, "/videos/example.mp4");
        start(&mut state, &job_id);
        let _ = state.queue.request_cancel(&job_id);

        let _ = state.update(Message::RipCompleted {
            job_id,
            result: Ok(RipTermination::Cancelled { forced: false }),
        });

        assert_eq!(state.notifications.len(), 1);
        assert!(state.error.is_none());
        assert!(matches!(
            state.queue.selected_status(),
            Some(JobStatus::Cancelled)
        ));
    }
}
