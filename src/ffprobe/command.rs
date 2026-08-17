use std::process::Output;

use tokio::process::Command;

use crate::{ffprobe::ProbeError, model::media::MediaInfo};

use super::{error::ProbeResult, output::ProbeOutput};

/// Runs `ffprobe` asynchronously to inspect the first audio stream and any
/// attached cover-art stream in a media file.
///
/// The command requests JSON containing all stream dispositions and the
/// allowlisted container/stream tags needed to build [`MediaInfo`]. The
/// converter selects the first audio stream instead of assuming stream zero,
/// which keeps cover-art and video streams from changing audio selection.
///
/// # Parameters
///
/// - `input`: Path to the media file to inspect.
///
/// # Returns
///
/// The captured `ffprobe` process output after the command completes.
///
/// # Errors
///
/// Returns an error when `ffprobe` cannot inspect the input:
///
/// - The operating system cannot start `ffprobe`.
/// - `ffprobe` exits with a non-success status.
pub async fn probe(input: &str) -> ProbeResult<Output> {
    let cmd = Command::new("ffprobe")
        .arg("-v")
        .arg("error")
        .arg("-show_entries")
        .arg("format=format_name,duration,bit_rate:format_tags=creation_time,title,artist,album,album_artist,date,track,disc,genre,composer,comment,copyright,language:stream=index,codec_type,codec_name,sample_rate,channels,channel_layout,bit_rate,width,height:stream_disposition=attached_pic:stream_tags=language,title,artist,album,album_artist,date,track,disc,genre,composer,comment,copyright")
        .arg("-of")
        .arg("json")
        .arg(input)
        .output()
        .await?;

    if !cmd.status.success() {
        return Err(ProbeError::Ffprobe(
            String::from_utf8_lossy(&cmd.stderr).to_string(),
        ));
    }

    Ok(cmd)
}

/// Converts successful `ffprobe` output into Demux media metadata.
///
/// # Parameters
///
/// - `output`: Successful JSON output captured from `ffprobe`.
///
/// # Returns
///
/// Parsed container and first-audio-stream metadata.
///
/// # Errors
///
/// Returns an error when the probe output cannot be converted:
///
/// - The output is not valid JSON matching [`ProbeOutput`].
/// - The media contains no audio stream.
/// - The reported duration is not a valid floating-point number.
pub fn metadata(output: &Output) -> ProbeResult<MediaInfo> {
    let probe: ProbeOutput = serde_json::from_slice(&output.stdout)?;

    MediaInfo::try_from(probe)
}

/// Inspects a media file and returns domain metadata in one operation.
///
/// This is the application-facing probing API; raw process output and JSON
/// conversion remain internal details of the adapter.
///
/// # Parameters
///
/// - `input`: Path to the media file to inspect.
///
/// # Returns
///
/// Parsed container and first-audio-stream metadata.
///
/// # Errors
///
/// Returns an error when:
///
/// - `ffprobe` cannot inspect the input.
/// - The probe output cannot be converted into [`MediaInfo`].
#[tracing::instrument(name = "ffprobe_inspect", level = "debug", skip_all)]
pub async fn inspect(input: &str) -> ProbeResult<MediaInfo> {
    tracing::trace!(input = %input, "launching ffprobe");
    let output = probe(input).await?;

    tracing::trace!(stdout_bytes = output.stdout.len(), "parsing ffprobe output");

    metadata(&output)
}
