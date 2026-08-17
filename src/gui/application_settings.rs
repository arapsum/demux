use iced::font::Weight;
use iced::widget::{button, checkbox, column, container, row, space, text};
use iced::{Element, Font, Padding};

use crate::ffmpeg::DependencyState;

use super::{
    icon,
    presentation::DependencyPresentation,
    style::{
        DANGER_TEXT, ICON_MUTED, TEXT_MUTED, destructive_action, dialog_panel, secondary_action,
    },
};

#[derive(Debug)]
pub struct ApplicationSettings {
    open: bool,
    reset_confirmation: bool,
    checking_dependencies: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Message {
    Open,
    Close,
    RememberGeometryToggled(bool),
    RecheckDependencies,
    ResetRequested,
    ResetConfirmed,
    ResetCancelled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    None,
    RememberGeometryChanged(bool),
    RecheckDependencies,
    ResetOutputDefaults,
}

impl ApplicationSettings {
    pub(crate) const fn new() -> Self {
        Self {
            open: false,
            reset_confirmation: false,
            checking_dependencies: false,
        }
    }

    pub(crate) const fn update(&mut self, message: Message, locked: bool) -> Action {
        match message {
            Message::Open => {
                self.open = true;
                Action::None
            }
            Message::Close => {
                self.open = false;
                self.reset_confirmation = false;
                Action::None
            }
            Message::RememberGeometryToggled(remember) => Action::RememberGeometryChanged(remember),
            Message::RecheckDependencies if !locked => {
                self.checking_dependencies = true;
                Action::RecheckDependencies
            }
            Message::RecheckDependencies | Message::ResetRequested | Message::ResetConfirmed
                if locked =>
            {
                Action::None
            }
            Message::ResetRequested if !locked => {
                self.reset_confirmation = true;
                Action::None
            }
            Message::ResetConfirmed if !locked => {
                self.reset_confirmation = false;
                Action::ResetOutputDefaults
            }
            Message::RecheckDependencies | Message::ResetRequested | Message::ResetConfirmed => {
                Action::None
            }
            Message::ResetCancelled => {
                self.reset_confirmation = false;
                Action::None
            }
        }
    }

    pub(crate) const fn dependency_check_finished(&mut self) {
        self.checking_dependencies = false;
    }

    pub(crate) const fn is_open(&self) -> bool {
        self.open
    }

    #[allow(clippy::too_many_lines)]
    pub(crate) fn view<'a>(
        &'a self,
        dependency_state: &DependencyState,
        remember_geometry: bool,
        locked: bool,
    ) -> Option<Element<'a, Message>> {
        if !self.open {
            return None;
        }

        let dependencies = DependencyPresentation::from(dependency_state);
        let dependency_detail = match dependency_state {
            DependencyState::Ready(dependencies) => column![
                text("FFmpeg").size(12).color(TEXT_MUTED),
                text(dependencies.ffmpeg_version.clone()).size(12),
                text("FFprobe").size(12).color(TEXT_MUTED),
                text(dependencies.ffprobe_version.clone()).size(12),
            ]
            .spacing(3),
            DependencyState::Checking => column![text("Checking FFmpeg and FFprobe…").size(13)],
            DependencyState::Missing { program } => column![
                text(format!("{program} is not available")).size(13),
                text("Install FFmpeg, ensure both tools are on PATH, then recheck.")
                    .size(12)
                    .color(TEXT_MUTED),
            ]
            .spacing(4),
            DependencyState::Failed { message, .. } => column![
                text("Dependency check failed").size(13),
                text(message.clone()).size(12).color(DANGER_TEXT),
            ]
            .spacing(4),
        };

        let recheck = button(
            row![
                icon::refresh(if self.checking_dependencies {
                    ICON_MUTED
                } else {
                    super::style::BUTTON_TEXT
                }),
                text(if self.checking_dependencies {
                    "Checking…"
                } else {
                    "Recheck dependencies"
                })
                .size(12),
            ]
            .spacing(7)
            .align_y(iced::Alignment::Center),
        )
        .padding(Padding::from([8, 11]))
        .style(secondary_action)
        .on_press_maybe(
            (!locked && !self.checking_dependencies).then_some(Message::RecheckDependencies),
        );

