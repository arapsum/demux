use std::{path::PathBuf, sync::Arc};

use tracing::{Instrument, info_span};

use crate::{
    Error, Result,
    app::{output, preferences},
    ffmpeg::{
        self, CancellationSignal, Dependencies, FfmpegAudioRipper, FfmpegLogEvent,
        PauseControlEvent, PauseControlSignal, RipProgressEvent, RipRequest, RipTermination,
        TokioProcessRunner,
    },
    ffprobe,
    model::{encoding::RipOptions, job::JobId, media::MediaInfo, source::DestinationPolicy},
};

pub const PROBE_CONCURRENCY: usize = 4;

/// Probes one input while limiting concurrent `ffprobe` processes.
///
/// # Parameters
///
/// - `job_id`: Queue identifier used for tracing the probe.
/// - `input`: Media file to inspect.
///
/// # Returns
///
/// Metadata for the first audio stream after a probe permit is acquired.
///
/// # Errors
///
/// Returns an error when:
///
/// - The probe semaphore is closed.
/// - `ffprobe` cannot inspect or convert the input.
pub async fn probe_bounded(job_id: JobId, input: PathBuf) -> Result<MediaInfo> {
    static SEMAPHORE: std::sync::OnceLock<Arc<tokio::sync::Semaphore>> = std::sync::OnceLock::new();
    let semaphore = SEMAPHORE
        .get_or_init(|| Arc::new(tokio::sync::Semaphore::new(PROBE_CONCURRENCY)))
        .clone();
    let permit = semaphore.acquire_owned().await?;
    let result = probe(job_id, input).await;
    drop(permit);
    result
}

/// Runs production dependency detection away from the GUI runtime thread.
///
/// # Returns
///
/// Version descriptions for the required `ffmpeg` and `ffprobe` executables.
///
/// # Errors
///
/// Returns an error when:
///
/// - The blocking dependency task cannot complete.
/// - Either executable is missing, cannot start, or exits unsuccessfully.
pub async fn check_dependencies() -> Result<Dependencies> {
    Ok(tokio::task::spawn_blocking(ffmpeg::detect_dependencies).await??)
}

/// Opens a trusted product link with the platform's default browser.
///
/// # Parameters
///
/// - `url`: Trusted URL to open.
///
/// # Returns
///
/// `Ok(())` after the platform browser command starts.
///
/// # Errors
///
/// Returns an error when:
///
/// - The blocking browser-launch task cannot complete.
/// - The operating system cannot start the platform browser command.
pub async fn open_external_link(url: &'static str) -> Result<()> {
    tokio::task::spawn_blocking(move || {
        #[cfg(target_os = "windows")]
        let mut command = {
            let mut command = std::process::Command::new("cmd");
            command.args(["/C", "start", ""]);
            command
        };

        #[cfg(target_os = "macos")]
        let mut command = std::process::Command::new("open");

        #[cfg(all(unix, not(target_os = "macos")))]
        let mut command = std::process::Command::new("xdg-open");

        command
            .arg(url)
            .spawn()
            .map(|_| ())
            .map_err(|source| Error::ExternalLink {
                url: url.to_owned(),
                source,
            })
    })
    .await??;
    Ok(())
}

/// Loads persisted application defaults without blocking the GUI runtime.
///
/// # Returns
///
/// The stored encoding, destination, and window defaults.
///
/// # Errors
///
/// Returns an error when:
///
/// - The preferences directory cannot be determined.
/// - The stored preferences cannot be read or parsed.
pub async fn load_preferences() -> Result<preferences::PreferenceDefaults> {
    preferences::load().await
}

pub fn next_preferences_revision() -> u64 {
    preferences::next_revision()
}

/// Persists application defaults without blocking the GUI runtime.
///
/// # Parameters
///
/// - `options`: Encoding defaults to save.
/// - `destination`: Destination policy to save.
/// - `window`: Window preferences to save.
/// - `revision`: Monotonic revision identifying this save request.
///
/// # Returns
///
/// `Ok(())` after the defaults are saved or a newer revision supersedes them.
///
/// # Errors
///
/// Returns an error when:
///
/// - The preferences directory cannot be determined or created.
/// - The preferences cannot be serialized or written.
pub async fn save_preferences(
    options: RipOptions,
    destination: DestinationPolicy,
    window: preferences::WindowPreferences,
    revision: u64,
) -> Result<()> {
    preferences::save(options, destination, window, revision).await
}

/// Probes one media input through the production `FFprobe` adapter.
///
/// # Parameters
///
/// - `job_id`: Queue identifier used for tracing the probe.
/// - `input`: Media file to inspect.
///
/// # Returns
///
/// Parsed metadata for the input's first audio stream.
///
/// # Errors
///
/// Returns an error when:
///
/// - `ffprobe` cannot start or exits unsuccessfully.
/// - The probe output cannot be converted into media metadata.
pub async fn probe(job_id: JobId, input: PathBuf) -> Result<MediaInfo> {
    let span = info_span!("media_probe_job", job_id = job_id.0);
    async move { Ok(ffprobe::inspect(&input.to_string_lossy()).await?) }
        .instrument(span)
        .await
}

