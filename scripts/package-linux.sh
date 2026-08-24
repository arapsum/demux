#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
REPO_ROOT=$(cd -- "${SCRIPT_DIR}/.." && pwd)
DIST_DIR="${REPO_ROOT}/dist"
SKIP_BUILD=false
VERSION=''

usage() {
    cat <<'EOF'
Usage: scripts/package-linux.sh [OPTIONS] [VERSION]

Build a reproducible Demux .deb for Linux x86_64.

Options:
  --skip-build  Package the existing target/release/demux binary.
  -h, --help    Show this help message.
EOF
}

while (($# > 0)); do
    case "$1" in
        --skip-build)
            SKIP_BUILD=true
            ;;
        -h|--help)
            usage
            exit 0
            ;;
        --*)
            printf 'error: unknown option: %s\n' "$1" >&2
            usage >&2
            exit 2
            ;;
        *)
            if [[ -n "$VERSION" ]]; then
                printf 'error: more than one version was provided\n' >&2
                exit 2
            fi
            VERSION=$1
            ;;
    esac
    shift
done

if [[ -z "$VERSION" ]]; then
    VERSION=$(sed -nE 's/^version = "([0-9][^"]*)"/\1/p' "${REPO_ROOT}/Cargo.toml" | head -n 1)
fi

if [[ ! "$VERSION" =~ ^[0-9]+\.[0-9]+\.[0-9]+([.-][0-9A-Za-z.-]+)?$ ]]; then
    printf 'error: invalid package version: %s\n' "$VERSION" >&2
    exit 2
fi

case "$(uname -m)" in
    x86_64) ARCH=amd64 ;;
    *)
        printf 'error: this release only supports x86_64 (found %s)\n' "$(uname -m)" >&2
        exit 1
        ;;
esac

for command in cargo dpkg-deb install sha256sum sed awk find git mktemp touch xargs grep tr; do
    if ! command -v "$command" >/dev/null 2>&1; then
        printf 'error: required command is not installed: %s\n' "$command" >&2
        exit 1
    fi
done

BINARY="${REPO_ROOT}/target/release/demux"
if [[ "$SKIP_BUILD" != true ]]; then
    cargo build --locked --release --manifest-path "${REPO_ROOT}/Cargo.toml"
fi
if [[ ! -x "$BINARY" ]]; then
    printf 'error: release binary is missing or not executable: %s\n' "$BINARY" >&2
    exit 1
fi

for required_file in \
    README.md \
    LICENSE \
    INSTALL.md \
    CHANGELOG.md \
    packaging/linux/demux.desktop \
    packaging/linux/demux.svg \
    packaging/linux/io.github.arapsum.Demux.metainfo.xml \
    packaging/linux/copyright; do
    if [[ ! -f "${REPO_ROOT}/${required_file}" ]]; then
        printf 'error: missing required package file: %s\n' "$required_file" >&2
        exit 1
    fi
done

mkdir -p "$DIST_DIR"
stage=$(mktemp -d "${TMPDIR:-/tmp}/demux-deb.XXXXXX")
cleanup() {
    rm -rf "$stage"
}
trap cleanup EXIT

install -d \
    "$stage/DEBIAN" \
    "$stage/usr/bin" \
    "$stage/usr/share/applications" \
    "$stage/usr/share/icons/hicolor/scalable/apps" \
    "$stage/usr/share/metainfo" \
    "$stage/usr/share/doc/demux"

install -m 0755 "$BINARY" "$stage/usr/bin/demux"
install -m 0644 \
    "$REPO_ROOT/packaging/linux/demux.desktop" \
    "$stage/usr/share/applications/demux.desktop"
install -m 0644 \
    "$REPO_ROOT/packaging/linux/io.github.arapsum.Demux.metainfo.xml" \
    "$stage/usr/share/metainfo/io.github.arapsum.Demux.metainfo.xml"
install -m 0644 \
    "$REPO_ROOT/packaging/linux/demux.svg" \
    "$stage/usr/share/icons/hicolor/scalable/apps/demux.svg"

