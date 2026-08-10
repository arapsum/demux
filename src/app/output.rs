use std::path::{Path, PathBuf};

const RIP_TARGET_EXTENSION: &str = "mp3";

pub(super) fn output_path(
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

#[cfg(test)]
mod tests {
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
}
