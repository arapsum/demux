use std::{collections::BTreeMap, time::Duration};

use serde::Deserialize;
use serde_json::Value;

use crate::model::media::{ArtworkInfo, AudioMetadata, MediaInfo, MetadataTags};

use super::error::ProbeError;

/// Represents the top-level JSON document returned by `ffprobe`.
///
/// This wire-format type is internal to the probing module. It is converted
/// into [`MediaInfo`] after deserialization so the rest of Demux does not
/// depend on `ffprobe`'s JSON schema.
///
/// # Fields
///
/// - `streams`: Stream entries returned by the `ffprobe` command.
/// - `format`: Container-level metadata for the input media.
#[derive(Debug, Clone, Deserialize)]
pub(super) struct ProbeOutput {
    pub streams: Vec<StreamInfo>,
    pub format: ProbeFormat,
}

/// Represents a single raw stream entry from `ffprobe` JSON.
///
/// String-valued numeric fields preserve the source representation and are
/// parsed into numeric domain fields during conversion to [`MediaInfo`].
///
/// # Fields
///
/// - `index`: The stream index in the source media file.
/// - `codec_type`: The stream kind, such as `audio` or `video`.
/// - `codec_name`: The codec identifier, when reported.
/// - `sample_rate`: The sample rate as text, when reported.
/// - `channels`: The number of audio channels, when reported.
/// - `channel_layout`: The named channel arrangement, when reported.
/// - `bit_rate`: The stream bitrate as text, when reported.
/// - `width` and `height`: Image dimensions, when reported.
/// - `disposition`: Stream disposition flags used to identify cover art.
/// - `tags`: Associated stream tags, defaulting to an empty set.
#[derive(Debug, Clone, Default, Deserialize)]
pub(super) struct StreamInfo {
    pub index: usize,
    pub codec_type: Option<String>,
    pub codec_name: Option<String>,
    pub sample_rate: Option<String>,
    pub channels: Option<u8>,
    pub channel_layout: Option<String>,
    pub bit_rate: Option<String>,
    pub width: Option<u32>,
    pub height: Option<u32>,
    #[serde(default)]
    pub disposition: StreamDisposition,
    #[serde(default)]
    pub tags: StreamTags,
}

/// Raw stream dispositions used to identify attached cover art.
#[derive(Debug, Clone, Default, Deserialize)]
pub(super) struct StreamDisposition {
    #[serde(default)]
    pub attached_pic: u8,
}

/// Represents raw tags attached to a media stream or container in `ffprobe`
/// JSON. FFmpeg preserves the source key casing, so lookups are intentionally
/// case-insensitive. Non-string values are ignored as malformed metadata.
#[derive(Debug, Clone, Default, Deserialize)]
pub(super) struct RawTags {
    #[serde(flatten)]
    pub values: BTreeMap<String, Value>,
}

impl RawTags {
    fn value(&self, key: &str) -> Option<String> {
        self.values
            .iter()
            .find(|(candidate, _)| candidate.eq_ignore_ascii_case(key))
            .and_then(|(_, value)| value.as_str().map(ToOwned::to_owned))
    }
}

pub(super) type StreamTags = RawTags;

/// Represents raw container details from `ffprobe` JSON.
///
/// # Fields
///
/// - `format_name`: The comma-separated container format names.
/// - `duration`: The total media duration as text.
/// - `bit_rate`: The overall bitrate as text, when reported.
/// - `tags`: Associated container tags, defaulting to an empty set.
#[derive(Debug, Clone, Default, Deserialize)]
pub(super) struct ProbeFormat {
    pub format_name: String,
    pub duration: String,
    pub bit_rate: Option<String>,
    #[serde(default)]
    pub tags: FormatTags,
}

/// Represents raw tags attached to the media container in `ffprobe` JSON.
///
/// # Fields
///
/// - `creation_time`: The container creation timestamp, when present.
/// - safe audio tags: Optional title, artist, album, and related fields.
pub(super) type FormatTags = RawTags;

