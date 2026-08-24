# Demux release procedure

Demux publishes a portable Linux archive and a Debian package from version
tags. The Debian package targets Linux x86_64 (`amd64`) and declares FFmpeg as
a runtime dependency.

## Prepare a release

1. Update the version in `Cargo.toml`.
2. Add a matching release section to `CHANGELOG.md`.
3. Add the release entry to `packaging/linux/io.github.arapsum.Demux.metainfo.xml`.
4. Run the local checks:

   ```sh
   cargo fmt --all -- --check
   cargo clippy --all-targets --all-features -- -D warnings
   cargo test --all-targets --all-features
   ```

5. Build and validate both Linux artifacts:

   ```sh
   ./scripts/package-tarball.sh
   ./scripts/package-linux.sh
   ./scripts/validate-linux-package.sh dist/demux_<version>_amd64.deb
   ```

   The clean installation check is optional and requires Docker:

   ```sh
   ./scripts/verify-linux-install.sh dist/demux_<version>_amd64.deb
   ```

6. Commit the release preparation and create a version tag:

   ```sh
   git tag -a v<version> -m "Demux <version>"
   git push origin main v<version>
   ```

## Automated release

The release workflow verifies the tag against `Cargo.toml`, runs the Rust and
FFmpeg checks, builds both artifacts, validates Debian metadata and package
contents, verifies checksums, and publishes the archive, package, and checksum
files to the GitHub release.

The package installs the application at `/usr/bin/demux`, its launcher at
`/usr/share/applications/demux.desktop`, and its icon at
`/usr/share/icons/hicolor/scalable/apps/demux.svg`. Removing the package does
not remove Demux preferences or logs from the user's home directory.
