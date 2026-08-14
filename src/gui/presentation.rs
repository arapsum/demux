use std::path::Path;

use iced::Color;

use crate::{
    ffmpeg::DependencyState,
    model::job::{JobId, JobStatus, RipJob},
};

use super::style::{ACCENT, DANGER, SUCCESS, TEXT_MUTED, WARNING};

pub(crate) struct DependencyPresentation {
    pub(crate) label: &'static str,
    pub(crate) color: Color,
}

impl From<&DependencyState> for DependencyPresentation {
    fn from(state: &DependencyState) -> Self {
        match state {
            DependencyState::Checking => Self {
                label: "Checking FFmpeg…",
                color: WARNING,
            },
            DependencyState::Ready(_) => Self {
                label: "FFmpeg ready",
                color: SUCCESS,
            },
            DependencyState::Missing { .. } | DependencyState::Failed { .. } => Self {
                label: "FFmpeg unavailable",
                color: DANGER,
            },
        }
    }
}

pub(crate) struct StatusPresentation {
    pub(crate) label: &'static str,
    pub(crate) color: Color,
}

impl From<&JobStatus> for StatusPresentation {
    fn from(status: &JobStatus) -> Self {
        match status {
            JobStatus::Pending => Self {
                label: "Pending",
                color: TEXT_MUTED,
            },
            JobStatus::Probing => Self {
                label: "Probing…",
                color: WARNING,
            },
            JobStatus::Ready => Self {
                label: "Ready",
                color: SUCCESS,
            },
            JobStatus::Queued => Self {
                label: "Queued",
                color: TEXT_MUTED,
            },
            JobStatus::Ripping => Self {
                label: "Ripping…",
                color: ACCENT,
            },
            JobStatus::Cancelling => Self {
                label: "Cancelling…",
                color: WARNING,
            },
            JobStatus::Completed => Self {
                label: "Completed",
                color: SUCCESS,
            },
            JobStatus::Failed(_) => Self {
                label: "Failed",
                color: DANGER,
            },
            JobStatus::Cancelled => Self {
                label: "Cancelled",
                color: TEXT_MUTED,
            },
            JobStatus::Skipped(_) => Self {
                label: "Skipped",
                color: TEXT_MUTED,
            },
        }
    }
}

pub(crate) struct JobPresentation {
    pub(crate) id: JobId,
    pub(crate) filename: String,
    pub(crate) duration: String,
    pub(crate) output_details: String,
    pub(crate) size: String,
    pub(crate) status: StatusPresentation,
    pub(crate) terminal_detail: Option<String>,
}

impl From<&RipJob> for JobPresentation {
    fn from(job: &RipJob) -> Self {
        let filename = Path::new(&job.input)
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or(&job.input)
            .to_owned();
        let duration = job
            .metadata
            .as_ref()
            .map(|metadata| format_duration(metadata.duration))
            .unwrap_or_else(|| "—".into());
        Self {
            id: job.id.clone(),
            filename,
            duration,
            output_details: format!(
                "{} ({} kbps)",
                job.options.format,
                job.options.bitrate.kbps()
            ),
            size: job.input_size.map_or_else(|| "—".into(), format_size),
            status: StatusPresentation::from(&job.status),
            terminal_detail: match &job.status {
                JobStatus::Failed(message) | JobStatus::Skipped(message) => Some(message.clone()),
                _ => None,
            },
        }
    }
}

fn format_duration(duration: std::time::Duration) -> String {
    let seconds = duration.as_secs();
    let hours = seconds / 3_600;
    let minutes = (seconds % 3_600) / 60;
    let seconds = seconds % 60;

    if hours > 0 {
        format!("{hours:02}:{minutes:02}:{seconds:02}")
    } else {
        format!("{minutes:02}:{seconds:02}")
    }
}

pub(crate) fn format_size(bytes: u64) -> String {
    const MIB: f64 = 1_048_576.0;
    const GIB: f64 = 1_073_741_824.0;
    if bytes >= 1_073_741_824 {
        format!("{:.2} GB", bytes as f64 / GIB)
    } else {
        format!("{:.1} MB", bytes as f64 / MIB)
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use crate::model::{
        encoding::{Mp3Bitrate, RipOptions},
        job::JobId,
        media::{AudioMetadata, MediaInfo},
    };

    use super::*;

    #[test]
    fn job_metadata_is_prepared_for_display_in_one_place() {
        let mut job = RipJob::new(
            JobId::new(1),
            "/videos/example.mp4".into(),
            "/music/example.mp3".into(),
        );
        job.record_metadata(MediaInfo {
            duration: Duration::from_secs(3_661),
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
        });
        job.set_options(RipOptions {
            bitrate: Mp3Bitrate::Kbps320,
            ..RipOptions::default()
        });

        let presentation = JobPresentation::from(&job);

        assert_eq!(presentation.filename, "example.mp4");
        assert_eq!(presentation.duration, "01:01:01");
        assert_eq!(presentation.output_details, "MP3 (320 kbps)");
        assert_eq!(presentation.status.label, "Ready");
    }
}
