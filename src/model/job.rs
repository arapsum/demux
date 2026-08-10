use super::media::MediaInfo;

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
    pub metadata: Option<MediaInfo>,
    pub progress: RipProgress,
}

impl RipJob {
    /// Creates a pending audio-ripping job.
    ///
    /// # Parameters
    ///
    /// - `id`: The identifier assigned to the job.
    /// - `input`: The source media path.
    /// - `output`: The destination audio path.
    ///
    /// # Returns
    ///
    /// A new job with no metadata and zeroed progress.
    #[must_use]
    pub fn new(id: JobId, input: String, output: String) -> Self {
        Self {
            id,
            input,
            output,
            status: JobStatus::Pending,
            metadata: None,
            progress: RipProgress::default(),
        }
    }

    pub(crate) fn start_probing(&mut self) {
        self.status = JobStatus::Probing;
    }

    pub(crate) fn record_metadata(&mut self, metadata: MediaInfo) {
        self.progress.duration = metadata.duration;
        self.metadata = Some(metadata);
        self.status = JobStatus::Ready;
    }

    pub(crate) fn start_ripping(&mut self) {
        self.status = JobStatus::Ripping;
    }

    pub(crate) fn complete(&mut self) {
        self.status = JobStatus::Completed;
    }

    pub(crate) fn fail(&mut self, message: String) {
        self.status = JobStatus::Failed(message);
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct RipProgress {
    pub elapsed: Duration,
    pub duration: Duration,
    pub percent: f64,
    pub speed: Option<f64>,
}

impl Default for RipProgress {
    fn default() -> Self {
        Self {
            elapsed: Duration::ZERO,
            duration: Duration::ZERO,
            percent: 0.0,
            speed: None,
        }
    }
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
