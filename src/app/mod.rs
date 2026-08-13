mod cli;
pub(crate) mod intake;
mod output;
pub(crate) mod preferences;
pub(crate) mod queue_runner;
pub(crate) mod runtime;
mod services;
mod workflow;

use crate::{
    ffmpeg::DependencyState,
    model::{encoding::RipOptions, job::RipJob, queue::JobQueue},
};

pub use self::{
    cli::Cli,
    services::{
        AudioRipper, DependencyChecker, FfprobeMediaProbe, MediaProbe, SystemAudioRipper,
        SystemDependencyChecker,
    },
    workflow::{RipWorkflow, WorkflowEvent, WorkflowReporter, WorkflowStage},
};

/// Stores application state independently of any user interface or process
/// implementation.
#[derive(Debug)]
pub struct App {
    dependency_state: DependencyState,
    jobs: JobQueue,
}

impl App {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            dependency_state: DependencyState::Checking,
            jobs: JobQueue::new(),
        }
    }

    #[must_use]
    pub const fn dependency_state(&self) -> &DependencyState {
        &self.dependency_state
    }

    #[must_use]
    pub fn current_job(&self) -> Option<&RipJob> {
        self.jobs.current()
    }

    pub(crate) fn set_dependency_state(&mut self, state: DependencyState) {
        self.dependency_state = state;
    }

    pub(crate) fn create_job(
        &mut self,
        input: String,
        output: String,
        options: RipOptions,
    ) -> RipJob {
        self.jobs.create(input, output, options)
    }

    pub(crate) fn finish_job(&mut self, job: RipJob) {
        self.jobs.finish(job);
    }
}

impl Default for App {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn starts_with_dependencies_pending_and_no_job() {
        let app = App::new();

        assert_eq!(app.dependency_state(), &DependencyState::Checking);
        assert!(app.current_job().is_none());
    }
}
