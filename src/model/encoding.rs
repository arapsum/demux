use std::fmt;

use serde::{Deserialize, Serialize};

/// Audio container and encoder combinations currently supported by Demux.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OutputFormat {
    #[default]
    Mp3,
}

impl OutputFormat {
    pub const ALL: [Self; 1] = [Self::Mp3];

    #[must_use]
    pub const fn encoder(self) -> &'static str {
        match self {
            Self::Mp3 => "libmp3lame",
        }
    }

    #[must_use]
    pub const fn extension(self) -> &'static str {
        match self {
            Self::Mp3 => "mp3",
        }
    }
}

impl fmt::Display for OutputFormat {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Mp3 => "MP3",
        })
    }
}

/// Constant MP3 bitrates exposed by the application.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Mp3Bitrate {
    Kbps128,
    #[default]
    Kbps192,
    Kbps256,
    Kbps320,
}

impl Mp3Bitrate {
    pub const ALL: [Self; 4] = [Self::Kbps128, Self::Kbps192, Self::Kbps256, Self::Kbps320];

    #[must_use]
    pub const fn kbps(self) -> u32 {
        match self {
            Self::Kbps128 => 128,
            Self::Kbps192 => 192,
            Self::Kbps256 => 256,
            Self::Kbps320 => 320,
        }
    }
}

impl TryFrom<u32> for Mp3Bitrate {
    type Error = EncodingOptionError;

    fn try_from(value: u32) -> Result<Self, Self::Error> {
        match value {
            128 => Ok(Self::Kbps128),
            192 => Ok(Self::Kbps192),
            256 => Ok(Self::Kbps256),
            320 => Ok(Self::Kbps320),
            value => Err(EncodingOptionError::Bitrate(value)),
        }
    }
}

impl fmt::Display for Mp3Bitrate {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{} kbps", self.kbps())
    }
}

/// Sample rates supported by the MP3 encoder policy.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SampleRate {
    #[default]
    Hz44100,
    Hz48000,
}

impl SampleRate {
    pub const ALL: [Self; 2] = [Self::Hz44100, Self::Hz48000];

    #[must_use]
    pub const fn hz(self) -> u32 {
        match self {
            Self::Hz44100 => 44_100,
            Self::Hz48000 => 48_000,
        }
    }
}

impl TryFrom<u32> for SampleRate {
    type Error = EncodingOptionError;

    fn try_from(value: u32) -> Result<Self, Self::Error> {
        match value {
            44_100 => Ok(Self::Hz44100),
            48_000 => Ok(Self::Hz48000),
            value => Err(EncodingOptionError::SampleRate(value)),
        }
    }
}

impl fmt::Display for SampleRate {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Hz44100 => "44.1 kHz",
            Self::Hz48000 => "48 kHz",
        })
    }
}

/// Channel layouts supported by the MP3 encoder policy.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChannelMode {
    Mono,
    #[default]
    Stereo,
}

impl ChannelMode {
    pub const ALL: [Self; 2] = [Self::Mono, Self::Stereo];

    #[must_use]
    pub const fn channels(self) -> u8 {
        match self {
            Self::Mono => 1,
            Self::Stereo => 2,
        }
    }
}

impl TryFrom<u8> for ChannelMode {
    type Error = EncodingOptionError;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            1 => Ok(Self::Mono),
            2 => Ok(Self::Stereo),
            value => Err(EncodingOptionError::ChannelCount(value)),
        }
    }
}

impl fmt::Display for ChannelMode {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Mono => "Mono",
            Self::Stereo => "Stereo",
        })
    }
}

/// A validated snapshot of the encoding settings for one extraction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct RipOptions {
    pub format: OutputFormat,
    pub bitrate: Mp3Bitrate,
    pub sample_rate: SampleRate,
    pub channels: ChannelMode,
    #[serde(default = "default_enabled")]
    pub embed_metadata: bool,
    #[serde(default = "default_enabled")]
    pub extract_artwork: bool,
    #[serde(default)]
    pub normalize_audio: bool,
}

impl Default for RipOptions {
    fn default() -> Self {
        Self {
            format: OutputFormat::default(),
            bitrate: Mp3Bitrate::default(),
            sample_rate: SampleRate::default(),
            channels: ChannelMode::default(),
            embed_metadata: true,
            extract_artwork: true,
            normalize_audio: false,
        }
    }
}

const fn default_enabled() -> bool {
    true
}

impl RipOptions {
    pub fn try_new(
        bitrate_kbps: u32,
        sample_rate_hz: u32,
        channels: u8,
    ) -> Result<Self, EncodingOptionError> {
        Ok(Self {
            format: OutputFormat::Mp3,
            bitrate: Mp3Bitrate::try_from(bitrate_kbps)?,
            sample_rate: SampleRate::try_from(sample_rate_hz)?,
            channels: ChannelMode::try_from(channels)?,
            ..Self::default()
        })
    }

    #[must_use]
    pub const fn encoder(self) -> &'static str {
        self.format.encoder()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum EncodingOptionError {
    #[error("unsupported MP3 bitrate: {0} kbps")]
    Bitrate(u32),
    #[error("unsupported MP3 sample rate: {0} Hz")]
    SampleRate(u32),
    #[error("unsupported audio channel count: {0}")]
    ChannelCount(u8),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn raw_values_are_validated_at_the_encoding_boundary() {
        assert_eq!(
            RipOptions::try_new(256, 48_000, 1).unwrap(),
            RipOptions {
                format: OutputFormat::Mp3,
                bitrate: Mp3Bitrate::Kbps256,
                sample_rate: SampleRate::Hz48000,
                channels: ChannelMode::Mono,
                embed_metadata: true,
                extract_artwork: true,
                normalize_audio: false,
            }
        );
        assert_eq!(
            RipOptions::try_new(224, 44_100, 2).unwrap_err(),
            EncodingOptionError::Bitrate(224)
        );
        assert_eq!(
            RipOptions::try_new(192, 96_000, 2).unwrap_err(),
            EncodingOptionError::SampleRate(96_000)
        );
        assert_eq!(
            RipOptions::try_new(192, 44_100, 6).unwrap_err(),
            EncodingOptionError::ChannelCount(6)
        );
    }

    #[test]
    fn persisted_options_default_new_metadata_controls_to_enabled() {
        let options: RipOptions = serde_json::from_str(
            r#"{"format":"mp3","bitrate":"kbps192","sample_rate":"hz44100","channels":"stereo"}"#,
        )
        .unwrap();

        assert!(options.embed_metadata);
        assert!(options.extract_artwork);
        assert!(!options.normalize_audio);
    }
}
