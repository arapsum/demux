use tracing::{Instrument, info_span};

use crate::{
    App, Result,
    ffmpeg::{Dependencies, DependencyState, RipRequest},
    model::{
        job::JobId,
        media::{ArtworkInfo, MediaInfo},
    },
};

use super::services::{AudioRipper, DependencyChecker, MediaProbe};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkflowStage {
    Probing,
    Ripping,
}

/// UI-neutral notifications emitted while a workflow advances.
#[derive(Debug, Clone)]
pub enum WorkflowEvent {
    CheckingDependencies,
    DependenciesReady(Dependencies),
    MetadataReady(Box<MediaInfo>),
    Ripping,
    Completed {
        output: String,
        status: String,
    },
    Failed {
        stage: WorkflowStage,
        message: String,
    },
    Finished,
}

/// Receives workflow events without making the workflow depend on a terminal
/// or desktop UI.
pub trait WorkflowReporter: Send {
    fn report(&mut self, event: WorkflowEvent);
}

/// Coordinates job state while delegating all external work to injected ports.
#[derive(Debug)]
pub struct RipWorkflow<D, P, R> {
    dependency_checker: D,
    media_probe: P,
    audio_ripper: R,
}

impl<D, P, R> RipWorkflow<D, P, R> {
    #[must_use]
    pub const fn new(dependency_checker: D, media_probe: P, audio_ripper: R) -> Self {
        Self {
            dependency_checker,
            media_probe,
            audio_ripper,
        }
    }
}

impl<D: DependencyChecker, P, R> RipWorkflow<D, P, R> {
    /// Detects the external tools required by the workflow.
    ///
    /// # Parameters
    ///
    /// - `app`: Application state that receives the dependency status.
    /// - `reporter`: Sink for UI-neutral dependency events.
    ///
    /// # Returns
    ///
    /// `Ok(())` when both dependencies are available.
    ///
    /// # Errors
    ///
    /// Returns an error when:
    ///
    /// - The dependency checker reports an unavailable or unusable executable.
    pub fn detect_dependencies(
        &self,
        app: &mut App,
        reporter: &mut impl WorkflowReporter,
    ) -> Result<()> {
        app.set_dependency_state(DependencyState::Checking);
        reporter.report(WorkflowEvent::CheckingDependencies);

        match self.dependency_checker.detect() {
            Ok(dependencies) => {
                tracing::info!(ffmpeg = %dependencies.ffmpeg_version, ffprobe = %dependencies.ffprobe_version, "external dependencies detected");

                app.set_dependency_state(DependencyState::Ready(dependencies.clone()));
                reporter.report(WorkflowEvent::DependenciesReady(dependencies));
                Ok(())
            }
            Err(error) => {
                tracing::error!(error = %error, "external dependency check failed");

                app.set_dependency_state(DependencyState::from(&error));
                Err(error.into())
            }
        }
    }
}

