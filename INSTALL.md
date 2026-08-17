# Installing Demux

## Requirements

Demux requires the following runtime dependencies:

- `ffmpeg`
- `ffprobe`

Both commands must be available on your `PATH`.

Install FFmpeg with your distribution's package manager.

On Debian or Ubuntu:

```bash
sudo apt update
sudo apt install ffmpeg
```

On Fedora:

```bash
sudo dnf install ffmpeg
```

On Arch Linux:

```bash
sudo pacman -S ffmpeg
```

## Run Demux

Extract the release archive, then run the executable:

```bash
tar -xzf demux-0.1.0-x86_64-unknown-linux-gnu.tar.gz
cd demux-0.1.0-x86_64-unknown-linux-gnu
./demux
```

If necessary, make the executable runnable:

```bash
chmod +x demux
```

## Optional desktop integration

Install the executable, launcher, and icon for the current user:

```bash
install -Dm755 demux ~/.local/bin/demux
install -Dm644 share/applications/demux.desktop \
  ~/.local/share/applications/demux.desktop
install -Dm644 share/icons/hicolor/scalable/apps/demux.svg \
  ~/.local/share/icons/hicolor/scalable/apps/demux.svg
```

Ensure `~/.local/bin` is on your `PATH`. Sign out and back in if Demux does
not appear in the application launcher immediately.

## Linux display support

Demux currently runs through X11 or XWayland on Linux. Native Wayland support
is not enabled yet because file-drop events are not currently available through
the selected Iced backend.

## Troubleshooting

Verify that the required tools are available:

```bash
ffmpeg -version
ffprobe -version
```

If either command is unavailable, install FFmpeg using your distribution's
package manager and restart Demux.
