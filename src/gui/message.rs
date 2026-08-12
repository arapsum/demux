use std::path::PathBuf;

use crate::{
    ffmpeg::{Dependencies, RipOutcome},
    model::{job::JobId, media::MediaInfo},
};

use super::{output_settings, toast};

#[derive(Debug, Clone)]
pub enum Message {
    DependenciesChecked(Result<Dependencies, String>),
    AddFile,
    FileSelected(Option<PathBuf>),
    ProbeCompleted {
        job_id: JobId,
        result: Result<MediaInfo, String>,
    },
    OutputSettings(output_settings::Message),
    RipCompleted {
        job_id: JobId,
        result: Result<RipOutcome, String>,
    },
    Notifications(toast::Message),
}