impl<D: Sync, P: MediaProbe + Sync, R: AudioRipper + Sync> RipWorkflow<D, P, R> {
    /// Runs one probe-and-rip job and reports each workflow transition.
    ///
    /// # Parameters
    ///
    /// - `app`: Application state that owns the job lifecycle.
    /// - `request`: Typed input, output, and encoding request.
    /// - `reporter`: Sink for UI-neutral job events.
    ///
    /// # Returns
    ///
    /// The stable identifier assigned to the job. Failures are reported as
    /// workflow events and do not change the return type.
    pub async fn run_job(
        &self,
        app: &mut App,
        mut request: RipRequest,
        reporter: &mut impl WorkflowReporter,
    ) -> JobId {
        let input = request.input.to_string_lossy().into_owned();
        let output = request.output.to_string_lossy().into_owned();
        let mut job = app.create_job(input, output, request.options);

        let span = info_span!(
            "rip_job",
            job_id = job.id.0,
            encoder = request.options.encoder(),
            bitrate_kbps = request.options.bitrate.kbps(),
            sample_rate_hz = request.options.sample_rate.hz(),
            channels = request.options.channels.channels(),
            embed_metadata = request.options.embed_metadata,
            extract_artwork = request.options.extract_artwork,
        );

        let id = job.id.clone();

        async move {
            tracing::info!("job started");
            tracing::debug!(
                input = %job.input,
                output = %job.output,
                "resolved job paths"
            );

            job.start_probing();

            match self.media_probe.inspect(&job.input).await {
                Ok(metadata) => {
                    tracing::debug!(
                        duration_ms = %metadata.duration.as_millis(),
                        container = %metadata.container,
                        audio_codec = %metadata.audio.codec,
                        stream_index = metadata.audio.stream_index,
                        sample_rate = ?metadata.audio.sample_rate,
                        channels = ?metadata.audio.channels,
                        metadata_tags = !metadata.tags.is_empty(),
                        artwork = metadata.artwork.as_ref().map(ArtworkInfo::format_label),
                        "media probe completed"
                    );

                    job.record_metadata(metadata.clone());
                    request.metadata = Some(metadata.clone());
                    reporter.report(WorkflowEvent::MetadataReady(Box::new(metadata)));
                }
                Err(error) => {
                    tracing::warn!(error = %error, stage = "probing", "job failed");

                    let message = error.to_string();
                    job.fail(message.clone());
                    reporter.report(WorkflowEvent::Failed {
                        stage: WorkflowStage::Probing,
                        message,
                    });
                    app.finish_job(job);
                    reporter.report(WorkflowEvent::Finished);
                    return id;
                }
            }

            job.start_ripping();
            tracing::info!("audio extraction started");
            reporter.report(WorkflowEvent::Ripping);

            match self.audio_ripper.rip(&request).await {
                Ok(outcome) => {
                    tracing::info!(status = %outcome.status, "job completed");

                    job.complete();
                    reporter.report(WorkflowEvent::Completed {
                        output: job.output.clone(),
                        status: outcome.status,
                    });
                }
                Err(error) => {
                    tracing::error!(error = %error, stage = "ripping", "job failed");

                    let message = error.to_string();
                    job.fail(message.clone());
                    reporter.report(WorkflowEvent::Failed {
                        stage: WorkflowStage::Ripping,
                        message,
                    });
                }
            }

            app.finish_job(job);
            reporter.report(WorkflowEvent::Finished);
            id
        }
        .instrument(span)
        .await
    }
}

#[cfg(test)]
mod tests {
    use std::{future::Future, pin::Pin, time::Duration};

    use crate::{
        ffmpeg::{Dependencies, DependencyError, FFmpegResult, RipOutcome, RipRequest},
        ffprobe::{ProbeError, ProbeResult},
        model::{
            job::JobStatus,
            media::{AudioMetadata, MediaInfo},
        },
    };

    use super::*;

    struct ReadyDependencies;

    impl DependencyChecker for ReadyDependencies {
        fn detect(&self) -> Result<Dependencies, DependencyError> {
            Ok(Dependencies {
                ffmpeg_version: "ffmpeg test".into(),
                ffprobe_version: "ffprobe test".into(),
            })
        }
    }

    struct MissingDependencies;

    impl DependencyChecker for MissingDependencies {
        fn detect(&self) -> Result<Dependencies, DependencyError> {
            Err(DependencyError::Missing { program: "ffmpeg" })
        }
    }

    struct SuccessfulProbe;

