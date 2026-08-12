use std::path::PathBuf;

use tracing::{Instrument, info_span};

use crate::{
    ffmpeg::{self, Dependencies, FfmpegAudioRipper, RipOutcome, RipRequest, TokioProcessRunner},
    ffprobe,
    model::{job::JobId, media::MediaInfo},
};

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
