use iced::font::Weight;
use iced::widget::{button, column, container, row, space, svg, text};
use iced::{Element, Fill, Font, Length, Padding};

use super::{
    icon,
    style::{ICON_MUTED, TEXT_MUTED, dialog_panel, secondary_action},
};

const PROJECT_URL: &str = "https://github.com/arapsum/demux";
const FFMPEG_URL: &str = "https://ffmpeg.org/";
const LICENSE_URL: &str = "https://www.gnu.org/licenses/gpl-3.0.html";

/// Owns visibility and link actions for the About dialog.
#[derive(Debug)]
pub struct About {
    open: bool,
}

/// Events produced by controls in the About dialog.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Message {
    Open,
    Close,
    OpenProject,
    OpenFfmpeg,
    OpenLicense,
}

/// Trusted external resources exposed by the About dialog.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Link {
    Project,
    Ffmpeg,
    License,
}

/// Effects requested by the About dialog after handling a message.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    None,
    OpenLink(Link),
}

impl About {
    pub(crate) const fn new() -> Self {
        Self { open: false }
    }

    pub(crate) const fn update(&mut self, message: Message) -> Action {
        match message {
            Message::Open => {
                self.open = true;
                Action::None
            }
            Message::Close => {
                self.open = false;
                Action::None
            }
            Message::OpenProject => Action::OpenLink(Link::Project),
            Message::OpenFfmpeg => Action::OpenLink(Link::Ffmpeg),
            Message::OpenLicense => Action::OpenLink(Link::License),
        }
    }

    pub(crate) const fn is_open(&self) -> bool {
        self.open
    }

    pub(crate) fn view(&self) -> Option<Element<'_, Message>> {
        if !self.open {
            return None;
        }

        Some(
            container(
                column![
                    row![
                        text("About Demux").size(20).font(Font {
                            weight: Weight::Semibold,
                            ..Font::default()
                        }),
                        space::horizontal(),
                        button(icon::close(ICON_MUTED))
                            .padding(8)
                            .style(secondary_action)
                            .on_press(Message::Close),
                    ]
                    .align_y(iced::Alignment::Center),
                    svg(svg::Handle::from_memory(include_bytes!(
                        "../../assets/demux-logo.svg"
                    )))
                    .width(Length::Fixed(250.0))
                    .height(Length::Fixed(65.0)),
                    text("Extract clean audio from video files with FFmpeg.")
                        .size(14)
                        .color(TEXT_MUTED),
                    container(
                        column![
                            row![
                                text("Version").size(12).color(TEXT_MUTED),
                                space::horizontal(),
                                text(env!("CARGO_PKG_VERSION")).size(12)
                            ],
                            row![
                                text("Authors").size(12).color(TEXT_MUTED),
                                space::horizontal(),
                                text(env!("CARGO_PKG_AUTHORS")).size(12)
                            ],
                            row![
                                text("License").size(12).color(TEXT_MUTED),
                                space::horizontal(),
                                text("GPL-3.0-only").size(12)
                            ],
                        ]
                        .spacing(6),
                    )
                    .width(Fill)
                    .padding(14)
                    .style(super::style::inset_panel),
                    column![
                        text("Links").size(14).font(Font {
                            weight: Weight::Semibold,
                            ..Font::default()
                        }),
                        link_button("Demux project", Message::OpenProject),
                        link_button("FFmpeg", Message::OpenFfmpeg),
                        link_button("GNU GPL v3.0", Message::OpenLicense),
                    ]
                    .spacing(7),
                ]
                .spacing(14),
            )
            .width(460)
            .padding(22)
            .style(dialog_panel)
            .into(),
        )
    }

    pub(crate) const fn url(link: Link) -> &'static str {
        match link {
            Link::Project => PROJECT_URL,
            Link::Ffmpeg => FFMPEG_URL,
            Link::License => LICENSE_URL,
        }
    }
}

fn link_button(label: &str, message: Message) -> Element<'_, Message> {
    button(
        row![
            text(label).size(13),
            space::horizontal(),
            icon::external_link(ICON_MUTED)
        ]
        .align_y(iced::Alignment::Center),
    )
    .width(Fill)
    .padding(Padding::from([8, 10]))
    .style(secondary_action)
    .on_press(message)
    .into()
}
