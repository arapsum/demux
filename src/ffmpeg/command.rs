use std::{
    future::Future,
    path::{Path, PathBuf},
    pin::Pin,
    process::Output,
};

use tokio::process::Command;

use crate::ffmpeg::{FFmpegError, FFmpegResult};

/// Encoding policy for one audio extraction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RipOptions {
    pub encoder: String,
    pub bitrate_kbps: u32,
}

impl Default for RipOptions {
    fn default() -> Self {
        Self {
            encoder: "libmp3lame".to_owned(),
            bitrate_kbps: 192,
        }
    }
}

/// Describes an extraction without coupling it to process execution.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RipRequest {
    pub input: PathBuf,
    pub output: PathBuf,
    pub options: RipOptions,
}

impl RipRequest {
    #[must_use]
    pub fn new(input: impl Into<PathBuf>, output: impl Into<PathBuf>) -> Self {
        Self {
            input: input.into(),
            output: output.into(),
            options: RipOptions::default(),
        }
    }
}

/// Successful process information returned by an audio extraction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RipOutcome {
    pub status: String,
}

/// Builds `FFmpeg` commands from extraction policy.
#[derive(Debug, Default, Clone, Copy)]
pub struct FfmpegCommandBuilder;

impl FfmpegCommandBuilder {
    #[must_use]
    pub fn build_rip(request: &RipRequest) -> Command {
        let mut command = Command::new("ffmpeg");
        command
            .arg("-i")
            .arg(&request.input)
            .arg("-vn")
            .arg("-c:a")
            .arg(&request.options.encoder)
            .arg("-b:a")
            .arg(format!("{}k", request.options.bitrate_kbps))
            .arg(&request.output);
        command
    }
}

/// Runs a prepared child-process command.
pub trait ProcessRunner {
    fn run<'a>(
        &'a self,
        command: Command,
    ) -> Pin<Box<dyn Future<Output = std::io::Result<Output>> + Send + 'a>>;
}

/// Tokio-backed production process runner.
#[derive(Debug, Default, Clone, Copy)]
pub struct TokioProcessRunner;

impl ProcessRunner for TokioProcessRunner {
    fn run<'a>(
        &'a self,
        mut command: Command,
    ) -> Pin<Box<dyn Future<Output = std::io::Result<Output>> + Send + 'a>> {
        Box::pin(async move { command.output().await })
    }
}

/// Executes typed extraction requests using a separately supplied runner.
#[derive(Debug, Default)]
pub struct FfmpegAudioRipper<R = TokioProcessRunner> {
    runner: R,
}

impl<R> FfmpegAudioRipper<R> {
    #[must_use]
    pub const fn new(runner: R) -> Self {
        Self { runner }
    }
}

impl<R: ProcessRunner> FfmpegAudioRipper<R> {
    #[tracing::instrument(
        name = "ffmpeg_rip",
        level = "debug",
        skip_all,
        fields(
            encoder = %request.options.encoder,
            bitrate_kbps = request.options.bitrate_kbps,
        )
    )]
    pub async fn rip(&self, request: &RipRequest) -> FFmpegResult<RipOutcome> {
        tracing::debug!("launching ffmpeg process");

        let output = self
            .runner
            .run(FfmpegCommandBuilder::build_rip(request))
            .await?;

        tracing::debug!(
            status = %output.status,
            stderr_bytes = output.stderr.len(),
            "ffmpeg process exited"
        );

        if !output.status.success() {
            return Err(FFmpegError::ProcessFailed {
                status: output.status,
                stderr: String::from_utf8_lossy(&output.stderr).trim().to_owned(),
            });
        }

        Ok(RipOutcome {
            status: output.status.to_string(),
        })
    }
}

/// Convenience entry point using the production process runner.
pub async fn rip(input: impl AsRef<Path>, output: impl AsRef<Path>) -> FFmpegResult<RipOutcome> {
    FfmpegAudioRipper::<TokioProcessRunner>::default()
        .rip(&RipRequest::new(input.as_ref(), output.as_ref()))
        .await
}

#[cfg(test)]
mod tests {
    use std::ffi::OsStr;

    use super::*;

    #[test]
    fn command_builder_applies_request_options() {
        let request = RipRequest {
            input: "input.mov".into(),
            output: "output.mp3".into(),
            options: RipOptions {
                encoder: "custom-encoder".into(),
                bitrate_kbps: 256,
            },
        };

        let command = FfmpegCommandBuilder::build_rip(&request);
        let arguments: Vec<&OsStr> = command.as_std().get_args().collect();

        assert_eq!(command.as_std().get_program(), "ffmpeg");
        assert_eq!(
            arguments,
            [
                "-i",
                "input.mov",
                "-vn",
                "-c:a",
                "custom-encoder",
                "-b:a",
                "256k",
                "output.mp3"
            ]
            .map(OsStr::new)
        );
    }
}
