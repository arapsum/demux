use std::{io, process::ExitStatus};

use thiserror::Error;

/// Represents failures while starting `FFmpeg` commands.
///
/// # Variants
///
/// - [`Dependency`](Self::Dependency): A required `FFmpeg` dependency could
///   not be detected or run.
/// - [`Io`](Self::Io): The operating system could not start or communicate
///   with the command process.
/// - [`ProcessFailed`](Self::ProcessFailed): `FFmpeg` ran but returned an
///   unsuccessful exit status.
#[derive(Debug, Error)]
pub enum FFmpegError {
    /// Wraps a failure while detecting or running a required dependency.
    #[error(transparent)]
    Dependency(#[from] DependencyError),
    /// Wraps an I/O failure from the operating system.
    #[error(transparent)]
    Io(#[from] io::Error),
    /// Captures diagnostic output from an `FFmpeg` process that exited
    /// unsuccessfully.
    #[error("ffmpeg exited with {status}: {stderr}")]
    ProcessFailed { status: ExitStatus, stderr: String },
}

pub type FFmpegResult<T> = std::result::Result<T, FFmpegError>;

/// Represents failures while checking an `FFmpeg` dependency.
///
/// # Variants
///
/// - [`Missing`](Self::Missing): The executable was not found on `PATH`.
/// - [`Launch`](Self::Launch): The operating system could not start the
///   executable.
/// - [`Failed`](Self::Failed): The executable started but exited unsuccessfully.
#[derive(Debug, Error)]
pub enum DependencyError {
    /// The required executable was not found on `PATH`.
    #[error("{program} was not found on PATH. Install FFmpeg and ensure `{program}` is available.")]
    Missing { program: &'static str },
    /// The operating system could not start the required executable.
    #[error("failed to launch {program}: {source}")]
    Launch {
        program: &'static str,
        #[source]
        source: io::Error,
    },
    /// The executable started but returned a non-success exit status.
    #[error("failed to run {program}: {status}; {stderr}")]
    Failed {
        program: &'static str,
        status: ExitStatus,
        stderr: String,
    },
}
