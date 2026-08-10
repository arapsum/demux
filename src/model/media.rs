use std::time::Duration;

/// Describes the container and primary audio stream in a media file.
///
/// Demux derives this domain representation from raw `ffprobe` JSON after
/// selecting the first audio stream from the source media.
///
/// # Fields
///
/// - `duration`: Total duration of the media file.
/// - `container`: The container format name reported by `ffprobe`.
/// - `bitrate`: Overall container bitrate in bits per second, when reported.
/// - `creation_time`: Container creation timestamp, when reported.
/// - `audio`: Details of the selected audio stream.
#[derive(Debug, Clone)]
pub struct MediaInfo {
    pub duration: Duration,
    pub container: String,
    pub bitrate: Option<u64>,
    pub creation_time: Option<String>,
    pub audio: AudioMetadata,
}

/// Describes the audio stream selected for a ripping job.
///
/// # Fields
///
/// - `stream_index`: The source stream index used to identify the audio.
/// - `codec`: The audio codec name, or `"unknown"` when it is unavailable.
/// - `sample_rate`: Samples per second, when reported.
/// - `channels`: Number of audio channels, when reported.
/// - `channel_layout`: Named channel arrangement, such as `stereo`.
/// - `bitrate`: Stream bitrate in bits per second, when reported.
/// - `language`: Stream language tag, when reported.
#[derive(Debug, Clone)]
pub struct AudioMetadata {
    pub stream_index: usize,
    pub codec: String,
    pub sample_rate: Option<u32>,
    pub channels: Option<u8>,
    pub channel_layout: Option<String>,
    pub bitrate: Option<u64>,
    pub language: Option<String>,
}
