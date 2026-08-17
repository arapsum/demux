use std::{future::Future, pin::Pin};

use crate::{
    ffmpeg::{
        self, Dependencies, DependencyError, FFmpegResult, FfmpegAudioRipper, RipOutcome,
        RipRequest, TokioProcessRunner,
    },
    ffprobe::{self, ProbeResult},
    model::media::MediaInfo,
};

/// Checks whether required external tools are available.
pub trait DependencyChecker {
    /// Detects the required external tools.
    ///
    /// # Returns
    ///
    /// Versions for the detected `ffmpeg` and `ffprobe` executables.
    ///
    /// # Errors
    ///
    /// Returns a dependency error when:
    ///
    /// - Either executable is unavailable.
    /// - Either executable cannot report its version.
    fn detect(&self) -> Result<Dependencies, DependencyError>;
}

/// Converts an input media path into domain metadata.
pub trait MediaProbe {
    /// Inspects one media input through an asynchronous probe adapter.
    ///
    /// # Parameters
    ///
    /// - `input`: Path to the media file to inspect.
    ///
    /// # Returns
    ///
    /// A future resolving to metadata for the input's first audio stream.
    ///
    /// # Errors
    ///
    /// The returned future can fail when:
    ///
    /// - The probe executable cannot inspect the input.
    /// - The probe output cannot be converted into media metadata.
    fn inspect<'a>(
        &'a self,
        input: &'a str,
    ) -> Pin<Box<dyn Future<Output = ProbeResult<MediaInfo>> + Send + 'a>>;
}

/// Performs one typed audio-extraction request.
pub trait AudioRipper {
    /// Runs one asynchronous typed audio-extraction request.
    ///
    /// # Parameters
    ///
    /// - `request`: Source, destination, metadata, and encoding policy.
    ///
    /// # Returns
    ///
    /// A future resolving to the completed extraction outcome.
    ///
    /// # Errors
    ///
    /// The returned future can fail when:
    ///
    /// - The output directory cannot be prepared.
    /// - `ffmpeg` cannot run or reports an unsuccessful status.
    fn rip<'a>(
        &'a self,
        request: &'a RipRequest,
    ) -> Pin<Box<dyn Future<Output = FFmpegResult<RipOutcome>> + Send + 'a>>;
}

/// Production dependency checker backed by executable detection.
#[derive(Debug, Default, Clone, Copy)]
pub struct SystemDependencyChecker;

impl DependencyChecker for SystemDependencyChecker {
    /// Detects the production `ffmpeg` and `ffprobe` executables.
    ///
    /// # Returns
    ///
    /// Versions for the detected `ffmpeg` and `ffprobe` executables.
    ///
    /// # Errors
    ///
    /// Returns an error when:
    ///
    /// - Either executable is unavailable.
    /// - Either executable cannot report its version.
    fn detect(&self) -> Result<Dependencies, DependencyError> {
        ffmpeg::detect_dependencies()
    }
}

/// Production media probe backed by `ffprobe`.
#[derive(Debug, Default, Clone, Copy)]
pub struct FfprobeMediaProbe;

impl MediaProbe for FfprobeMediaProbe {
    /// Inspects media through the production `ffprobe` adapter.
    ///
    /// # Parameters
    ///
    /// - `input`: Path to the media file to inspect.
    ///
    /// # Returns
    ///
    /// A future resolving to metadata for the input's first audio stream.
    ///
    /// # Errors
    ///
    /// The returned future can fail when:
    ///
    /// - `ffprobe` cannot inspect the input.
    /// - The probe output cannot be converted into media metadata.
    fn inspect<'a>(
        &'a self,
        input: &'a str,
    ) -> Pin<Box<dyn Future<Output = ProbeResult<MediaInfo>> + Send + 'a>> {
        Box::pin(ffprobe::inspect(input))
    }
}

/// Production audio ripper backed by `FFmpeg` and Tokio process execution.
#[derive(Debug, Default)]
pub struct SystemAudioRipper(FfmpegAudioRipper<TokioProcessRunner>);

impl AudioRipper for SystemAudioRipper {
    /// Extracts audio through the production `FFmpeg` adapter.
    ///
    /// # Parameters
    ///
    /// - `request`: Source, destination, metadata, and encoding policy.
    ///
    /// # Returns
    ///
    /// A future resolving to the completed extraction outcome.
    ///
    /// # Errors
    ///
    /// The returned future can fail when:
    ///
    /// - The output directory cannot be prepared.
    /// - `ffmpeg` cannot run or reports an unsuccessful status.
    fn rip<'a>(
        &'a self,
        request: &'a RipRequest,
    ) -> Pin<Box<dyn Future<Output = FFmpegResult<RipOutcome>> + Send + 'a>> {
        Box::pin(self.0.rip(request))
    }
}
