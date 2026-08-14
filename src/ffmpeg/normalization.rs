use serde::Deserialize;

use super::error::{FFmpegError, FFmpegResult};

pub const TARGET_INTEGRATED_LUFS: f64 = -23.0;
/// Leave codec headroom during encoding; decoded MP3 output is expected to
/// remain below the public -1 dBTP ceiling.
pub const FILTER_TRUE_PEAK: f64 = -2.0;
pub const OUTPUT_TRUE_PEAK_LIMIT: f64 = -1.0;
/// A ceiling rather than a compression target: preserve the source dynamics.
pub const LRA_CEILING: f64 = 50.0;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LoudnessMeasurement {
    pub integrated_lufs: f64,
    pub loudness_range: f64,
    pub true_peak: f64,
    pub threshold: f64,
    pub offset: f64,
}

impl LoudnessMeasurement {
    pub fn parse(stderr: &[u8]) -> FFmpegResult<Self> {
        let text = String::from_utf8_lossy(stderr);
        let mut candidate = String::new();
        let mut in_object = false;
        let mut depth = 0_u32;
        let mut parsed = None;

        for line in text.lines() {
            let trimmed = line.trim();
            if !in_object && trimmed.starts_with('{') {
                candidate.clear();
                in_object = true;
                depth = 0;
            }
            if in_object {
                candidate.push_str(trimmed);
                if trimmed.starts_with('{') {
                    depth = depth.saturating_add(1);
                }
                if trimmed.ends_with('}') {
                    depth = depth.saturating_sub(1);
                    if depth == 0 {
                        if let Ok(raw) = serde_json::from_str::<RawMeasurement>(&candidate) {
                            parsed = Some(raw);
                        }
                        in_object = false;
                    }
                }
            }
        }

        let raw = parsed.ok_or(FFmpegError::LoudnessMeasurementMissing)?;
        let measurement = Self {
            integrated_lufs: raw.number("input_i", &raw.input_i)?,
            loudness_range: raw.number("input_lra", &raw.input_lra)?,
            true_peak: raw.number("input_tp", &raw.input_tp)?,
            threshold: raw.number("input_thresh", &raw.input_thresh)?,
            offset: raw.number("target_offset", &raw.target_offset)?,
        };
        if [
            measurement.integrated_lufs,
            measurement.loudness_range,
            measurement.true_peak,
            measurement.threshold,
            measurement.offset,
        ]
        .iter()
        .all(|value| value.is_finite())
        {
            Ok(measurement)
        } else {
            Err(FFmpegError::LoudnessMeasurementInvalid(
                "measurement contains a non-finite value".into(),
            ))
        }
    }
}

#[derive(Debug, Deserialize)]
struct RawMeasurement {
    input_i: serde_json::Value,
    input_lra: serde_json::Value,
    input_tp: serde_json::Value,
    input_thresh: serde_json::Value,
    target_offset: serde_json::Value,
}

impl RawMeasurement {
    fn number(&self, field: &'static str, value: &serde_json::Value) -> FFmpegResult<f64> {
        let number = match value {
            serde_json::Value::Number(number) => number.as_f64(),
            serde_json::Value::String(value) => value.parse().ok(),
            _ => None,
        }
        .ok_or_else(|| FFmpegError::LoudnessMeasurementInvalid(field.into()))?;
        if number.is_finite() {
            Ok(number)
        } else {
            Err(FFmpegError::LoudnessMeasurementInvalid(field.into()))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_the_json_block_emitted_by_loudnorm() {
        let measurement = LoudnessMeasurement::parse(
            br#"
 loudnorm summary
 {
   "input_i" : "-27.10",
   "input_lra" : "4.20",
   "input_tp" : "-1.20",
   "input_thresh" : "-38.00",
   "output_i" : "-23.00",
   "output_lra" : "4.00",
   "output_tp" : "-2.00",
   "output_thresh" : "-34.00",
   "normalization_type" : "dynamic",
   "target_offset" : "0.00"
 }
 "#,
        )
        .unwrap();
        assert_eq!(measurement.integrated_lufs, -27.10);
        assert_eq!(measurement.offset, 0.0);
    }

    #[test]
    fn rejects_missing_or_non_finite_measurements() {
        assert!(matches!(
            LoudnessMeasurement::parse(b"no json"),
            Err(FFmpegError::LoudnessMeasurementMissing)
        ));
        assert!(matches!(
            LoudnessMeasurement::parse(
                br#"{"input_i":"-inf","input_lra":"4","input_tp":"-1","input_thresh":"-2","target_offset":"0"}"#
            ),
            Err(FFmpegError::LoudnessMeasurementInvalid(_))
        ));
    }
}
