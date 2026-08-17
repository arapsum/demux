use crate::{
    ffmpeg::{CancellationHandle, DependencyState, PauseControlHandle},
    model::job::JobId,
};

use super::about::About;
use super::application_settings::ApplicationSettings;
use super::close_confirmation::CloseConfirmation;
use super::logs::Logs;
use super::output_settings::OutputSettings;
use super::progress::Progress;
use super::queue::Queue;
use super::toast::Notifications;
use super::window_state::WindowState;

#[derive(Debug)]
pub struct Demux {
    pub(crate) dependency_state: DependencyState,
    pub(crate) queue: Queue,
    pub(crate) output_settings: OutputSettings,
    pub(crate) progress: Progress,
    pub(crate) logs: Logs,
    pub(crate) error: Option<String>,
    pub(crate) notifications: Notifications,
    pub(crate) application_settings: ApplicationSettings,
    pub(crate) about: About,
    pub(crate) close_confirmation: CloseConfirmation,
    pub(crate) window: WindowState,
    pub(crate) active_cancellation: Option<(JobId, CancellationHandle)>,
    pub(crate) active_pause_control: Option<(JobId, PauseControlHandle)>,
    pub(crate) exit_after_queue: bool,
    pub(crate) pending_window_revision: Option<u64>,
    pub(crate) exit_after_preferences: bool,
}

impl Default for Demux {
    fn default() -> Self {
        Self {
            dependency_state: DependencyState::Checking,
            queue: Queue::new(),
            output_settings: OutputSettings::new(),
            progress: Progress::new(),
            logs: Logs::new(),
            error: None,
            notifications: Notifications::new(),
            application_settings: ApplicationSettings::new(),
            about: About::new(),
            close_confirmation: CloseConfirmation::default(),
            window: WindowState::default(),
            active_cancellation: None,
            active_pause_control: None,
            exit_after_queue: false,
            pending_window_revision: None,
            exit_after_preferences: false,
        }
    }
}

impl Demux {
    pub(crate) fn refresh_output_path(&mut self) {
        let settings = &self.output_settings;
        self.queue.set_output_paths(|job| {
            settings.output_path(
                std::path::Path::new(&job.input),
                job.source_hierarchy.as_ref(),
            )
        });
    }

    pub(crate) fn refresh_encoding_options(&mut self) {
        self.queue.set_options(self.output_settings.options());
    }

    pub(crate) fn can_start(&self) -> bool {
        matches!(self.dependency_state, DependencyState::Ready(_))
            && self.queue.can_start()
            && self.output_settings.has_folder()
    }
}
