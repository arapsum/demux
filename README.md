# Demux

Demux is a native Rust desktop utility for extracting audio from video files with [FFmpeg](https://ffmpeg.org/). Its goal is to make a common command-line workflow feel clear and approachable: choose your videos, configure the output, start the queue, and watch the work happen.

> **Status:** early development. Demux now has a functional first Iced shell for
> selecting and probing multiple videos, then extracting every eligible job
> sequentially with configurable MP3 settings, two-pass EBU R128 normalization,
> folder-preserving destinations, live progress, and estimates. Safe queue
> cancellation is implemented; the complete reference interface remains
> planned.

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
- MP3 output with persisted bitrate, sample-rate, and channel defaults plus
  output-location controls; additional formats can follow the validated model.
- Per-job progress with percentage, elapsed time, remaining time, speed, and audio properties.
- Optional two-pass loudness normalization targeting −23 LUFS with a decoded
  −1 dBTP ceiling.
- Optional preservation of paths relative to a selected folder import.
- Start, pause where supported, and cancel controls.
- A readable FFmpeg log with options to clear or save it.
- Startup checks for both `ffmpeg` and `ffprobe`, with actionable error messages when either dependency is missing.

## MVP scope

The first usable release should keep the core pipeline small and reliable:

```text
video file → FFmpeg → audio file
```

The current GUI slice supports multi-file and folder intake, asynchronous media
probing, a shared output folder, and sequential MP3 extraction with per-row
terminal states, live process measurements, and a queue summary. Cancellation
stops the active FFmpeg process and remaining queue, removes partial output, and
also protects application shutdown. MP3 jobs snapshot validated bitrate, sample
rate, channel mode, metadata, artwork, normalization, and destination policies
before execution, while user defaults persist between launches. Allowlisted
source tags and compatible embedded cover art are preserved when their controls
are enabled; absent or unsupported artwork remains non-fatal. Preserved folder
imports retain safe paths relative to their selected root. Presets, parallel
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

Subsystem adapters retain their specific error types, while application runtime
operations promote failures into the unified `demux::Error`. Iced task messages
share those typed errors through `Arc` so messages remain cloneable; conversion
to human-readable text happens only when persistent job or interface state is
updated.

The GUI follows a composed-state architecture. `Demux` is the composition root,
while each independent surface owns its state, local messages, initialization,
update logic, and view. The root maps child tasks and translates explicit child
actions when a workflow crosses surface boundaries. Queue, progress, output
settings, and notifications use this structure today.

```text
Demux
  ├── Queue            → file intake, probing, selection, and job presentation
  ├── Progress         → active extraction measurements and estimates
  ├── OutputSettings  → folder selection and Start action
  └── Notifications   → toast lifecycle and overlay
```

## Requirements

For the planned application:

- Rust and Cargo
- FFmpeg available as `ffmpeg` on `PATH`
- FFprobe available as `ffprobe` on `PATH`
- A desktop environment supported by Iced

On Linux, Demux currently uses X11 or XWayland because Iced 0.14 does not emit
desktop file-drop events on its Wayland backend. Native Wayland support can be
enabled once those events are implemented upstream.

Iced is a build dependency. FFmpeg and FFprobe are runtime requirements and do
not need to be installed to compile or test Demux.

## Getting started

From the repository root, launch the Demux desktop interface:

```bash
cargo run
```

The current GUI supports building and probing a queue, choosing an output
folder, and extracting eligible jobs sequentially without blocking the
interface. The active extraction reports elapsed time, percentage, speed,
bitrate, output size, and remaining-time estimates when those values are known.
Existing output files receive numbered names instead of being overwritten. Run
the test suite with:

```bash
cargo test
```

Useful checks while developing:

```bash
cargo fmt --check
cargo clippy --all-targets --all-features
```

## Roadmap

See [ROADMAP.md](ROADMAP.md) for the engine-first vertical-slice plan that
connects each reference-interface feature to its required backend behavior.

- [x] Create the initial Rust package and named binary.
- [x] Add the first Iced application shell and desktop layout.
- [x] Define queue, settings, and job models.
- [x] Add FFmpeg/FFprobe detection and a safe argument-based command builder.
- [ ] Implement cancellation and log streaming.
- [x] Connect multi-file and folder intake, desktop drops, queue selection, and removal.
- [x] Connect sequential queue execution and queue-aware completion summaries.
- [x] Connect the progress panel.
- [x] Add validated MP3 settings, metadata policies, and compatible artwork.
- [x] Add two-pass EBU R128 normalization and folder-preserving destinations.
- [ ] Add packaging and platform-specific distribution guidance.

## License

Demux is licensed under the [GNU General Public License v3.0 only](LICENSE).
