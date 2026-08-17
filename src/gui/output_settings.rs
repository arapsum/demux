use std::path::{Path, PathBuf};

use iced::font::Weight;
use iced::widget::{
    button, checkbox, column, container, pick_list, row, scrollable, text, text_input,
};
use iced::{Element, Fill, FillPortion, Font, Padding, Task};

use crate::{
    app::output,
    ffmpeg::DependencyState,
    model::{
        encoding::{ChannelMode, Mp3Bitrate, OutputFormat, RipOptions, SampleRate},
        job::{JobStatus, RipJob},
        source::DestinationPolicy,
    },
};

use super::{
    presentation::{DependencyPresentation, StatusPresentation},
    style::{TEXT_MUTED, inset_panel, panel, secondary_action, settings_select},
};

/// Events produced by the output-settings surface.
#[derive(Debug, Clone)]
pub enum Message {
    FormatChanged(OutputFormat),
    BitrateChanged(Mp3Bitrate),
    SampleRateChanged(SampleRate),
    ChannelsChanged(ChannelMode),
    EmbedMetadataToggled(bool),
    ExtractArtworkToggled(bool),
    NormalizeAudioToggled(bool),
    PreserveFoldersToggled(bool),
    FolderChanged(String),
    Browse,
    FolderSelected(Option<PathBuf>),
}

/// Effects requested after output settings change.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    None,
    OutputChanged,
    EncodingChanged(RipOptions),
    DestinationChanged(DestinationPolicy),
}

/// Owns editable encoding and destination defaults for queued jobs.
#[derive(Debug)]
pub struct OutputSettings {
    folder: String,
    options: RipOptions,
    destination: DestinationPolicy,
    defaults_modified: bool,
}

