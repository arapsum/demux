---
name: Demux
description: A focused native workspace for visible, dependable media extraction.
colors:
  workspace: "rgb(0.055, 0.06, 0.075)"
  panel: "rgb(0.085, 0.092, 0.112)"
  inset: "rgb(0.065, 0.07, 0.087)"
  inset-selected: "rgb(0.075, 0.08, 0.10)"
  border: "rgb(0.17, 0.18, 0.22)"
  border-inset: "rgb(0.19, 0.20, 0.24)"
  border-selected: "rgb(0.30, 0.27, 0.55)"
  text: "rgb(0.92, 0.93, 0.96)"
  text-muted: "rgb(0.62, 0.64, 0.70)"
  primary: "rgb(0.43, 0.36, 0.96)"
  primary-tile: "rgb(0.20, 0.17, 0.43)"
  primary-tile-border: "rgb(0.34, 0.29, 0.67)"
  success: "rgb(0.35, 0.78, 0.57)"
  success-surface: "rgb(0.075, 0.12, 0.10)"
  success-border: "rgb(0.18, 0.42, 0.30)"
  warning: "rgb(0.96, 0.68, 0.30)"
  danger: "rgb(0.94, 0.39, 0.42)"
  danger-text: "rgb(0.95, 0.76, 0.77)"
  danger-surface: "rgb(0.16, 0.075, 0.085)"
  danger-border: "rgb(0.38, 0.14, 0.16)"
  shadow: "rgba(0, 0, 0, 0.22)"
typography:
  title:
    fontFamily: "system-ui, sans-serif"
    fontSize: "28px"
    fontWeight: 700
  heading:
    fontFamily: "system-ui, sans-serif"
    fontSize: "18px"
    fontWeight: 600
  body:
    fontFamily: "system-ui, sans-serif"
    fontSize: "14px"
    fontWeight: 400
  label:
    fontFamily: "system-ui, sans-serif"
    fontSize: "12px"
    fontWeight: 400
rounded:
  control: "12px"
  panel: "14px"
spacing:
  xs: "4px"
  sm: "8px"
  md: "14px"
  lg: "18px"
  xl: "26px"
components:
  button-primary:
    backgroundColor: "{colors.primary}"
    textColor: "{colors.text}"
    rounded: "{rounded.control}"
    padding: "13px 16px"
  panel:
    backgroundColor: "{colors.panel}"
    textColor: "{colors.text}"
    rounded: "{rounded.panel}"
    padding: "18px"
  input:
    backgroundColor: "{colors.inset}"
    textColor: "{colors.text}"
    rounded: "{rounded.control}"
    padding: "12px"
---

# Design System: Demux

## Overview

**Creative North Star: "The Quiet Signal Desk"**

Demux is a compact native work surface where media state stays legible while
external tools run. Its character comes from calm dark layering, precise
status language, and one restrained violet action signal—not ornamental
chrome. Familiar desktop controls keep attention on the file, destination,
encoding choices, and outcome.

**Key Characteristics:**

- Dense enough for operational work, with generous separation between tasks.
- Violet is reserved for primary action and active work.
- State is always expressed with words as well as color.
- Advanced controls appear only when the engine supports them honestly.

## Colors

The palette uses near-black neutral layers with a single violet action color
and distinct semantic colors.

- **Workspace** (`rgb(0.055, 0.06, 0.075)`): application background.
- **Panel** (`rgb(0.085, 0.092, 0.112)`): queue and settings surfaces.
- **Inset** (`rgb(0.065, 0.07, 0.087)`): chooser, selected job, and fixed-value fields.
- **Border** (`rgb(0.17, 0.18, 0.22)`): restrained surface separation.
- **Primary** (`rgb(0.43, 0.36, 0.96)`): primary action and active extraction.
- **Success** (`rgb(0.35, 0.78, 0.57)`): ready and completed states.
- **Warning** (`rgb(0.96, 0.68, 0.30)`): probing and dependency checks.
- **Danger** (`rgb(0.94, 0.39, 0.42)`): failures and recovery messages.

**The One Signal Rule.** Violet belongs to the primary action or current work,
not passive decoration.

## Typography

Use the platform UI sans throughout. The hierarchy is compact: 28px bold for
the product title, 17–18px semibold for panel headings, 14–16px for content and
controls, and 12–13px muted text for metadata and guidance.

## Layout

The desktop shell opens at 1180×760 with a minimum of 860×600. A compact
identity header sits over a 70/30 work/settings split. The left side owns file
selection, queue information, metadata, and errors. The right side owns MP3
encoding defaults, destination, selected status, dependency status, and the
primary action anchored near the bottom. Outer padding is 24–26px; major gaps
are 18px; grouped controls use 7–14px.

## Elevation & Depth

