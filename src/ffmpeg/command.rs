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
    CancellationSignal, FFmpegError, FFmpegResult, FILTER_TRUE_PEAK, FfmpegProgress, LRA_CEILING,
    LoudnessMeasurement, ProgressParser, RipPhase, RipProgressEvent, TARGET_INTEGRATED_LUFS,
};
use crate::model::{encoding::RipOptions, media::MediaInfo};

const CANCELLATION_GRACE_PERIOD: Duration = Duration::from_secs(3);

/// Describes an extraction without coupling it to process execution.
///
/// A request carries the probe snapshot used for safe tag and cover-art
/// mapping. It is optional so callers that only need a plain audio conversion
/// can continue to use [`RipRequest::new`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RipRequest {
    pub input: PathBuf,
    pub output: PathBuf,
    pub options: RipOptions,
    pub metadata: Option<MediaInfo>,
}

impl RipRequest {
    #[must_use]
    pub fn new(input: impl Into<PathBuf>, output: impl Into<PathBuf>) -> Self {
        Self {
            input: input.into(),
            output: output.into(),
            options: RipOptions::default(),
            metadata: None,
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
            metadata: None,
        }
    }

    #[must_use]
    pub fn with_options_and_metadata(
        input: impl Into<PathBuf>,
        output: impl Into<PathBuf>,
        options: RipOptions,
        metadata: Option<MediaInfo>,
    ) -> Self {
        Self {
            input: input.into(),
            output: output.into(),
            options,
            metadata,
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
        Self::build_rip_with_measurement(request, None)
            .expect("build_rip is only valid for non-normalized requests")
    }

    pub fn build_rip_with_measurement(
        request: &RipRequest,
        measurement: Option<&LoudnessMeasurement>,
    ) -> FFmpegResult<Command> {
        if request.options.normalize_audio && measurement.is_none() {
            return Err(FFmpegError::LoudnessMeasurementMissing);
        }
        let mut command = Command::new("ffmpeg");
        command
            .arg("-n")
            .arg("-i")
            .arg(&request.input)
            .arg("-map")
            .arg("0:a:0")
            .arg("-map_metadata")
            .arg("-1")
            .arg("-c:a")
            .arg(request.options.encoder())
            .arg("-b:a")
            .arg(format!("{}k", request.options.bitrate.kbps()))
            .arg("-ar")
            .arg(request.options.sample_rate.hz().to_string())
            .arg("-ac")
            .arg(request.options.channels.channels().to_string());

        if request.options.normalize_audio {
            let measurement = measurement.expect("validated above");
            let dual_mono = dual_mono(request);
            command.arg("-af").arg(format!(
                "loudnorm=I={TARGET_INTEGRATED_LUFS}:LRA={LRA_CEILING}:TP={FILTER_TRUE_PEAK}:measured_I={}:measured_LRA={}:measured_TP={}:measured_thresh={}:offset={}:linear=true:dual_mono={dual_mono}",
                measurement.integrated_lufs,
                measurement.loudness_range,
                measurement.true_peak,
                measurement.threshold,
                measurement.offset,
            ));
        }

        if request.options.embed_metadata
            && let Some(metadata) = &request.metadata
        {
            for (key, value) in metadata.tags.entries() {
                command.arg("-metadata").arg(format!("{key}={value}"));
            }
        }

        if request.options.extract_artwork
            && let Some(artwork) = request
                .metadata
                .as_ref()
                .and_then(|metadata| metadata.artwork.as_ref())
                .filter(|artwork| artwork.supports_mp3())
        {
            command
                .arg("-map")
                .arg(format!("0:{}", artwork.stream_index))
                .arg("-c:v")
                .arg("mjpeg")
                .arg("-disposition:v:0")
                .arg("attached_pic");
        }

        command
            .arg("-progress")
            .arg("pipe:1")
            .arg("-nostats")
            .arg(&request.output);
        Ok(command)
    }

    #[must_use]
    pub fn build_loudness_analysis(request: &RipRequest) -> Command {
        let dual_mono = dual_mono(request);
        let mut command = Command::new("ffmpeg");
        command
            .arg("-hide_banner")
            .arg("-i")
            .arg(&request.input)
            .arg("-map")
            .arg("0:a:0")
            .arg("-af")
            .arg(format!(
                "loudnorm=I={TARGET_INTEGRATED_LUFS}:LRA={LRA_CEILING}:TP={FILTER_TRUE_PEAK}:dual_mono={dual_mono}:print_format=json"
            ))
            .arg("-f")
            .arg("null")
            .arg("-progress")
            .arg("pipe:1")
            .arg("-nostats")
            .arg("-");
        command
    }
}

fn dual_mono(request: &RipRequest) -> bool {
    request
        .metadata
        .as_ref()
        .and_then(|metadata| metadata.audio.channels)
        == Some(1)
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
            embed_metadata = request.options.embed_metadata,
            extract_artwork = request.options.extract_artwork,
        )
    )]
    pub async fn rip(&self, request: &RipRequest) -> FFmpegResult<RipOutcome> {
        tracing::debug!("launching ffmpeg process");
        ensure_output_parent(&request.output).await?;

        let measurement = if request.options.normalize_audio {
            Some(self.analyze(request).await?)
        } else {
            None
        };
        let output = self
            .runner
            .run(FfmpegCommandBuilder::build_rip_with_measurement(
                request,
                measurement.as_ref(),
            )?)
            .await?;

        tracing::debug!(
            status = %output.status,
            stderr_bytes = output.stderr.len(),
            "ffmpeg process exited"
        );

        outcome(output)
    }

    async fn analyze(&self, request: &RipRequest) -> FFmpegResult<LoudnessMeasurement> {
        let output = self
            .runner
            .run(FfmpegCommandBuilder::build_loudness_analysis(request))
            .await?;
        if !output.status.success() {
            return Err(FFmpegError::ProcessFailed {
                status: output.status,
                stderr: String::from_utf8_lossy(&output.stderr).trim().to_owned(),
            });
        }
        LoudnessMeasurement::parse(&output.stderr)
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
            embed_metadata = request.options.embed_metadata,
            extract_artwork = request.options.extract_artwork,
        )
    )]
    pub async fn rip_with_progress(
        &self,
        request: &RipRequest,
        progress: mpsc::Sender<RipProgressEvent>,
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
            embed_metadata = request.options.embed_metadata,
            extract_artwork = request.options.extract_artwork,
        )
    )]
    pub async fn rip_with_progress_cancellable(
        &self,
        request: &RipRequest,
        progress: mpsc::Sender<RipProgressEvent>,
        cancellation: CancellationSignal,
    ) -> FFmpegResult<RipTermination> {
        tracing::debug!("launching ffmpeg process with progress reporting");
        ensure_output_parent(&request.output).await?;
        let measurement = if request.options.normalize_audio {
            let exit = self
                .run_phase(
                    FfmpegCommandBuilder::build_loudness_analysis(request),
                    RipPhase::Analyzing,
                    progress.clone(),
                    cancellation.clone(),
                )
                .await?;
            match exit {
                ProcessExit::Exited(output) => {
                    if !output.status.success() {
                        return Err(FFmpegError::ProcessFailed {
                            status: output.status,
                            stderr: String::from_utf8_lossy(&output.stderr).trim().to_owned(),
                        });
                    }
                    Some(LoudnessMeasurement::parse(&output.stderr)?)
                }
                ProcessExit::Cancelled { forced, .. } => {
                    return Ok(RipTermination::Cancelled { forced });
                }
            }
        } else {
            None
        };
        let exit = self
            .run_phase(
                FfmpegCommandBuilder::build_rip_with_measurement(request, measurement.as_ref())?,
                RipPhase::Encoding,
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

    async fn run_phase(
        &self,
        command: Command,
        phase: RipPhase,
        progress: mpsc::Sender<RipProgressEvent>,
        cancellation: CancellationSignal,
    ) -> FFmpegResult<ProcessExit> {
        let (sender, mut receiver) = mpsc::channel(32);
        let process = self.runner.run_with_progress(command, sender, cancellation);
        tokio::pin!(process);
        let mut result = None;
        let mut receiver_open = true;

        while result.is_none() || receiver_open {
            tokio::select! {
                biased;
                process_result = &mut process, if result.is_none() => {
                    result = Some(process_result?);
                }
                snapshot = receiver.recv(), if receiver_open => {
                    match snapshot {
                        Some(snapshot) => {
                            let _ = progress.send(RipProgressEvent { phase, progress: snapshot }).await;
                        }
                        None => receiver_open = false,
                    }
                }
            }
        }
        result
            .ok_or_else(|| FFmpegError::Io(std::io::Error::other("ffmpeg process did not return")))
    }
}

async fn ensure_output_parent(path: &Path) -> std::io::Result<()> {
    let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    else {
        return Ok(());
    };
    tokio::fs::create_dir_all(parent).await
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
    use std::{ffi::OsStr, time::Duration};

    use crate::ffmpeg::{ChannelMode, Mp3Bitrate, OutputFormat, SampleRate};
    use crate::model::media::{ArtworkInfo, AudioMetadata, MediaInfo, MetadataTags};

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
                ..RipOptions::default()
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
                "-map",
                "0:a:0",
                "-map_metadata",
                "-1",
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
                            ..RipOptions::default()
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

    fn source_metadata(codec: &str) -> MediaInfo {
        MediaInfo {
            duration: Duration::from_secs(12),
            container: "mov".to_owned(),
            bitrate: None,
            creation_time: None,
            tags: Box::new(MetadataTags {
                title: Some("A song".to_owned()),
                artist: Some("An artist".to_owned()),
                album: Some("An album".to_owned()),
                ..MetadataTags::default()
            }),
            artwork: Some(ArtworkInfo {
                stream_index: 3,
                codec: codec.to_owned(),
                width: Some(640),
                height: Some(640),
                mime_type: None,
            }),
            audio: AudioMetadata {
                stream_index: 1,
                codec: "aac".to_owned(),
                sample_rate: Some(48_000),
                channels: Some(2),
                channel_layout: Some("stereo".to_owned()),
                bitrate: None,
                language: None,
            },
        }
    }

    #[test]
    fn command_builder_maps_safe_tags_and_compatible_cover_art() {
        let request = RipRequest::with_options_and_metadata(
            "input.mov",
            "output.mp3",
            RipOptions::default(),
            Some(source_metadata("mjpeg")),
        );

        let command = FfmpegCommandBuilder::build_rip(&request);
        let arguments: Vec<String> = command
            .as_std()
            .get_args()
            .map(|argument| argument.to_string_lossy().into_owned())
            .collect();

        assert!(
            arguments
                .windows(2)
                .any(|pair| pair == ["-metadata", "title=A song"])
        );
        assert!(
            arguments
                .windows(2)
                .any(|pair| pair == ["-metadata", "artist=An artist"])
        );
        assert!(
            arguments
                .windows(2)
                .any(|pair| pair == ["-metadata", "album=An album"])
        );
        assert!(arguments.windows(2).any(|pair| pair == ["-map", "0:3"]));
        assert!(arguments.windows(2).any(|pair| pair == ["-c:v", "mjpeg"]));
        assert!(
            arguments
                .windows(2)
                .any(|pair| pair == ["-disposition:v:0", "attached_pic"])
        );
    }

    #[test]
    fn command_builder_skips_metadata_and_unsupported_art_when_disabled() {
        let request = RipRequest::with_options_and_metadata(
            "input.mov",
            "output.mp3",
            RipOptions {
                embed_metadata: false,
                extract_artwork: true,
                ..RipOptions::default()
            },
            Some(source_metadata("webp")),
        );

        let command = FfmpegCommandBuilder::build_rip(&request);
        let arguments: Vec<String> = command
            .as_std()
            .get_args()
            .map(|argument| argument.to_string_lossy().into_owned())
            .collect();

        assert!(!arguments.iter().any(|argument| argument == "-metadata"));
        assert!(!arguments.windows(2).any(|pair| pair == ["-map", "0:3"]));
        assert!(
            arguments
                .windows(2)
                .any(|pair| pair == ["-map_metadata", "-1"])
        );
    }

    #[test]
    fn normalized_commands_require_measurements_and_apply_the_two_pass_filter() {
        let request = RipRequest::with_options(
            "input.wav",
            "output.mp3",
            RipOptions {
                normalize_audio: true,
                ..RipOptions::default()
            },
        );
        assert!(matches!(
            FfmpegCommandBuilder::build_rip_with_measurement(&request, None),
            Err(FFmpegError::LoudnessMeasurementMissing)
        ));

        let measurement = LoudnessMeasurement {
            integrated_lufs: -28.0,
            loudness_range: 5.0,
            true_peak: -2.0,
            threshold: -38.0,
            offset: 0.0,
        };
        let command =
            FfmpegCommandBuilder::build_rip_with_measurement(&request, Some(&measurement)).unwrap();
        let arguments: Vec<String> = command
            .as_std()
            .get_args()
            .map(|argument| argument.to_string_lossy().into_owned())
            .collect();
        assert!(
            arguments
                .windows(2)
                .any(|pair| pair[0] == "-af" && pair[1].contains("I=-23:LRA=50:TP=-2"))
        );
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
