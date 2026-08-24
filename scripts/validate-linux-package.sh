#!/usr/bin/env bash
set -euo pipefail

usage() {
    printf 'Usage: scripts/validate-linux-package.sh PATH_TO_DEB\n'
}

if (($# != 1)) || [[ "$1" == '-h' || "$1" == '--help' ]]; then
    usage >&2
    exit $(( $# == 1 ? 0 : 2 ))
fi

package=$(realpath "$1")
if [[ ! -f "$package" ]]; then
    printf 'error: package does not exist: %s\n' "$package" >&2
    exit 1
fi

for command in appstreamcli desktop-file-validate dpkg-deb realpath grep tr; do
    if ! command -v "$command" >/dev/null 2>&1; then
        printf 'error: required command is not installed: %s\n' "$command" >&2
        exit 1
    fi
done

metadata_dir=$(mktemp -d "${TMPDIR:-/tmp}/demux-package-check.XXXXXX")
extracted_dir=$(mktemp -d "${TMPDIR:-/tmp}/demux-package-root.XXXXXX")
cleanup() {
    rm -rf "$metadata_dir" "$extracted_dir"
}
trap cleanup EXIT

dpkg-deb --info "$package" >/dev/null
dpkg-deb --contents "$package" >"$metadata_dir/contents"
dpkg-deb --extract "$package" "$extracted_dir"

declared_dependencies=$(dpkg-deb --field "$package" Depends)
for expected_dependency in ffmpeg libx11-6; do
    if ! printf '%s\n' "$declared_dependencies" \
        | tr ',' '\n' \
        | grep -Eq "^[[:space:]]*${expected_dependency}([[:space:]]|\\(|$)"; then
        printf 'error: package does not declare runtime dependency %s\n' \
            "$expected_dependency" >&2
        exit 1
    fi
done

desktop_file="$extracted_dir/usr/share/applications/demux.desktop"
metainfo_file="$extracted_dir/usr/share/metainfo/io.github.arapsum.Demux.metainfo.xml"
binary="$extracted_dir/usr/bin/demux"
icon="$extracted_dir/usr/share/icons/hicolor/scalable/apps/demux.svg"

desktop-file-validate "$desktop_file"
appstreamcli validate --no-net "$metainfo_file"

for expected in \
    './usr/bin/demux' \
    './usr/share/applications/demux.desktop' \
    './usr/share/metainfo/io.github.arapsum.Demux.metainfo.xml' \
    './usr/share/icons/hicolor/scalable/apps/demux.svg' \
    './usr/share/doc/demux/README.md' \
    './usr/share/doc/demux/LICENSE' \
    './usr/share/doc/demux/copyright'; do
    if ! grep -Fq "$expected" "$metadata_dir/contents"; then
        printf 'error: package is missing %s\n' "$expected" >&2
        exit 1
    fi
done

if [[ ! -x "$binary" ]]; then
    printf 'error: installed binary is not executable\n' >&2
    exit 1
fi
if [[ ! -s "$icon" ]]; then
    printf 'error: installed icon is empty\n' >&2
    exit 1
fi

printf 'Validated %s\n' "$package"
printf 'Package version: %s\n' "$(dpkg-deb --field "$package" Version)"