impl TryFrom<ProbeOutput> for MediaInfo {
    type Error = ProbeError;

    fn try_from(value: ProbeOutput) -> Result<Self, Self::Error> {
        let ProbeOutput { streams, format } = value;
        let mut attached_artwork = streams.iter().filter(|stream| {
            stream.codec_type.as_deref() == Some("video") && stream.disposition.attached_pic > 0
        });
        let artwork = attached_artwork
            .clone()
            .find(|stream| is_supported_artwork_codec(stream.codec_name.as_deref()))
            .or_else(|| attached_artwork.next())
            .map(|stream| ArtworkInfo {
                stream_index: stream.index,
                codec: stream
                    .codec_name
                    .clone()
                    .unwrap_or_else(|| "unknown".to_owned()),
                width: stream.width,
                height: stream.height,
                mime_type: mime_type_for_codec(stream.codec_name.as_deref()),
            });

        let audio_index = streams
            .iter()
            .position(|stream| stream.codec_type.as_deref() == Some("audio"))
            .or_else(|| (streams.len() == 1).then_some(0))
            .ok_or(ProbeError::NoAudio)?;
        let audio = streams
            .into_iter()
            .nth(audio_index)
            .ok_or(ProbeError::NoAudio)?;

        let duration = format
            .duration
            .parse::<f64>()
            .map_err(ProbeError::InvalidDuration)?;
        let tags = merge_tags(&format.tags, &audio.tags);

        Ok(Self {
            duration: Duration::from_secs_f64(duration),
            container: format.format_name,
            bitrate: format.bit_rate.and_then(|value| value.parse::<u64>().ok()),
            creation_time: clean_tag(format.tags.value("creation_time")),
            tags: Box::new(tags),
            artwork,
            audio: AudioMetadata {
                stream_index: audio.index,
                codec: audio.codec_name.unwrap_or_else(|| "unknown".into()),
                sample_rate: audio
                    .sample_rate
                    .and_then(|value| value.parse::<u32>().ok()),
                channels: audio.channels,
                channel_layout: audio.channel_layout,
                bitrate: audio.bit_rate.and_then(|value| value.parse::<u64>().ok()),
                language: clean_tag(audio.tags.value("language")),
            },
        })
    }
}

fn merge_tags(format: &FormatTags, stream: &StreamTags) -> MetadataTags {
    MetadataTags {
        title: prefer(format.value("title"), stream.value("title")),
        artist: prefer(format.value("artist"), stream.value("artist")),
        album: prefer(format.value("album"), stream.value("album")),
        album_artist: prefer(format.value("album_artist"), stream.value("album_artist")),
        date: prefer(
            format
                .value("date")
                .or_else(|| format.value("creation_time")),
            stream.value("date"),
        ),
        track: prefer(format.value("track"), stream.value("track")),
        disc: prefer(format.value("disc"), stream.value("disc")),
        genre: prefer(format.value("genre"), stream.value("genre")),
        composer: prefer(format.value("composer"), stream.value("composer")),
        comment: prefer(format.value("comment"), stream.value("comment")),
        copyright: prefer(format.value("copyright"), stream.value("copyright")),
        language: prefer(format.value("language"), stream.value("language")),
    }
}

fn prefer(primary: Option<String>, fallback: Option<String>) -> Option<String> {
    clean_tag(primary).or_else(|| clean_tag(fallback))
}

fn clean_tag(value: Option<String>) -> Option<String> {
    let value = value?.replace('\0', "");
    let value = value.trim();
    (!value.is_empty()).then(|| value.to_owned())
}

fn mime_type_for_codec(codec: Option<&str>) -> Option<String> {
    match codec?.to_ascii_lowercase().as_str() {
        "mjpeg" | "jpeg" | "jpg" => Some("image/jpeg".to_owned()),
        "png" => Some("image/png".to_owned()),
        _ => None,
    }
}