    impl MediaProbe for SuccessfulProbe {
        fn inspect<'a>(
            &'a self,
            _input: &'a str,
        ) -> Pin<Box<dyn Future<Output = ProbeResult<MediaInfo>> + Send + 'a>> {
            Box::pin(async { Ok(media_info()) })
        }
    }

    struct FailedProbe;

    impl MediaProbe for FailedProbe {
        fn inspect<'a>(
            &'a self,
            _input: &'a str,
        ) -> Pin<Box<dyn Future<Output = ProbeResult<MediaInfo>> + Send + 'a>> {
            Box::pin(async { Err(ProbeError::NoAudio) })
        }
    }

    struct SuccessfulRipper;

    impl AudioRipper for SuccessfulRipper {
        fn rip<'a>(
            &'a self,
            _request: &'a RipRequest,
        ) -> Pin<Box<dyn Future<Output = FFmpegResult<RipOutcome>> + Send + 'a>> {
            Box::pin(async {
                Ok(RipOutcome {
                    status: "exit status: 0".into(),
                })
            })
        }
    }

    struct UnexpectedRipper;

    impl AudioRipper for UnexpectedRipper {
        fn rip<'a>(
            &'a self,
            _request: &'a RipRequest,
        ) -> Pin<Box<dyn Future<Output = FFmpegResult<RipOutcome>> + Send + 'a>> {
            Box::pin(async { panic!("ripper must not run after probing fails") })
        }
    }

    struct FailedRipper;

    impl AudioRipper for FailedRipper {
        fn rip<'a>(
            &'a self,
            _request: &'a RipRequest,
        ) -> Pin<Box<dyn Future<Output = FFmpegResult<RipOutcome>> + Send + 'a>> {
            Box::pin(async {
                Err(std::io::Error::new(
                    std::io::ErrorKind::PermissionDenied,
                    "cannot write output",
                )
                .into())
            })
        }
    }

    #[derive(Default)]
    struct RecordingReporter(Vec<WorkflowEvent>);

    impl WorkflowReporter for RecordingReporter {
        fn report(&mut self, event: WorkflowEvent) {
            self.0.push(event);
        }
    }

    fn media_info() -> MediaInfo {
        MediaInfo {
            duration: Duration::from_secs(30),
            container: "mp4".into(),
            bitrate: None,
            creation_time: None,
            tags: Default::default(),
            artwork: None,
            audio: AudioMetadata {
                stream_index: 0,
                codec: "aac".into(),
                sample_rate: None,
                channels: None,
                channel_layout: None,
                bitrate: None,
                language: None,
            },
        }
    }

    #[test]
    fn dependency_check_updates_state_without_presenting_itself() {
        let workflow = RipWorkflow::new(ReadyDependencies, (), ());
        let mut app = App::new();
        let mut reporter = RecordingReporter::default();

        workflow
            .detect_dependencies(&mut app, &mut reporter)
            .unwrap();

        assert!(matches!(app.dependency_state(), DependencyState::Ready(_)));
        assert!(matches!(
            reporter.0.as_slice(),
            [
                WorkflowEvent::CheckingDependencies,
                WorkflowEvent::DependenciesReady(_)
            ]
        ));
    }

    #[test]
    fn dependency_failure_is_preserved_in_application_state() {
        let workflow = RipWorkflow::new(MissingDependencies, (), ());
        let mut app = App::new();
        let mut reporter = RecordingReporter::default();

        let error = workflow
            .detect_dependencies(&mut app, &mut reporter)
            .unwrap_err();

        assert!(error.to_string().contains("ffmpeg was not found"));
        assert_eq!(
            app.dependency_state(),
            &DependencyState::Missing { program: "ffmpeg" }
        );
    }

    #[tokio::test]
    async fn successful_services_drive_the_job_to_completion() {
        let workflow = RipWorkflow::new((), SuccessfulProbe, SuccessfulRipper);
        let mut app = App::new();
        let mut reporter = RecordingReporter::default();

        workflow
            .run_job(
                &mut app,
                RipRequest::new("input.mp4", "output.mp3"),
                &mut reporter,
            )
            .await;

        let job = app.current_job().unwrap();
        assert_eq!(job.status, JobStatus::Completed);
        assert_eq!(job.progress.duration, Duration::from_secs(30));
        assert!(matches!(
            reporter.0.as_slice(),
            [
                WorkflowEvent::MetadataReady(_),
                WorkflowEvent::Ripping,
                WorkflowEvent::Completed { .. },
                WorkflowEvent::Finished
            ]
        ));
    }

    #[tokio::test]
    async fn probing_failure_is_recorded_and_stops_extraction() {
        let workflow = RipWorkflow::new((), FailedProbe, UnexpectedRipper);
        let mut app = App::new();
        let mut reporter = RecordingReporter::default();

        workflow
            .run_job(
                &mut app,
                RipRequest::new("silent.mp4", "output.mp3"),
                &mut reporter,
            )
            .await;

        assert!(matches!(
            app.current_job().map(|job| &job.status),
            Some(JobStatus::Failed(message)) if message == "media contains no audio stream"
        ));
        assert!(matches!(
            reporter.0.as_slice(),
            [
                WorkflowEvent::Failed {
                    stage: WorkflowStage::Probing,
                    ..
                },
                WorkflowEvent::Finished
            ]
        ));
    }

    #[tokio::test]
    async fn ripping_failure_is_recorded_after_successful_probe() {
        let workflow = RipWorkflow::new((), SuccessfulProbe, FailedRipper);
        let mut app = App::new();
        let mut reporter = RecordingReporter::default();

        workflow
            .run_job(
                &mut app,
                RipRequest::new("input.mp4", "protected/output.mp3"),
                &mut reporter,
            )
            .await;

        assert!(matches!(
            app.current_job().map(|job| &job.status),
            Some(JobStatus::Failed(message)) if message == "cannot write output"
        ));
        assert!(reporter.0.iter().any(|event| matches!(
            event,
            WorkflowEvent::Failed {
                stage: WorkflowStage::Ripping,
                ..
            }
        )));
    }
}
