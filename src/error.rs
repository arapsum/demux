use crate::ffmpeg::{DependencyError, FFmpegError};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Dependency(#[from] DependencyError),
    #[error(transparent)]
    FFmpeg(#[from] FFmpegError),
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

pub type Result<T, E = Error> = std::result::Result<T, E>;
