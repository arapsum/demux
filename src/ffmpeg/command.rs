use std::{
    future::Future,
    path::{Path, PathBuf},
    pin::Pin,
    process::{Output, Stdio},
    time::{Duration, SystemTime},
};

use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::process::Command;
use tokio::sync::mpsc;

use crate::ffmpeg::{
    CancellationSignal, FFmpegError, FFmpegResult, FILTER_TRUE_PEAK, FfmpegLogEvent,
    FfmpegProgress, LRA_CEILING, LoudnessMeasurement, PauseControlEvent, PauseControlOperation,
    PauseControlSignal, ProgressParser, RipPhase, RipProgressEvent, TARGET_INTEGRATED_LUFS,
};
use crate::model::{encoding::RipOptions, media::MediaInfo};

const CANCELLATION_GRACE_PERIOD: Duration = Duration::from_secs(3);
const STDERR_TAIL_LIMIT: usize = 64 * 1024;
const STDERR_READ_BUFFER: usize = 8 * 1024;
const LOG_LINE_LIMIT: usize = 8 * 1024;

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
/// Terminal outcome of a cancellable extraction process.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RipTermination {
    Completed(RipOutcome),
    Cancelled { forced: bool },
}

/// Raw process result produced by a cancellable process runner.
/// Raw process output paired with cancellation state from a progress runner.
#[derive(Debug)]
pub enum ProcessExit {
    Exited(Output),
    Cancelled { output: Output, forced: bool },
}

/// Paths that are replaced before `FFmpeg` output is exposed to the UI.
#[derive(Debug, Clone, Default)]
pub struct FfmpegLogRedactions {
    replacements: Vec<(String, String)>,
}

impl FfmpegLogRedactions {
    #[must_use]
    fn for_request(request: &RipRequest) -> Self {
        let mut replacements = Vec::new();
        for (path, label) in [(&request.input, "<input>"), (&request.output, "<output>")] {
            let path_text = path.to_string_lossy().into_owned();
            if path_text.is_empty() {
                continue;
            }
            let filename = path
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("file");
            replacements.push((path_text, format!("{label}/{filename}")));
        }
        replacements.sort_by_key(|replacement| std::cmp::Reverse(replacement.0.len()));
        Self { replacements }
    }

    fn apply(&self, line: &str) -> String {
        self.replacements
            .iter()
            .fold(line.to_owned(), |line, (path, replacement)| {
                line.replace(path, replacement)
            })
    }
}

/// Builds `FFmpeg` commands from extraction policy.
#[derive(Debug, Default, Clone, Copy)]
pub struct FfmpegCommandBuilder;

impl FfmpegCommandBuilder {
    /// Builds a non-normalized MP3 extraction command.
    ///
    /// # Parameters
    ///
    /// - `request`: Input, output, metadata, and encoding options.
    ///
    /// # Returns
    ///
    /// A configured `ffmpeg` command.
    ///
    /// # Panics
    ///
    /// Panics when `request.options.normalize_audio` is enabled. Normalized
    /// requests must use [`Self::build_rip_with_measurement`] after analysis.
    #[must_use]
    pub fn build_rip(request: &RipRequest) -> Command {
        Self::build_rip_with_measurement(request, None)
            .expect("build_rip is only valid for non-normalized requests")
    }

    /// Builds an extraction command, optionally using a loudness measurement.
    ///
    /// # Parameters
    ///
    /// - `request`: Input, output, metadata, and encoding options.
    /// - `measurement`: First-pass loudness data for normalized requests.
    ///
    /// # Returns
    ///
    /// A configured `ffmpeg` command.
    ///
    /// # Errors
    ///
    /// Returns an error when:
    ///
    /// - Normalization is enabled without a first-pass measurement.
    ///
    /// # Panics
    ///
    /// Panics only if the validated normalization invariant is violated
    /// between the check and the measurement lookup.
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
pub trait ProcessRunner: Sync {
    /// Runs a prepared child-process command asynchronously.
    ///
    /// # Parameters
    ///
    /// - `command`: Prepared command to execute.
    ///
    /// # Returns
    ///
    /// A future resolving to the captured process output.
    ///
    /// # Errors
    ///
    /// The returned future can fail when:
    ///
    /// - The operating system cannot start the process.
    /// - The process output cannot be collected.
    fn run<'a>(
        &'a self,
        command: Command,
    ) -> Pin<Box<dyn Future<Output = std::io::Result<Output>> + Send + 'a>>;
}

