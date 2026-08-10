use std::time::Duration;

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct JobId(pub u64);

impl JobId {
    #[must_use]
    pub const fn new(id: u64) -> Self {
        Self(id)
    }
}

#[derive(Debug, Clone)]
pub struct RipJob {
    pub id: JobId,
    pub input: String,
    pub output: String,
    pub status: JobStatus,
    pub metadata: MediaInfo,
    pub progress: RipProgress,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RipProgress {
    pub elapsed: Duration,
    pub duration: Duration,
    pub percent: f64,
    pub speed: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum JobStatus {
    Pending,
    Probing,
    Ready,
    Ripping,
    Completed,
    Failed(String),
    Cancelled,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MediaInfo {
    pub duration: Duration,
    pub audio_codec: Option<String>,
    pub sample_rate: Option<u32>,
    pub channels: Option<u8>,
}