for document in README.md LICENSE CHANGELOG.md INSTALL.md packaging/linux/copyright; do
    install -m 0644 \
        "$REPO_ROOT/$document" \
        "$stage/usr/share/doc/demux/$(basename "$document")"
done

# dpkg-shlibdeps provides the exact shared-library dependencies visible in the
# ELF binary. Iced and its Linux adapters load several integrations dynamically,
# so these supplemental dependencies must remain explicit as well.
runtime_dependencies='libstdc++6, libfontconfig1, libfreetype6, libvulkan1, libx11-6, libx11-xcb1, libxau6, libxcb1, libxcb-render0, libxcb-shape0, libxcb-xfixes0, libxcb-xkb1, libxdmcp6, libxext6, libxfixes3, libxi6, libxkbcommon0, libxkbcommon-x11-0, libxrandr2, libxcursor1, libxrender1, libdbus-1-3, libwayland-client0, libwayland-cursor0, libwayland-egl1, xdg-desktop-portal, ffmpeg'
fallback_dependencies="libc6, libgcc-s1, ${runtime_dependencies}"
dependencies="$fallback_dependencies"

if command -v dpkg-shlibdeps >/dev/null 2>&1; then
    dependency_workspace=$(mktemp -d "${TMPDIR:-/tmp}/demux-shlibdeps.XXXXXX")
    install -d "$dependency_workspace/debian"
    printf 'Source: demux\nSection: sound\nPriority: optional\nMaintainer: Kibet Bittok <kibetarapsum@gmail.com>\n\nPackage: demux\nArchitecture: any\nDepends: \nDescription: Demux shared-library dependency probe\n Demux shared-library dependency probe.\n' \
        >"$dependency_workspace/debian/control"
    shlib_output=$(
        cd "$dependency_workspace"
        dpkg-shlibdeps -O --package=demux -e "$stage/usr/bin/demux" 2>/dev/null || true
    )
    generated_dependencies=$(printf '%s\n' "$shlib_output" | sed -n 's/^shlibs:Depends=//p')
    rm -rf "$dependency_workspace"
    if [[ -n "$generated_dependencies" ]]; then
        dependencies="$generated_dependencies"
    fi
fi

has_dependency() {
    local dependency=$1
    printf '%s\n' "$dependencies" \
        | tr ',' '\n' \
        | grep -Eq "^[[:space:]]*${dependency}([[:space:]]|\\(|$)"
}

append_dependency() {
    local dependency=$1
    if ! has_dependency "$dependency"; then
        dependencies="${dependencies}, ${dependency}"
    fi
}

while IFS= read -r supplemental_dependency; do
    append_dependency "$supplemental_dependency"
done < <(printf '%s\n' "$runtime_dependencies" | tr ',' '\n' | sed 's/^[[:space:]]*//')

printf 'Package: demux\nVersion: %s\nArchitecture: %s\nSection: sound\nPriority: optional\nMaintainer: Kibet Bittok <kibetarapsum@gmail.com>\nHomepage: https://github.com/arapsum/demux\nDepends: %s\nDescription: extract clean audio from video files\n Demux is a native desktop application for extracting audio from local video files.\n It makes the FFmpeg workflow visible and approachable.\n' \
    "$VERSION" "$ARCH" "$dependencies" >"$stage/DEBIAN/control"

source_date_epoch=$(git -C "$REPO_ROOT" show -s --format=%ct HEAD 2>/dev/null || date +%s)
find "$stage" -type d -exec chmod 0755 {} +
find "$stage" -print0 | xargs -0 touch --date "@${source_date_epoch}"

output="${DIST_DIR}/demux_${VERSION}_${ARCH}.deb"
rm -f "$output"
SOURCE_DATE_EPOCH="$source_date_epoch" dpkg-deb --build --root-owner-group "$stage" "$output" >/dev/null

(cd "$DIST_DIR" && sha256sum "$(basename "$output")" >"$(basename "$output").sha256")
printf 'Created %s\n' "$output"
printf 'Checksum: %s\n' "${output}.sha256"
