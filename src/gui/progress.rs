use std::path::Path;

use iced::font::Weight;
use iced::widget::{button, column, container, progress_bar, row, space, text};
use iced::{Element, Fill, Font, Padding};

use crate::{
    ffmpeg::{RipPhase, RipProgressEvent, RipRequest},
    model::{
        encoding::RipOptions,
        job::{JobId, RipProgress},
    },
};

use super::{
    icon,
    style::{
        DANGER, DANGER_TEXT, ICON_MUTED, SUCCESS, TEXT_MUTED, WARNING, destructive_action,
        inset_panel, panel,
    },
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    None,
    CancelRequested(JobId),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TerminalStatus {
    Completed,
    Cancelled,
    Failed,
}

#[derive(Debug, Clone)]
pub enum Message {
    Started {
        job_id: JobId,
        request: Box<RipRequest>,
        progress: RipProgress,
    },
    Advanced {
        job_id: JobId,
        progress: RipProgressEvent,
    },
    Finished {
        job_id: JobId,
        status: TerminalStatus,
    },
    Cancel,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Status {
    Running,
    Cancelling,
    Completed,
    Cancelled,
    Failed,
}

#[derive(Debug)]
struct ActiveProgress {
    job_id: JobId,
    filename: String,
    options: RipOptions,
    phase: RipPhase,
    progress: RipProgress,
    status: Status,
}

#[derive(Debug)]
pub struct Progress {
    active: Option<ActiveProgress>,
}

impl Progress {
    pub(crate) const fn new() -> Self {
        Self { active: None }
    }

    pub(crate) fn update(&mut self, message: Message) -> Action {
        match message {
            Message::Started {
                job_id,
                request,
                progress,
            } => {
                self.active = Some(ActiveProgress {
                    job_id,
                    filename: filename(&request.input),
                    options: request.options,
                    phase: if request.options.normalize_audio {
                        RipPhase::Analyzing
                    } else {
                        RipPhase::Encoding
                    },
                    progress,
                    status: Status::Running,
                });
                Action::None
            }
            Message::Advanced { job_id, progress } => {
                let Some(active) = self
                    .active
                    .as_mut()
                    .filter(|active| active.job_id == job_id && active.status == Status::Running)
                else {
                    return Action::None;
                };
                if active.phase != progress.phase {
                    active.phase = progress.phase;
                    active.progress.reset_for_phase();
                }
                active.progress.update(
                    progress.progress.elapsed,
                    progress.progress.speed,
                    progress.progress.bitrate_kbps,
                    progress.progress.output_size,
                );
                Action::None
            }
            Message::Finished { job_id, status } => {
                let Some(active) = self
                    .active
                    .as_mut()
                    .filter(|active| active.job_id == job_id)
                else {
                    return Action::None;
                };
                active.status = match status {
                    TerminalStatus::Completed => {
                        active.progress.finish();
                        Status::Completed
                    }
                    TerminalStatus::Cancelled => Status::Cancelled,
                    TerminalStatus::Failed => Status::Failed,
                };
                Action::None
            }
            Message::Cancel => {
                let Some(active) = self
                    .active
                    .as_mut()
                    .filter(|active| active.status == Status::Running)
                else {
                    return Action::None;
                };
                active.status = Status::Cancelling;
                Action::CancelRequested(active.job_id.clone())
            }
        }
    }

    pub(crate) fn mark_cancelling(&mut self, job_id: &JobId) {
        if let Some(active) = self
            .active
            .as_mut()
            .filter(|active| &active.job_id == job_id && active.status == Status::Running)
        {
            active.status = Status::Cancelling;
        }
    }

    #[allow(clippy::too_many_lines)]
    pub(crate) fn view(&self) -> Element<'_, Message> {
        let heading = text("Progress").size(17).font(Font {
            weight: Weight::Semibold,
            ..Font::default()
        });

        let Some(active) = &self.active else {
            return container(
                column![
                    heading,
                    container(
                        column![
                            text("No extraction in progress").font(Font {
                                weight: Weight::Semibold,
                                ..Font::default()
                            }),
                            text("Live elapsed time, speed, bitrate, and estimates will appear when the queue starts.")
                                .size(12)
                                .color(TEXT_MUTED),
                        ]
                        .spacing(5),
                    )
                    .width(Fill)
                    .padding(14)
                    .style(inset_panel),
                ]
                .spacing(10),
            )
            .width(Fill)
            .padding(16)
            .style(panel)
            .into();
        };

        let status_label = match active.status {
            Status::Running => match active.phase {
                RipPhase::Analyzing => "Analyzing loudness (1 of 2)",
                RipPhase::Encoding => "Ripping audio (2 of 2)",
            },
            Status::Cancelling => "Cancelling",
            Status::Completed => "Completed",
            Status::Cancelled => "Cancelled",
            Status::Failed => "Failed",
        };
        let status_color = match active.status {
            Status::Running => super::style::ACCENT,
            Status::Cancelling => WARNING,
            Status::Completed => SUCCESS,
            Status::Cancelled => TEXT_MUTED,
            Status::Failed => DANGER,
        };
        let duration_known = !active.progress.duration.is_zero();
        let percent_label = if duration_known {
            format!("{:.0}%", active.progress.percent)
        } else {
            "Indeterminate".to_owned()
        };
        let bar_value = if duration_known {
            let elapsed = active.progress.elapsed.as_secs_f32();
            let duration = active.progress.duration.as_secs_f32();
            (elapsed / duration * 100.0).clamp(0.0, 100.0)
        } else {
            0.0
        };

        let mut metrics = row![
            metric("Elapsed", format_duration(active.progress.elapsed)),
            metric(
                "Remaining",
                active
                    .progress
                    .remaining
                    .map_or_else(|| "Unknown".to_owned(), format_duration)
            ),
            metric(
                "Speed",
                active
                    .progress
                    .speed
                    .map_or_else(|| "Unknown".to_owned(), |speed| format!("{speed:.2}×"))
            ),
        ]
        .spacing(18);
        if active.phase == RipPhase::Encoding {
            metrics = metrics
                .push(metric(
                    "Bitrate",
                    active.progress.bitrate_kbps.map_or_else(
                        || format!("{} target", active.options.bitrate),
                        |bitrate| format!("{bitrate:.0} kbps"),
                    ),
                ))
                .push(metric(
                    "Output size",
                    active
                        .progress
                        .output_size
                        .map_or_else(|| "Unknown".to_owned(), format_size),
                ));
        }

        let cancel: Element<'_, Message> = match active.status {
            Status::Running | Status::Cancelling => {
                let enabled = active.status == Status::Running;
                button(
                    row![
                        icon::stop(if enabled { DANGER_TEXT } else { ICON_MUTED }),
                        text(if enabled { "Cancel" } else { "Cancelling…" }).size(12),
                    ]
                    .spacing(7)
                    .align_y(iced::Alignment::Center),
                )
                .padding(Padding::from([7, 12]))
                .style(destructive_action)
                .on_press_maybe(enabled.then_some(Message::Cancel))
                .into()
            }
            Status::Completed | Status::Cancelled | Status::Failed => space::horizontal().into(),
        };

        container(
            column![
                row![
                    heading,
                    space::horizontal(),
                    text(status_label).size(12).color(status_color)
                ]
                .align_y(iced::Alignment::Center),
                row![
                    text(format!("{}: {}", status_label, active.filename))
                        .size(14)
                        .font(Font {
                            weight: Weight::Semibold,
                            ..Font::default()
                        }),
                    space::horizontal(),
                    text(format!(
                        "{} / {}",
                        format_duration(active.progress.elapsed),
                        if duration_known {
                            format_duration(active.progress.duration)
                        } else {
                            "Unknown".to_owned()
                        }
                    ))
                    .size(12)
                    .color(TEXT_MUTED),
                ]
                .align_y(iced::Alignment::Center),
                row![
                    progress_bar(0.0..=100.0, bar_value).girth(8).length(Fill),
                    text(percent_label).size(12).width(88),
                ]
                .spacing(12)
                .align_y(iced::Alignment::Center),
                metrics,
                row![
                    text(format!(
                        "Audio: {} · {} · {} · {}",
                        active.options.format,
                        active.options.bitrate,
                        active.options.sample_rate,
                        active.options.channels
                    ))
                    .size(11)
                    .color(TEXT_MUTED),
                    space::horizontal(),
                    cancel,
                ]
                .align_y(iced::Alignment::Center),
            ]
            .spacing(9),
        )
        .width(Fill)
        .padding(Padding::from([14, 16]))
        .style(panel)
        .into()
    }
}

