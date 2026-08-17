use crate::{
    ffmpeg::{Dependencies, FfmpegLogEvent, PauseControlEvent, RipProgressEvent, RipTermination},
    model::job::JobId,
};

use super::TaskResult;
use super::{logs, output_settings, progress, queue, toast};

#[derive(Debug, Clone)]
pub enum Message {
    DependenciesChecked(TaskResult<Dependencies>),
    PreferencesLoaded(TaskResult<crate::app::preferences::PreferenceDefaults>),
    PreferencesSaved(TaskResult<()>),
    Queue(queue::Message),
    OutputSettings(output_settings::Message),
    Progress(progress::Message),
    Logs(logs::Message),
    RipLogs {
        job_id: JobId,
        events: Vec<FfmpegLogEvent>,
    },
    RipStarted {
        job_id: JobId,
        filename: String,
    },
    RipFinished {
        job_id: JobId,
        status: logs::JobTerminalStatus,
    },
    RipControl {
        job_id: JobId,
        event: PauseControlEvent,
    },
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
