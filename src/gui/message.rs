use crate::{
    ffmpeg::{Dependencies, RipOutcome},
    model::job::JobId,
};

use super::TaskResult;
use super::{output_settings, queue, toast};

#[derive(Debug, Clone)]
pub enum Message {
    DependenciesChecked(TaskResult<Dependencies>),
    Queue(queue::Message),
    OutputSettings(output_settings::Message),
    RipCompleted {
        job_id: JobId,
        result: TaskResult<RipOutcome>,
    },
    Notifications(toast::Message),
}
