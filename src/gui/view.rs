use std::path::Path;

use iced::{
    Border, Color, Element, Fill, FillPortion, Font, Padding, Shadow, Theme,
    font::Weight,
    widget::{button, column, container, row, rule, scrollable, space, text, text_input},
};

use crate::{
    ffmpeg::DependencyState,
    model::job::{JobStatus, RipJob},
};

use super::{message::Message, state::Demux, toast};

const TEXT_MUTED: Color = Color::from_rgb(0.62, 0.64, 0.70);
const ACCENT: Color = Color::from_rgb(0.43, 0.36, 0.96);
const SUCCESS: Color = Color::from_rgb(0.35, 0.78, 0.57);
const WARNING: Color = Color::from_rgb(0.96, 0.68, 0.30);
const DANGER: Color = Color::from_rgb(0.94, 0.39, 0.42);

impl Demux {
    pub fn view(&self) -> Element<'_, Message> {
        let header = row![
            container(
                text("D")
                    .size(22)
                    .font(Font {
                        weight: Weight::Bold,
                        ..Font::default()
                    })
                    .color(Color::WHITE)
            )
            .width(48)
            .height(48)
            .center(48)
            .style(accent_tile),
            column![
                text("Demux").size(28).font(Font {
                    weight: Weight::Bold,
                    ..Font::default()
                }),
                text("Extract clean audio from video with FFmpeg")
                    .size(14)
                    .color(TEXT_MUTED),
            ]
            .spacing(3)
        ]
        .spacing(14)
        .align_y(iced::Alignment::Center);

        let workspace = row![self.work_area(), self.settings_panel()]
            .spacing(16)
            .height(Fill);

        let content = container(column![header, workspace].spacing(18))
            .width(Fill)
            .height(Fill)
            .padding(Padding::from([24, 26]))
            .style(app_background);

