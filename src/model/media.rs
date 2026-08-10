use std::time::Duration;

/// Describes the container and primary audio stream in a media file.
#[derive(Debug, Clone)]
pub struct MediaInfo {
    pub duration: Duration,
    pub container: String,
    pub bitrate: Option<u64>,
    pub creation_time: Option<String>,
    pub audio: AudioMetadata,
}

/// Describes the audio stream selected for a ripping job.
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
