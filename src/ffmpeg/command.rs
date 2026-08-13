use std::{
    future::Future,
    path::{Path, PathBuf},
    pin::Pin,
    process::{Output, Stdio},
    time::Duration,
};

use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::process::Command;
use tokio::sync::mpsc;

use crate::ffmpeg::{
    CancellationSignal, FFmpegError, FFmpegResult, FfmpegProgress, ProgressParser,
};
use crate::model::encoding::RipOptions;

const CANCELLATION_GRACE_PERIOD: Duration = Duration::from_secs(3);

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

    #[must_use]
    pub fn with_options(
        input: impl Into<PathBuf>,
        output: impl Into<PathBuf>,
        options: RipOptions,
    ) -> Self {
        Self {
            input: input.into(),
            output: output.into(),
            options,
        }
    }
}

/// Successful process information returned by an audio extraction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RipOutcome {
    pub status: String,
}

/// Terminal result of a cancellable extraction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RipTermination {
    Completed(RipOutcome),
    Cancelled { forced: bool },
}

/// Raw process result produced by a cancellable process runner.
#[derive(Debug)]
pub enum ProcessExit {
    Exited(Output),
    Cancelled { output: Output, forced: bool },
}

/// Builds `FFmpeg` commands from extraction policy.
#[derive(Debug, Default, Clone, Copy)]
pub struct FfmpegCommandBuilder;

impl FfmpegCommandBuilder {
    #[must_use]
    pub fn build_rip(request: &RipRequest) -> Command {
        let mut command = Command::new("ffmpeg");
        command
            .arg("-n")
            .arg("-i")
            .arg(&request.input)
            .arg("-vn")
            .arg("-c:a")
            .arg(request.options.encoder())
            .arg("-b:a")
            .arg(format!("{}k", request.options.bitrate.kbps()))
            .arg("-ar")
            .arg(request.options.sample_rate.hz().to_string())
            .arg("-ac")
            .arg(request.options.channels.channels().to_string())
            .arg("-progress")
            .arg("pipe:1")
            .arg("-nostats")
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

/// Runs FFmpeg while forwarding machine-readable progress snapshots.
pub trait ProgressProcessRunner {
    fn run_with_progress<'a>(
        &'a self,
        command: Command,
        progress: mpsc::Sender<FfmpegProgress>,
        cancellation: CancellationSignal,
    ) -> Pin<Box<dyn Future<Output = std::io::Result<ProcessExit>> + Send + 'a>>;
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

impl ProgressProcessRunner for TokioProcessRunner {
    fn run_with_progress<'a>(
        &'a self,
        command: Command,
        progress: mpsc::Sender<FfmpegProgress>,
        cancellation: CancellationSignal,
    ) -> Pin<Box<dyn Future<Output = std::io::Result<ProcessExit>> + Send + 'a>> {
        Box::pin(run_with_progress(
            command,
            progress,
            cancellation,
            CANCELLATION_GRACE_PERIOD,
        ))
    }
}

