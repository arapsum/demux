# Demux

Demux is a native Rust desktop utility for extracting audio from video files with [FFmpeg](https://ffmpeg.org/). Its goal is to make a common command-line workflow feel clear and approachable: choose your videos, configure the output, start the queue, and watch the work happen.

> **Status:** early development. Demux now has a functional first Iced shell for
> selecting, probing, and extracting one video at a time. Queue execution,
> streaming progress, cancellation, and the complete interface remain planned.

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

The first GUI slice focuses on selecting one file, probing its audio stream,
choosing an output folder, starting extraction, and reporting completion or an
actionable error. Queue execution, progress streaming, and cancellation come
next. Metadata editing, artwork extraction, normalization, presets, parallel
ripping, and advanced stream selection can follow once that pipeline is solid.
Pause is also treated as a later, platform-dependent refinement.

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

The GUI follows a composed-state architecture. `Demux` is the composition root,
while each independent surface owns its state, local messages, initialization,
update logic, and view. The root maps child tasks and translates explicit child
actions when a workflow crosses surface boundaries. Output settings and
notifications use this structure today; the queue/job surface is the next
candidate for extraction.

```text
Demux
  ├── OutputSettings  → folder selection and Start action
  ├── Notifications   → toast lifecycle and overlay
  └── queue/job state → next extraction target
```

## Requirements

For the planned application:

- Rust and Cargo
- FFmpeg available as `ffmpeg` on `PATH`
- FFprobe available as `ffprobe` on `PATH`
- A desktop environment supported by Iced

Iced is a build dependency. FFmpeg and FFprobe are runtime requirements and do
not need to be installed to compile or test Demux.

## Getting started

From the repository root, launch the Demux desktop interface:

```bash
cargo run
```

The first GUI milestone supports selecting one video, asynchronously probing
its audio stream, choosing an output folder, and extracting an MP3 without
blocking the interface. Run the test suite with:

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
- [x] Add the first Iced application shell and desktop layout.
- [ ] Define queue, settings, job, and progress models.
- [x] Add FFmpeg/FFprobe detection and a safe argument-based command builder.
- [ ] Implement extraction, cancellation, progress parsing, and log streaming.
- [ ] Connect the drop zone, queue, output settings, progress panel, and controls.
- [ ] Add packaging and platform-specific distribution guidance.

## License

Demux is licensed under the [GNU General Public License v3.0 only](LICENSE).
