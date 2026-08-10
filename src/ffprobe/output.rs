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