async fn run_with_progress(
    mut command: Command,
    progress: mpsc::Sender<FfmpegProgress>,
    cancellation: CancellationSignal,
    grace_period: Duration,
) -> std::io::Result<ProcessExit> {
    command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    let mut child = command.spawn()?;
    let mut stdin = child.stdin.take();
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| std::io::Error::other("ffmpeg stdout was not captured"))?;
    let mut stderr = child
        .stderr
        .take()
        .ok_or_else(|| std::io::Error::other("ffmpeg stderr was not captured"))?;

    let stderr_reader = tokio::spawn(async move {
        let mut bytes = Vec::new();
        stderr.read_to_end(&mut bytes).await?;
        Ok::<_, std::io::Error>(bytes)
    });

    let stdout_reader = tokio::spawn(async move {
        let mut lines = BufReader::new(stdout).lines();
        let mut parser = ProgressParser::default();
        while let Some(line) = lines.next_line().await? {
            if let Some(snapshot) = parser.push_line(&line) {
                let _ = progress.try_send(snapshot);
            }
        }
        Ok::<_, std::io::Error>(())
    });

    let process_state = tokio::select! {
        biased;
        status = child.wait() => (status?, false, false),
        () = cancellation.cancelled() => {
            tracing::info!("requesting cooperative ffmpeg cancellation");
            if let Some(mut stdin) = stdin.take() {
                if let Err(error) = stdin.write_all(b"q\n").await {
                    tracing::debug!(%error, "could not send ffmpeg's cooperative quit command");
                } else if let Err(error) = stdin.flush().await {
                    tracing::debug!(%error, "could not flush ffmpeg's cooperative quit command");
                }
            }

            match tokio::time::timeout(grace_period, child.wait()).await {
                Ok(status) => (status?, true, false),
                Err(_) => {
                    tracing::warn!(
                        grace_period_ms = grace_period.as_millis(),
                        "ffmpeg did not stop cooperatively; forcing termination"
                    );
                    child.kill().await?;
                    (child.wait().await?, true, true)
                }
            }
        }
    };

    stdout_reader.await.map_err(std::io::Error::other)??;
    let stderr = stderr_reader.await.map_err(std::io::Error::other)??;

    let output = Output {
        status: process_state.0,
        stdout: Vec::new(),
        stderr,
    };
    if process_state.1 {
        Ok(ProcessExit::Cancelled {
            output,
            forced: process_state.2,
        })
    } else {
        Ok(ProcessExit::Exited(output))
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
            encoder = request.options.encoder(),
            bitrate_kbps = request.options.bitrate.kbps(),
            sample_rate_hz = request.options.sample_rate.hz(),
            channels = request.options.channels.channels(),
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

        outcome(output)
    }
}

impl<R: ProgressProcessRunner> FfmpegAudioRipper<R> {
    #[tracing::instrument(
        name = "ffmpeg_rip_with_progress",
        level = "debug",
        skip_all,
        fields(
            encoder = request.options.encoder(),
            bitrate_kbps = request.options.bitrate.kbps(),
            sample_rate_hz = request.options.sample_rate.hz(),
            channels = request.options.channels.channels(),
        )
    )]
    pub async fn rip_with_progress(
        &self,
        request: &RipRequest,
        progress: mpsc::Sender<FfmpegProgress>,
    ) -> FFmpegResult<RipOutcome> {
        let (_handle, cancellation) = crate::ffmpeg::cancellation_pair();
        match self
            .rip_with_progress_cancellable(request, progress, cancellation)
            .await?
        {
            RipTermination::Completed(outcome) => Ok(outcome),
            RipTermination::Cancelled { .. } => {
                unreachable!(
                    "the compatibility entry point does not expose its cancellation handle"
                )
            }
        }
    }

    #[tracing::instrument(
        name = "ffmpeg_rip_with_progress_cancellable",
        level = "debug",
        skip_all,
        fields(
            encoder = request.options.encoder(),
            bitrate_kbps = request.options.bitrate.kbps(),
            sample_rate_hz = request.options.sample_rate.hz(),
            channels = request.options.channels.channels(),
        )
    )]
    pub async fn rip_with_progress_cancellable(
        &self,
        request: &RipRequest,
        progress: mpsc::Sender<FfmpegProgress>,
        cancellation: CancellationSignal,
    ) -> FFmpegResult<RipTermination> {
        tracing::debug!("launching ffmpeg process with progress reporting");
        let exit = self
            .runner
            .run_with_progress(
                FfmpegCommandBuilder::build_rip(request),
                progress,
                cancellation,
            )
            .await?;

        match exit {
            ProcessExit::Exited(output) => {
                tracing::debug!(
                    status = %output.status,
                    stderr_bytes = output.stderr.len(),
                    "ffmpeg progress process exited"
                );
                outcome(output).map(RipTermination::Completed)
            }
            ProcessExit::Cancelled { output, forced } => {
                tracing::info!(
                    status = %output.status,
                    stderr_bytes = output.stderr.len(),
                    forced,
                    "ffmpeg process cancelled"
                );
                Ok(RipTermination::Cancelled { forced })
            }
        }
    }
}