/// Runs `FFmpeg` while forwarding machine-readable progress snapshots.
pub trait ProgressProcessRunner: Sync {
    /// Runs one `FFmpeg` phase with progress, diagnostics, cancellation, and
    /// capability-gated pause/resume control.
    ///
    /// # Parameters
    ///
    /// - `command`: Prepared argument-based `FFmpeg` command.
    /// - `phase`: Analysis or encoding phase represented by emitted events.
    /// - `progress`: Channel receiving parsed progress snapshots.
    /// - `logs`: Channel receiving bounded diagnostic events.
    /// - `redactions`: Path redactions applied to diagnostic output.
    /// - `cancellation`: Signal used to stop the child process.
    /// - `pause_control`: Receiver for pause and resume requests.
    /// - `control_events`: Channel receiving transition acknowledgements.
    ///
    /// # Returns
    ///
    /// A future resolving to the child process exit state.
    ///
    /// # Errors
    ///
    /// The returned future can fail when:
    ///
    /// - The process cannot be spawned.
    /// - Process output cannot be read.
    /// - Cooperative or forced shutdown cannot complete.
    #[allow(clippy::too_many_arguments)]
    fn run_with_progress<'a>(
        &'a self,
        command: Command,
        phase: RipPhase,
        progress: mpsc::Sender<FfmpegProgress>,
        logs: mpsc::Sender<FfmpegLogEvent>,
        redactions: FfmpegLogRedactions,
        cancellation: CancellationSignal,
        pause_control: &'a mut PauseControlSignal,
        control_events: mpsc::Sender<PauseControlEvent>,
    ) -> Pin<Box<dyn Future<Output = std::io::Result<ProcessExit>> + Send + 'a>>;
}

/// Tokio-backed production process runner.
#[derive(Debug, Default, Clone, Copy)]
pub struct TokioProcessRunner;

impl ProcessRunner for TokioProcessRunner {
    /// Runs a prepared Tokio process command asynchronously.
    ///
    /// # Parameters
    ///
    /// - `command`: Prepared command to execute.
    ///
    /// # Returns
    ///
    /// A future resolving to the captured process output.
    ///
    /// # Errors
    ///
    /// The returned future can fail when:
    ///
    /// - The operating system cannot start the process.
    /// - The process output cannot be collected.
    fn run<'a>(
        &'a self,
        mut command: Command,
    ) -> Pin<Box<dyn Future<Output = std::io::Result<Output>> + Send + 'a>> {
        Box::pin(async move { command.output().await })
    }
}

impl ProgressProcessRunner for TokioProcessRunner {
    /// Runs one Tokio process with progress and control forwarding.
    ///
    /// # Parameters
    ///
    /// - `command`: Prepared `FFmpeg` command.
    /// - `phase`: Analysis or encoding phase represented by emitted events.
    /// - `progress`: Channel receiving progress snapshots.
    /// - `logs`: Channel receiving diagnostic events.
    /// - `redactions`: Path redactions applied to diagnostics.
    /// - `cancellation`: Signal used to stop the process.
    /// - `pause_control`: Receiver for pause and resume requests.
    /// - `control_events`: Channel receiving transition acknowledgements.
    ///
    /// # Returns
    ///
    /// A future resolving to the process exit state.
    ///
    /// # Errors
    ///
    /// The returned future can fail when:
    ///
    /// - The process cannot be spawned.
    /// - Process output cannot be read.
    /// - Cooperative or forced shutdown cannot complete.
    #[allow(clippy::too_many_arguments)]
    fn run_with_progress<'a>(
        &'a self,
        command: Command,
        phase: RipPhase,
        progress: mpsc::Sender<FfmpegProgress>,
        logs: mpsc::Sender<FfmpegLogEvent>,
        redactions: FfmpegLogRedactions,
        cancellation: CancellationSignal,
        pause_control: &'a mut PauseControlSignal,
        control_events: mpsc::Sender<PauseControlEvent>,
    ) -> Pin<Box<dyn Future<Output = std::io::Result<ProcessExit>> + Send + 'a>> {
        Box::pin(run_with_progress(
            command,
            phase,
            progress,
            logs,
            redactions,
            cancellation,
            pause_control,
            control_events,
            CANCELLATION_GRACE_PERIOD,
        ))
    }
}