Depth is primarily tonal. Major panels add one soft downward ambient shadow;
inset surfaces use a border without a second shadow. Never combine decorative
glow with structural borders.

## Shapes

Controls and inset surfaces use 12px corners. Major panels use 14px corners.
Borders remain one pixel and low contrast. Small status text is not placed in
pill-shaped decoration; its wording and semantic color carry the state.

## Components

### Buttons

Primary buttons use violet, readable light text, 12px corners, and 13px
vertical padding. Disabled buttons retain the same geometry and rely on Iced's
native disabled state. Secondary actions use the neutral widget treatment.

### Cards / Containers

Major work panels use the panel surface, 14px corners, one border, and 18–20px
internal padding. Inset content uses the darker surface and 12px corners.

### Inputs / Fields

Text fields use the inset surface with 12px padding. Local filesystem paths
pair with a labeled native Browse action. Inputs that would have no effect
during or after extraction are disabled.

### Output Settings

The output panel uses compact native selectors for the supported format,
bitrate, sample rate, and channel mode. Bitrate and sample rate share one row;
channels and destination retain full width for scanability. Selectors use the
same inset surface and 12px geometry as path fields, with violet reserved for
hover, focus, or open state. During queue execution, values become quiet
read-only fields so the visible settings match the active job snapshot. The
metadata, artwork, normalization, and folder-preservation policies sit beneath
the encoding selectors as independent, plainly labelled checks. Preserve folder
structure is visibly unavailable until a folder import supplies source-root
provenance. The controls area scrolls at the minimum window height while status
and Start remain reachable.

### Job Row

The queue is one compact data table rather than a stack of nested cards. Its
columns show position, filename, duration, textual status, MP3 output settings,
and source size. Each output value includes the job's effective MP3 bitrate
snapshot instead of the latest default. Rows share quiet horizontal separators
so values remain easy to compare down each column. The active selection
receives a restrained violet-neutral background, and a failed row carries its
actionable process error beneath the filename. Unknown probe values use an em
dash; they never fall back to zero. The footer reports the number of jobs and
total source size without presenting it as an estimated output size.

### Queue Execution

Start Ripping operates on every eligible row in insertion order. The active row
uses Analyzing loudness (1 of 2) for a normalized job and Ripping audio (2 of 2)
for its encoding pass; later eligible rows say Queued. Completed, failed, and
skipped rows remain visible with textual terminal states. Intake, removal, and
destination edits are unavailable only while the runner owns the queue. The
final toast reports completed, failed, and skipped counts, and failed rows
retain their detailed process error for inspection.

The selected-job detail beneath Output Settings carries the effective metadata,
artwork, normalization, and folder policy plus the planned destination path and
a compact title/artist/album summary. It names missing artwork as unavailable
and unsupported formats as skipped, never presenting either state as an
extraction failure.

### Progress Surface

The Progress surface sits beneath the queue and follows the active extraction.
It leads with filename and explicit textual state, followed by elapsed and total
time, a determinate bar when probed duration is known, and a clearly labelled
indeterminate state otherwise. Compact measurements report speed, bitrate,
output size, and estimated remaining time during encoding. Loudness analysis
intentionally hides bitrate and output size because no encoded stream exists
yet. Missing or malformed FFmpeg fields say “Unknown” rather than presenting a
misleading zero, while terminal success
sets known-duration progress to 100 percent even if a final snapshot was
dropped under load.

### Intake Surface

File intake begins with a large dashed drop target containing a restrained
media-file symbol, a direct instruction, and the supported-format summary. A
separate toolbar beneath it groups Add Files, Add Folder, and Remove in that
order. Add Files is the primary violet action, Add Folder is neutral secondary,
and Remove uses a restrained destructive outline only when a removable selection
exists. Lucide glyphs keep these actions and the media marker optically
consistent. The dashed outline turns violet while supported desktop drop events
hover over the window, while status copy communicates selection and discovery
states. Linux builds use X11 or XWayland while Iced's native Wayland backend
lacks the file-hover and file-drop events required by this interaction.

### Toasts

Extraction outcomes appear as compact notices in the upper-right corner without
blocking the workspace. Success notices name the generated MP3 and disappear
after six seconds. Failure notices point back to the persistent error area and
remain for ten seconds. Every notice also offers an explicit Dismiss action.

## Do's and Don'ts

### Do:

- **Do** keep task state visible in plain language.
- **Do** reserve the largest area for the queue and current media information.
- **Do** disable controls while their changes could race an active process.
- **Do** use native dialogs for local file and directory selection.

### Don't:

- **Don't** add pause or log controls before their
  engine behavior exists.
- **Don't** use violet for inactive decoration.
- **Don't** render missing metadata as zero.
- **Don't** introduce nested cards or decorative icon tiles.
