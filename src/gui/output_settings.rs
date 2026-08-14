use std::path::{Path, PathBuf};

use iced::font::Weight;
use iced::widget::{button, column, container, pick_list, row, scrollable, text, text_input};
use iced::{Element, Fill, FillPortion, Font, Padding, Task};

use crate::{
    ffmpeg::DependencyState,
    model::{
        encoding::{ChannelMode, Mp3Bitrate, OutputFormat, RipOptions, SampleRate},
        job::JobStatus,
    },
};

use super::{
    presentation::{DependencyPresentation, StatusPresentation},
    style::{TEXT_MUTED, inset_panel, panel, primary_action, settings_select},
};

#[derive(Debug, Clone)]
pub enum Message {
    FormatChanged(OutputFormat),
    BitrateChanged(Mp3Bitrate),
    SampleRateChanged(SampleRate),
    ChannelsChanged(ChannelMode),
    FolderChanged(String),
    Browse,
    FolderSelected(Option<PathBuf>),
    StartRipping,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Action {
    None,
    OutputChanged,
    EncodingChanged(RipOptions),
    StartRipping,
}

#[derive(Debug)]
pub(crate) struct OutputSettings {
    folder: String,
    options: RipOptions,
    defaults_modified: bool,
}

impl OutputSettings {
    pub(crate) fn new() -> Self {
        Self {
            folder: String::new(),
            options: RipOptions::default(),
            defaults_modified: false,
        }
    }

    pub(crate) fn update(&mut self, message: Message, locked: bool) -> (Action, Task<Message>) {
        if locked
            && matches!(
                message,
                Message::FormatChanged(_)
                    | Message::BitrateChanged(_)
                    | Message::SampleRateChanged(_)
                    | Message::ChannelsChanged(_)
                    | Message::FolderChanged(_)
                    | Message::Browse
                    | Message::FolderSelected(_)
            )
        {
            return (Action::None, Task::none());
        }

        match message {
            Message::FormatChanged(format) => {
                self.options.format = format;
                self.defaults_modified = true;
                (Action::EncodingChanged(self.options), Task::none())
            }
            Message::BitrateChanged(bitrate) => {
                self.options.bitrate = bitrate;
                self.defaults_modified = true;
                (Action::EncodingChanged(self.options), Task::none())
            }
            Message::SampleRateChanged(sample_rate) => {
                self.options.sample_rate = sample_rate;
                self.defaults_modified = true;
                (Action::EncodingChanged(self.options), Task::none())
            }
            Message::ChannelsChanged(channels) => {
                self.options.channels = channels;
                self.defaults_modified = true;
                (Action::EncodingChanged(self.options), Task::none())
            }
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

    pub(crate) const fn options(&self) -> RipOptions {
        self.options
    }

    pub(crate) fn apply_loaded_defaults(&mut self, options: RipOptions) -> bool {
        if self.defaults_modified || self.options == options {
            return false;
        }
        self.options = options;
        true
    }

    pub(crate) fn output_path(&self, input: &Path) -> PathBuf {
        let filename = input
            .file_name()
            .map_or_else(|| PathBuf::from("output"), PathBuf::from)
            .with_extension(self.options.format.extension());

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
        run_progress: Option<(usize, usize)>,
        can_start: bool,
    ) -> Element<'a, Message> {
        let dependencies = DependencyPresentation::from(dependency_state);
        let output_locked = run_progress.is_some();
        let selected_status = run_progress.map_or_else(
            || {
                job_status.map(StatusPresentation::from).map_or_else(
                    || "Waiting for a file".to_owned(),
                    |status| status.label.into(),
                )
            },
            |(position, total)| format!("Ripping {position} of {total}"),
        );

        let output_input = text_input("Choose an output folder", &self.folder)
            .padding(12)
            .size(14);
        let output_input = if output_locked {
            output_input
        } else {
            output_input.on_input(Message::FolderChanged)
        };
        let browse = button(text("Browse…").size(14))
            .padding(Padding::from([11, 14]))
            .style(primary_action);
        let browse = if output_locked {
            browse
        } else {
            browse.on_press(Message::Browse)
        };
        let start_label = run_progress.map_or_else(
            || "Start Ripping".to_owned(),
            |(position, total)| format!("Ripping {position} of {total}…"),
        );
        let start = button(text(start_label).size(15))
            .width(Fill)
            .padding(13)
            .style(primary_action)
            .on_press_maybe(can_start.then_some(Message::StartRipping));

        let format: Element<'_, Message> = if output_locked {
            locked_value(self.options.format.to_string())
        } else {
            pick_list(
                OutputFormat::ALL,
                Some(self.options.format),
                Message::FormatChanged,
            )
            .width(Fill)
            .padding(12)
            .text_size(14)
            .style(settings_select)
            .into()
        };
        let bitrate: Element<'_, Message> = if output_locked {
            locked_value(self.options.bitrate.to_string())
        } else {
            pick_list(
                Mp3Bitrate::ALL,
                Some(self.options.bitrate),
                Message::BitrateChanged,
            )
            .width(Fill)
            .padding(12)
            .text_size(14)
            .style(settings_select)
            .into()
        };
        let sample_rate: Element<'_, Message> = if output_locked {
            locked_value(self.options.sample_rate.to_string())
        } else {
            pick_list(
                SampleRate::ALL,
                Some(self.options.sample_rate),
                Message::SampleRateChanged,
            )
            .width(Fill)
            .padding(12)
            .text_size(14)
            .style(settings_select)
            .into()
        };
        let channels: Element<'_, Message> = if output_locked {
            locked_value(self.options.channels.to_string())
        } else {
            pick_list(
                ChannelMode::ALL,
                Some(self.options.channels),
                Message::ChannelsChanged,
            )
            .width(Fill)
            .padding(12)
            .text_size(14)
            .style(settings_select)
            .into()
        };

        let controls = column![
            column![text("Output format").size(13).color(TEXT_MUTED), format,].spacing(7),
            row![
                column![
                    text("Bitrate / quality").size(13).color(TEXT_MUTED),
                    bitrate,
                ]
                .spacing(7)
                .width(Fill),
                column![text("Sample rate").size(13).color(TEXT_MUTED), sample_rate,]
                    .spacing(7)
                    .width(Fill),
            ]
            .spacing(10),
            column![text("Audio channels").size(13).color(TEXT_MUTED), channels,].spacing(7),
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
                    text(if run_progress.is_some() {
                        "Queue execution"
                    } else {
                        "Selected job"
                    })
                    .size(12)
                    .color(TEXT_MUTED),
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
        ]
        .spacing(16);

        container(
            column![
                text("Output Settings").size(18).font(Font {
                    weight: Weight::Semibold,
                    ..Font::default()
                }),
                scrollable(controls).height(Fill),
                start,
                text(if can_start {
                    "Ready to process every eligible job"
                } else if run_progress.is_some() {
                    "The queue is running one extraction at a time"
                } else {
                    "Add a valid video and wait for every probe to finish"
                })
                .size(12)
                .color(TEXT_MUTED),
            ]
            .spacing(14),
        )
        .width(FillPortion(3))
        .height(Fill)
        .padding(20)
        .style(panel)
        .into()
    }
}