fn is_supported_artwork_codec(codec: Option<&str>) -> bool {
    matches!(
        codec.map(str::to_ascii_lowercase).as_deref(),
        Some("mjpeg" | "jpeg" | "jpg" | "png")
    )
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
                    "codec_type": "audio",
                    "codec_name": "aac",
                    "sample_rate": "48000",
                    "channels": 2,
                    "channel_layout": "stereo",
                    "bit_rate": "192000",
                    "tags": {"LANGUAGE": "eng", "ARTIST": "Stream Artist"}
                }],
                "format": {
                    "format_name": "mov,mp4,m4a,3gp,3g2,mj2",
                    "duration": "125.5",
                    "bit_rate": "1000000",
                    "tags": {
                        "creation_time": "2026-08-10T12:00:00Z",
                        "TITLE": "  Unicode 東京  ",
                        "ARTIST": "Format Artist",
                        "ALBUM": "Album"
                    }
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
        assert_eq!(metadata.tags.title.as_deref(), Some("Unicode 東京"));
        assert_eq!(metadata.tags.artist.as_deref(), Some("Format Artist"));
        assert_eq!(metadata.tags.album.as_deref(), Some("Album"));
        assert!(metadata.artwork.is_none());
    }

    #[test]
    fn prefers_a_compatible_artwork_stream_when_multiple_are_embedded() {
        let output: ProbeOutput = serde_json::from_str(
            r#"
            {
                "streams": [
                    {"index": 0, "codec_type": "audio", "codec_name": "aac"},
                    {
                        "index": 3,
                        "codec_type": "video",
                        "codec_name": "webp",
                        "width": 640,
                        "height": 480,
                        "disposition": {"attached_pic": 1}
                    },
                    {
                        "index": 4,
                        "codec_type": "video",
                        "codec_name": "png",
                        "width": 320,
                        "height": 320,
                        "disposition": {"attached_pic": 1}
                    }
                ],
                "format": {"format_name": "mp4", "duration": "1"}
            }
            "#,
        )
        .unwrap();

        let metadata = MediaInfo::try_from(output).unwrap();
        let artwork = metadata
            .artwork
            .expect("attached picture should be detected");
        assert_eq!(artwork.stream_index, 4);
        assert_eq!(artwork.format_label(), "PNG");
        assert!(artwork.supports_mp3());
    }

    #[test]
    fn keeps_unsupported_artwork_non_fatal_when_no_compatible_stream_exists() {
        let output = ProbeOutput {
            streams: vec![
                StreamInfo {
                    index: 0,
                    codec_type: Some("audio".to_owned()),
                    ..StreamInfo::default()
                },
                StreamInfo {
                    index: 3,
                    codec_type: Some("video".to_owned()),
                    codec_name: Some("webp".to_owned()),
                    disposition: StreamDisposition { attached_pic: 1 },
                    ..StreamInfo::default()
                },
            ],
            format: ProbeFormat {
                format_name: "mp4".to_owned(),
                duration: "1".to_owned(),
                ..ProbeFormat::default()
            },
        };

        let metadata = MediaInfo::try_from(output).unwrap();
        let artwork = metadata
            .artwork
            .expect("attached picture should be detected");
        assert_eq!(artwork.stream_index, 3);
        assert!(!artwork.supports_mp3());
    }

    #[test]
    fn malformed_tag_values_are_ignored_without_failing_probe_conversion() {
        let output: ProbeOutput = serde_json::from_str(
            r#"
            {
                "streams": [{
                    "index": 0,
                    "codec_type": "audio",
                    "tags": {"TITLE": 42}
                }],
                "format": {
                    "format_name": "mp4",
                    "duration": "1",
                    "tags": {"ARTIST": ["not", "a", "string"]}
                }
            }
            "#,
        )
        .unwrap();

        let metadata = MediaInfo::try_from(output).unwrap();
        assert!(metadata.tags.is_empty());
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
