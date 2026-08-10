use std::{io, process::ExitStatus};

use thiserror::Error;

#[derive(Debug, Error)]
pub enum FFmpegError {
    #[error(transparent)]
    Dependency(#[from] DependencyError),
    #[error(transparent)]
    Io(#[from] io::Error),
}

pub type FFmpegResult<T> = std::result::Result<T, FFmpegError>;

#[derive(Debug, Error)]
pub enum DependencyError {
    #[error("{program} was not found on PATH. Install FFmpeg and ensure `{program}` is available.")]
    Missing { program: &'static str },
    #[error("failed to launch {program}: {source}")]
    Launch {
        program: &'static str,
        #[source]
        source: io::Error,
    },
    #[error("failed to run {program}: {status}; {stderr}")]
    Failed {
        program: &'static str,
        status: ExitStatus,
        stderr: String,
    },
}