fn locked_value(value: String) -> Element<'static, Message> {
    container(text(value).size(14).color(TEXT_MUTED))
        .width(Fill)
        .padding(12)
        .style(inset_panel)
        .into()
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

        let (action, _) = settings.update(Message::FolderChanged("/music".into()), false);

        assert_eq!(action, Action::OutputChanged);
        assert_eq!(
            settings.output_path(Path::new("/videos/example.mov")),
            PathBuf::from("/music/example.mp3")
        );
    }

    #[test]
    fn encoding_controls_emit_a_complete_valid_snapshot() {
        let mut settings = OutputSettings::new();

        let (action, _) = settings.update(Message::BitrateChanged(Mp3Bitrate::Kbps320), false);
        assert_eq!(
            action,
            Action::EncodingChanged(RipOptions {
                bitrate: Mp3Bitrate::Kbps320,
                ..RipOptions::default()
            })
        );
        let (action, _) = settings.update(Message::SampleRateChanged(SampleRate::Hz48000), false);
        assert_eq!(
            action,
            Action::EncodingChanged(RipOptions {
                bitrate: Mp3Bitrate::Kbps320,
                sample_rate: SampleRate::Hz48000,
                ..RipOptions::default()
            })
        );
        let (action, _) = settings.update(Message::ChannelsChanged(ChannelMode::Mono), false);
        assert_eq!(
            action,
            Action::EncodingChanged(RipOptions {
                bitrate: Mp3Bitrate::Kbps320,
                sample_rate: SampleRate::Hz48000,
                channels: ChannelMode::Mono,
                ..RipOptions::default()
            })
        );
    }

    #[test]
    fn running_queue_ignores_stale_setting_messages() {
        let mut settings = OutputSettings::new();

        let (action, _) = settings.update(Message::BitrateChanged(Mp3Bitrate::Kbps320), true);

        assert_eq!(action, Action::None);
        assert_eq!(settings.options(), RipOptions::default());
    }

    #[test]
    fn late_disk_load_does_not_overwrite_a_user_edit() {
        let mut settings = OutputSettings::new();
        let _ = settings.update(Message::ChannelsChanged(ChannelMode::Mono), false);
        let loaded = RipOptions {
            bitrate: Mp3Bitrate::Kbps320,
            ..RipOptions::default()
        };

        assert!(!settings.apply_loaded_defaults(loaded));
        assert_eq!(settings.options().channels, ChannelMode::Mono);
        assert_eq!(settings.options().bitrate, Mp3Bitrate::Kbps192);
    }
}
