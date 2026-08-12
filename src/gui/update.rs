use std::{
    path::{Path, PathBuf},
    time::Duration,
};

use iced::Task;
use tracing::{Instrument, info_span};

use crate::{
    ffmpeg::{
        self, DependencyState, FfmpegAudioRipper, RipOutcome, RipRequest, TokioProcessRunner,
    },
    ffprobe,
    model::media::MediaInfo,
};

use super::{
    message::Message,
    state::Demux,
    toast::{Toast, ToastId},
};

const SUCCESS_TOAST_DURATION: Duration = Duration::from_secs(6);
const FAILURE_TOAST_DURATION: Duration = Duration::from_secs(10);

impl Demux {
    pub fn new() -> (Self, Task<Message>) {
        (
            Self::default(),
            Task::perform(check_dependencies(), Message::DependenciesChecked),
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
            Message::AddFile => {
                self.error = None;
                self.picking_file = true;
                Task::perform(pick_video_file(), Message::FileSelected)
            }
            Message::FileSelected(path) => {
                self.picking_file = false;
                let Some(path) = path else {
                    return Task::none();
                };

                self.error = None;
                let job_id = self.add_job(path.clone());
                Task::perform(probe_file(job_id.clone(), path), move |result| {
                    Message::ProbeCompleted { job_id, result }
                })
            }
            Message::ProbeCompleted { job_id, result } => {
                match result {
                    Ok(metadata) => {
                        if let Some(job) = self.job_mut(&job_id) {
                            job.record_metadata(metadata);
                        }
                    }
                    Err(message) => {
                        if let Some(job) = self.job_mut(&job_id) {
                            job.fail(message.clone());
                        }
                        self.error = Some(message);
                    }
                }
                Task::none()
            }
            Message::OutputFolderChanged(output_folder) => {
                self.output_folder = output_folder;
                self.refresh_output_path();
                Task::none()
            }
            Message::BrowseOutputFolder => {
                Task::perform(pick_output_folder(), Message::OutputFolderSelected)
            }
            Message::OutputFolderSelected(path) => {
                if let Some(path) = path {
                    self.output_folder = path.to_string_lossy().into_owned();
                    self.refresh_output_path();
                }
                Task::none()
            }
            Message::StartRipping => {
                if !self.can_start() {
                    return Task::none();
                }

                self.error = None;
                let Some(job) = self.selected_job_mut() else {
                    return Task::none();
                };
                job.start_ripping();

                let job_id = job.id.clone();
                let request = RipRequest::new(job.input.clone(), job.output.clone());
                Task::perform(rip_file(job_id.clone(), request), move |result| {
                    Message::RipCompleted { job_id, result }
                })
            }
            Message::RipCompleted { job_id, result } => {
                let (toast, duration) = match result {
                    Ok(_) => {
                        let output_name = self
                            .jobs
                            .iter()
                            .find(|job| job.id == job_id)
                            .and_then(|job| Path::new(&job.output).file_name())
                            .and_then(|name| name.to_str())
                            .unwrap_or("Your MP3")
                            .to_owned();
                        if let Some(job) = self.job_mut(&job_id) {
                            job.complete();
                        }
                        (
                            Toast::success(
                                "Ripping complete",
                                format!("{output_name} is ready in your output folder."),
                            ),
                            SUCCESS_TOAST_DURATION,
                        )
                    }
                    Err(message) => {
                        if let Some(job) = self.job_mut(&job_id) {
                            job.fail(message.clone());
                        }
                        self.error = Some(message);
                        (
                            Toast::danger(
                                "Ripping failed",
                                "Review the error message, then try the extraction again.",
                            ),
                            FAILURE_TOAST_DURATION,
                        )
                    }
                };

                let toast_id = self.push_toast(toast);
                Task::perform(dismiss_toast_after(toast_id, duration), |toast_id| {
                    Message::DismissToast(toast_id)
                })
            }
            Message::DismissToast(toast_id) => {
                self.dismiss_toast(toast_id);
                Task::none()
            }
        }
    }
}

