use std::path::{Path, PathBuf};

use crate::model::{
    encoding::OutputFormat,
    source::{DestinationPolicy, SourceHierarchy},
};

const RIP_TARGET_EXTENSION: &str = "mp3";

pub(crate) async fn available_output_path(requested: &Path) -> std::io::Result<PathBuf> {
    if !tokio::fs::try_exists(requested).await? {
        return Ok(requested.to_path_buf());
    }

    let parent = requested.parent().map_or_else(PathBuf::new, PathBuf::from);
    let stem = requested
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or("output");
    let extension = requested
        .extension()
        .and_then(|extension| extension.to_str());

    for suffix in 2_u64.. {
        let mut filename = PathBuf::from(format!("{stem} ({suffix})"));
        if let Some(extension) = extension {
            filename.set_extension(extension);
        }
        let candidate = parent.join(filename);
        if !tokio::fs::try_exists(&candidate).await? {
            return Ok(candidate);
        }
    }

    unreachable!("the numeric output suffix space is unbounded")
}

pub(crate) fn output_path(
    input: &str,
    output_directory: Option<&str>,
    output_filename: Option<&str>,
) -> String {
    let input = Path::new(input);
    let filename = output_filename.map_or_else(
        || {
            input
                .file_name()
                .map_or_else(|| PathBuf::from("output"), PathBuf::from)
                .with_extension(RIP_TARGET_EXTENSION)
        },
        PathBuf::from,
    );
    let directory = output_directory.map_or_else(
        || input.parent().map_or_else(PathBuf::new, PathBuf::from),
        PathBuf::from,
    );

    directory.join(filename).to_string_lossy().into_owned()
}

/// Derives a destination while preserving the relative path of a folder
/// import when that policy is enabled.
pub(crate) fn destination_path(
    input: &Path,
    hierarchy: Option<&SourceHierarchy>,
    output_directory: Option<&Path>,
    format: OutputFormat,
    policy: DestinationPolicy,
) -> PathBuf {
    let filename = input
        .file_name()
        .map_or_else(|| PathBuf::from("output"), PathBuf::from)
        .with_extension(format.extension());
    let base = output_directory.map_or_else(
        || {
            hierarchy
                .filter(|_| policy.preserve_folder_structure)
                .map_or_else(
                    || input.parent().map_or_else(PathBuf::new, PathBuf::from),
                    |source| source.root().to_path_buf(),
                )
        },
        PathBuf::from,
    );

    if policy.preserve_folder_structure
        && let Some(hierarchy) = hierarchy
    {
        return base
            .join(hierarchy.relative_path())
            .with_extension(format.extension());
    }
    base.join(filename)
}

pub(crate) async fn ensure_output_parent(path: &Path) -> std::io::Result<()> {
    let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    else {
        return Ok(());
    };
    tokio::fs::create_dir_all(parent).await
}

#[cfg(test)]
mod tests {
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::*;

    #[test]
    fn derives_an_mp3_path_in_the_input_directory() {
        let output = output_path("/videos/reality.mp4", None, None);

        assert_eq!(output, "/videos/reality.mp3");
    }

    #[test]
    fn derives_an_mp3_filename_in_a_selected_directory() {
        let output = output_path("/videos/reality.mp4", Some("/music"), None);

        assert_eq!(output, "/music/reality.mp3");
    }

    #[test]
    fn uses_a_selected_filename_in_the_input_directory() {
        let output = output_path("/videos/reality.mp4", None, Some("favourite-track.mp3"));

        assert_eq!(output, "/videos/favourite-track.mp3");
    }

    #[test]
    fn replaces_a_missing_input_extension() {
        let output = output_path("/videos/reality", None, None);

        assert_eq!(output, "/videos/reality.mp3");
    }

    #[test]
    fn combines_a_selected_directory_and_filename() {
        let output = output_path("/videos/reality.mp4", Some("/music"), Some("track.mp3"));

        assert_eq!(output, "/music/track.mp3");
    }

    #[test]
    fn preserves_a_folder_import_relative_to_its_selected_root() {
        let hierarchy = SourceHierarchy::new("/videos", "Season 1/episode.mp4").unwrap();
        let output = destination_path(
            Path::new("/videos/Season 1/episode.mp4"),
            Some(&hierarchy),
            Some(Path::new("/music")),
            OutputFormat::Mp3,
            DestinationPolicy::default(),
        );

        assert_eq!(output, PathBuf::from("/music/Season 1/episode.mp3"));
    }

    #[test]
    fn flattening_a_folder_import_uses_only_the_filename() {
        let hierarchy = SourceHierarchy::new("/videos", "Season 1/episode.mp4").unwrap();
        let output = destination_path(
            Path::new("/videos/Season 1/episode.mp4"),
            Some(&hierarchy),
            Some(Path::new("/music")),
            OutputFormat::Mp3,
            DestinationPolicy {
                preserve_folder_structure: false,
            },
        );

        assert_eq!(output, PathBuf::from("/music/episode.mp3"));
    }

    #[tokio::test]
    async fn collision_policy_chooses_the_first_available_numbered_name() {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!("demux-output-{suffix}"));
        tokio::fs::create_dir_all(&root).await.unwrap();
        tokio::fs::write(root.join("song.mp3"), b"existing")
            .await
            .unwrap();
        tokio::fs::write(root.join("song (2).mp3"), b"existing")
            .await
            .unwrap();

        let resolved = available_output_path(&root.join("song.mp3")).await.unwrap();

        assert_eq!(resolved, root.join("song (3).mp3"));
        tokio::fs::remove_dir_all(root).await.unwrap();
    }
}
