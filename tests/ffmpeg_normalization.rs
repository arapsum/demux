//! End-to-end normalization checks. These tests are ignored for ordinary
//! development because they require the system FFmpeg binary.

use std::{
    path::{Path, PathBuf},
    process::Command,
    time::{SystemTime, UNIX_EPOCH},
};

use demux::{
    ffmpeg::{FfmpegAudioRipper, LoudnessMeasurement, RipRequest, TokioProcessRunner},
    model::encoding::RipOptions,
};

#[test]
#[ignore = "requires system ffmpeg"]
fn normalized_outputs_hit_the_broadcast_loudness_target() {
    let root = temporary_directory();
    std::fs::create_dir_all(&root).unwrap();

    let fixtures = [
        (
            "quiet-mono",
            "sine=frequency=440:duration=2",
            "volume=0.08",
            1_u8,
        ),
        (
            "dynamic-stereo",
            "sine=frequency=220:duration=2",
            "volume=0.25",
            2_u8,
        ),
    ];

    for (name, source, volume, channels) in fixtures {
        let input = root.join(format!("{name}.wav"));
        let output = root.join(format!("{name}.mp3"));
        let mut generate = Command::new("ffmpeg");
        generate
            .args([
                "-hide_banner",
                "-loglevel",
                "error",
                "-f",
                "lavfi",
                "-i",
                source,
                "-af",
                volume,
                "-ac",
                &channels.to_string(),
                "-c:a",
                "pcm_s16le",
            ])
            .arg(&input);
        assert!(generate.status().unwrap().success());

        let request = RipRequest::with_options(
            &input,
            &output,
            RipOptions {
                normalize_audio: true,
                ..RipOptions::default()
            },
        );
        let runtime = tokio::runtime::Runtime::new().unwrap();
        runtime
            .block_on(FfmpegAudioRipper::<TokioProcessRunner>::default().rip(&request))
            .unwrap();

        let measurement = measure(&output);
        assert!(
            (measurement.integrated_lufs + 23.0).abs() <= 0.8,
            "{name} integrated loudness was {} LUFS",
            measurement.integrated_lufs
        );
        assert!(
            measurement.true_peak <= -0.7,
            "{name} true peak was {} dBTP",
            measurement.true_peak
        );
    }

    std::fs::remove_dir_all(root).unwrap();
}

fn measure(path: &Path) -> LoudnessMeasurement {
    let output = Command::new("ffmpeg")
        .args(["-hide_banner", "-loglevel", "info", "-i"])
        .arg(path)
        .args([
            "-af",
            "loudnorm=I=-23:LRA=50:TP=-1:print_format=json",
            "-f",
            "null",
            "-",
        ])
        .output()
        .unwrap();
    assert!(output.status.success());
    LoudnessMeasurement::parse(&output.stderr).unwrap()
}

fn temporary_directory() -> PathBuf {
    let suffix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!("demux-normalization-{suffix}"))
}
