# Demux Roadmap

This roadmap transforms the current single-file Iced shell into the complete
Demux interface represented by the supplied finished design and wireframe.

The implementation follows one rule throughout:

> Implement and verify engine behavior first, then expose that behavior in the
> GUI. Never add a control that suggests a capability the engine cannot yet
> perform.

Each milestone is a vertical slice. At its end, Demux must compile, pass its
tests, and remain usable even though later reference features are still absent.

## Target experience

The finished application should provide:

- Branded Demux header with Settings and About actions.
- Drag-and-drop file and folder intake plus explicit Add Files and Add Folder
  actions.
- A multi-file queue with selection, removal, metadata, output details, size,
  and per-job status.
- Configurable MP3 extraction settings.
- Sequential queue execution with live progress and estimates.
- Cancellation and, where the platform supports it reliably, pause and resume.
- Metadata, artwork, normalization, and folder-structure options.
- A visible, bounded FFmpeg log with clear and save actions.
- Persistent job state and transient completion/failure notifications.

## Architectural direction

`Demux` remains the composition root. Each substantial surface owns its local
state, messages, initialization, update logic, and view, following the same
composition pattern used by Rusty Editor.

```text
Demux
  ├── Intake
  ├── Queue
  ├── OutputSettings
  ├── Progress
  ├── Logs
  ├── Notifications
  └── ApplicationDialogs
```

Child surfaces return explicit actions when work crosses a boundary. The root
coordinates those actions and maps child tasks without allowing one surface to
mutate another surface directly.

## Milestone 0 — Complete the GUI composition foundation

Purpose: make later vertical slices easy to add without allowing the root state
or view to become a second monolith.

### Engine and state

- Extract the current job collection and selected-job behavior into a `Queue`
  surface state.
- Define queue actions for selecting, adding, removing, probing, starting, and
  reporting terminal outcomes.
- Keep FFmpeg and FFprobe adapters outside GUI modules.
- Establish shared presentation models for dependency state, job status, and
  display-ready metadata.

### UI

- Move the chooser and queue list out of the root view.
- Preserve the current layout and single-file behavior during extraction.
- Keep shared colors, spacing, typography, and surface styles centralized.

### Exit criteria

- Root `Demux` primarily wires child messages, tasks, and actions.
- Existing single-file probing, extraction, errors, and toasts behave exactly as
  before.
- Unit tests cover child action translation and stale asynchronous results.

## Milestone 1 — Multi-file intake and asynchronous probing

Purpose: deliver the first major reference capability: building a real queue.

### Engine first

- Support adding multiple file paths in one operation.
- Support recursively discovering compatible media within a selected folder.
- Define supported extensions in one engine-owned policy.
- Deduplicate canonical paths and reject unsupported or unreadable inputs with
  per-item errors.
- Probe jobs asynchronously with a bounded concurrency limit.
- Preserve input order even when probes finish out of order.
- Add queue-safe job identifiers and stale-result protection.

### UI second

- Replace Add File with Add Files.
- Add Add Folder and Remove actions.
- Add native multi-file and folder pickers.
- Add desktop drag-and-drop for files and folders.
- Render one row per job with filename, duration, status, output format, and
  known size.
- Add selected-row treatment and an empty/drop-zone state matching the
  reference.
- Show queue count and per-item probe failures without blocking valid files.

### Exit criteria

- A mixed group of valid, invalid, duplicate, and silent videos produces a
  stable, selectable queue.
- The interface remains responsive while multiple probes run.
- Removal cannot be undone by a late probe result.

## Milestone 2 — Sequential queue execution

Purpose: make Start Ripping operate on the queue instead of one selected job.

**Status: complete.** Ready jobs run in insertion order through one FFmpeg
process at a time. Probe failures are skipped, extraction failures do not stop
later jobs, and a queue summary records completed, failed, and skipped counts.
Existing output files are preserved by choosing the first available numbered
name (`track (2).mp3`, `track (3).mp3`, and so on) and FFmpeg also receives its
no-overwrite flag as a final safety boundary.

### Engine first

- Add a queue runner that selects the next ready job and runs one FFmpeg process
  at a time.
- Define queued, active, completed, failed, cancelled, and skipped outcomes.
- Continue to the next eligible job after an individual failure.
- Prevent output collisions and define an explicit overwrite/naming policy.
- Produce a queue summary when execution ends.

### UI second

- Change Start Ripping to start all eligible queued jobs.
- Show `Ripping (n of total)` for the active item.
- Keep completed and failed rows visible with terminal status.
- Disable intake and incompatible settings only while required by engine
  guarantees.
- Update completion toasts with queue-aware summaries.

### Exit criteria

