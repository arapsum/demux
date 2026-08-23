# Changelog

All notable changes to Demux are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.2] - 2026-08-23

### Changed

- Set the application icon's intrinsic SVG dimensions to 64 × 64 while
  preserving its original square view box and artwork.

## [0.1.1] - 2026-08-23

### Fixed

- Restored loudness normalization on FFmpeg 4.4 by using its supported maximum
  loudness-range ceiling of 20 LU.
- Kept normalization measurements and command-level regression coverage aligned
  across FFmpeg 4.4 and current FFmpeg releases.

## [0.1.0] - 2026-08-18

### Added

- Added a complete Iced desktop interface composed from independent queue,
  output settings, progress, log, notification, settings, and about surfaces.
- Added native multi-file and folder pickers, recursive folder discovery,
  drag-and-drop intake, supported-media filtering, and duplicate detection.
- Added asynchronous FFprobe inspection with stable queue ordering, bounded
  concurrency, metadata display, and stale-result protection.
- Added deterministic sequential queue extraction with collision-safe output
  naming and per-job completed, failed, cancelled, and skipped states.
- Added live FFmpeg progress reporting for elapsed time, percentage, speed,
  bitrate, output size, and estimated remaining time.
- Added managed cancellation with cooperative shutdown, forced termination after
  a grace period, and partial-output cleanup.
- Added pause and resume support on capable Unix platforms.
- Added validated MP3 controls for bitrate, sample rate, and mono or stereo
  output, with immutable per-job option snapshots.
- Added optional source metadata, compatible cover artwork, two-pass EBU R128
  normalization, and preserved folder structures.
- Added a bounded in-application FFmpeg log with clear and save actions.
- Added completion and failure toasts that remain reliable after asynchronous
  queue work.
- Added persistent output preferences and optional window geometry restoration.
- Added Settings and About dialogs, dependency diagnostics, keyboard shortcuts,
  branded assets, and desktop integration files.
- Added structured tracing with rotating production log files and a configurable
  `DEMUX_LOG_DIR` override.
- Added portable Linux archive packaging, SHA-256 checksums, installation
  documentation, and tag-driven GitHub Release automation.

### Changed

- Separated workflow policy, FFmpeg and FFprobe adapters, domain models, and GUI
  surfaces to keep responsibilities explicit and independently testable.
- Replaced stringly typed workflow failures with typed application, FFmpeg, and
  FFprobe errors.
- Routed probe and extraction diagnostics through tracing instead of standard
  output.
- Disabled Iced's native Wayland backend on Linux until desktop file-drop events
  are supported, while retaining X11 and XWayland operation.

### Fixed

- Ensured toast notifications render above application content after successful
  asynchronous extraction.
- Restored Linux desktop file drops through the supported Iced backend.
- Prevented late probe results, repeated start requests, and duplicate terminal
  events from corrupting queue state.
- Prevented normalization, pause, cancellation, and output-collision workflows
  from leaving unmanaged processes or partial state.
- Corrected modal backdrop event routing and kept Settings and About dialogs
  centred over the application.
- Removed an unused Wayland system dependency from CI and made generated archive
  checksums portable.

[Unreleased]: https://github.com/arapsum/demux/compare/v0.1.2...HEAD
[0.1.2]: https://github.com/arapsum/demux/compare/v0.1.1...v0.1.2
[0.1.1]: https://github.com/arapsum/demux/compare/v0.1.0...v0.1.1
[0.1.0]: https://github.com/arapsum/demux/releases/tag/v0.1.0
