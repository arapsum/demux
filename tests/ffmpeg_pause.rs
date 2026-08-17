use std::{path::PathBuf, time::Duration};

use demux::ffmpeg::{
    FfmpegLogRedactions, PauseCapability, PauseControlEvent, RipPhase, TokioProcessRunner,
    cancellation_pair, pause_control_pair,
};
use tokio::{process::Command, sync::mpsc};

#[tokio::test]
#[ignore = "requires system ffmpeg and ffprobe"]
async fn ffmpeg_pause_resume_completes_with_a_valid_output() {
    assert!(
        dependency_available("ffmpeg").await,
        "FFmpeg must be installed for the pause integration test"
    );
    assert!(
        dependency_available("ffprobe").await,
        "FFprobe must be installed for the pause integration test"
    );

    let directory = temporary_directory();
    tokio::fs::create_dir_all(&directory).await.unwrap();
    let output = directory.join("paused.mp3");
    let mut command = Command::new("ffmpeg");
    command
        .args([
            "-hide_banner",
            "-loglevel",
            "error",
            "-re",
            "-f",
            "lavfi",
            "-i",
            "sine=frequency=1000:sample_rate=48000",
            "-t",
            "5",
            "-c:a",
            "libmp3lame",
            "-b:a",
            "128k",
            "-progress",
            "pipe:1",
            "-nostats",
        ])
        .arg(&output);

    let (cancel_handle, cancellation) = cancellation_pair();
    let (pause_handle, mut pause_control) = pause_control_pair(PauseCapability::Supported);
    let (control_events, mut control_receiver) = mpsc::channel(8);
    let (progress_sender, mut progress_receiver) = mpsc::channel(32);
    let (logs, _logs_receiver) = mpsc::channel(32);

    let mut task = tokio::spawn(async move {
        <TokioProcessRunner as demux::ffmpeg::ProgressProcessRunner>::run_with_progress(
            &TokioProcessRunner,
            command,
            RipPhase::Encoding,
            progress_sender,
            logs,
            FfmpegLogRedactions::default(),
            cancellation,
            &mut pause_control,
            control_events,
        )
        .await
    });

    tokio::time::timeout(Duration::from_secs(2), progress_receiver.recv())
        .await
        .expect("FFmpeg should emit progress before the pause request")
        .expect("progress channel should remain open while FFmpeg runs");
    pause_handle
        .request(demux::ffmpeg::PauseControlOperation::Pause)
        .unwrap();
    assert!(matches!(
        control_receiver.recv().await,
        Some(PauseControlEvent::Paused {
            phase: RipPhase::Encoding
        })
    ));
    assert!(
        tokio::time::timeout(Duration::from_millis(300), &mut task)
            .await
            .is_err()
    );

    pause_handle
        .request(demux::ffmpeg::PauseControlOperation::Resume)
        .unwrap();
    assert!(matches!(
        control_receiver.recv().await,
        Some(PauseControlEvent::Resumed {
            phase: RipPhase::Encoding
        })
    ));
    let process = task.await.unwrap().unwrap();
    let demux::ffmpeg::ProcessExit::Exited(output_status) = process else {
        panic!("FFmpeg should complete after pause and resume");
    };
    assert!(output_status.status.success());
    assert!(output.is_file());

    let probe = Command::new("ffprobe")
        .args([
            "-v",
            "error",
            "-show_entries",
            "format=duration",
            "-of",
            "default=nw=1:nk=1",
        ])
        .arg(&output)
        .output()
        .await
        .unwrap();
    assert!(probe.status.success());
    assert!(!probe.stdout.is_empty());

    tokio::fs::remove_dir_all(directory).await.unwrap();
    assert!(!cancel_handle.is_cancelled());
}

async fn dependency_available(program: &str) -> bool {
    Command::new(program)
        .arg("-version")
        .output()
        .await
        .is_ok_and(|output| output.status.success())
}

fn temporary_directory() -> PathBuf {
    std::env::temp_dir().join(format!(
        "demux-pause-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
    ))
}
