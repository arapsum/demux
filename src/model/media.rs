use std::time::Duration;

/// Describes the container and primary audio stream in a media file.
///
/// Demux derives this domain representation from raw `ffprobe` JSON after
/// selecting the first audio stream from the source media.
///
/// # Fields
///
/// - `duration`: Total duration of the media file.
/// - `container`: The container format name reported by `ffprobe`.
/// - `bitrate`: Overall container bitrate in bits per second, when reported.
/// - `creation_time`: Container creation timestamp, when reported.
/// - `tags`: Safe source tags that may be copied to the output.
/// - `artwork`: Embedded cover art discovered in the source, when present.
/// - `audio`: Details of the selected audio stream.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MediaInfo {
    pub duration: Duration,
    pub container: String,
    pub bitrate: Option<u64>,
    pub creation_time: Option<String>,
    pub tags: Box<MetadataTags>,
    pub artwork: Option<ArtworkInfo>,
    pub audio: AudioMetadata,
}

/// Safe, user-facing tags that Demux may copy to an extracted audio file.
///
/// The allowlist deliberately excludes arbitrary container metadata. Values
/// remain optional because media files commonly contain only a subset of tags.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MetadataTags {
    pub title: Option<String>,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub album_artist: Option<String>,
    pub date: Option<String>,
    pub track: Option<String>,
    pub disc: Option<String>,
    pub genre: Option<String>,
    pub composer: Option<String>,
    pub comment: Option<String>,
    pub copyright: Option<String>,
    pub language: Option<String>,
}

impl MetadataTags {
    /// Returns sanitized FFmpeg metadata entries in a stable order.
    pub fn entries(&self) -> impl Iterator<Item = (&'static str, String)> + '_ {
        [
            ("title", self.title.as_deref()),
            ("artist", self.artist.as_deref()),
            ("album", self.album.as_deref()),
            ("album_artist", self.album_artist.as_deref()),
            ("date", self.date.as_deref()),
            ("track", self.track.as_deref()),
            ("disc", self.disc.as_deref()),
            ("genre", self.genre.as_deref()),
            ("composer", self.composer.as_deref()),
            ("comment", self.comment.as_deref()),
            ("copyright", self.copyright.as_deref()),
            ("language", self.language.as_deref()),
        ]
        .into_iter()
        .filter_map(|(key, value)| sanitize_metadata_value(value).map(|value| (key, value)))
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries().next().is_none()
    }
}

fn sanitize_metadata_value(value: Option<&str>) -> Option<String> {
    let value = value?.replace('\0', "");
    let value = value.trim();
    (!value.is_empty()).then(|| value.to_owned())
}

/// Describes embedded cover art discovered in a source media file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArtworkInfo {
    pub stream_index: usize,
    pub codec: String,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub mime_type: Option<String>,
}

impl ArtworkInfo {
    /// MP3 cover art is written as an ID3 attached-picture stream. JPEG and
    /// PNG are the formats Demux can confidently preserve across players.
    #[must_use]
    pub fn supports_mp3(&self) -> bool {
        matches!(
            self.codec.to_ascii_lowercase().as_str(),
            "mjpeg" | "jpeg" | "jpg" | "png"
        )
    }

    #[must_use]
    pub fn format_label(&self) -> &'static str {
        match self.codec.to_ascii_lowercase().as_str() {
            "mjpeg" | "jpeg" | "jpg" => "JPEG",
            "png" => "PNG",
            _ => "Unsupported",
        }
    }
}

/// Describes the audio stream selected for a ripping job.
///
/// # Fields
///
/// - `stream_index`: The source stream index used to identify the audio.
/// - `codec`: The audio codec name, or `"unknown"` when it is unavailable.
/// - `sample_rate`: Samples per second, when reported.
/// - `channels`: Number of audio channels, when reported.
/// - `channel_layout`: Named channel arrangement, such as `stereo`.
/// - `bitrate`: Stream bitrate in bits per second, when reported.
/// - `language`: Stream language tag, when reported.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AudioMetadata {
    pub stream_index: usize,
    pub codec: String,
    pub sample_rate: Option<u32>,
    pub channels: Option<u8>,
    pub channel_layout: Option<String>,
    pub bitrate: Option<u64>,
    pub language: Option<String>,
}