impl OutputSettings {
    pub(crate) fn new() -> Self {
        Self {
            folder: String::new(),
            options: RipOptions::default(),
            destination: DestinationPolicy::default(),
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
                    | Message::EmbedMetadataToggled(_)
                    | Message::ExtractArtworkToggled(_)
                    | Message::NormalizeAudioToggled(_)
                    | Message::PreserveFoldersToggled(_)
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
            Message::EmbedMetadataToggled(enabled) => {
                self.options.embed_metadata = enabled;
                self.defaults_modified = true;
                (Action::EncodingChanged(self.options), Task::none())
            }
            Message::ExtractArtworkToggled(enabled) => {
                self.options.extract_artwork = enabled;
                self.defaults_modified = true;
                (Action::EncodingChanged(self.options), Task::none())
            }
            Message::NormalizeAudioToggled(enabled) => {
                self.options.normalize_audio = enabled;
                self.defaults_modified = true;
                (Action::EncodingChanged(self.options), Task::none())
            }
            Message::PreserveFoldersToggled(enabled) => {
                self.destination.preserve_folder_structure = enabled;
                self.defaults_modified = true;
                (Action::DestinationChanged(self.destination), Task::none())
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
        }
    }

    pub(crate) fn set_default_from_input(
        &mut self,
        input: &Path,
        hierarchy: Option<&crate::model::source::SourceHierarchy>,
    ) {
        if self.folder.is_empty()
            && let Some(parent) = hierarchy
                .map(crate::model::source::SourceHierarchy::root)
                .or_else(|| input.parent())
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

    pub(crate) const fn destination_policy(&self) -> DestinationPolicy {
        self.destination
    }

    pub(crate) fn reset_defaults(&mut self) {
        self.options = RipOptions::default();
        self.destination = DestinationPolicy::default();
        self.defaults_modified = true;
    }

    pub(crate) fn apply_loaded_defaults(
        &mut self,
        options: RipOptions,
        destination: DestinationPolicy,
    ) -> bool {
        if self.defaults_modified || (self.options == options && self.destination == destination) {
            return false;
        }
        self.options = options;
        self.destination = destination;
        true
    }

    pub(crate) fn output_path(
        &self,
        input: &Path,
        hierarchy: Option<&crate::model::source::SourceHierarchy>,
    ) -> PathBuf {
        output::destination_path(
            input,
            hierarchy,
            (!self.folder.trim().is_empty()).then(|| Path::new(self.folder.trim())),
            self.options.format,
            self.destination,
        )
    }

    #[allow(clippy::too_many_lines)]
    pub(crate) fn view<'a>(
        &'a self,
        dependency_state: &DependencyState,
        job_status: Option<&JobStatus>,
        selected_job: Option<&RipJob>,
        run_progress: Option<(usize, usize)>,
        has_folder_hierarchy: bool,
        compact: bool,
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
            |(position, total)| {
                if selected_job.is_some_and(|job| matches!(job.status, JobStatus::Analyzing)) {
                    format!("Analyzing loudness {position} of {total}")
                } else {
                    format!("Ripping audio {position} of {total}")
                }
            },
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
            .style(secondary_action);
        let browse = if output_locked {
            browse
        } else {
            browse.on_press(Message::Browse)
        };
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

        let embed_metadata = checkbox(self.options.embed_metadata)
            .label("Embed metadata (title, artist, album)")
            .text_size(13)
            .on_toggle_maybe((!output_locked).then_some(Message::EmbedMetadataToggled));
        let extract_artwork = checkbox(self.options.extract_artwork)
            .label("Extract artwork when available")
            .text_size(13)
            .on_toggle_maybe((!output_locked).then_some(Message::ExtractArtworkToggled));
        let normalize_audio = checkbox(self.options.normalize_audio)
            .label("Normalize audio (EBU R128)")
            .text_size(13)
            .on_toggle_maybe((!output_locked).then_some(Message::NormalizeAudioToggled));
        let preserve_folders = checkbox(self.destination.preserve_folder_structure)
            .label("Preserve folder structure")
            .text_size(13)
            .on_toggle_maybe(
                (!output_locked && has_folder_hierarchy).then_some(Message::PreserveFoldersToggled),
            );

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
                embed_metadata,
                extract_artwork,
                normalize_audio,
                preserve_folders
            ]
            .spacing(10),
            column![
                text("Output folder").size(13).color(TEXT_MUTED),
                row![output_input.width(Fill), browse].spacing(8),
                text(
                    if has_folder_hierarchy && self.destination.preserve_folder_structure {
                        "Folder imports keep paths relative to the selected folder."
                    } else if has_folder_hierarchy {
                        "Folder structure is disabled; outputs use the source filename."
                    } else {
                        "The MP3 filename is derived from the selected video."
                    }
                )
                .size(12)
                .color(TEXT_MUTED),
            ]
            .spacing(7),
            selected_job_detail(selected_job, run_progress, selected_status, &dependencies),
        ]
        .spacing(16);

        container(
            column![
                text("Output Settings").size(18).font(Font {
                    weight: Weight::Semibold,
                    ..Font::default()
                }),
                scrollable(controls).height(Fill),
                text(if run_progress.is_some() {
                    "The queue is running one extraction at a time"
                } else if matches!(dependency_state, DependencyState::Ready(_)) {
                    "Start extraction from Progress when every job is ready"
                } else {
                    "FFmpeg must be ready before extraction can start"
                })
                .size(12)
                .color(TEXT_MUTED),
            ]
            .spacing(14),
        )
        .width(if compact { Fill } else { FillPortion(3) })
        .height(Fill)
        .padding(20)
        .style(panel)
        .into()
    }
}

