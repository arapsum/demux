use iced::font::Weight;
use iced::widget::{button, column, container, row, space, text};
use iced::{Element, Font, Padding};

use super::style::{DANGER_TEXT, TEXT_MUTED, destructive_action, dialog_panel, secondary_action};

/// Owns the confirmation dialog shown when closing during extraction.
#[derive(Debug, Default)]
pub struct CloseConfirmation {
    open: bool,
}

/// Events produced by the close-confirmation dialog.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Message {
    Open,
    KeepWorking,
    CancelAndClose,
}

/// Effects requested after a close-confirmation decision.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    None,
    KeepWorking,
    CancelAndClose,
}

impl CloseConfirmation {
    pub(crate) const fn update(&mut self, message: Message) -> Action {
        match message {
            Message::Open => {
                self.open = true;
                Action::None
            }
            Message::KeepWorking => {
                self.open = false;
                Action::KeepWorking
            }
            Message::CancelAndClose => {
                self.open = false;
                Action::CancelAndClose
            }
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
                    text("Cancel extraction and close?").size(20).font(Font {
                        weight: Weight::Semibold,
                        ..Font::default()
                    }),
                    text("Demux is still processing the queue. Cancelling will stop FFmpeg, remove partial output, and close the window after cleanup.")
                        .size(13)
                        .color(TEXT_MUTED),
                    row![
                        space::horizontal(),
                        button(text("Keep working").size(13))
                            .padding(Padding::from([9, 13]))
                            .style(secondary_action)
                            .on_press(Message::KeepWorking),
                        button(text("Cancel and close").size(13).color(DANGER_TEXT))
                            .padding(Padding::from([9, 13]))
                            .style(destructive_action)
                            .on_press(Message::CancelAndClose),
                    ]
                    .spacing(8),
                ]
                .spacing(14),
            )
            .width(500)
            .padding(22)
            .style(dialog_panel)
            .into(),
        )
    }
}