/// Runs one `FFmpeg` phase while forwarding progress and diagnostics.
///
/// # Parameters
///
/// - `command`: Prepared `FFmpeg` command.
/// - `phase`: Analysis or encoding phase represented by emitted events.
/// - `progress`: Channel receiving progress snapshots.
/// - `logs`: Channel receiving diagnostic events.
/// - `redactions`: Path redactions applied to diagnostics.
/// - `cancellation`: Signal used to stop the process.
/// - `pause_control`: Receiver for pause and resume requests.
/// - `control_events`: Channel receiving transition acknowledgements.
/// - `grace_period`: Time to wait for cooperative cancellation.
///
/// # Returns
///
/// The completed or cancelled child-process exit state.
///
/// # Errors
///
/// Returns an error when:
///
/// - The process cannot be spawned or waited on.
/// - Standard output or standard error cannot be read.
/// - Cooperative or forced cancellation cannot complete.
#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
async fn run_with_progress(
    mut command: Command,
    phase: RipPhase,
    progress: mpsc::Sender<FfmpegProgress>,
    logs: mpsc::Sender<FfmpegLogEvent>,
    redactions: FfmpegLogRedactions,
    cancellation: CancellationSignal,
    pause_control: &mut PauseControlSignal,
    control_events: mpsc::Sender<PauseControlEvent>,
    grace_period: Duration,
) -> std::io::Result<ProcessExit> {
    command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    #[cfg(unix)]
    command.process_group(0);

    let mut child = command.spawn()?;
    let child_id = child.id();
    let mut stdin = child.stdin.take();
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| std::io::Error::other("ffmpeg stdout was not captured"))?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| std::io::Error::other("ffmpeg stderr was not captured"))?;

    let stderr_reader = tokio::spawn(read_stderr(stderr, phase, logs, redactions));

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

    let mut control_open = pause_control.capability().is_supported();
    let mut paused = false;
    let process_state = loop {
        tokio::select! {
            biased;
            status = child.wait() => break (status?, false, false),
            () = cancellation.cancelled() => {
                if paused
                    && let Some(child_id) = child_id
                    && let Err(error) = signal_process_group(child_id, PauseControlOperation::Resume)
                {
                    tracing::debug!(%error, "could not resume ffmpeg before cancellation");
                }
                tracing::info!("requesting cooperative ffmpeg cancellation");
                if let Some(mut stdin) = stdin.take() {
                    if let Err(error) = stdin.write_all(b"q\n").await {
                        tracing::debug!(%error, "could not send ffmpeg's cooperative quit command");
                    } else if let Err(error) = stdin.flush().await {
                        tracing::debug!(%error, "could not flush ffmpeg's cooperative quit command");
                    }
                }

                if let Ok(status) = tokio::time::timeout(grace_period, child.wait()).await {
                    break (status?, true, false);
                }

                tracing::warn!(
                    grace_period_ms = grace_period.as_millis(),
                    "ffmpeg did not stop cooperatively; forcing termination"
                );
                #[cfg(unix)]
                if let Some(child_id) = child_id {
                    if let Err(error) = kill_process_group(child_id) {
                        tracing::debug!(%error, "could not terminate the full ffmpeg process group");
                        child.kill().await?;
                    }
                } else {
                    child.kill().await?;
                }
                #[cfg(not(unix))]
                child.kill().await?;
                break (child.wait().await?, true, true);
            }
            operation = pause_control.recv(), if control_open => {
                let Some(operation) = operation else {
                    control_open = false;
                    continue;
                };
                let should_apply = match operation {
                    PauseControlOperation::Pause => !paused,
                    PauseControlOperation::Resume => paused,
                };
                if !should_apply {
                    continue;
                }
                let Some(child_id) = child_id else {
                    send_control_event(
                        &control_events,
                        PauseControlEvent::Failed {
                            operation,
                            phase,
                            message: "FFmpeg did not expose a process identifier".to_owned(),
                        },
                    )
                    .await;
                    continue;
                };
                match signal_process_group(child_id, operation) {
                    Ok(()) => {
                        paused = operation == PauseControlOperation::Pause;
                        send_control_event(
                            &control_events,
                            match operation {
                                PauseControlOperation::Pause => PauseControlEvent::Paused { phase },
                                PauseControlOperation::Resume => PauseControlEvent::Resumed { phase },
                            },
                        )
                        .await;
                    }
                    Err(error) => {
                        send_control_event(
                            &control_events,
                            PauseControlEvent::Failed {
                                operation,
                                phase,
                                message: error.to_string(),
                            },
                        )
                        .await;
                    }
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

async fn send_control_event(sender: &mpsc::Sender<PauseControlEvent>, event: PauseControlEvent) {
    if sender.send(event).await.is_err() {
        tracing::debug!("pause-control consumer closed before receiving an event");
    }
}

/// Sends a pause or resume signal to an `FFmpeg` process group.
///
/// # Parameters
///
/// - `child_id`: Identifier of the child whose dedicated process group owns
///   the `FFmpeg` processes.
/// - `operation`: Pause or resume transition to request.
///
/// # Returns
///
/// `Ok(())` after the operating system accepts the signal.
///
/// # Errors
///
/// Returns an error when:
///
/// - The child identifier cannot be represented as a process-group ID.
/// - The operating system rejects the signal.
/// - The platform does not support process suspension.
fn signal_process_group(child_id: u32, operation: PauseControlOperation) -> std::io::Result<()> {
    #[cfg(unix)]
    {
        let process_group = i32::try_from(child_id).map_err(|_| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "FFmpeg process identifier does not fit the platform process-group type",
            )
        })?;
        let signal = match operation {
            PauseControlOperation::Pause => libc::SIGSTOP,
            PauseControlOperation::Resume => libc::SIGCONT,
        };
        // SAFETY: The process group was created for this child immediately
        // before spawning FFmpeg, and the signal is one of the two constants
        // selected above.
        let result = unsafe { libc::kill(-process_group, signal) };
        if result == 0 {
            Ok(())
        } else {
            Err(std::io::Error::last_os_error())
        }
    }

    #[cfg(not(unix))]
    {
        let _ = (child_id, operation);
        Err(std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            "process suspension is unsupported on this platform",
        ))
    }
}

#[cfg(unix)]
/// Sends a termination signal to the dedicated `FFmpeg` process group.
///
/// # Parameters
///
/// - `child_id`: Identifier of the child whose dedicated process group owns
///   the `FFmpeg` processes.
///
/// # Returns
///
/// `Ok(())` after the operating system accepts the termination signal.
///
/// # Errors
///
/// Returns an error when:
///
/// - The child identifier cannot be represented as a process-group ID.
/// - The operating system rejects the termination signal.
fn kill_process_group(child_id: u32) -> std::io::Result<()> {
    let process_group = i32::try_from(child_id).map_err(|_| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "FFmpeg process identifier does not fit the platform process-group type",
        )
    })?;
    // SAFETY: The process group was created for this child immediately before
    // spawning FFmpeg, and SIGKILL is a valid process signal.
    let result = unsafe { libc::kill(-process_group, libc::SIGKILL) };
    if result == 0 {
        Ok(())
    } else {
        Err(std::io::Error::last_os_error())
    }
}

