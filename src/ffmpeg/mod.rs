mod command;
mod dependencies;
mod error;

pub use self::{
    command::{
        FfmpegAudioRipper, FfmpegCommandBuilder, ProcessRunner, RipOptions, RipOutcome, RipRequest,
        TokioProcessRunner, rip,
    },
    dependencies::{Dependencies, DependencyState, detect_dependencies},
    error::{DependencyError, FFmpegError, FFmpegResult},
};
