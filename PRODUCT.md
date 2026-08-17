# Product

<!-- impeccable:product-schema 1 -->

## Platform

adaptive

## Stack

Rust desktop application using Iced for the native interface, Tokio for asynchronous process work, and FFmpeg/FFprobe as external media tools.

## Users

People who want to extract audio from video files without composing FFmpeg commands themselves.

## Product Purpose

Demux turns video-to-audio extraction into a visible, understandable workflow: choose media, inspect it, choose an output location, start extraction, and see whether the job completed or needs attention.

## Operating Context

Demux is a cross-platform desktop utility used with local media files and locally installed FFmpeg and FFprobe executables. The current GUI supports a sequential queue and configurable MP3 output.

## Capabilities and Constraints

- Probe media asynchronously and display duration, container, and audio-stream metadata.
- Extract MP3 audio with libmp3lame using validated 128–320 kbps bitrate,
  44.1/48 kHz sample-rate, and mono/stereo settings.
- Keep the interface responsive while probing and extracting.
- Accept multiple files, recursively discover supported media in folders, and
  accept desktop file or folder drops.
- Run eligible jobs sequentially, continue after individual failures, preserve
  existing outputs with numbered filenames, and report queue-wide outcomes.
- Stream live elapsed time, speed, bitrate, output size, percentage, and
  remaining-time estimates from machine-readable FFmpeg progress records.
- Cancel the full queue through cooperative and bounded forced FFmpeg shutdown,
  remove partial output, and reuse that recovery path when closing the app.
- Snapshot effective encoding settings per job and restore valid user defaults
  from the platform configuration directory on restart.
- Copy an allowlisted set of source tags and compatible embedded cover art into
  MP3 outputs when enabled, while treating missing or unsupported artwork as a
  non-fatal condition.
- Normalize audio with a two-pass EBU R128 policy (−23 LUFS target and −1 dBTP
  decoded ceiling) while showing analysis and encoding as separate phases.
- Preserve the contents of selected folder imports under the output directory,
  with traversal-safe relative paths and collision-safe filenames.
- Stream bounded, timestamped FFmpeg diagnostics for the active and completed
  queue jobs, with clear and native Save Log actions.
- Retain at most 2,000 user-facing log lines or 512 KiB, redact parent paths
  while retaining filenames, and keep structured tracing independent.
- Pause remains a later, platform-dependent milestone.

## Brand Commitments

The product name is Demux. The supplied finished-interface reference and wireframe establish a compact desktop utility with a two-column work area, restrained neutral surfaces, clear status presentation, and a purple primary-action accent.

## Evidence on Hand

- Finished dark-interface reference supplied in the conversation.
- Structural wireframe supplied in the conversation.
- Existing domain, FFmpeg, FFprobe, workflow, and tracing implementation in `src/`.

## Product Principles

- Keep process complexity behind plain task-oriented controls.
- Make job state and failure recovery immediately visible.
- Preserve UI responsiveness during all media operations.
- Add advanced controls only when the engine can support them honestly.

## Accessibility & Inclusion

Use readable contrast, explicit text labels, keyboard-focusable native controls, and status language that does not rely on color alone.
