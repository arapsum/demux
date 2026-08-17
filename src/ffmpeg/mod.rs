mod cancellation;
mod command;
mod control;
mod dependencies;
mod error;
mod normalization;
mod progress;

pub use crate::model::encoding::{
    ChannelMode, EncodingOptionError, Mp3Bitrate, OutputFormat, RipOptions, SampleRate,
};

pub use self::{
    cancellation::{CancellationHandle, CancellationSignal, cancellation_pair},
    command::{
        FfmpegAudioRipper, FfmpegCommandBuilder, FfmpegLogRedactions, ProcessExit, ProcessRunner,
        ProgressProcessRunner, RipOutcome, RipRequest, RipTermination, TokioProcessRunner, rip,
    },
    control::{
        PauseCapability, PauseControlEvent, PauseControlHandle, PauseControlOperation,
        PauseControlRequestError, PauseControlSignal, pause_control_pair,
    },
    dependencies::{Dependencies, DependencyState, detect_dependencies},
    error::{DependencyError, FFmpegError, FFmpegResult},
    normalization::{
        FILTER_TRUE_PEAK, LRA_CEILING, LoudnessMeasurement, OUTPUT_TRUE_PEAK_LIMIT,
        TARGET_INTEGRATED_LUFS,
    },
    progress::{
        FfmpegLogEvent, FfmpegProgress, ProgressParser, ProgressStatus, RipPhase, RipProgressEvent,
    },
};
