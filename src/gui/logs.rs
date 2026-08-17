use std::{collections::VecDeque, path::PathBuf, sync::Arc, time::SystemTime};

use iced::{
    Color, Element, Fill, Font, Padding, Task,
    font::Weight,
    widget::{button, column, container, row, scrollable, space, text},
};
use time::{OffsetDateTime, UtcOffset, macros::format_description};

use crate::{
    Error,
    ffmpeg::{FfmpegLogEvent, RipPhase},
    model::job::JobId,
};

use super::{
    TaskResult, icon,
    style::{DANGER, ICON_MUTED, TEXT_MUTED, WARNING, inset_panel, panel, secondary_action},
};

const MAX_LINES: usize = 2_000;
const MAX_BYTES: usize = 512 * 1024;
const LOG_SCROLL_ID: &str = "ffmpeg-log-scroll";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JobTerminalStatus {
    Completed,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone)]
pub enum Message {
    JobStarted {
        job_id: JobId,
        filename: String,
    },
    Append {
        job_id: JobId,
        events: Vec<FfmpegLogEvent>,
    },
    JobFinished {
        job_id: JobId,
        status: JobTerminalStatus,
    },
    Clear,
    Scrolled(scrollable::Viewport),
    Save,
    SaveCompleted(TaskResult<Option<PathBuf>>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    None,
    Saved(PathBuf),
    SaveFailed(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Tone {
    Normal,
    Detail,
    Warning,
    Error,
    Omitted,
}

impl Tone {
    const fn color(self) -> Color {
        match self {
            Self::Normal => Color::from_rgb(0.82, 0.83, 0.87),
            Self::Detail => TEXT_MUTED,
            Self::Warning => WARNING,
            Self::Error => DANGER,
            Self::Omitted => Color::from_rgb(0.78, 0.62, 0.95),
        }
    }
}

#[derive(Debug, Clone)]
struct LogEntry {
    job_id: JobId,
    timestamp: SystemTime,
    phase: Option<RipPhase>,
    message: String,
    tone: Tone,
}

impl LogEntry {
    fn render(&self) -> String {
        let phase = self
            .phase
            .map_or_else(String::new, |phase| format!(" [{}]", phase_label(phase)));
        format!(
            "[{}] [job {}]{} {}",
            format_timestamp(self.timestamp),
            self.job_id.0,
            phase,
            self.message
        )
    }
}

#[derive(Debug)]
pub struct Logs {
    entries: VecDeque<LogEntry>,
    bytes: usize,
    evicted: usize,
    follow_tail: bool,
    saving: bool,
}

impl Logs {
    pub(crate) const fn new() -> Self {
        Self {
            entries: VecDeque::new(),
            bytes: 0,
            evicted: 0,
            follow_tail: true,
            saving: false,
        }
    }

    pub(crate) fn update(&mut self, message: Message) -> (Action, Task<Message>) {
        match message {
            Message::JobStarted { job_id, filename } => {
                self.push(LogEntry {
                    job_id,
                    timestamp: SystemTime::now(),
                    phase: None,
                    message: format!("── FFmpeg job started: {filename} ──"),
                    tone: Tone::Normal,
                });
                self.follow_task()
            }
            Message::Append { job_id, events } => {
                for event in events {
                    let tone = classify(&event);
                    self.push(LogEntry {
                        job_id: job_id.clone(),
                        timestamp: event.timestamp,
                        phase: Some(event.phase),
                        message: event.message,
                        tone,
                    });
                }
                self.follow_task()
            }
            Message::JobFinished { job_id, status } => {
                let (label, tone) = match status {
                    JobTerminalStatus::Completed => ("completed", Tone::Normal),
                    JobTerminalStatus::Failed => ("failed", Tone::Error),
                    JobTerminalStatus::Cancelled => ("cancelled", Tone::Warning),
                };
                self.push(LogEntry {
                    job_id,
                    timestamp: SystemTime::now(),
                    phase: None,
                    message: format!("── FFmpeg job {label} ──"),
                    tone,
                });
                self.follow_task()
            }
            Message::Clear => {
                self.entries.clear();
                self.bytes = 0;
                self.evicted = 0;
                self.follow_tail = true;
                (Action::None, Task::none())
            }
            Message::Scrolled(viewport) => {
                let content_height = viewport.content_bounds().height;
                let viewport_height = viewport.bounds().height;
                self.follow_tail =
                    content_height <= viewport_height + 1.0 || viewport.relative_offset().y >= 0.98;
                (Action::None, Task::none())
            }
            Message::Save => {
                if self.saving || self.entries.is_empty() {
                    return (Action::None, Task::none());
                }
                self.saving = true;
                let snapshot = self.snapshot();
                (
                    Action::None,
                    Task::perform(save_snapshot(snapshot), |result| {
                        Message::SaveCompleted(result.map_err(Arc::new))
                    }),
                )
            }
            Message::SaveCompleted(result) => {
                self.saving = false;
                match result {
                    Ok(Some(path)) => (Action::Saved(path), Task::none()),
                    Ok(None) => (Action::None, Task::none()),
                    Err(error) => (Action::SaveFailed(error.to_string()), Task::none()),
                }
            }
        }
    }

    fn push(&mut self, entry: LogEntry) {
        self.bytes = self.bytes.saturating_add(entry.message.len());
        self.entries.push_back(entry);
        while self.entries.len() > MAX_LINES || self.bytes > MAX_BYTES {
            if let Some(oldest) = self.entries.pop_front() {
                self.bytes = self.bytes.saturating_sub(oldest.message.len());
                self.evicted = self.evicted.saturating_add(1);
            }
        }
    }

    fn follow_task(&self) -> (Action, Task<Message>) {
        if self.follow_tail {
            (
                Action::None,
                iced::widget::operation::snap_to_end(LOG_SCROLL_ID),
            )
        } else {
            (Action::None, Task::none())
        }
    }

    fn snapshot(&self) -> String {
        let mut output = self
            .entries
            .iter()
            .map(LogEntry::render)
            .collect::<Vec<_>>()
            .join("\n");
        if !output.is_empty() {
            output.push('\n');
        }
        output
    }

    pub(crate) fn view(&self) -> Element<'_, Message> {
        let clear = button(
            row![icon::remove(ICON_MUTED), text("Clear Log").size(12)]
                .spacing(7)
                .align_y(iced::Alignment::Center),
        )
        .padding(Padding::from([7, 11]))
        .style(secondary_action)
        .on_press_maybe((!self.entries.is_empty()).then_some(Message::Clear));
        let save = button(
            row![
                icon::save(ICON_MUTED),
                text(if self.saving {
                    "Saving…"
                } else {
                    "Save Log…"
                })
                .size(12)
            ]
            .spacing(7)
            .align_y(iced::Alignment::Center),
        )
        .padding(Padding::from([7, 11]))
        .style(secondary_action)
        .on_press_maybe((!self.saving && !self.entries.is_empty()).then_some(Message::Save));

        let content: Element<'_, Message> = if self.entries.is_empty() {
            container(
                column![
                    text("FFmpeg output will appear when the queue starts.").size(13),
                    text("Structured tracing continues independently of this view.")
                        .size(12)
                        .color(TEXT_MUTED),
                ]
                .spacing(5),
            )
            .width(Fill)
            .padding(16)
            .into()
        } else {
            let rows = self
                .entries
                .iter()
                .fold(column![], |rows, entry| rows.push(log_row(entry)));
            scrollable(rows)
                .id(LOG_SCROLL_ID)
                .width(Fill)
                .height(Fill)
                .on_scroll(Message::Scrolled)
                .into()
        };

        let count = if self.evicted == 0 {
            format!("{} retained", self.entries.len())
        } else {
            format!(
                "{} retained · {} older lines evicted",
                self.entries.len(),
                self.evicted
            )
        };

        container(
            column![
                row![
                    text("FFmpeg Log").size(17).font(Font {
                        weight: Weight::Semibold,
                        ..Font::default()
                    }),
                    space::horizontal(),
                    text(count).size(11).color(TEXT_MUTED),
                    clear,
                    save,
                ]
                .spacing(8)
                .align_y(iced::Alignment::Center),
                container(content)
                    .width(Fill)
                    .height(Fill)
                    .style(inset_panel),
            ]
            .spacing(10),
        )
        .width(Fill)
        .height(Fill)
        .padding(Padding::from([14, 16]))
        .style(panel)
        .into()
    }
}

fn log_row(entry: &LogEntry) -> Element<'_, Message> {
    let phase = entry.phase.map_or("", phase_label);
    row![
        text(format_timestamp(entry.timestamp))
            .size(11)
            .color(TEXT_MUTED)
            .width(68),
        text(format!("job {}", entry.job_id.0))
            .size(11)
            .color(TEXT_MUTED)
            .width(44),
        text(phase).size(11).color(TEXT_MUTED).width(54),
        text(&entry.message).size(12).color(entry.tone.color()),
    ]
    .spacing(8)
    .align_y(iced::Alignment::Center)
    .into()
}