fn outcome(output: Output) -> FFmpegResult<RipOutcome> {
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

/// Convenience entry point using the production process runner.
pub async fn rip(input: impl AsRef<Path>, output: impl AsRef<Path>) -> FFmpegResult<RipOutcome> {
    FfmpegAudioRipper::<TokioProcessRunner>::default()
        .rip(&RipRequest::new(input.as_ref(), output.as_ref()))
        .await
}

#[cfg(test)]
mod tests {
    use std::ffi::OsStr;

    use crate::ffmpeg::{ChannelMode, Mp3Bitrate, OutputFormat, SampleRate};

    use super::*;

    #[test]
    fn command_builder_applies_request_options() {
        let request = RipRequest::with_options(
            "input.mov",
            "output.mp3",
            RipOptions {
                format: OutputFormat::Mp3,
                bitrate: Mp3Bitrate::Kbps256,
                sample_rate: SampleRate::Hz48000,
                channels: ChannelMode::Mono,
            },
        );

        let command = FfmpegCommandBuilder::build_rip(&request);
        let arguments: Vec<&OsStr> = command.as_std().get_args().collect();

        assert_eq!(command.as_std().get_program(), "ffmpeg");
        assert_eq!(
            arguments,
            [
                "-n",
                "-i",
                "input.mov",
                "-vn",
                "-c:a",
                "libmp3lame",
                "-b:a",
                "256k",
                "-ar",
                "48000",
                "-ac",
                "1",
                "-progress",
                "pipe:1",
                "-nostats",
                "output.mp3"
            ]
            .map(OsStr::new)
        );
    }

    #[test]
    fn command_builder_covers_every_selectable_mp3_combination() {
        let mut combinations = 0;
        for bitrate in Mp3Bitrate::ALL {
            for sample_rate in SampleRate::ALL {
                for channels in ChannelMode::ALL {
                    let request = RipRequest::with_options(
                        "input.mov",
                        "output.mp3",
                        RipOptions {
                            format: OutputFormat::Mp3,
                            bitrate,
                            sample_rate,
                            channels,
                        },
                    );
                    let command = FfmpegCommandBuilder::build_rip(&request);
                    let arguments: Vec<_> = command
                        .as_std()
                        .get_args()
                        .map(|argument| argument.to_string_lossy().into_owned())
                        .collect();

                    assert!(
                        arguments.windows(2).any(|pair| {
                            pair == ["-b:a", format!("{}k", bitrate.kbps()).as_str()]
                        })
                    );
                    assert!(
                        arguments
                            .windows(2)
                            .any(|pair| { pair == ["-ar", sample_rate.hz().to_string().as_str()] })
                    );
                    assert!(
                        arguments.windows(2).any(|pair| {
                            pair == ["-ac", channels.channels().to_string().as_str()]
                        })
                    );
                    combinations += 1;
                }
            }
        }

        assert_eq!(combinations, 16);
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn cancellation_stops_a_cooperative_child_without_forcing_it() {
        let mut command = Command::new("sh");
        command.arg("-c").arg("read line; test \"$line\" = q");
        let (handle, signal) = crate::ffmpeg::cancellation_pair();
        let (progress, _receiver) = mpsc::channel(1);

        let task = tokio::spawn(run_with_progress(
            command,
            progress,
            signal,
            Duration::from_secs(1),
        ));
        tokio::task::yield_now().await;
        assert!(handle.cancel());

        let exit = task.await.unwrap().unwrap();
        assert!(matches!(exit, ProcessExit::Cancelled { forced: false, .. }));
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn uncooperative_child_is_force_killed_after_the_grace_period() {
        let mut command = Command::new("sh");
        command
            .arg("-c")
            .arg("trap '' TERM; while true; do :; done");
        let (handle, signal) = crate::ffmpeg::cancellation_pair();
        let (progress, _receiver) = mpsc::channel(1);

        let task = tokio::spawn(run_with_progress(
            command,
            progress,
            signal,
            Duration::from_millis(30),
        ));
        tokio::task::yield_now().await;
        assert!(handle.cancel());

        let exit = tokio::time::timeout(Duration::from_secs(2), task)
            .await
            .expect("forced cancellation should be bounded")
            .unwrap()
            .unwrap();
        assert!(matches!(exit, ProcessExit::Cancelled { forced: true, .. }));
    }
}
