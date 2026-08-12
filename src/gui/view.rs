use std::path::Path;

use iced::{
    Color, Element, Fill, FillPortion, Font, Padding,
    font::Weight,
    widget::{button, column, container, row, rule, scrollable, space, text},
};

use crate::model::job::{JobStatus, RipJob};

use super::{
    message::Message,
    state::Demux,
    style::{
        ACCENT, DANGER, SUCCESS, TEXT_MUTED, WARNING, accent_tile, app_background, error_panel,
        inset_panel, panel, selected_row,
    },
};

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

        self.notifications.view(content)
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
        self.output_settings
            .view(
                &self.dependency_state,
                self.selected_job().map(|job| &job.status),
                self.can_start(),
            )
            .map(Message::OutputSettings)
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
