//! THESIS: Demux makes one media extraction legible from selection through outcome;
//! the shell avoids controls for engine capabilities that do not exist yet.
//!
//! OWN-WORLD: Near-black neutral surfaces, one restrained violet action color,
//! fine borders, compact native controls, and explicit semantic status colors.
//!
//! STORY: Choose a video, confirm its discovered audio stream and destination,
//! start extraction, then see Completed or a recoverable failure.
//!
//! FIRST VIEWPORT: Product identity leads into a 70/30 work-and-settings split;
//! the queue owns the large left field and the primary action anchors the right.
//!
//! FORM: Reference-inherited desktop utility shell. Unreviewed and undocumented
//! is unfinished; this build ends with the finish review, the verdict, and DESIGN.md.
//!
//! ARCHITECTURE: Demux is the composition root. Independent GUI surfaces own
//! their local state, messages, initialization, update logic, and view. The root
//! maps child tasks and translates child actions when a workflow crosses surface
//! boundaries.

mod drop_zone;
mod icon;
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

use std::sync::Arc;

use iced::{Color, Size, Subscription, Theme, event, theme::Palette, window};

pub use self::{message::Message, state::Demux};

pub(crate) type TaskResult<T> = std::result::Result<T, Arc<crate::Error>>;

fn share_error<T>(result: crate::Result<T>) -> TaskResult<T> {
    result.map_err(Arc::new)
}

pub fn run() -> iced::Result {
    iced::application(Demux::new, Demux::update, Demux::view)
        .title("Demux")
        .font(lucide_icons::LUCIDE_FONT_BYTES)
        .theme(app_theme)
        .subscription(subscription)
        .window(window::Settings {
            size: Size::new(1_180.0, 760.0),
            min_size: Some(Size::new(860.0, 600.0)),
            ..window::Settings::default()
        })
        .run()
}

fn subscription(_state: &Demux) -> Subscription<Message> {
    event::listen_with(|event, _, _| match event {
        iced::Event::Window(event) => window_event(event),
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
}