- A queue of several files runs deterministically from start to finish.
- One failed job does not corrupt or strand the remaining queue.
- Repeated Start presses cannot launch concurrent runners.

## Milestone 3 — Live progress and estimates

Purpose: support the reference progress panel with truthful process data.

**Status: complete.** FFmpeg emits machine-readable progress records that are
parsed into monotonic job snapshots and forwarded through bounded,
non-blocking channels. The composed Progress surface shows elapsed and total
time, percentage, speed, bitrate, output size, and defensive remaining-time
estimates while labelling unavailable measurements as unknown.

### Engine first

- Run FFmpeg with machine-readable progress output.
- Stream and parse elapsed media time, speed, bitrate, size, and terminal
  progress instead of waiting only for process completion.
- Calculate percent complete from probed duration.
- Estimate remaining time defensively when speed or duration is unavailable.
- Treat malformed or missing progress fields as unknown rather than zero.
- Deliver progress events through a bounded channel without blocking FFmpeg.

### UI second

- Add the Progress surface beneath the queue.
- Show active filename, elapsed time, total duration, percentage, speed, audio
  settings, and estimated remaining time.
- Add a determinate progress bar when duration is known and an indeterminate
  state otherwise.
- Keep progress accessible in text and never rely on the purple bar alone.

### Exit criteria

- Long extractions update smoothly without excessive redraws or log spam.
- Progress remains monotonic and reaches a terminal state on success or failure.
- Unknown metrics are labelled honestly.

## Milestone 4 — Cancellation and recovery

Purpose: give users a safe way to stop work before attempting pause/resume.

**Status: complete.** Cancelling stops the full queue, asks FFmpeg to quit
cooperatively, and force-terminates it after a bounded grace period. Demux waits
for process shutdown and partial-output cleanup before restoring controls,
reports cleanup failures persistently, and routes window-close requests through
the same managed shutdown path.

### Engine first

- Retain a controllable child-process handle for the active extraction.
- Implement cooperative cancellation followed by bounded forced termination.
- Define cleanup behavior for partial output files.
- Make cancellation idempotent and race-safe against natural completion.
- Decide whether cancelling the active item stops only that job or the entire
  queue; default to cancelling the queue with an explicit terminal summary.

### UI second

- Add the reference Cancel control with destructive styling.
- Show cancelling state while shutdown is in progress.
- Restore valid controls after the process and partial-file cleanup finish.
- Report cleanup failures in the persistent error area and notifications.

### Exit criteria

- Cancellation never leaves an unmanaged FFmpeg process.
- Partial output policy is covered by integration tests.
- Closing the application during extraction follows the same shutdown path.

## Milestone 5 — Configurable MP3 output

Purpose: make the visible output settings real while retaining MP3 as the first
supported format.

**Status: complete.** Demux exposes a typed MP3 format selector plus validated
128–320 kbps bitrate, 44.1/48 kHz sample-rate, and mono/stereo controls. Each
job keeps an immutable settings snapshot once queue execution starts, every
selectable combination maps to tested FFmpeg arguments, and valid defaults are
restored from the user's platform configuration directory on restart.

### Engine first

- Model MP3 bitrate, sample rate, and channel mode as validated encoding
  options.
- Map every supported option to safe FFmpeg arguments.
- Validate incompatible values before launching a process.
- Store a settings snapshot on each queued job so later edits do not silently
  change active or completed work.
- Persist user defaults across sessions.

### UI second

- Replace the fixed format field with an MP3 selector that clearly reflects the
  currently supported format set.
- Add bitrate/quality, sample-rate, and channel controls using only engine-backed
  values.
- Show the effective settings in each queue row and the progress panel.
- Lock only settings that cannot safely change during an active run.

### Exit criteria

- Command-builder tests cover every selectable combination.
- Restarting Demux restores the last valid defaults.
- The UI never displays a selectable value the encoder cannot honor.

## Milestone 6 — Metadata and artwork

Purpose: implement the first advanced options shown in the reference.

### Engine first

- Define metadata-copy policy for title, artist, album, date, track, and other
  safe audio tags.
- Detect embedded artwork and determine compatible MP3 cover-art handling.
- Add independent metadata and artwork options to the extraction request.
- Handle absent or unsupported metadata/artwork as non-fatal conditions.
- Test files with no tags, unusual Unicode, multiple artwork streams, and
  malformed metadata.

### UI second

- Add Embed metadata and Extract artwork controls.
- Explain unavailable artwork without treating it as an extraction failure.
- Reflect the effective options in job details or logs without crowding queue
  rows.

### Exit criteria

- Tags and compatible artwork survive extraction when enabled.
- Disabling either option produces an output without that content.
- Unsupported artwork does not fail otherwise valid audio extraction.

## Milestone 7 — Normalization and folder structure