/// Extracts one audio file and forwards bounded machine-readable progress.
///
/// # Parameters
///
/// - `job_id`: Queue identifier used for tracing and event correlation.
/// - `request`: Immutable input, output, and encoding policy snapshot.
/// - `progress`: Channel receiving machine-readable progress events.
/// - `logs`: Channel receiving bounded user-facing `FFmpeg` diagnostics.
/// - `cancellation`: Signal used to stop the active phase and clean up output.
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
/// - Process setup, progress handling, or `FFmpeg` execution fails.
/// - Cancelled-output cleanup fails.
pub async fn rip_with_progress(
    job_id: JobId,
    request: RipRequest,
    progress: tokio::sync::mpsc::Sender<RipProgressEvent>,
    logs: tokio::sync::mpsc::Sender<FfmpegLogEvent>,
    cancellation: CancellationSignal,
    pause_control: PauseControlSignal,
    control_events: tokio::sync::mpsc::Sender<PauseControlEvent>,
) -> Result<RipTermination> {
    let span = info_span!("audio_rip_progress_job", job_id = job_id.0);
    async move {
        let termination = FfmpegAudioRipper::<TokioProcessRunner>::default()
            .rip_with_progress_cancellable(
                &request,
                progress,
                logs,
                cancellation,
                pause_control,
                control_events,
            )
            .await?;

        if matches!(termination, RipTermination::Cancelled { .. }) {
            cleanup_partial_output(&request.output).await?;
        }

        Ok(termination)
    }
    .instrument(span)
    .await
}

/// Removes a partially written output after cancellation.
///
/// # Parameters
///
/// - `path`: Output path that may have been partially written.
///
/// # Returns
///
/// `Ok(())` after the partial file is removed or confirmed absent.
///
/// # Errors
///
/// Returns an error when:
///
/// - The partial output exists but cannot be removed.
async fn cleanup_partial_output(path: &std::path::Path) -> Result<()> {
    match tokio::fs::remove_file(path).await {
        Ok(()) => {
            tracing::info!(path = %path.display(), "removed partial output after cancellation");
            Ok(())
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(source) => Err(Error::PartialOutputCleanup {
            path: path.to_path_buf(),
            source,
        }),
    }
}

/// Resolves an output without overwriting an existing file.
///
/// # Parameters
///
/// - `job_id`: Queue identifier used for tracing output resolution.
/// - `requested`: Preferred output path.
///
/// # Returns
///
/// A collision-safe output path whose parent directory exists.
///
/// # Errors
///
/// Returns an error when:
///
/// - Existing output paths cannot be inspected.
/// - The resolved output directory cannot be created.
pub async fn resolve_output(job_id: JobId, requested: PathBuf) -> Result<PathBuf> {
    let span = info_span!("resolve_rip_output", job_id = job_id.0);
    async move {
        let resolved = output::available_output_path(&requested)
            .await
            .map_err(|source| Error::OutputInspection {
                path: requested.clone(),
                source,
            })?;
        output::ensure_output_parent(&resolved)
            .await
            .map_err(|source| Error::OutputInspection {
                path: resolved.clone(),
                source,
            })?;
        tracing::debug!(requested = %requested.display(), resolved = %resolved.display(), "resolved collision-safe output");
        Ok(resolved)
    }
    .instrument(span)
    .await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn cancellation_cleanup_removes_a_partial_output_and_tolerates_missing_files() {
        let directory = std::env::temp_dir().join(format!(
            "demux-runtime-cleanup-{}-{}",
            std::process::id(),
            JobId::new(11).0
        ));
        tokio::fs::create_dir_all(&directory).await.unwrap();
        let partial = directory.join("partial.mp3");
        tokio::fs::write(&partial, b"partial audio").await.unwrap();

        cleanup_partial_output(&partial).await.unwrap();
        assert!(!partial.exists());
        cleanup_partial_output(&partial).await.unwrap();

        tokio::fs::remove_dir(&directory).await.unwrap();
    }

    #[tokio::test]
    async fn cancellation_cleanup_keeps_a_typed_path_aware_error() {
        let directory = std::env::temp_dir().join(format!(
            "demux-runtime-cleanup-error-{}-{}",
            std::process::id(),
            JobId::new(12).0
        ));
        tokio::fs::create_dir_all(&directory).await.unwrap();

        let error = cleanup_partial_output(&directory).await.unwrap_err();
        assert!(matches!(
            error,
            Error::PartialOutputCleanup { path, .. } if path == directory
        ));

        tokio::fs::remove_dir(&directory).await.unwrap();
    }
}
