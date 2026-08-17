use std::time::{Duration, SystemTime};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RipPhase {
    Analyzing,
    Encoding,
}

/// One user-facing line emitted by an `FFmpeg` phase.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FfmpegLogEvent {
    pub timestamp: SystemTime,
    pub phase: RipPhase,
    pub message: String,
    pub omitted: Option<usize>,
}

impl FfmpegLogEvent {
    /// Creates a normal `FFmpeg` output event.
    ///
    /// # Parameters
    ///
    /// - `timestamp`: Wall-clock time when the line was observed.
    /// - `phase`: Extraction phase that produced the line.
    /// - `message`: Sanitized `FFmpeg` output text.
    ///
    /// # Returns
    ///
    /// A user-facing output event without an omission marker.
    #[must_use]
    pub const fn line(timestamp: SystemTime, phase: RipPhase, message: String) -> Self {
        Self {
            timestamp,
            phase,
            message,
            omitted: None,
        }
    }

    /// Creates an event describing lines dropped under bounded backpressure.
    ///
    /// # Parameters
    ///
    /// - `timestamp`: Wall-clock time when the omission was recorded.
    /// - `phase`: Extraction phase that dropped the lines.
    /// - `count`: Number of lines that were not retained.
    ///
    /// # Returns
    ///
    /// A user-facing omission event that preserves the dropped-line count.
    #[must_use]
    pub fn omitted(timestamp: SystemTime, phase: RipPhase, count: usize) -> Self {
        Self {
            timestamp,
            phase,
            message: format!(
                "{count} FFmpeg line{} omitted because the log consumer was busy",
                if count == 1 { " was" } else { "s were" }
            ),
            omitted: Some(count),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct RipProgressEvent {
    pub phase: RipPhase,
    pub progress: FfmpegProgress,
}

/// One machine-readable progress snapshot emitted by `FFmpeg`.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct FfmpegProgress {
    pub elapsed: Option<Duration>,
    pub speed: Option<f64>,
    pub bitrate_kbps: Option<f64>,
    pub output_size: Option<u64>,
    pub status: ProgressStatus,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ProgressStatus {
    #[default]
    Continuing,
    End,
}

/// Accumulates `FFmpeg` `key=value` lines into complete progress records.
#[derive(Debug, Default)]
pub struct ProgressParser {
    current: FfmpegProgress,
    latest_elapsed: Option<Duration>,
}

impl ProgressParser {
    #[must_use]
    pub fn push_line(&mut self, line: &str) -> Option<FfmpegProgress> {
        let (key, value) = line.trim().split_once('=')?;

        match key {
            "out_time_us" | "out_time_ms" => {
                if let Some(elapsed) = parse_microseconds(value) {
                    self.current.elapsed = Some(elapsed);
                }
            }
            "out_time" if self.current.elapsed.is_none() => {
                if let Some(elapsed) = parse_timestamp(value) {
                    self.current.elapsed = Some(elapsed);
                }
            }
            "speed" => self.current.speed = parse_suffixed_f64(value, "x"),
            "bitrate" => {
                self.current.bitrate_kbps = parse_suffixed_f64(value, "kbits/s");
            }
            "total_size" => self.current.output_size = value.parse().ok(),
            "progress" => {
                self.current.status = if value == "end" {
                    ProgressStatus::End
                } else {
                    ProgressStatus::Continuing
                };

                if let Some(elapsed) = self.current.elapsed {
                    let monotonic = self
                        .latest_elapsed
                        .map_or(elapsed, |latest| latest.max(elapsed));
                    self.current.elapsed = Some(monotonic);
                    self.latest_elapsed = Some(monotonic);
                }

                let snapshot = std::mem::take(&mut self.current);
                return Some(snapshot);
            }
            _ => {}
        }

        None
    }
}

fn parse_microseconds(value: &str) -> Option<Duration> {
    value.parse::<u64>().ok().map(Duration::from_micros)
}

fn parse_suffixed_f64(value: &str, suffix: &str) -> Option<f64> {
    let value = value.trim().strip_suffix(suffix)?.trim().parse().ok()?;
    (f64::is_finite(value) && value >= 0.0).then_some(value)
}

fn parse_timestamp(value: &str) -> Option<Duration> {
    let mut parts = value.trim().split(':');
    let hours: u64 = parts.next()?.parse().ok()?;
    let minutes: u64 = parts.next()?.parse().ok()?;
    let seconds = parts.next()?;
    if parts.next().is_some() || minutes >= 60 {
        return None;
    }

    let whole_seconds = parse_whole_seconds(seconds)?;
    if whole_seconds >= 60 {
        return None;
    }
    let whole = hours
        .checked_mul(3_600)?
        .checked_add(minutes.checked_mul(60)?)?
        .checked_add(whole_seconds)?;
    let fraction = if seconds.contains('.') {
        parse_fractional_seconds(seconds)?
    } else {
        Duration::ZERO
    };
    Duration::from_secs(whole).checked_add(fraction)
}

fn parse_whole_seconds(value: &str) -> Option<u64> {
    value.split('.').next()?.parse().ok()
}

fn parse_fractional_seconds(value: &str) -> Option<Duration> {
    let fraction = value.split_once('.')?.1;
    if fraction.is_empty() || !fraction.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    let mut nanos = 0_u32;
    let mut digits = 0_u32;
    for byte in fraction.bytes().take(9) {
        nanos = nanos.checked_mul(10)?.checked_add(u32::from(byte - b'0'))?;
        digits += 1;
    }
    let scale = 10_u32.pow(9 - digits);
    Some(Duration::from_nanos(u64::from(nanos.saturating_mul(scale))))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_complete_machine_readable_records() {
        let mut parser = ProgressParser::default();
        for line in [
            "total_size=24000",
            "out_time_us=2500000",
            "bitrate=192.0kbits/s",
            "speed=1.25x",
        ] {
            assert_eq!(parser.push_line(line), None);
        }

        assert_eq!(
            parser.push_line("progress=continue"),
            Some(FfmpegProgress {
                elapsed: Some(Duration::from_millis(2_500)),
                speed: Some(1.25),
                bitrate_kbps: Some(192.0),
                output_size: Some(24_000),
                status: ProgressStatus::Continuing,
            })
        );
    }

    #[test]
    fn malformed_fields_remain_unknown() {
        let mut parser = ProgressParser::default();
        for line in [
            "out_time_us=N/A",
            "bitrate=N/A",
            "speed=N/A",
            "total_size=N/A",
        ] {
            assert_eq!(parser.push_line(line), None);
        }

        assert_eq!(
            parser.push_line("progress=continue"),
            Some(FfmpegProgress::default())
        );

        assert_eq!(parse_timestamp("99:99:99.0"), None);
        assert_eq!(parse_timestamp("18446744073709551615:00:00.0"), None);
        assert_eq!(parse_suffixed_f64("-1.0x", "x"), None);
    }

    #[test]
    fn elapsed_time_never_moves_backwards() {
        let mut parser = ProgressParser::default();
        let _ = parser.push_line("out_time_us=3000000");
        let first = parser.push_line("progress=continue").unwrap();
        let _ = parser.push_line("out_time_us=2000000");
        let second = parser.push_line("progress=end").unwrap();

        assert_eq!(first.elapsed, Some(Duration::from_secs(3)));
        assert_eq!(second.elapsed, Some(Duration::from_secs(3)));
        assert_eq!(second.status, ProgressStatus::End);
    }

    #[test]
    fn falls_back_to_timestamp_when_microseconds_are_absent() {
        let mut parser = ProgressParser::default();
        let _ = parser.push_line("out_time=01:02:03.500000");
        let snapshot = parser.push_line("progress=continue").unwrap();

        assert_eq!(snapshot.elapsed, Some(Duration::from_millis(3_723_500)));
    }
}
