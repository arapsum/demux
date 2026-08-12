# Demux

Demux is a native Rust desktop utility for extracting audio from video files with [FFmpeg](https://ffmpeg.org/). Its goal is to make a common command-line workflow feel clear and approachable: choose your videos, configure the output, start the queue, and watch the work happen.

> **Status:** early development. The repository currently contains a minimal Rust package scaffold; the Iced interface and FFmpeg pipeline are planned.

## Product direction

Demux is designed as a focused, cross-platform audio-ripping application built with [Iced](https://github.com/iced-rs/iced). The name comes from *demuxing*: separating an audio stream from the other streams inside a multimedia container.

The intended workflow is:

1. Add video files or folders by dropping them into the application or selecting them from a file dialog.
2. Review the queue, including file names, durations, status, output format, and estimated size.
3. Choose an output folder and audio settings.
4. Start the queue and monitor progress, speed, remaining time, and FFmpeg output.
5. Cancel or inspect jobs when something goes wrong.

## Planned features

- Drag-and-drop support for common video formats such as MP4, MKV, MOV, AVI, WMV, FLV, and MPEG.
- A multi-file queue with queued, active, completed, and failed states.
- Output formats including MP3, WAV, FLAC, and AAC/M4A.
- Configurable bitrate, sample rate, channels, and output location.
- Per-job progress with percentage, elapsed time, remaining time, speed, and audio properties.
- Start, pause where supported, and cancel controls.
- A readable FFmpeg log with options to clear or save it.
- Startup checks for both `ffmpeg` and `ffprobe`, with actionable error messages when either dependency is missing.

## MVP scope

The first usable release should keep the core pipeline small and reliable:

```text
video file → FFmpeg → audio file
```

The initial milestone focuses on adding one or more files, selecting an output folder and format, starting extraction, reporting progress and errors, and cancelling a job. Metadata editing, artwork extraction, normalization, presets, parallel ripping, and advanced stream selection can follow once that pipeline is solid. Pause is also treated as a later, platform-dependent refinement rather than a requirement for the first slice.

## Architecture

The FFmpeg engine is intended to remain independent of the UI. Iced should drive the application state and render events from the extraction layer, while the extraction layer handles media probing, command construction, process management, progress parsing, logging, and cancellation.

```text
Iced UI
  ↓ messages and state updates
Application model
  ↓ jobs and settings
FFmpeg service
  ├── ffprobe: inspect media
  ├── ffmpeg: extract audio
  └── progress/log events: update the UI
```

This separation keeps process management out of the view layer and makes the FFmpeg integration testable without launching the desktop application.

## Requirements

For the planned application:

- Rust and Cargo
- FFmpeg available as `ffmpeg` on `PATH`
- FFprobe available as `ffprobe` on `PATH`
- A desktop environment supported by Iced

The current scaffold does not yet depend on Iced or require FFmpeg to build.

## Getting started

From the repository root:

```bash
cargo run
```

The current binary checks for FFmpeg and FFprobe, then prompts for an input
video and output location. Run the test suite with:

```bash
cargo test
```

Useful checks while developing:

```bash
cargo fmt --check
cargo clippy --all-targets --all-features
```

## Roadmap

- [x] Create the initial Rust package and named binary.
- [ ] Add the Iced application shell and desktop layout.
- [ ] Define queue, settings, job, and progress models.
- [ ] Add FFmpeg/FFprobe detection and a safe argument-based command builder.
- [ ] Implement extraction, cancellation, progress parsing, and log streaming.
- [ ] Connect the drop zone, queue, output settings, progress panel, and controls.
- [ ] Add packaging and platform-specific distribution guidance.

## License

Demux is licensed under the [GNU General Public License v3.0 only](LICENSE).
