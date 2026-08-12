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

mod message;
mod output_settings;
mod state;
mod style;
mod toast;
mod update;
mod view;

use iced::{Color, Size, Theme, theme::Palette, window};

pub use self::{message::Message, state::Demux};

pub fn run() -> iced::Result {
    iced::application(Demux::new, Demux::update, Demux::view)
        .title("Demux")
        .theme(app_theme)
        .window(window::Settings {
            size: Size::new(1_180.0, 760.0),
            min_size: Some(Size::new(860.0, 600.0)),
            ..window::Settings::default()
        })
        .run()
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