#[allow(clippy::too_many_lines)]
fn selected_job_detail(
    job: Option<&RipJob>,
    run_progress: Option<(usize, usize)>,
    selected_status: String,
    dependencies: &DependencyPresentation,
) -> Element<'static, Message> {
    let mut detail = column![
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
    .spacing(6);

    if let Some(job) = job {
        detail = detail.push(
            text(format!(
                "Output: {} · {} · {} · {} · {}",
                job.options.format,
                job.options.bitrate,
                if job.options.embed_metadata {
                    "metadata on"
                } else {
                    "metadata off"
                },
                if job.options.extract_artwork {
                    "artwork on"
                } else {
                    "artwork off"
                },
                if job.options.normalize_audio {
                    "normalization on"
                } else {
                    "normalization off"
                }
            ))
            .size(12)
            .color(TEXT_MUTED),
        );
        detail = detail
            .push(
                text(format!("Destination: {}", job.output))
                    .size(12)
                    .color(TEXT_MUTED),
            )
            .push(
                text(
                    if job.destination_policy.preserve_folder_structure
                        && job.source_hierarchy.is_some()
                    {
                        "Folder structure preserved from the selected root"
                    } else {
                        "Flat output destination"
                    },
                )
                .size(12)
                .color(TEXT_MUTED),
            );
        if let Some(metadata) = &job.metadata {
            let title = metadata
                .tags
                .title
                .clone()
                .unwrap_or_else(|| "Untitled".to_owned());
            let artist = metadata
                .tags
                .artist
                .clone()
                .unwrap_or_else(|| "Unknown artist".to_owned());
            let album = metadata
                .tags
                .album
                .clone()
                .unwrap_or_else(|| "Unknown album".to_owned());
            let artwork = match metadata.artwork.as_ref() {
                Some(artwork) if artwork.supports_mp3() => {
                    format!("{} cover art ready", artwork.format_label())
                }
                Some(_) => "Embedded artwork is unsupported; extraction will continue without it"
                    .to_owned(),
                None => "No embedded artwork detected".to_owned(),
            };

            detail = detail
                .push(text("Metadata").size(12).color(TEXT_MUTED))
                .push(text(format!("{title} · {artist}")).size(13))
                .push(text(format!("Album: {album}")).size(12).color(TEXT_MUTED))
                .push(text(artwork).size(12).color(TEXT_MUTED));
        } else {
            let message = if matches!(job.status, JobStatus::Pending | JobStatus::Probing) {
                "Metadata is still being probed"
            } else {
                "Metadata is unavailable for this job"
            };
            detail = detail.push(text(message).size(12).color(TEXT_MUTED));
        }
    }

    container(detail)
        .width(Fill)
        .padding(14)
        .style(inset_panel)
        .into()
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

        settings.set_default_from_input(Path::new("/videos/example.mov"), None);

        assert_eq!(settings.folder, "/videos");
        assert_eq!(
            settings.output_path(Path::new("/videos/example.mov"), None),
            PathBuf::from("/videos/example.mp3")
        );
    }

    #[test]
    fn changing_the_folder_changes_the_derived_output() {
        let mut settings = OutputSettings::new();

        let (action, _) = settings.update(Message::FolderChanged("/music".into()), false);

        assert_eq!(action, Action::OutputChanged);
        assert_eq!(
            settings.output_path(Path::new("/videos/example.mov"), None),
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

        let (action, _) = settings.update(Message::EmbedMetadataToggled(false), true);

        assert_eq!(action, Action::None);
        assert_eq!(settings.options(), RipOptions::default());
    }

    #[test]
    fn metadata_controls_emit_independent_snapshots() {
        let mut settings = OutputSettings::new();

        let (action, _) = settings.update(Message::EmbedMetadataToggled(false), false);
        assert_eq!(
            action,
            Action::EncodingChanged(RipOptions {
                embed_metadata: false,
                ..RipOptions::default()
            })
        );

        let (action, _) = settings.update(Message::ExtractArtworkToggled(false), false);
        assert_eq!(
            action,
            Action::EncodingChanged(RipOptions {
                embed_metadata: false,
                extract_artwork: false,
                ..RipOptions::default()
            })
        );
    }

    #[test]
    fn late_disk_load_does_not_overwrite_a_user_edit() {
        let mut settings = OutputSettings::new();
        let _ = settings.update(Message::ChannelsChanged(ChannelMode::Mono), false);
        let loaded = RipOptions {
            bitrate: Mp3Bitrate::Kbps320,
            ..RipOptions::default()
        };

        assert!(!settings.apply_loaded_defaults(loaded, DestinationPolicy::default()));
        assert_eq!(settings.options().channels, ChannelMode::Mono);
        assert_eq!(settings.options().bitrate, Mp3Bitrate::Kbps192);
    }
}
