use crate::{
    ffmpeg::{Dependencies, RipOutcome},
    model::job::JobId,
};

use super::{output_settings, queue, toast};

#[derive(Debug, Clone)]
pub enum Message {
    DependenciesChecked(Result<Dependencies, String>),
    Queue(queue::Message),
    OutputSettings(output_settings::Message),
    RipCompleted {
        job_id: JobId,
        result: Result<RipOutcome, String>,
    },
    Notifications(toast::Message),
}