async fn dismiss_toast_after(toast_id: ToastId, duration: Duration) -> ToastId {
    tokio::time::sleep(duration).await;
    toast_id
}

async fn check_dependencies() -> Result<ffmpeg::Dependencies, String> {
    tokio::task::spawn_blocking(ffmpeg::detect_dependencies)
        .await
        .map_err(|error| error.to_string())?
        .map_err(|error| error.to_string())
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

async fn pick_output_folder() -> Option<PathBuf> {
    rfd::AsyncFileDialog::new()
        .set_title("Choose an output folder")
        .pick_folder()
        .await
        .map(|folder| folder.path().to_path_buf())
}

async fn probe_file(job_id: crate::model::job::JobId, input: PathBuf) -> Result<MediaInfo, String> {
    let span = info_span!("gui_probe_job", job_id = job_id.0);
    async move {
        ffprobe::inspect(&input.to_string_lossy())
            .await
            .map_err(|error| error.to_string())
    }
    .instrument(span)
    .await
}

async fn rip_file(
    job_id: crate::model::job::JobId,
    request: RipRequest,
) -> Result<RipOutcome, String> {
    let span = info_span!("gui_rip_job", job_id = job_id.0);
    async move {
        FfmpegAudioRipper::<TokioProcessRunner>::default()
            .rip(&request)
            .await
            .map_err(|error| error.to_string())
    }
    .instrument(span)
    .await
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use crate::{
        ffmpeg::Dependencies,
        model::job::JobStatus,
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

    #[test]
    fn probe_completion_makes_selected_job_ready() {
        let mut state = Demux::default();
        state.dependency_state = DependencyState::Ready(Dependencies {
            ffmpeg_version: "ffmpeg test".into(),
            ffprobe_version: "ffprobe test".into(),
        });
        let job_id = state.add_job(PathBuf::from("/videos/example.mp4"));

        let _ = state.update(Message::ProbeCompleted {
            job_id,
            result: Ok(metadata()),
        });

        assert!(matches!(
            state.selected_job().map(|job| &job.status),
            Some(JobStatus::Ready)
        ));
        assert!(state.can_start());
    }

    #[test]
    fn probe_failure_is_visible_in_job_and_error_area() {
        let mut state = Demux::default();
        let job_id = state.add_job(PathBuf::from("/videos/silent.mp4"));

        let _ = state.update(Message::ProbeCompleted {
            job_id,
            result: Err("No audio stream was found".into()),
        });

        assert!(matches!(
            state.selected_job().map(|job| &job.status),
            Some(JobStatus::Failed(_))
        ));
        assert_eq!(state.error.as_deref(), Some("No audio stream was found"));
    }

    #[test]
    fn successful_rip_adds_a_completion_toast() {
        let mut state = Demux::default();
        let job_id = state.add_job(PathBuf::from("/videos/example.mp4"));
        state.job_mut(&job_id).unwrap().start_ripping();

        let _ = state.update(Message::RipCompleted {
            job_id,
            result: Ok(RipOutcome {
                status: "success".into(),
            }),
        });

        assert_eq!(state.toasts.len(), 1);
        assert_eq!(state.toasts[0].title, "Ripping complete");
        assert!(state.toasts[0].body.contains("example.mp3"));
        assert_eq!(
            state.toasts[0].status,
            super::super::toast::ToastStatus::Success
        );
    }

    #[test]
    fn failed_rip_adds_a_danger_toast_and_keeps_the_error() {
        let mut state = Demux::default();
        let job_id = state.add_job(PathBuf::from("/videos/example.mp4"));
        state.job_mut(&job_id).unwrap().start_ripping();

        let _ = state.update(Message::RipCompleted {
            job_id,
            result: Err("FFmpeg exited with status 1".into()),
        });

        assert_eq!(state.toasts.len(), 1);
        assert_eq!(state.toasts[0].title, "Ripping failed");
        assert_eq!(
            state.toasts[0].status,
            super::super::toast::ToastStatus::Danger
        );
        assert_eq!(state.error.as_deref(), Some("FFmpeg exited with status 1"));
    }
}
