use std::process::{Command, Output};

use crate::ffmpeg::FFmpegResult;

pub fn rip(input: &str, output: &str) -> FFmpegResult<Output> {
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
        .map_err(Into::into)
}
