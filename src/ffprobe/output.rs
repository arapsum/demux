use std::time::Duration;

use serde::Deserialize;

use crate::model::media::{AudioMetadata, MediaInfo};

use super::error::ProbeError;

/// Raw JSON output returned by `ffprobe`.
#[derive(Debug, Clone, Deserialize)]
pub(super) struct ProbeOutput {
    pub streams: Vec<StreamInfo>,
    pub format: ProbeFormat,
}

/// Raw JSON details for a single media stream.
#[derive(Debug, Clone, Default, Deserialize)]
pub(super) struct StreamInfo {
    pub index: usize,
    pub codec_name: Option<String>,
    pub sample_rate: Option<String>,
    pub channels: Option<u8>,
    pub channel_layout: Option<String>,
    pub bit_rate: Option<String>,
    #[serde(default)]
    pub tags: StreamTags,
}

/// Raw JSON tags attached to a media stream.
#[derive(Debug, Clone, Default, Deserialize)]
pub(super) struct StreamTags {
    pub language: Option<String>,
}

/// Raw JSON details for the media container.
#[derive(Debug, Clone, Default, Deserialize)]
pub(super) struct ProbeFormat {
    pub format_name: String,
    pub duration: String,
    pub bit_rate: Option<String>,
    #[serde(default)]
    pub tags: FormatTags,
}

/// Raw JSON tags attached to the media container.
#[derive(Debug, Clone, Default, Deserialize)]
pub(super) struct FormatTags {
    pub creation_time: Option<String>,
}

impl TryFrom<ProbeOutput> for MediaInfo {
    type Error = ProbeError;

    fn try_from(value: ProbeOutput) -> Result<Self, Self::Error> {
        let audio = value
            .streams
            .into_iter()
            .next()
            .ok_or(ProbeError::NoAudio)?;

        let duration = value
            .format
            .duration
            .parse::<f64>()
            .map_err(ProbeError::InvalidDuration)?;

        Ok(Self {
            duration: Duration::from_secs_f64(duration),
            container: value.format.format_name,
            bitrate: value
                .format
                .bit_rate
                .and_then(|value| value.parse::<u64>().ok()),
            creation_time: value.format.tags.creation_time,
            audio: AudioMetadata {
                stream_index: audio.index,
                codec: audio.codec_name.unwrap_or_else(|| "unknown".into()),
                sample_rate: audio
                    .sample_rate
                    .and_then(|value| value.parse::<u32>().ok()),
                channels: audio.channels,
                channel_layout: audio.channel_layout,
                bitrate: audio.bit_rate.and_then(|value| value.parse::<u64>().ok()),
                language: audio.tags.language,
            },
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn converts_probe_json_into_media_info() {
        let output: ProbeOutput = serde_json::from_str(
            r#"
            {
                "streams": [{
                    "index": 1,
                    "codec_name": "aac",
                    "sample_rate": "48000",
                    "channels": 2,
                    "channel_layout": "stereo",
                    "bit_rate": "192000",
                    "tags": {"language": "eng"}
                }],
                "format": {
                    "format_name": "mov,mp4,m4a,3gp,3g2,mj2",
                    "duration": "125.5",
                    "bit_rate": "1000000",
                    "tags": {"creation_time": "2026-08-10T12:00:00Z"}
                }
            }
            "#,
        )
        .unwrap();

        let metadata = MediaInfo::try_from(output).unwrap();

        assert_eq!(metadata.duration, Duration::from_secs_f64(125.5));
        assert_eq!(metadata.container, "mov,mp4,m4a,3gp,3g2,mj2");
        assert_eq!(metadata.bitrate, Some(1_000_000));
        assert_eq!(
            metadata.creation_time.as_deref(),
            Some("2026-08-10T12:00:00Z")
        );
        assert_eq!(metadata.audio.stream_index, 1);
        assert_eq!(metadata.audio.codec, "aac");
        assert_eq!(metadata.audio.sample_rate, Some(48_000));
        assert_eq!(metadata.audio.channels, Some(2));
        assert_eq!(metadata.audio.channel_layout.as_deref(), Some("stereo"));
        assert_eq!(metadata.audio.bitrate, Some(192_000));
        assert_eq!(metadata.audio.language.as_deref(), Some("eng"));
    }

    #[test]
    fn missing_audio_stream_returns_no_audio_error() {
        let output = ProbeOutput {
            streams: Vec::new(),
            format: ProbeFormat::default(),
        };

        assert!(matches!(
            MediaInfo::try_from(output),
            Err(ProbeError::NoAudio)
        ));
    }

    #[test]
    fn invalid_duration_returns_parse_error() {
        let output = ProbeOutput {
            streams: vec![StreamInfo::default()],
            format: ProbeFormat {
                duration: "not-a-duration".to_owned(),
                ..ProbeFormat::default()
            },
        };

        assert!(matches!(
            MediaInfo::try_from(output),
            Err(ProbeError::InvalidDuration(_))
        ));
    }

    #[test]
    fn invalid_optional_numeric_fields_are_ignored() {
        let output = ProbeOutput {
            streams: vec![StreamInfo {
                codec_name: None,
                sample_rate: Some("not-a-number".to_owned()),
                bit_rate: Some("not-a-number".to_owned()),
                ..StreamInfo::default()
            }],
            format: ProbeFormat {
                duration: "1".to_owned(),
                bit_rate: Some("not-a-number".to_owned()),
                ..ProbeFormat::default()
            },
        };

        let metadata = MediaInfo::try_from(output).unwrap();

        assert_eq!(metadata.bitrate, None);
        assert_eq!(metadata.audio.codec, "unknown");
        assert_eq!(metadata.audio.sample_rate, None);
        assert_eq!(metadata.audio.bitrate, None);
    }
}