const fn phase_label(phase: RipPhase) -> &'static str {
    match phase {
        RipPhase::Analyzing => "analysis",
        RipPhase::Encoding => "encode",
    }
}

fn classify(event: &FfmpegLogEvent) -> Tone {
    if event.omitted.is_some() {
        return Tone::Omitted;
    }
    let message = event.message.to_ascii_lowercase();
    if message.contains("error") || message.contains("failed") || message.contains("invalid") {
        Tone::Error
    } else if message.contains("warning") || message.contains("deprecated") {
        Tone::Warning
    } else if message.starts_with("input")
        || message.starts_with("duration")
        || message.starts_with("stream")
        || message.starts_with("ffmpeg version")
    {
        Tone::Detail
    } else {
        Tone::Normal
    }
}

fn format_timestamp(timestamp: SystemTime) -> String {
    let timestamp = OffsetDateTime::from(timestamp);
    let offset = UtcOffset::current_local_offset().unwrap_or(UtcOffset::UTC);
    timestamp
        .to_offset(offset)
        .format(&format_description!("[hour]:[minute]:[second]"))
        .unwrap_or_else(|_| "--:--:--".to_owned())
}

async fn save_snapshot(snapshot: String) -> crate::Result<Option<PathBuf>> {
    let filename = OffsetDateTime::now_utc()
        .format(&format_description!(
            "demux-[year][month][day]-[hour][minute][second].log"
        ))
        .unwrap_or_else(|_| "demux-ffmpeg.log".to_owned());
    let Some(file) = rfd::AsyncFileDialog::new()
        .set_title("Save FFmpeg log")
        .set_file_name(filename)
        .add_filter("Log files", &["log"])
        .save_file()
        .await
    else {
        return Ok(None);
    };

    let path = file.path().to_path_buf();
    file.write(snapshot.as_bytes())
        .await
        .map_err(|source| Error::LogWrite {
            path: path.clone(),
            source,
        })?;
    Ok(Some(path))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn event(message: &str) -> FfmpegLogEvent {
        FfmpegLogEvent::line(SystemTime::UNIX_EPOCH, RipPhase::Encoding, message.into())
    }

    #[test]
    fn retention_evicts_by_line_and_byte_limits() {
        let mut logs = Logs::new();
        for index in 0..MAX_LINES + 10 {
            let _ = logs.update(Message::Append {
                job_id: JobId::new(1),
                events: vec![event(&format!("line {index}"))],
            });
        }
        assert_eq!(logs.entries.len(), MAX_LINES);
        assert_eq!(logs.evicted, 10);

        for _ in 0..20 {
            let _ = logs.update(Message::Append {
                job_id: JobId::new(1),
                events: vec![event(&"x".repeat(40_000))],
            });
        }
        assert!(logs.bytes <= MAX_BYTES);
        assert!(logs.entries.len() < MAX_LINES);
    }

    #[test]
    fn clear_removes_retained_lines_and_eviction_count() {
        let mut logs = Logs::new();
        let _ = logs.update(Message::Append {
            job_id: JobId::new(2),
            events: vec![event("hello")],
        });
        let _ = logs.update(Message::Clear);
        assert_eq!(logs.entries.len(), 0);
        assert_eq!(logs.evicted, 0);
        assert_eq!(logs.bytes, 0);
    }

    #[test]
    fn snapshot_preserves_display_order_and_timestamp_prefixes() {
        let mut logs = Logs::new();
        let _ = logs.update(Message::Append {
            job_id: JobId::new(3),
            events: vec![event("first"), event("second")],
        });
        let snapshot = logs.snapshot();
        assert!(snapshot.contains("first\n"));
        assert!(snapshot.contains("second\n"));
        assert!(snapshot.ends_with('\n'));
    }

    #[test]
    fn classification_uses_semantic_tones_without_parsing_terminal_control() {
        assert_eq!(classify(&event("Error opening input")), Tone::Error);
        assert_eq!(classify(&event("Duration: 00:01")), Tone::Detail);
        assert_eq!(classify(&event("Press [q] to stop")), Tone::Normal);
    }
}
