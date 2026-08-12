use std::path::{Path, PathBuf};

use iced::font::Weight;
use iced::widget::{button, column, container, row, space, text, text_input};
use iced::{Element, Fill, FillPortion, Font, Padding, Task};

use crate::{ffmpeg::DependencyState, model::job::JobStatus};

use super::{
    presentation::{DependencyPresentation, StatusPresentation},
    style::{TEXT_MUTED, inset_panel, panel},
};

#[derive(Debug, Clone)]
pub enum Message {
    FolderChanged(String),
    Browse,
    FolderSelected(Option<PathBuf>),
    StartRipping,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Action {
    None,
    OutputChanged,
    StartRipping,
}

#[derive(Debug)]
pub(crate) struct OutputSettings {
    folder: String,
}

impl OutputSettings {
    pub(crate) fn new() -> Self {
        Self {
            folder: String::new(),
        }
    }

    pub(crate) fn update(&mut self, message: Message) -> (Action, Task<Message>) {
        match message {
            Message::FolderChanged(folder) => {
                self.folder = folder;
                (Action::OutputChanged, Task::none())
            }
            Message::Browse => (
                Action::None,
                Task::perform(pick_output_folder(), Message::FolderSelected),
            ),
            Message::FolderSelected(path) => {
                let Some(path) = path else {
                    return (Action::None, Task::none());
                };
                self.folder = path.to_string_lossy().into_owned();
                (Action::OutputChanged, Task::none())
            }
            Message::StartRipping => (Action::StartRipping, Task::none()),
        }
    }

    pub(crate) fn set_default_from_input(&mut self, input: &Path) {
        if self.folder.is_empty()
            && let Some(parent) = input.parent()
        {
            self.folder = parent.to_string_lossy().into_owned();
        }
    }

    pub(crate) fn has_folder(&self) -> bool {
        !self.folder.trim().is_empty()
    }

    pub(crate) fn output_path(&self, input: &Path) -> PathBuf {
        let filename = input
            .file_name()
            .map_or_else(|| PathBuf::from("output"), PathBuf::from)
            .with_extension("mp3");

        if self.folder.trim().is_empty() {
            input
                .parent()
                .map_or_else(PathBuf::new, PathBuf::from)
                .join(filename)
        } else {
            Path::new(self.folder.trim()).join(filename)
        }
    }

    pub(crate) fn view<'a>(
        &'a self,
        dependency_state: &DependencyState,
        job_status: Option<&JobStatus>,
        can_start: bool,
    ) -> Element<'a, Message> {
        let dependencies = DependencyPresentation::from(dependency_state);
        let output_locked = matches!(job_status, Some(JobStatus::Ripping | JobStatus::Completed));
        let selected_status = job_status
            .map(StatusPresentation::from)
            .map_or("Waiting for a file", |status| status.label);

        let output_input = text_input("Choose an output folder", &self.folder)
            .padding(12)
            .size(14);
        let output_input = if output_locked {
            output_input
        } else {
            output_input.on_input(Message::FolderChanged)
        };
        let browse = button(text("Browse…").size(14)).padding(Padding::from([11, 14]));
        let browse = if output_locked {
            browse
        } else {
            browse.on_press(Message::Browse)
        };
        let start = button(text("Start Ripping").size(15))
            .width(Fill)
            .padding(13)
            .style(button::primary)
            .on_press_maybe(can_start.then_some(Message::StartRipping));

        container(
            column![
                text("Output Settings").size(18).font(Font {
                    weight: Weight::Semibold,
                    ..Font::default()
                }),
                column![
                    text("Output format").size(13).color(TEXT_MUTED),
                    container(row![
                        text("MP3").size(15),
                        space::horizontal(),
                        text("192 kbps").size(13).color(TEXT_MUTED)
                    ])
                    .width(Fill)
                    .padding(12)
                    .style(inset_panel),
                ]
                .spacing(7),
                column![
                    text("Output folder").size(13).color(TEXT_MUTED),
                    row![output_input.width(Fill), browse].spacing(8),
                    text("The MP3 filename is derived from the selected video.")
                        .size(12)
                        .color(TEXT_MUTED),
                ]
                .spacing(7),
                container(
                    column![
                        text("Selected job").size(12).color(TEXT_MUTED),
                        text(selected_status).size(16).font(Font {
                            weight: Weight::Semibold,
                            ..Font::default()
                        }),
                        text(dependencies.label).size(13).color(dependencies.color),
                    ]
                    .spacing(6),
                )
                .width(Fill)
                .padding(14)
                .style(inset_panel),
                space::vertical(),
                start,
                text(if can_start {
                    "Ready to extract audio"
                } else {
                    "Add a valid video and wait for probing to finish"
                })
                .size(12)
                .color(TEXT_MUTED),
            ]
            .spacing(18),
        )
        .width(FillPortion(3))
        .height(Fill)
        .padding(20)
        .style(panel)
        .into()
    }
}

async fn pick_output_folder() -> Option<PathBuf> {
    rfd::AsyncFileDialog::new()
        .set_title("Choose an output folder")
        .pick_folder()
        .await
        .map(|folder| folder.path().to_path_buf())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_to_the_input_directory() {
        let mut settings = OutputSettings::new();

        settings.set_default_from_input(Path::new("/videos/example.mov"));

        assert_eq!(settings.folder, "/videos");
        assert_eq!(
            settings.output_path(Path::new("/videos/example.mov")),
            PathBuf::from("/videos/example.mp3")
        );
    }

    #[test]
    fn changing_the_folder_changes_the_derived_output() {
        let mut settings = OutputSettings::new();

        let (action, _) = settings.update(Message::FolderChanged("/music".into()));

        assert_eq!(action, Action::OutputChanged);
        assert_eq!(
            settings.output_path(Path::new("/videos/example.mov")),
            PathBuf::from("/music/example.mp3")
        );
    }
}