        let reset: Element<'_, Message> = if self.reset_confirmation {
            row![
                text("Reset saved MP3 defaults?")
                    .size(12)
                    .color(DANGER_TEXT),
                space::horizontal(),
                button(text("Cancel").size(12))
                    .padding(Padding::from([7, 10]))
                    .style(secondary_action)
                    .on_press(Message::ResetCancelled),
                button(text("Reset").size(12))
                    .padding(Padding::from([7, 10]))
                    .style(destructive_action)
                    .on_press_maybe((!locked).then_some(Message::ResetConfirmed)),
            ]
            .spacing(7)
            .align_y(iced::Alignment::Center)
            .into()
        } else {
            button(text("Reset saved output defaults").size(12))
                .padding(Padding::from([8, 11]))
                .style(secondary_action)
                .on_press_maybe((!locked).then_some(Message::ResetRequested))
                .into()
        };

        let shortcuts = column![
            shortcut("Add files", "Ctrl/⌘ + O"),
            shortcut("Add folder", "Ctrl/⌘ + Shift + O"),
            shortcut("Remove selected", "Delete"),
            shortcut("Start queue", "Ctrl/⌘ + Enter"),
            shortcut("Pause or resume", "Space"),
            shortcut("Dismiss dialog", "Escape"),
        ]
        .spacing(5);

        Some(
            container(
                column![
                    row![
                        column![
                            text("Settings").size(20).font(Font {
                                weight: Weight::Semibold,
                                ..Font::default()
                            }),
                            text("Application behavior and diagnostics").size(12).color(TEXT_MUTED),
                        ]
                        .spacing(3),
                        space::horizontal(),
                        button(icon::close(ICON_MUTED))
                            .padding(8)
                            .style(secondary_action)
                            .on_press(Message::Close),
                    ]
                    .align_y(iced::Alignment::Center),
                    container(
                        column![
                            text("Window behavior").size(14).font(Font {
                                weight: Weight::Semibold,
                                ..Font::default()
                            }),
                            checkbox(remember_geometry)
                                .label("Remember window size and position")
                                .size(16)
                                .on_toggle(Message::RememberGeometryToggled),
                            text("The last usable desktop geometry will be restored on the next launch.")
                                .size(12)
                                .color(TEXT_MUTED),
                        ]
                        .spacing(8),
                    )
                    .padding(14)
                    .style(super::style::inset_panel),
                    container(
                        column![
                            row![
                                text("Dependencies").size(14).font(Font {
                                    weight: Weight::Semibold,
                                    ..Font::default()
                                }),
                                space::horizontal(),
                                text(dependencies.label).size(12).color(dependencies.color),
                            ]
                            .align_y(iced::Alignment::Center),
                            dependency_detail,
                            recheck,
                        ]
                        .spacing(8),
                    )
                    .padding(14)
                    .style(super::style::inset_panel),
                    container(
                        column![
                            text("Saved defaults").size(14).font(Font {
                                weight: Weight::Semibold,
                                ..Font::default()
                            }),
                            text("Reset the persisted MP3 and destination policies to their validated defaults.")
                                .size(12)
                                .color(TEXT_MUTED),
                            reset,
                        ]
                        .spacing(8),
                    )
                    .padding(14)
                    .style(super::style::inset_panel),
                    container(
                        column![
                            text("Keyboard shortcuts").size(14).font(Font {
                                weight: Weight::Semibold,
                                ..Font::default()
                            }),
                            shortcuts,
                        ]
                        .spacing(8),
                    )
                    .padding(14)
                    .style(super::style::inset_panel),
                ]
                .spacing(14),
            )
            .width(520)
            .padding(22)
            .style(dialog_panel)
            .into(),
        )
    }
}

fn shortcut<'a>(label: &'a str, keys: &'a str) -> Element<'a, Message> {
    row![
        text(label).size(12),
        space::horizontal(),
        text(keys).size(12).color(TEXT_MUTED)
    ]
    .into()
}
