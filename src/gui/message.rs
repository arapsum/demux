use std::path::PathBuf;

use crate::{
    ffmpeg::{Dependencies, RipOutcome},
    model::{job::JobId, media::MediaInfo},
};

use super::toast::ToastId;

#[derive(Debug, Clone)]
pub enum Message {
    DependenciesChecked(Result<Dependencies, String>),
    AddFile,
    FileSelected(Option<PathBuf>),
    ProbeCompleted {
        job_id: JobId,
        result: Result<MediaInfo, String>,
    },
    OutputFolderChanged(String),
    BrowseOutputFolder,
    OutputFolderSelected(Option<PathBuf>),
    StartRipping,
    RipCompleted {
        job_id: JobId,
        result: Result<RipOutcome, String>,
    },
    DismissToast(ToastId),
}
