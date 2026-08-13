use std::{path::PathBuf, sync::Arc};

use tracing::{Instrument, info_span};

use crate::{
    Error, Result,
    app::output,
    ffmpeg::{
        self, CancellationSignal, Dependencies, FfmpegAudioRipper, FfmpegProgress, RipRequest,
        RipTermination, TokioProcessRunner,
    },
    ffprobe,
    model::{job::JobId, media::MediaInfo},
};

pub const PROBE_CONCURRENCY: usize = 4;

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
pub async fn check_dependencies() -> Result<Dependencies> {
    Ok(tokio::task::spawn_blocking(ffmpeg::detect_dependencies).await??)
}

/// Probes one media input through the production FFprobe adapter.
pub async fn probe(job_id: JobId, input: PathBuf) -> Result<MediaInfo> {
    let span = info_span!("media_probe_job", job_id = job_id.0);
    async move { Ok(ffprobe::inspect(&input.to_string_lossy()).await?) }
        .instrument(span)
        .await
}

/// Extracts one audio file and forwards bounded machine-readable progress.
pub async fn rip_with_progress(
    job_id: JobId,
    request: RipRequest,
    progress: tokio::sync::mpsc::Sender<FfmpegProgress>,
    cancellation: CancellationSignal,
) -> Result<RipTermination> {
    let span = info_span!("audio_rip_progress_job", job_id = job_id.0);
    async move {
        let termination = FfmpegAudioRipper::<TokioProcessRunner>::default()
            .rip_with_progress_cancellable(&request, progress, cancellation)
            .await?;

        if matches!(termination, RipTermination::Cancelled { .. }) {
            cleanup_partial_output(&request.output).await?;
        }

        Ok(termination)
    }
    .instrument(span)
    .await
}

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
pub async fn resolve_output(job_id: JobId, requested: PathBuf) -> Result<PathBuf> {
    let span = info_span!("resolve_rip_output", job_id = job_id.0);
    async move {
        let resolved = output::available_output_path(&requested)
            .await
            .map_err(|source| Error::OutputInspection {
                path: requested.clone(),
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