fn metric(label: &str, value: String) -> Element<'_, Message> {
    column![text(label).size(11).color(TEXT_MUTED), text(value).size(12),]
        .spacing(2)
        .into()
}

fn filename(path: &Path) -> String {
    path.file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("Unknown file")
        .to_owned()
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

fn format_size(bytes: u64) -> String {
    const MIB: u64 = 1_048_576;
    if bytes >= MIB {
        format_scaled_size(bytes, MIB, "MB")
    } else {
        format_scaled_size(bytes, 1_024, "KB")
    }
}

fn format_scaled_size(bytes: u64, unit: u64, label: &str) -> String {
    let whole = bytes / unit;
    let fraction = (bytes % unit)
        .saturating_mul(10)
        .checked_div(unit)
        .unwrap_or_default();
    format!("{whole}.{fraction} {label}")
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use crate::ffmpeg::FfmpegProgress;

    use super::*;

    fn started() -> Message {
        Message::Started {
            job_id: JobId::new(1),
            request: Box::new(RipRequest::new("/videos/example.mp4", "/music/example.mp3")),
            progress: RipProgress {
                duration: Duration::from_secs(120),
                ..RipProgress::default()
            },
        }
    }

    #[test]
    fn ignores_stale_updates_and_finishes_the_active_job() {
        let mut state = Progress::new();
        state.update(started());
        state.update(Message::Advanced {
            job_id: JobId::new(99),
            progress: RipProgressEvent {
                phase: RipPhase::Encoding,
                progress: FfmpegProgress {
                    elapsed: Some(Duration::from_secs(60)),
                    ..FfmpegProgress::default()
                },
            },
        });
        assert_eq!(
            state.active.as_ref().unwrap().progress.elapsed,
            Duration::ZERO
        );

        state.update(Message::Advanced {
            job_id: JobId::new(1),
            progress: RipProgressEvent {
                phase: RipPhase::Encoding,
                progress: FfmpegProgress {
                    elapsed: Some(Duration::from_secs(60)),
                    speed: Some(2.0),
                    ..FfmpegProgress::default()
                },
            },
        });
        state.update(Message::Finished {
            job_id: JobId::new(1),
            status: TerminalStatus::Completed,
        });

        let active = state.active.as_ref().unwrap();
        assert_eq!(active.status, Status::Completed);
        assert_eq!(active.progress.percent, 100.0);
    }

    #[test]
    fn formatters_label_measurements_compactly() {
        assert_eq!(format_duration(Duration::from_secs(3_661)), "01:01:01");
        assert_eq!(format_size(1_572_864), "1.5 MB");
    }

    #[test]
    fn cancel_action_is_emitted_once_and_enters_cancelling_state() {
        let mut state = Progress::new();
        let _ = state.update(started());

        assert_eq!(
            state.update(Message::Cancel),
            Action::CancelRequested(JobId::new(1))
        );
        assert_eq!(state.active.as_ref().unwrap().status, Status::Cancelling);
        assert_eq!(state.update(Message::Cancel), Action::None);

        let _ = state.update(Message::Finished {
            job_id: JobId::new(1),
            status: TerminalStatus::Cancelled,
        });
        assert_eq!(state.active.as_ref().unwrap().status, Status::Cancelled);
    }
}
