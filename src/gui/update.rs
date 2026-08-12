use std::path::PathBuf;

use iced::Task;
use tracing::{Instrument, info_span};

use crate::{
    ffmpeg::{
        self, DependencyState, FfmpegAudioRipper, RipOutcome, RipRequest, TokioProcessRunner,
    },
    ffprobe,
    model::media::MediaInfo,
};

use super::{message::Message, state::Demux};

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
                match result {
                    Ok(_) => {
                        if let Some(job) = self.job_mut(&job_id) {
                            job.complete();
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
        }
    }
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
}
