use std::{path::PathBuf, sync::Arc};

use tracing::{Instrument, info_span};

use crate::{
    app::output,
    ffmpeg::{self, Dependencies, FfmpegAudioRipper, RipOutcome, RipRequest, TokioProcessRunner},
    ffprobe,
    model::{job::JobId, media::MediaInfo},
};

pub const PROBE_CONCURRENCY: usize = 4;

pub async fn probe_bounded(job_id: JobId, input: PathBuf) -> Result<MediaInfo, String> {
    static SEMAPHORE: std::sync::OnceLock<Arc<tokio::sync::Semaphore>> = std::sync::OnceLock::new();
    let semaphore = SEMAPHORE
        .get_or_init(|| Arc::new(tokio::sync::Semaphore::new(PROBE_CONCURRENCY)))
        .clone();
    let permit = semaphore
        .acquire_owned()
        .await
        .map_err(|error| format!("Could not schedule media probe: {error}"))?;
    let result = probe(job_id, input).await;
    drop(permit);
    result
}

/// Runs production dependency detection away from the GUI runtime thread.
pub async fn check_dependencies() -> Result<Dependencies, String> {
    tokio::task::spawn_blocking(ffmpeg::detect_dependencies)
        .await
        .map_err(|error| error.to_string())?
        .map_err(|error| error.to_string())
}

/// Probes one media input through the production FFprobe adapter.
pub async fn probe(job_id: JobId, input: PathBuf) -> Result<MediaInfo, String> {
    let span = info_span!("media_probe_job", job_id = job_id.0);
    async move {
        ffprobe::inspect(&input.to_string_lossy())
            .await
            .map_err(|error| error.to_string())
    }
    .instrument(span)
    .await
}

/// Extracts one audio file through the production FFmpeg adapter.
pub async fn rip(job_id: JobId, request: RipRequest) -> Result<RipOutcome, String> {
    let span = info_span!("audio_rip_job", job_id = job_id.0);
    async move {
        FfmpegAudioRipper::<TokioProcessRunner>::default()
            .rip(&request)
            .await
            .map_err(|error| error.to_string())
    }
    .instrument(span)
    .await
}

/// Resolves an output without overwriting an existing file.
pub async fn resolve_output(job_id: JobId, requested: PathBuf) -> Result<PathBuf, String> {
    let span = info_span!("resolve_rip_output", job_id = job_id.0);
    async move {
        let resolved = output::available_output_path(&requested)
            .await
            .map_err(|error| format!("Could not inspect the output folder: {error}"))?;
        tracing::debug!(requested = %requested.display(), resolved = %resolved.display(), "resolved collision-safe output");
        Ok(resolved)
    }
    .instrument(span)
    .await
}