Purpose: complete the remaining output-policy controls in the reference.

### Engine first

- Implement EBU R128 normalization using a tested FFmpeg filter policy.
- Decide between one-pass and two-pass normalization based on accuracy and
  performance requirements; document the chosen trade-off.
- Model source roots and relative paths for folder intake.
- Derive safe destination paths when Preserve folder structure is enabled.
- Reject path traversal and resolve filename collisions deterministically.

### UI second

- Add Normalize audio (EBU R128).
- Add Preserve folder structure and enable it only when source hierarchy exists.
- Show normalization as an additional processing phase when it affects timing.
- Preview the resulting output location for the selected job.

### Exit criteria

- Loudness behavior is verified against representative audio fixtures.
- Nested folder imports reproduce safe relative paths in the output directory.
- Single-file inputs remain straightforward.

## Milestone 8 — FFmpeg log surface

Purpose: make diagnostics visible without replacing structured tracing.

### Engine first

- Stream FFmpeg stderr as timestamped, job-associated log events.
- Separate operational tracing from user-facing FFmpeg output.
- Store a bounded in-memory log with an explicit retention limit.
- Redact or avoid sensitive path exposure where appropriate.
- Support exporting the current visible log safely.

### UI second

- Add the bottom FFmpeg Log surface.
- Follow the active job by default while retaining earlier queue messages.
- Add Clear Log and Save Log actions.
- Use restrained semantic emphasis for meaningful FFmpeg lines without building
  a fragile full terminal emulator.
- Keep the log scrollable, selectable where practical, and bounded in height.

### Exit criteria

- Large FFmpeg output cannot grow memory without bound or freeze rendering.
- Saved logs match the retained user-facing events.
- Clearing the UI log does not disable structured tracing.

## Milestone 9 — Pause and resume feasibility

Purpose: add the reference Pause control only where it can be implemented
honestly and predictably.

### Engine first

- Research and prototype process suspension on each supported platform.
- Define supported-platform behavior and failure recovery.
- Verify that suspended FFmpeg processes retain handles, output integrity, and
  cancellation support.
- Prefer an explicit unsupported capability over platform-specific behavior that
  can strand processes.

### UI second

- Add Pause/Resume only on platforms where the engine reports support.
- Reflect pausing, paused, resuming, and failure states.
- Keep Cancel available while paused.
- Omit or disable the control with an explanation on unsupported platforms.

### Exit criteria

- Pause/resume integration tests pass on every platform where it is enabled.
- Application shutdown and cancellation remain reliable from the paused state.

## Milestone 10 — Complete shell and visual parity

Purpose: converge on the supplied finished design after all pictured controls
have real behavior.

### Application behavior

- Add Settings and About surfaces with their own composed state and messages.
- Define keyboard navigation and shortcuts for common actions.
- Add accessible labels, focus order, contrast checks, and text scaling review.
- Persist window and application preferences where useful.
- Add close-with-active-work confirmation and dependency recovery guidance.

### UI

- Replace the temporary letter tile with the final Demux icon and identity.
- Match the reference topology: header, intake, queue, progress, settings, and
  logs.
- Refine spacing, table alignment, dividers, typography, disabled states,
  hover/focus states, and semantic colors against both references.
- Adapt the two-column layout for the minimum supported window size without
  clipping paths, metadata, or controls.
- Add deliberate empty, probing, running, completed, failed, and dependency
  states.

### Exit criteria

- Every visible control is functional, accessible, and backed by tested engine
  behavior.
- The default desktop window closely matches the finished reference while the
  wireframe remains the source of structural truth.
- A complete multi-file workflow can be executed without consulting FFmpeg or a
  terminal.

## Cross-cutting quality gates

Every milestone must satisfy these gates before the next begins:

- `cargo fmt --all -- --check`
- `cargo test`
- `cargo clippy --all-targets --all-features -- -D warnings`
- Tests for new engine policies, child-surface actions, and asynchronous race
  conditions.
- Native startup and manual walkthrough of empty, working, success, failure,
  cancellation, and long-content states relevant to the milestone.
- No placeholder control for a future capability.
- No direct shell-command string construction; FFmpeg arguments remain typed and
  argument-based.
- No unbounded queue, progress, or log channel.

## Suggested commit rhythm

Keep each vertical slice reviewable with focused commits:

1. Engine model and policy.
2. Engine adapter or process behavior.
3. Engine tests and fixtures when substantial enough to stand alone.
4. GUI surface state, local messages, update logic, and mapped actions.
5. GUI view and interaction states.
6. Documentation and visual-system updates when they materially change.

Small milestones may combine adjacent steps, but engine behavior and its UI
exposure should remain distinct in history whenever either side is substantial.