/// Reads `FFmpeg` diagnostics while retaining a bounded error tail.
///
/// # Parameters
///
/// - `stderr`: Child process diagnostic stream.
/// - `phase`: Analysis or encoding phase represented by emitted events.
/// - `logs`: Channel receiving bounded diagnostic events.
/// - `redactions`: Path redactions applied to each diagnostic event.
///
/// # Returns
///
/// The final bounded standard-error tail for process-failure reporting.
///
/// # Errors
///
/// Returns an error when:
///
/// - The child standard-error stream cannot be read.
async fn read_stderr(
    mut stderr: tokio::process::ChildStderr,
    phase: RipPhase,
    logs: mpsc::Sender<FfmpegLogEvent>,
    redactions: FfmpegLogRedactions,
) -> std::io::Result<Vec<u8>> {
    let mut tail = Vec::with_capacity(STDERR_TAIL_LIMIT);
    let mut buffer = vec![0_u8; STDERR_READ_BUFFER];
    let mut line = Vec::with_capacity(LOG_LINE_LIMIT);
    let mut line_truncated = false;
    let mut dropped = 0;
    let mut logs_closed = false;

    loop {
        let bytes_read = stderr.read(&mut buffer).await?;
        if bytes_read == 0 {
            break;
        }
        append_tail(&mut tail, &buffer[..bytes_read]);
        for byte in &buffer[..bytes_read] {
            if *byte == b'\n' {
                if logs_closed {
                    line.clear();
                } else {
                    logs_closed = dispatch_log_line(
                        &logs,
                        phase,
                        &redactions,
                        std::mem::take(&mut line),
                        line_truncated,
                        &mut dropped,
                    );
                }
                line_truncated = false;
            } else if line.len() < LOG_LINE_LIMIT {
                line.push(*byte);
            } else {
                line_truncated = true;
            }
        }
    }

    if !line.is_empty() && !logs_closed {
        logs_closed = dispatch_log_line(
            &logs,
            phase,
            &redactions,
            line,
            line_truncated,
            &mut dropped,
        );
    }

    if !logs_closed && dropped > 0 {
        let _ = logs
            .send(FfmpegLogEvent::omitted(SystemTime::now(), phase, dropped))
            .await;
    }

    Ok(tail)
}

fn dispatch_log_line(
    logs: &mpsc::Sender<FfmpegLogEvent>,
    phase: RipPhase,
    redactions: &FfmpegLogRedactions,
    mut bytes: Vec<u8>,
    truncated: bool,
    dropped: &mut usize,
) -> bool {
    while bytes.last() == Some(&b'\r') {
        bytes.pop();
    }
    if bytes.is_empty() {
        return false;
    }

    if *dropped > 0 {
        match logs.try_send(FfmpegLogEvent::omitted(SystemTime::now(), phase, *dropped)) {
            Ok(()) => *dropped = 0,
            Err(mpsc::error::TrySendError::Closed(_)) => return true,
            Err(mpsc::error::TrySendError::Full(_)) => {}
        }
    }

    let mut message = String::from_utf8_lossy(&bytes).into_owned();
    if truncated {
        message.push_str(" … [line truncated]");
    }
    let event = FfmpegLogEvent::line(SystemTime::now(), phase, redactions.apply(&message));
    match logs.try_send(event) {
        Ok(()) => false,
        Err(mpsc::error::TrySendError::Closed(_)) => true,
        Err(mpsc::error::TrySendError::Full(_)) => {
            *dropped = dropped.saturating_add(1);
            false
        }
    }
}

