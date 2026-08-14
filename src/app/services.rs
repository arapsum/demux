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
    /// Returns a dependency error when either executable is unavailable or
    /// cannot report its version.
    fn detect(&self) -> Result<Dependencies, DependencyError>;
}

/// Converts an input media path into domain metadata.
pub trait MediaProbe {
    fn inspect<'a>(
        &'a self,
        input: &'a str,
    ) -> Pin<Box<dyn Future<Output = ProbeResult<MediaInfo>> + Send + 'a>>;
}

/// Performs one typed audio-extraction request.
pub trait AudioRipper {
    fn rip<'a>(
        &'a self,
        request: &'a RipRequest,
    ) -> Pin<Box<dyn Future<Output = FFmpegResult<RipOutcome>> + Send + 'a>>;
}

/// Production dependency checker backed by executable detection.
#[derive(Debug, Default, Clone, Copy)]
pub struct SystemDependencyChecker;

impl DependencyChecker for SystemDependencyChecker {
    fn detect(&self) -> Result<Dependencies, DependencyError> {
        ffmpeg::detect_dependencies()
    }
}

/// Production media probe backed by `ffprobe`.
#[derive(Debug, Default, Clone, Copy)]
pub struct FfprobeMediaProbe;

impl MediaProbe for FfprobeMediaProbe {
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
    fn rip<'a>(
        &'a self,
        request: &'a RipRequest,
    ) -> Pin<Box<dyn Future<Output = FFmpegResult<RipOutcome>> + Send + 'a>> {
        Box::pin(self.0.rip(request))
    }
}
