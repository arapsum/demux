//! THESIS: Demux makes one media extraction legible from selection through outcome;
//! the bounded `FFmpeg` log keeps external process diagnostics visible without
//! turning the shell into a terminal.
//!
//! OWN-WORLD: Near-black neutral surfaces, one restrained violet action color,
//! fine borders, compact native controls, and explicit semantic status colors.
//!
//! STORY: Choose a video, confirm its discovered audio stream and destination,
//! start extraction, follow progress and diagnostics, then see Completed or a
//! recoverable failure.
//!
//! FIRST VIEWPORT: Product identity and utility actions lead into a 70/30
//! work-and-settings split; the queue owns the large left field, Progress owns
//! the primary action, and the bounded log spans the shell beneath the workspace.
//!
//! FORM: Reference-inherited desktop utility shell. Unreviewed and undocumented
//! is unfinished; this build ends with the finish review, the verdict, and DESIGN.md.
//!
//! ARCHITECTURE: Demux is the composition root. Independent GUI surfaces own
//! their local state, messages, initialization, update logic, and view. The root
//! maps child tasks and translates child actions when a workflow crosses surface
//! boundaries, including bounded `FFmpeg` log events.

mod about;
mod application_settings;
mod close_confirmation;
mod dialog;
mod drop_zone;
mod icon;
mod logs;
mod message;
mod output_settings;
mod presentation;
mod progress;
mod queue;
mod state;
mod style;
mod toast;
mod update;
mod view;
mod window_state;

use std::sync::Arc;

use iced::{Color, Subscription, Theme, event, keyboard, theme::Palette, window};

pub use self::{message::Message, state::Demux};

pub(crate) type TaskResult<T> = std::result::Result<T, Arc<crate::Error>>;

/// Converts a shared application result into a GUI task result.
///
/// # Parameters
///
/// - `result`: Application task result to share with the GUI.
///
/// # Returns
///
/// The successful value or the error wrapped for cheap task-message cloning.
///
/// # Errors
///
/// Returns an error when:
///
/// - `result` contains an application error.
fn share_error<T>(result: crate::Result<T>) -> TaskResult<T> {
    result.map_err(Arc::new)
}

/// Starts the desktop `iced` application.
///
/// # Returns
///
/// The result returned by the `iced` runtime after the window closes.
///
/// # Errors
///
/// Returns an error when:
///
/// - The window cannot be initialized.
/// - The selected renderer cannot be initialized.
pub fn run() -> iced::Result {
    iced::application(Demux::new, Demux::update, Demux::view)
        .title("Demux")
        .font(lucide_icons::LUCIDE_FONT_BYTES)
        .theme(app_theme)
        .subscription(subscription)
        .window(window::Settings {
            size: window_state::DEFAULT_WINDOW_SIZE,
            min_size: Some(window_state::MIN_WINDOW_SIZE),
            exit_on_close_request: false,
            ..window::Settings::default()
        })
        .run()
}

fn subscription(_state: &Demux) -> Subscription<Message> {
    event::listen_with(|event, status, _| match event {
        iced::Event::Window(event) => window_event(event),
        iced::Event::Keyboard(event) if status == event::Status::Ignored => keyboard_event(event),
        _ => None,
    })
}

fn window_event(event: window::Event) -> Option<Message> {
    match event {
        window::Event::FileHovered(path) => {
            tracing::trace!(path = %path.display(), "media path hovered over window");
            Some(Message::Queue(queue::Message::DropHoverChanged(true)))
        }
        window::Event::FilesHoveredLeft => {
            tracing::trace!("media paths left window");
            Some(Message::Queue(queue::Message::DropHoverChanged(false)))
        }
        window::Event::FileDropped(path) => {
            tracing::info!(path = %path.display(), "media path dropped into queue");
            Some(Message::Queue(queue::Message::PathsDropped(vec![path])))
        }
        window::Event::Resized(size) => Some(Message::WindowResized(size)),
        window::Event::Moved(position) => Some(Message::WindowMoved(position)),
        window::Event::CloseRequested => Some(Message::CloseRequested),
        _ => None,
    }
}

fn keyboard_event(event: keyboard::Event) -> Option<Message> {
    let keyboard::Event::KeyPressed { key, modifiers, .. } = event else {
        return None;
    };

    let command = modifiers.command();
    match key.as_ref() {
        keyboard::Key::Character("o" | "O") if command => {
            Some(Message::Shortcut(if modifiers.shift() {
                message::Shortcut::AddFolder
            } else {
                message::Shortcut::AddFiles
            }))
        }
        keyboard::Key::Named(keyboard::key::Named::Delete) => {
            Some(Message::Shortcut(message::Shortcut::RemoveSelected))
        }
        keyboard::Key::Named(keyboard::key::Named::Enter) if command => {
            Some(Message::Shortcut(message::Shortcut::StartQueue))
        }
        keyboard::Key::Named(keyboard::key::Named::Space) => {
            Some(Message::Shortcut(message::Shortcut::TogglePause))
        }
        keyboard::Key::Named(keyboard::key::Named::Escape) => {
            Some(Message::Shortcut(message::Shortcut::Dismiss))
        }
        _ => None,
    }
}

fn theme() -> Theme {
    Theme::custom(
        "Demux",
        Palette {
            background: Color::from_rgb(0.055, 0.06, 0.075),
            text: Color::from_rgb(0.92, 0.93, 0.96),
            primary: Color::from_rgb(0.43, 0.36, 0.96),
            success: Color::from_rgb(0.35, 0.78, 0.57),
            danger: Color::from_rgb(0.94, 0.39, 0.42),
            warning: Color::from_rgb(0.96, 0.68, 0.30),
        },
    )
}

fn app_theme(_state: &Demux) -> Theme {
    theme()
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    #[test]
    fn dropped_file_window_event_enters_queue_intake() {
        let path = PathBuf::from("concert.mkv");
        let message = window_event(window::Event::FileDropped(path.clone()));

        assert!(matches!(
            message,
            Some(Message::Queue(queue::Message::PathsDropped(paths))) if paths == vec![path]
        ));
    }

    #[test]
    fn file_hover_window_events_update_drop_feedback() {
        let hovered = window_event(window::Event::FileHovered(PathBuf::from("concert.mkv")));
        let left = window_event(window::Event::FilesHoveredLeft);

        assert!(matches!(
            hovered,
            Some(Message::Queue(queue::Message::DropHoverChanged(true)))
        ));
        assert!(matches!(
            left,
            Some(Message::Queue(queue::Message::DropHoverChanged(false)))
        ));
    }

    #[test]
    fn close_requests_are_forwarded_to_the_application_shutdown_path() {
        assert!(matches!(
            window_event(window::Event::CloseRequested),
            Some(Message::CloseRequested)
        ));
    }
}
