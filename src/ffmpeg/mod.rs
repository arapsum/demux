mod cancellation;
mod command;
mod dependencies;
mod error;
mod progress;

pub use self::{
    cancellation::{CancellationHandle, CancellationSignal, cancellation_pair},
    command::{
        FfmpegAudioRipper, FfmpegCommandBuilder, ProcessExit, ProcessRunner, ProgressProcessRunner,
        RipOptions, RipOutcome, RipRequest, RipTermination, TokioProcessRunner, rip,
    },
    dependencies::{Dependencies, DependencyState, detect_dependencies},
    error::{DependencyError, FFmpegError, FFmpegResult},
    progress::{FfmpegProgress, ProgressParser, ProgressStatus},
};