        toast::overlay(content, &self.toasts)
    }

    fn work_area(&self) -> Element<'_, Message> {
        let choose_copy = if self.picking_file {
            "Waiting for file selection…"
        } else if self.is_busy() {
            "Demux is working on the selected video"
        } else if self.jobs.is_empty() {
            "Choose a video to begin"
        } else {
            "Replace the selected video"
        };

        let add_button = button(text("Add File").size(15))
            .padding(Padding::from([10, 16]))
            .style(button::primary);
        let add_button = if self.picking_file || self.is_busy() {
            add_button
        } else {
            add_button.on_press(Message::AddFile)
        };

        let chooser = container(
            column![
                text(choose_copy).size(18).font(Font {
                    weight: Weight::Semibold,
                    ..Font::default()
                }),
                text("MP4, MKV, MOV, AVI, WMV, FLV, MPEG")
                    .size(13)
                    .color(TEXT_MUTED),
                add_button,
            ]
            .spacing(10)
            .align_x(iced::Alignment::Center),
        )
        .width(Fill)
        .padding(Padding::from([24, 18]))
        .center_x(Fill)
        .style(inset_panel);

        let mut content = column![chooser, self.queue_panel()].spacing(14);

        if let Some(error) = &self.error {
            content = content.push(
                container(
                    column![
                        text("The operation could not be completed")
                            .font(Font {
                                weight: Weight::Semibold,
                                ..Font::default()
                            })
                            .color(DANGER),
                        text(error)
                            .size(13)
                            .color(Color::from_rgb(0.95, 0.76, 0.77)),
                    ]
                    .spacing(5),
                )
                .width(Fill)
                .padding(14)
                .style(error_panel),
            );
        }

        container(content).width(FillPortion(7)).height(Fill).into()
    }

    fn queue_panel(&self) -> Element<'_, Message> {
        let heading = row![
            text("Queue").size(17).font(Font {
                weight: Weight::Semibold,
                ..Font::default()
            }),
            space::horizontal(),
            text(if self.jobs.is_empty() {
                "No files"
            } else {
                "1 file"
            })
            .size(13)
            .color(TEXT_MUTED),
        ]
        .align_y(iced::Alignment::Center);

        let queue: Element<'_, Message> = match self.selected_job() {
            Some(job) => self.job_row(job),
            None => container(
                column![
                    text("Your queue is empty")
                        .size(17)
                        .font(Font {
                            weight: Weight::Semibold,
                            ..Font::default()
                        }),
                    text("Add a video above. Demux will inspect its audio stream before you can start ripping.")
                        .size(14)
                        .color(TEXT_MUTED),
                ]
                .spacing(7)
                .max_width(520),
            )
            .height(180)
            .padding(22)
            .center_y(180)
            .into(),
        };

        container(column![heading, rule::horizontal(1), scrollable(queue)].spacing(12))
            .width(Fill)
            .height(Fill)
            .padding(18)
            .style(panel)
            .into()
    }

    fn job_row<'a>(&self, job: &'a RipJob) -> Element<'a, Message> {
        let filename = Path::new(&job.input)
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or(&job.input);
        let (status, status_color) = status_label(&job.status);
        let duration = job
            .metadata
            .as_ref()
            .map(|metadata| format_duration(metadata.duration))
            .unwrap_or_else(|| "—".into());
        let details = job.metadata.as_ref().map_or_else(
            || "Inspecting media details".to_owned(),
            |metadata| {
                let sample_rate = metadata
                    .audio
                    .sample_rate
                    .map_or_else(|| "Unknown rate".into(), |rate| format!("{rate} Hz"));
                let channels = metadata.audio.channels.map_or_else(
                    || "Unknown channels".into(),
                    |channels| {
                        format!("{channels} channel{}", if channels == 1 { "" } else { "s" })
                    },
                );

                format!(
                    "{} · {} · {} · {}",
                    metadata.container,
                    metadata.audio.codec.to_uppercase(),
                    sample_rate,
                    channels,
                )
            },
        );

        container(
            column![
                row![
                    column![
                        text(filename).size(16).font(Font {
                            weight: Weight::Semibold,
                            ..Font::default()
                        }),
                        text(&job.input).size(12).color(TEXT_MUTED),
                    ]
                    .spacing(4)
                    .width(Fill),
                    text(status)
                        .size(13)
                        .font(Font {
                            weight: Weight::Semibold,
                            ..Font::default()
                        })
                        .color(status_color),
                ]
                .spacing(14)
                .align_y(iced::Alignment::Start),
                rule::horizontal(1),
                row![
                    column![text("Duration").size(12).color(TEXT_MUTED), text(duration)]
                        .spacing(4)
                        .width(FillPortion(1)),
                    column![
                        text("Audio stream").size(12).color(TEXT_MUTED),
                        text(details)
                    ]
                    .spacing(4)
                    .width(FillPortion(2)),
                    column![
                        text("Output").size(12).color(TEXT_MUTED),
                        text("MP3 · 192 kbps")
                    ]
                    .spacing(4)
                    .width(FillPortion(1)),
                ]
                .spacing(18),
            ]
            .spacing(14),
        )
        .width(Fill)
        .padding(18)
        .style(selected_row)
        .into()
    }

    fn settings_panel(&self) -> Element<'_, Message> {
        let dependencies = match &self.dependency_state {
            DependencyState::Checking => ("Checking FFmpeg…", WARNING),
            DependencyState::Ready(_) => ("FFmpeg ready", SUCCESS),
            DependencyState::Missing { .. } | DependencyState::Failed { .. } => {
                ("FFmpeg unavailable", DANGER)
            }
        };

        let output_input = text_input("Choose an output folder", &self.output_folder)
            .padding(12)
            .size(14);
        let output_locked = matches!(
            self.selected_job().map(|job| &job.status),
            Some(JobStatus::Ripping | JobStatus::Completed)
        );
        let output_input = if output_locked {
            output_input
        } else {
            output_input.on_input(Message::OutputFolderChanged)
        };
        let browse = button(text("Browse…").size(14)).padding(Padding::from([11, 14]));
        let browse = if output_locked {
            browse
        } else {
            browse.on_press(Message::BrowseOutputFolder)
        };

        let start = button(text("Start Ripping").size(15))
            .width(Fill)
            .padding(13)
            .style(button::primary);
        let start = if self.can_start() {
            start.on_press(Message::StartRipping)
        } else {
            start
        };

        let selected_status = self
            .selected_job()
            .map(|job| status_label(&job.status).0)
            .unwrap_or("Waiting for a file");

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
                        text(dependencies.0).size(13).color(dependencies.1),
                    ]
                    .spacing(6),
                )
                .width(Fill)
                .padding(14)
                .style(inset_panel),
                space::vertical(),
                start,
                text(if self.can_start() {
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

fn status_label(status: &JobStatus) -> (&str, Color) {
    match status {
        JobStatus::Pending => ("Pending", TEXT_MUTED),
        JobStatus::Probing => ("Probing…", WARNING),
        JobStatus::Ready => ("Ready", SUCCESS),
        JobStatus::Ripping => ("Ripping…", ACCENT),
        JobStatus::Completed => ("Completed", SUCCESS),
        JobStatus::Failed(_) => ("Failed", DANGER),
        JobStatus::Cancelled => ("Cancelled", TEXT_MUTED),
    }
}

fn format_duration(duration: std::time::Duration) -> String {
    let seconds = duration.as_secs();
    let hours = seconds / 3_600;
    let minutes = (seconds % 3_600) / 60;
    let seconds = seconds % 60;

    if hours > 0 {
        format!("{hours:02}:{minutes:02}:{seconds:02}")
    } else {
        format!("{minutes:02}:{seconds:02}")
    }
}

fn app_background(_theme: &Theme) -> container::Style {
    container::Style::default().background(Color::from_rgb(0.055, 0.06, 0.075))
}

fn panel(_theme: &Theme) -> container::Style {
    container::Style::default()
        .background(Color::from_rgb(0.085, 0.092, 0.112))
        .border(Border {
            color: Color::from_rgb(0.17, 0.18, 0.22),
            width: 1.0,
            radius: 14.0.into(),
        })
        .shadow(Shadow {
            color: Color::from_rgba(0.0, 0.0, 0.0, 0.22),
            offset: iced::Vector::new(0.0, 5.0),
            blur_radius: 18.0,
        })
}

fn inset_panel(_theme: &Theme) -> container::Style {
    container::Style::default()
        .background(Color::from_rgb(0.065, 0.07, 0.087))
        .border(Border {
            color: Color::from_rgb(0.19, 0.20, 0.24),
            width: 1.0,
            radius: 12.0.into(),
        })
}

fn selected_row(_theme: &Theme) -> container::Style {
    container::Style::default()
        .background(Color::from_rgb(0.075, 0.08, 0.10))
        .border(Border {
            color: Color::from_rgb(0.30, 0.27, 0.55),
            width: 1.0,
            radius: 12.0.into(),
        })
}

fn accent_tile(_theme: &Theme) -> container::Style {
    container::Style::default()
        .background(Color::from_rgb(0.20, 0.17, 0.43))
        .border(Border {
            color: Color::from_rgb(0.34, 0.29, 0.67),
            width: 1.0,
            radius: 12.0.into(),
        })
}

fn error_panel(_theme: &Theme) -> container::Style {
    container::Style::default()
        .background(Color::from_rgb(0.16, 0.075, 0.085))
        .border(Border {
            color: Color::from_rgb(0.38, 0.14, 0.16),
            width: 1.0,
            radius: 12.0.into(),
        })
}