fn append_tail(tail: &mut Vec<u8>, bytes: &[u8]) {
    if bytes.len() >= STDERR_TAIL_LIMIT {
        tail.clear();
        tail.extend_from_slice(&bytes[bytes.len() - STDERR_TAIL_LIMIT..]);
        return;
    }
    let overflow = tail
        .len()
        .saturating_add(bytes.len())
        .saturating_sub(STDERR_TAIL_LIMIT);
    if overflow > 0 {
        tail.drain(..overflow);
    }
    tail.extend_from_slice(bytes);
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
    /// Executes one extraction request without progress reporting.
    ///
    /// # Parameters
    ///
    /// - `request`: Input, output, metadata, and encoding options.
    ///
    /// # Returns
    ///
    /// The completed process status.
    ///
    /// # Errors
    ///
    /// Returns an error when:
    ///
    /// - The output directory cannot be prepared.
    /// - `ffmpeg` cannot run or exits unsuccessfully.
    /// - Normalization analysis fails.
    pub async fn rip(&self, request: &RipRequest) -> FFmpegResult<RipOutcome> {
        tracing::debug!("launching ffmpeg process");
        ensure_output_parent(&request.output).await?;
        let redactions = FfmpegLogRedactions::for_request(request);

        let measurement = if request.options.normalize_audio {
            Some(self.analyze(request, &redactions).await?)
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

        outcome(&output, &redactions)
    }

    /// Runs the first pass required for loudness normalization.
    ///
    /// # Parameters
    ///
    /// - `request`: Input and encoding policy to analyze.
    /// - `redactions`: Path redactions applied to process diagnostics.
    ///
    /// # Returns
    ///
    /// A finite loudness measurement suitable for encoding's second pass.
    ///
    /// # Errors
    ///
    /// Returns an error when:
    ///
    /// - `ffmpeg` cannot run the analysis command.
    /// - The analysis process exits unsuccessfully.
    /// - The emitted loudness measurement is missing or invalid.
    async fn analyze(
        &self,
        request: &RipRequest,
        redactions: &FfmpegLogRedactions,
    ) -> FFmpegResult<LoudnessMeasurement> {
        let output = self
            .runner
            .run(FfmpegCommandBuilder::build_loudness_analysis(request))
            .await?;
        if !output.status.success() {
            return Err(FFmpegError::ProcessFailed {
                status: output.status,
                stderr: redactions.apply(String::from_utf8_lossy(&output.stderr).trim()),
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
    /// Executes an extraction while forwarding phase progress snapshots.
    ///
    /// # Parameters
    ///
    /// - `request`: Input, output, metadata, and encoding options.
    /// - `progress`: Channel receiving machine-readable progress events.
    ///
    /// # Returns
    ///
    /// The completed process status.
    ///
    /// # Errors
    ///
    /// Returns an error when:
    ///
    /// - Process setup or progress forwarding fails.
    /// - Normalization analysis fails.
    /// - `ffmpeg` execution fails.
    pub async fn rip_with_progress(
        &self,
        request: &RipRequest,
        progress: mpsc::Sender<RipProgressEvent>,
    ) -> FFmpegResult<RipOutcome> {
        let (logs, receiver) = mpsc::channel(1);
        drop(receiver);
        let (_handle, cancellation) = crate::ffmpeg::cancellation_pair();
        let (_pause_handle, pause_control) =
            crate::ffmpeg::pause_control_pair(crate::ffmpeg::PauseCapability::current());
        let (control_events, receiver) = mpsc::channel(1);
        drop(receiver);
        match self
            .rip_with_progress_cancellable(
                request,
                progress,
                logs,
                cancellation,
                pause_control,
                control_events,
            )
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
    /// Executes an extraction with progress reporting and cancellation.
    ///
    /// # Parameters
    ///
    /// - `request`: Input, output, metadata, and encoding options.
    /// - `progress`: Channel receiving machine-readable progress events.
    /// - `cancellation`: Signal used to stop both normalization and encoding.
    /// - `pause_control`: Signal receiving pause and resume requests.
    /// - `control_events`: Channel receiving pause and resume acknowledgements.
    ///
    /// # Returns
    ///
    /// A completed or cancelled process termination.
    ///
    /// # Errors
    ///
    /// Returns an error when:
    ///
    /// - Process setup or progress forwarding fails.
    /// - Normalization analysis fails.
    /// - `ffmpeg` execution fails.
    pub async fn rip_with_progress_cancellable(
        &self,
        request: &RipRequest,
        progress: mpsc::Sender<RipProgressEvent>,
        logs: mpsc::Sender<FfmpegLogEvent>,
        cancellation: CancellationSignal,
        mut pause_control: PauseControlSignal,
        control_events: mpsc::Sender<PauseControlEvent>,
    ) -> FFmpegResult<RipTermination> {
        tracing::debug!("launching ffmpeg process with progress reporting");
        ensure_output_parent(&request.output).await?;
        let redactions = FfmpegLogRedactions::for_request(request);
        let measurement = if request.options.normalize_audio {
            let exit = self
                .run_phase(
                    FfmpegCommandBuilder::build_loudness_analysis(request),
                    RipPhase::Analyzing,
                    progress.clone(),
                    logs.clone(),
                    redactions.clone(),
                    cancellation.clone(),
                    &mut pause_control,
                    control_events.clone(),
                )
                .await?;
            match exit {
                ProcessExit::Exited(output) => {
                    if !output.status.success() {
                        return Err(FFmpegError::ProcessFailed {
                            status: output.status,
                            stderr: redactions
                                .apply(String::from_utf8_lossy(&output.stderr).trim()),
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
                logs,
                redactions.clone(),
                cancellation,
                &mut pause_control,
                control_events,
            )
            .await?;

        match exit {
            ProcessExit::Exited(output) => {
                tracing::debug!(
                    status = %output.status,
                    stderr_bytes = output.stderr.len(),
                    "ffmpeg progress process exited"
                );
                outcome(&output, &redactions).map(RipTermination::Completed)
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

    /// Runs one `FFmpeg` phase while relaying progress and diagnostics.
    ///
    /// # Parameters
    ///
    /// - `command`: Prepared command for this phase.
    /// - `phase`: Analysis or encoding phase to assign to events.
    /// - `progress`: Channel receiving relayed progress events.
    /// - `logs`: Channel receiving relayed diagnostic events.
    /// - `redactions`: Path redactions applied by the process runner.
    /// - `cancellation`: Signal used to stop the active process.
    /// - `pause_control`: Receiver for pause and resume requests.
    /// - `control_events`: Channel receiving transition acknowledgements.
    ///
    /// # Returns
    ///
    /// The completed or cancelled process exit state.
    ///
    /// # Errors
    ///
    /// Returns an error when:
    ///
    /// - The process runner cannot execute the phase.
    /// - The process runner completes without returning an exit state.
    #[allow(clippy::too_many_arguments)]
    async fn run_phase(
        &self,
        command: Command,
        phase: RipPhase,
        progress: mpsc::Sender<RipProgressEvent>,
        logs: mpsc::Sender<FfmpegLogEvent>,
        redactions: FfmpegLogRedactions,
        cancellation: CancellationSignal,
        pause_control: &mut PauseControlSignal,
        control_events: mpsc::Sender<PauseControlEvent>,
    ) -> FFmpegResult<ProcessExit> {
        let (progress_sender, mut progress_receiver) = mpsc::channel(32);
        let (log_sender, mut log_receiver) = mpsc::channel(256);
        let process = self.runner.run_with_progress(
            command,
            phase,
            progress_sender,
            log_sender,
            redactions,
            cancellation,
            pause_control,
            control_events,
        );
        tokio::pin!(process);
        let mut result = None;
        let mut progress_open = true;
        let mut logs_open = true;

        while result.is_none() || progress_open || logs_open {
            tokio::select! {
                biased;
                process_result = &mut process, if result.is_none() => {
                    result = Some(process_result?);
                }
                snapshot = progress_receiver.recv(), if progress_open => {
                    match snapshot {
                        Some(snapshot) => {
                            let _ = progress.send(RipProgressEvent { phase, progress: snapshot }).await;
                        }
                        None => progress_open = false,
                    }
                }
                log = log_receiver.recv(), if logs_open => {
                    match log {
                        Some(log) => {
                            let _ = logs.try_send(log);
                        }
                        None => logs_open = false,
                    }
                }
            }
        }
        result
            .ok_or_else(|| FFmpegError::Io(std::io::Error::other("ffmpeg process did not return")))
    }
}

/// Creates the parent directory required by an `FFmpeg` output path.
///
/// # Parameters
///
/// - `path`: Output file whose parent directory should exist.
///
/// # Returns
///
/// `Ok(())` after the parent directory exists, or immediately when `path`
/// has no meaningful parent.
///
/// # Errors
///
/// Returns an error when:
///
/// - The parent directory cannot be created.
async fn ensure_output_parent(path: &Path) -> std::io::Result<()> {
    let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    else {
        return Ok(());
    };
    tokio::fs::create_dir_all(parent).await
}

/// Converts a completed process output into a successful extraction outcome.
///
/// # Parameters
///
/// - `output`: Captured `FFmpeg` process output.
/// - `redactions`: Path redactions applied to failure diagnostics.
///
/// # Returns
///
/// The successful process status.
///
/// # Errors
///
/// Returns an error when:
///
/// - `ffmpeg` exits with a non-success status.
fn outcome(output: &Output, redactions: &FfmpegLogRedactions) -> FFmpegResult<RipOutcome> {
    if !output.status.success() {
        return Err(FFmpegError::ProcessFailed {
            status: output.status,
            stderr: redactions.apply(String::from_utf8_lossy(&output.stderr).trim()),
        });
    }

    Ok(RipOutcome {
        status: output.status.to_string(),
    })
}

/// Runs a plain extraction using the production process runner.
///
/// # Parameters
///
/// - `input`: Source media path.
/// - `output`: Destination audio path.
///
/// # Returns
///
/// The completed process status.
///
/// # Errors
///
/// Returns an error when:
///
/// - The output directory cannot be prepared.
/// - `ffmpeg` cannot run or reports a failure.
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
                .any(|pair| pair[0] == "-af" && pair[1].contains("I=-23:LRA=20:TP=-2"))
        );
    }

    #[test]
    fn log_redactions_prefer_longer_paths_and_keep_filenames() {
        let request = RipRequest::new("/media/video", "/media/video.mp3");
        let redactions = FfmpegLogRedactions::for_request(&request);

        assert_eq!(
            redactions.apply("Input /media/video -> /media/video.mp3"),
            "Input <input>/video -> <output>/video.mp3"
        );
    }

    #[test]
    fn stderr_tail_is_bounded_to_the_latest_bytes() {
        let mut tail = Vec::new();
        append_tail(&mut tail, &vec![b'a'; STDERR_TAIL_LIMIT + 10]);
        assert_eq!(tail.len(), STDERR_TAIL_LIMIT);
        assert!(tail.iter().all(|byte| *byte == b'a'));

        append_tail(&mut tail, b"tail");
        assert_eq!(&tail[tail.len() - 4..], b"tail");
        assert!(tail.len() <= STDERR_TAIL_LIMIT);
    }

    #[tokio::test]
    async fn stderr_reader_emits_lines_with_phase_and_redacts_paths() {
        let mut command = Command::new("sh");
        command
            .arg("-c")
            .arg("printf 'Input /media/video.mp4\\r\\nDuration: 00:01\\n' >&2");
        command.stderr(Stdio::piped());
        let mut child = command.spawn().unwrap();
        let stderr = child.stderr.take().unwrap();
        let (logs, mut receiver) = mpsc::channel(8);
        let task = tokio::spawn(read_stderr(
            stderr,
            RipPhase::Encoding,
            logs,
            FfmpegLogRedactions {
                replacements: vec![("/media/video.mp4".into(), "<input>/video.mp4".into())],
            },
        ));

        child.wait().await.unwrap();
        let tail = task.await.unwrap().unwrap();
        let first = receiver.recv().await.unwrap();
        let second = receiver.recv().await.unwrap();

        assert_eq!(first.phase, RipPhase::Encoding);
        assert_eq!(first.message, "Input <input>/video.mp4");
        assert_eq!(second.message, "Duration: 00:01");
        assert!(tail.starts_with(b"Input"));
    }

    #[test]
    fn dispatch_log_line_handles_invalid_utf8_and_marks_truncation() {
        let (logs, mut receiver) = mpsc::channel(4);
        let redactions = FfmpegLogRedactions::default();
        let mut dropped = 0;
        assert!(!dispatch_log_line(
            &logs,
            RipPhase::Analyzing,
            &redactions,
            vec![0xff, b'!'],
            true,
            &mut dropped,
        ));

        let event = receiver.try_recv().unwrap();
        assert!(event.message.contains('�'));
        assert!(event.message.ends_with("[line truncated]"));
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn cancellation_stops_a_cooperative_child_without_forcing_it() {
        let mut command = Command::new("sh");
        command.arg("-c").arg("read line; test \"$line\" = q");
        let (handle, signal) = crate::ffmpeg::cancellation_pair();
        let (progress, _receiver) = mpsc::channel(1);
        let (logs, _receiver) = mpsc::channel(1);
        let (_pause_handle, mut pause_control) =
            crate::ffmpeg::pause_control_pair(crate::ffmpeg::PauseCapability::Supported);
        let (control_events, _receiver) = mpsc::channel(1);

        let task = tokio::spawn(async move {
            run_with_progress(
                command,
                RipPhase::Encoding,
                progress,
                logs,
                FfmpegLogRedactions::default(),
                signal,
                &mut pause_control,
                control_events,
                Duration::from_secs(1),
            )
            .await
        });
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
        let (logs, _receiver) = mpsc::channel(1);
        let (_pause_handle, mut pause_control) =
            crate::ffmpeg::pause_control_pair(crate::ffmpeg::PauseCapability::Supported);
        let (control_events, _receiver) = mpsc::channel(1);

        let task = tokio::spawn(async move {
            run_with_progress(
                command,
                RipPhase::Encoding,
                progress,
                logs,
                FfmpegLogRedactions::default(),
                signal,
                &mut pause_control,
                control_events,
                Duration::from_millis(30),
            )
            .await
        });
        tokio::task::yield_now().await;
        assert!(handle.cancel());

        let exit = tokio::time::timeout(Duration::from_secs(2), task)
            .await
            .expect("forced cancellation should be bounded")
            .unwrap()
            .unwrap();
        assert!(matches!(exit, ProcessExit::Cancelled { forced: true, .. }));
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn pause_resume_and_cancellation_preserve_a_live_process_handle() {
        let mut command = Command::new("sh");
        command
            .arg("-c")
            .arg("while read line; do test \"$line\" = q && exit 0; done");
        let (cancel_handle, cancellation) = crate::ffmpeg::cancellation_pair();
        let (pause_handle, mut pause_control) =
            crate::ffmpeg::pause_control_pair(crate::ffmpeg::PauseCapability::Supported);
        let (control_events, mut control_receiver) = mpsc::channel(8);
        let (progress, _progress_receiver) = mpsc::channel(1);
        let (logs, _logs_receiver) = mpsc::channel(1);

        let mut task = tokio::spawn(async move {
            run_with_progress(
                command,
                RipPhase::Encoding,
                progress,
                logs,
                FfmpegLogRedactions::default(),
                cancellation,
                &mut pause_control,
                control_events,
                Duration::from_secs(1),
            )
            .await
        });
        tokio::task::yield_now().await;

        pause_handle
            .request(crate::ffmpeg::PauseControlOperation::Pause)
            .unwrap();
        assert_eq!(
            control_receiver.recv().await,
            Some(crate::ffmpeg::PauseControlEvent::Paused {
                phase: RipPhase::Encoding,
            })
        );
        assert!(
            tokio::time::timeout(Duration::from_millis(50), &mut task)
                .await
                .is_err()
        );

        pause_handle
            .request(crate::ffmpeg::PauseControlOperation::Resume)
            .unwrap();
        assert_eq!(
            control_receiver.recv().await,
            Some(crate::ffmpeg::PauseControlEvent::Resumed {
                phase: RipPhase::Encoding,
            })
        );

        assert!(cancel_handle.cancel());
        let exit = task.await.unwrap().unwrap();
        assert!(matches!(exit, ProcessExit::Cancelled { forced: false, .. }));
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn cancellation_while_paused_resumes_before_quitting() {
        let mut command = Command::new("sh");
        command
            .arg("-c")
            .arg("while read line; do test \"$line\" = q && exit 0; done");
        let (cancel_handle, cancellation) = crate::ffmpeg::cancellation_pair();
        let (pause_handle, mut pause_control) =
            crate::ffmpeg::pause_control_pair(crate::ffmpeg::PauseCapability::Supported);
        let (control_events, mut control_receiver) = mpsc::channel(8);
        let (progress, _progress_receiver) = mpsc::channel(1);
        let (logs, _logs_receiver) = mpsc::channel(1);

        let task = tokio::spawn(async move {
            run_with_progress(
                command,
                RipPhase::Encoding,
                progress,
                logs,
                FfmpegLogRedactions::default(),
                cancellation,
                &mut pause_control,
                control_events,
                Duration::from_secs(1),
            )
            .await
        });
        tokio::task::yield_now().await;

        pause_handle
            .request(crate::ffmpeg::PauseControlOperation::Pause)
            .unwrap();
        assert_eq!(
            control_receiver.recv().await,
            Some(crate::ffmpeg::PauseControlEvent::Paused {
                phase: RipPhase::Encoding,
            })
        );
        assert!(cancel_handle.cancel());

        let exit = tokio::time::timeout(Duration::from_secs(2), task)
            .await
            .expect("cancellation from a paused process should be bounded")
            .unwrap()
            .unwrap();
        assert!(matches!(exit, ProcessExit::Cancelled { forced: false, .. }));
    }
}
