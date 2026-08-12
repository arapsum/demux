use crate::ffmpeg::DependencyState;

use super::output_settings::OutputSettings;
use super::queue::Queue;
use super::toast::Notifications;

#[derive(Debug)]
pub struct Demux {
    pub(crate) dependency_state: DependencyState,
    pub(crate) queue: Queue,
    pub(crate) output_settings: OutputSettings,
    pub(crate) error: Option<String>,
    pub(crate) notifications: Notifications,
}

impl Default for Demux {
    fn default() -> Self {
        Self {
            dependency_state: DependencyState::Checking,
            queue: Queue::new(),
            output_settings: OutputSettings::new(),
            error: None,
            notifications: Notifications::new(),
        }
    }
}

impl Demux {
    pub(crate) fn refresh_output_path(&mut self) {
        let Some(input) = self.queue.selected_input().map(ToOwned::to_owned) else {
            return;
        };
        self.queue
            .set_selected_output(self.output_settings.output_path(&input));
    }

    pub(crate) fn can_start(&self) -> bool {
        matches!(self.dependency_state, DependencyState::Ready(_))
            && self.queue.is_ready()
            && self.output_settings.has_folder()
    }
}
