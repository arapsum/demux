use crate::{
    ffmpeg::{Dependencies, RipProgressEvent, RipTermination},
    model::job::JobId,
};

use super::TaskResult;
use super::{output_settings, progress, queue, toast};

#[derive(Debug, Clone)]
pub enum Message {
    DependenciesChecked(TaskResult<Dependencies>),
    PreferencesLoaded(TaskResult<crate::app::preferences::PreferenceDefaults>),
    PreferencesSaved(TaskResult<()>),
    Queue(queue::Message),
    OutputSettings(output_settings::Message),
    Progress(progress::Message),
    RipProgress {
        job_id: JobId,
        progress: RipProgressEvent,
    },
    RipCompleted {
        job_id: JobId,
        result: TaskResult<RipTermination>,
    },
    CloseRequested,
    Notifications(toast::Message),
}
