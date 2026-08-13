use std::path::PathBuf;

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
/// - [`BackgroundTask`](Self::BackgroundTask): Tokio could not complete a
///   blocking runtime task.
/// - [`ProbeScheduling`](Self::ProbeScheduling): A bounded probe could not
///   acquire its concurrency permit.
/// - [`OutputInspection`](Self::OutputInspection): An output path could not
///   be inspected while applying the collision policy.
/// - [`PartialOutputCleanup`](Self::PartialOutputCleanup): A cancelled rip's
///   incomplete output could not be removed.
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
    /// Wraps a failure reported by a Tokio background task.
    #[error("background task failed: {0}")]
    BackgroundTask(#[from] tokio::task::JoinError),
    /// Wraps a failure to acquire a bounded media-probe permit.
    #[error("could not schedule media probe: {0}")]
    ProbeScheduling(#[from] tokio::sync::AcquireError),
    /// Adds the requested output path to an I/O failure from collision checks.
    #[error("could not inspect output path `{path}`: {source}")]
    OutputInspection {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    /// Adds the partial output path to a cancellation cleanup failure.
    #[error("ripping was cancelled, but partial output `{path}` could not be removed: {source}")]
    PartialOutputCleanup {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("could not determine the user settings directory")]
    PreferencesDirectoryUnavailable,
    #[error("could not read settings from `{path}`: {source}")]
    PreferencesRead {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("settings in `{path}` are invalid: {source}")]
    PreferencesParse {
        path: PathBuf,
        #[source]
        source: serde_json::Error,
    },
    #[error("could not save settings to `{path}`: {source}")]
    PreferencesWrite {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("could not serialize settings: {0}")]
    PreferencesSerialize(#[from] serde_json::Error),
}

pub type Result<T, E = Error> = std::result::Result<T, E>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn subsystem_errors_keep_their_typed_variant() {
        let error = Error::from(ffprobe::ProbeError::NoAudio);

        assert!(matches!(error, Error::Probe(ffprobe::ProbeError::NoAudio)));
    }

    #[test]
    fn output_inspection_errors_include_the_requested_path() {
        let error = Error::OutputInspection {
            path: PathBuf::from("/music/song.mp3"),
            source: std::io::Error::new(std::io::ErrorKind::PermissionDenied, "denied"),
        };

        assert_eq!(
            error.to_string(),
            "could not inspect output path `/music/song.mp3`: denied"
        );
    }

    #[test]
    fn partial_output_errors_explain_that_cancellation_succeeded() {
        let error = Error::PartialOutputCleanup {
            path: PathBuf::from("/music/song.mp3"),
            source: std::io::Error::new(std::io::ErrorKind::PermissionDenied, "denied"),
        };

        assert_eq!(
            error.to_string(),
            "ripping was cancelled, but partial output `/music/song.mp3` could not be removed: denied"
        );
    }
}
