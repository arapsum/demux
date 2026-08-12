use std::{path::PathBuf, sync::Arc};

use tracing::{Instrument, info_span};

use crate::{
    Error, Result,
    app::output,
    ffmpeg::{
        self, Dependencies, FfmpegAudioRipper, FfmpegProgress, RipOutcome, RipRequest,
        TokioProcessRunner,
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
) -> Result<RipOutcome> {
    let span = info_span!("audio_rip_progress_job", job_id = job_id.0);
    async move {
        Ok(FfmpegAudioRipper::<TokioProcessRunner>::default()
            .rip_with_progress(&request, progress)
            .await?)
    }
    .instrument(span)
    .await
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
