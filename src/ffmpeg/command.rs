use std::process::Output;

use tokio::process::Command;

use crate::ffmpeg::FFmpegResult;

/// Extracts an MP3 audio track from a media file with `FFmpeg`.
///
/// The generated command disables video output and encodes the selected audio
/// stream with `libmp3lame` at 192 kbps:
///
/// ```text
/// ffmpeg -i <input> -vn -c:a libmp3lame -b:a 192k <output>
/// ```
///
/// # Parameters
///
/// - `input`: Path to the source media file.
/// - `output`: Path for the generated MP3 file.
///
/// # Returns
///
/// The captured `FFmpeg` process output after the asynchronous command
/// completes. Callers must check
/// [`Output::status`] because `FFmpeg` failures, such as an unreadable input or
/// unwritable output, result in a non-success status rather than an
/// [`Err`][Result::Err].
///
/// # Errors
///
/// Returns an error when the `ffmpeg` process cannot be started:
///
/// - `ffmpeg` is missing from `PATH`.
/// - The operating system cannot execute `ffmpeg`.
///
/// # Examples
///
/// ```no_run
/// use demux::ffmpeg::rip;
///
/// # async fn example() -> demux::Result<()> {
/// let result = rip("input.mp4", "output.mp3").await?;
/// assert!(result.status.success());
/// # Ok(())
/// # }
/// ```
pub async fn rip(input: &str, output: &str) -> FFmpegResult<Output> {
    Command::new("ffmpeg")
        .arg("-i")
        .arg(input)
        .arg("-vn")
        .arg("-c:a")
        .arg("libmp3lame")
        .arg("-b:a")
        .arg("192k")
        .arg(output)
        .output()
        .await
        .map_err(Into::into)
}
