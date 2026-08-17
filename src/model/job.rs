use super::{
    encoding::RipOptions,
    media::MediaInfo,
    source::{DestinationPolicy, SourceHierarchy},
};

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
/// - `metadata`: Media metadata, safe tags, and artwork discovered during
///   probing, when successful.
/// - `progress`: The latest measured or estimated ripping progress.
#[derive(Debug, Clone)]
pub struct RipJob {
    pub id: JobId,
    pub input: String,
    pub output: String,
    pub options: RipOptions,
    pub input_size: Option<u64>,
    pub source_hierarchy: Option<SourceHierarchy>,
    pub destination_policy: DestinationPolicy,
    pub status: JobStatus,
    pub metadata: Option<MediaInfo>,
    pub progress: RipProgress,
    pause_phase_analyzing: Option<bool>,
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
        Self::with_options(id, input, output, RipOptions::default())
    }

    #[must_use]
    pub fn with_options(id: JobId, input: String, output: String, options: RipOptions) -> Self {
        Self::with_options_and_destination(
            id,
            input,
            output,
            options,
            DestinationPolicy::default(),
            None,
        )
    }

    #[must_use]
    pub fn with_options_and_destination(
        id: JobId,
        input: String,
        output: String,
        options: RipOptions,
        destination_policy: DestinationPolicy,
        source_hierarchy: Option<SourceHierarchy>,
    ) -> Self {
        Self {
            id,
            input,
            output,
            options,
            input_size: None,
            source_hierarchy,
            destination_policy,
            status: JobStatus::Pending,
            metadata: None,
            progress: RipProgress::default(),
            pause_phase_analyzing: None,
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

    pub(crate) fn start_analyzing(&mut self) {
        self.status = JobStatus::Analyzing;
        self.progress.reset_for_phase();
    }

    pub(crate) fn reset_for_encoding(&mut self) {
        self.status = JobStatus::Ripping;
        self.progress.reset_for_phase();
    }

    pub(crate) fn record_progress(
        &mut self,
        elapsed: Option<Duration>,
        speed: Option<f64>,
        bitrate_kbps: Option<f64>,
        output_size: Option<u64>,
    ) {
        self.progress
            .update(elapsed, speed, bitrate_kbps, output_size);
    }

    pub(crate) fn queue(&mut self) {
        self.status = JobStatus::Queued;
    }

    pub(crate) const fn set_options(&mut self, options: RipOptions) {
        self.options = options;
    }

    pub(crate) fn skip(&mut self, message: String) {
        self.status = JobStatus::Skipped(message);
    }

    pub(crate) fn complete(&mut self) {
        self.progress.finish();
        self.status = JobStatus::Completed;
    }

    pub(crate) fn fail(&mut self, message: String) {
        self.status = JobStatus::Failed(message);
    }

    pub(crate) fn start_cancelling(&mut self) {
        self.status = JobStatus::Cancelling;
    }

    pub(crate) fn start_pausing(&mut self, analyzing: bool) {
        self.pause_phase_analyzing = Some(analyzing);
        self.status = JobStatus::Pausing;
    }

    pub(crate) fn mark_paused(&mut self, analyzing: bool) {
        self.pause_phase_analyzing = Some(analyzing);
        self.status = JobStatus::Paused;
    }

    pub(crate) fn start_resuming(&mut self) {
        self.status = JobStatus::Resuming;
    }

    pub(crate) fn mark_resumed(&mut self) {
        self.status = if self.pause_phase_analyzing == Some(true) {
            JobStatus::Analyzing
        } else {
            JobStatus::Ripping
        };
    }

    pub(crate) fn is_analyzing(&self) -> bool {
        matches!(self.status, JobStatus::Analyzing)
            || (matches!(
                self.status,
                JobStatus::Pausing | JobStatus::Paused | JobStatus::Resuming
            ) && self.pause_phase_analyzing == Some(true))
    }

    pub(crate) fn control_failed(&mut self, operation: crate::ffmpeg::PauseControlOperation) {
        self.status = match operation {
            crate::ffmpeg::PauseControlOperation::Pause => {
                if self.pause_phase_analyzing == Some(true) {
                    JobStatus::Analyzing
                } else {
                    JobStatus::Ripping
                }
            }
            crate::ffmpeg::PauseControlOperation::Resume => JobStatus::Paused,
        };
    }

    pub(crate) fn cancel(&mut self) {
        self.progress.remaining = None;
        self.status = JobStatus::Cancelled;
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
/// - `bitrate_kbps`: Current encoded bitrate, when reported.
/// - `output_size`: Current output size in bytes, when reported.
/// - `remaining`: Estimated wall-clock time remaining, when calculable.
#[derive(Debug, Clone, PartialEq)]
pub struct RipProgress {
    pub elapsed: Duration,
    pub duration: Duration,
    pub percent: f64,
    pub speed: Option<f64>,
    pub bitrate_kbps: Option<f64>,
    pub output_size: Option<u64>,
    pub remaining: Option<Duration>,
}

impl Default for RipProgress {
    fn default() -> Self {
        Self {
            elapsed: Duration::ZERO,
            duration: Duration::ZERO,
            percent: 0.0,
            speed: None,
            bitrate_kbps: None,
            output_size: None,
            remaining: None,
        }
    }
}

impl RipProgress {
    pub(crate) fn update(
        &mut self,
        elapsed: Option<Duration>,
        speed: Option<f64>,
        bitrate_kbps: Option<f64>,
        output_size: Option<u64>,
    ) {
        if let Some(elapsed) = elapsed {
            self.elapsed = self.elapsed.max(elapsed);
        }
        if speed.is_some_and(|speed| speed.is_finite() && speed > 0.0) {
            self.speed = speed;
        }
        if bitrate_kbps.is_some_and(|bitrate| bitrate.is_finite() && bitrate >= 0.0) {
            self.bitrate_kbps = bitrate_kbps;
        }
        if let Some(output_size) = output_size {
            self.output_size = Some(
                self.output_size
                    .map_or(output_size, |size| size.max(output_size)),
            );
        }

        if !self.duration.is_zero() {
            let measured = self.elapsed.as_secs_f64() / self.duration.as_secs_f64() * 100.0;
            self.percent = self.percent.max(measured.clamp(0.0, 100.0));
        }

        self.remaining = self.speed.and_then(|speed| {
            let remaining = self.duration.saturating_sub(self.elapsed);
            (!self.duration.is_zero())
                .then(|| Duration::from_secs_f64(remaining.as_secs_f64() / speed))
        });
    }

    pub(crate) const fn reset_for_phase(&mut self) {
        self.elapsed = Duration::ZERO;
        self.percent = 0.0;
        self.speed = None;
        self.bitrate_kbps = None;
        self.output_size = None;
        self.remaining = None;
    }

    pub(crate) const fn finish(&mut self) {
        if !self.duration.is_zero() {
            self.elapsed = self.duration;
        }
        self.percent = 100.0;
        self.remaining = Some(Duration::ZERO);
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum JobStatus {
    Pending,
    Probing,
    Ready,
    Queued,
    Analyzing,
    Ripping,
    Cancelling,
    Pausing,
    Paused,
    Resuming,
    Completed,
    Failed(String),
    Cancelled,
    Skipped(String),
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
            tags: Default::default(),
            artwork: None,
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
        assert_eq!(job.options, RipOptions::default());
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
        assert_eq!(job.progress.percent, 100.0);
    }

    #[test]
    fn failed_job_keeps_the_failure_message() {
        let mut job = RipJob::new(JobId::new(1), "input.mp4".into(), "output.mp3".into());

        job.fail("ffprobe failed".to_owned());

        assert_eq!(job.status, JobStatus::Failed("ffprobe failed".to_owned()));
    }

    #[test]
    fn ready_jobs_can_be_queued_and_failed_probes_can_be_skipped() {
        let mut queued = RipJob::new(JobId::new(1), "input.mp4".into(), "output.mp3".into());
        queued.record_metadata(media_info());
        queued.queue();

        let mut skipped = RipJob::new(JobId::new(2), "silent.mp4".into(), "silent.mp3".into());
        skipped.fail("No audio stream was found".into());
        skipped.skip("No audio stream was found".into());

        assert_eq!(queued.status, JobStatus::Queued);
        assert_eq!(
            skipped.status,
            JobStatus::Skipped("No audio stream was found".into())
        );
    }

    #[test]
    fn cancellation_has_an_explicit_transitional_and_terminal_state() {
        let mut job = RipJob::new(JobId::new(1), "input.mp4".into(), "output.mp3".into());
        job.record_metadata(media_info());
        job.start_ripping();

        job.start_cancelling();
        assert_eq!(job.status, JobStatus::Cancelling);
        job.cancel();
        assert_eq!(job.status, JobStatus::Cancelled);
    }

    #[test]
    fn progress_is_monotonic_and_estimates_remaining_time() {
        let mut job = RipJob::new(JobId::new(1), "input.mp4".into(), "output.mp3".into());
        job.record_metadata(media_info());
        job.start_ripping();

        job.record_progress(
            Some(Duration::from_secs(30)),
            Some(2.0),
            Some(192.0),
            Some(1_024),
        );
        job.record_progress(Some(Duration::from_secs(20)), None, None, Some(512));

        assert_eq!(job.progress.elapsed, Duration::from_secs(30));
        assert!((job.progress.percent - 100.0 / 3.0).abs() < 1e-10);
        assert_eq!(job.progress.speed, Some(2.0));
        assert_eq!(job.progress.bitrate_kbps, Some(192.0));
        assert_eq!(job.progress.output_size, Some(1_024));
        assert_eq!(job.progress.remaining, Some(Duration::from_secs(30)));
    }
}
