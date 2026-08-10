pub type ProbeResult<T, E = ProbeError> = std::result::Result<T, E>;

/// Represents failures while probing media metadata with `ffprobe`.
///
/// # Variants
///
/// - [`Ffprobe`](Self::Ffprobe): The `ffprobe` process exited unsuccessfully.
/// - [`Io`](Self::Io): The operating system could not start or communicate
///   with the `ffprobe` process.
/// - [`Json`](Self::Json): `ffprobe` output could not be decoded as JSON.
/// - [`NoAudio`](Self::NoAudio): The media contains no audio stream.
/// - [`InvalidDuration`](Self::InvalidDuration): The reported duration could
///   not be parsed as a floating-point value.
#[derive(Debug, thiserror::Error)]
pub enum ProbeError {
    /// Captures standard error from an unsuccessful `ffprobe` process.
    #[error("ffprobe failed: {0}")]
    Ffprobe(String),
    /// Wraps an I/O failure while starting or communicating with `ffprobe`.
    #[error(transparent)]
    Io(#[from] std::io::Error),
    /// Wraps a JSON decoding failure from `ffprobe` output.
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    /// Indicates that the inspected media has no audio stream.
    #[error("media contains no audio stream")]
    NoAudio,
    /// Wraps a failure to parse the media duration as a floating-point value.
    #[error("invalid media duration: {0}")]
    InvalidDuration(#[from] std::num::ParseFloatError),
}
