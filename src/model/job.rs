use super::media::MediaInfo;

use std::time::Duration;

/// Identifies a single audio-ripping job.
///
/// Job identifiers are assigned by the owning workflow or GUI queue in creation
/// order and remain stable for the lifetime of a [`RipJob`].
///
/// # Fields
///
/// - `0`: The numeric job identifier.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct JobId(pub u64);

impl JobId {
    #[must_use]
    pub const fn new(id: u64) -> Self {
        Self(id)
    }
}

/// Represents one requested audio extraction operation.
///
/// A job is created in the [`Pending`](JobStatus::Pending) state and advances
/// through probing, readiness, ripping, and a terminal completed or failed
/// state.
///
/// # Fields
///
/// - `id`: The stable identifier assigned to this job.
/// - `input`: The source media file path.
/// - `output`: The destination path for the extracted audio.
/// - `status`: The current lifecycle state of the job.
/// - `metadata`: Media metadata discovered during probing, when successful.
/// - `progress`: The latest measured or estimated ripping progress.
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

/// Records the current progress of an audio-ripping job.
///
/// Values are initialized to zero and are updated as Demux learns the media
/// duration and receives progress information from `FFmpeg`.
///
/// # Fields
///
/// - `elapsed`: Media time processed so far.
/// - `duration`: Total media duration when it is known.
/// - `percent`: Completion percentage in the inclusive range from 0 to 100.
/// - `speed`: Processing speed relative to real time, when reported.
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

#[cfg(test)]
mod tests {
    use super::*;

    fn media_info() -> MediaInfo {
        MediaInfo {
            duration: Duration::from_secs(90),
            container: "mp4".to_owned(),
            bitrate: Some(1_000_000),
            creation_time: None,
            audio: super::super::media::AudioMetadata {
                stream_index: 0,
                codec: "aac".to_owned(),
                sample_rate: Some(48_000),
                channels: Some(2),
                channel_layout: Some("stereo".to_owned()),
                bitrate: Some(192_000),
                language: Some("eng".to_owned()),
            },
        }
    }

    #[test]
    fn new_job_starts_pending_with_default_progress() {
        let job = RipJob::new(JobId::new(7), "input.mp4".into(), "output.mp3".into());

        assert_eq!(job.id, JobId::new(7));
        assert_eq!(job.status, JobStatus::Pending);
        assert!(job.metadata.is_none());
        assert_eq!(job.progress, RipProgress::default());
    }

    #[test]
    fn job_lifecycle_records_metadata_and_terminal_status() {
        let mut job = RipJob::new(JobId::new(1), "input.mp4".into(), "output.mp3".into());
        let metadata = media_info();

        job.start_probing();
        assert_eq!(job.status, JobStatus::Probing);

        job.record_metadata(metadata);
        assert_eq!(job.status, JobStatus::Ready);
        assert_eq!(job.progress.duration, Duration::from_secs(90));
        assert!(job.metadata.is_some());

        job.start_ripping();
        assert_eq!(job.status, JobStatus::Ripping);

        job.complete();
        assert_eq!(job.status, JobStatus::Completed);
    }

    #[test]
    fn failed_job_keeps_the_failure_message() {
        let mut job = RipJob::new(JobId::new(1), "input.mp4".into(), "output.mp3".into());

        job.fail("ffprobe failed".to_owned());

        assert_eq!(job.status, JobStatus::Failed("ffprobe failed".to_owned()));
    }
}
