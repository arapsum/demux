use crate::{
    ffmpeg::{DependencyError, FFmpegError},
    ffprobe,
};

/// Represents errors returned by the Demux application.
///
/// This enum unifies dependency detection, `FFmpeg`, and standard I/O failures
/// so application entry points can return a single error type.
///
/// # Variants
///
/// - [`Dependency`](Self::Dependency): A required `FFmpeg` executable could
///   not be detected or run.
/// - [`FFmpeg`](Self::FFmpeg): An `FFmpeg` command could not be started.
/// - [`Io`](Self::Io): An application I/O operation failed.
/// - [`Probe`](Self::Probe): Media metadata could not be probed with
///   `ffprobe`.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// Wraps a failure while detecting or running a required dependency.
    #[error(transparent)]
    Dependency(#[from] DependencyError),
    /// Wraps a failure while starting an `FFmpeg` command.
    #[error(transparent)]
    FFmpeg(#[from] FFmpegError),
    /// Wraps a failure from the operating system's I/O facilities.
    #[error(transparent)]
    Io(#[from] std::io::Error),
    /// Wraps a failure while probing media metadata with `ffprobe`.
    #[error(transparent)]
    Probe(#[from] ffprobe::ProbeError),
}

pub type Result<T, E = Error> = std::result::Result<T, E>;
