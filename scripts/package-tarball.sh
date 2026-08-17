#!/usr/bin/env bash

# Build a portable Linux release archive for Demux.
set -euo pipefail

readonly project_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
readonly manifest_path="${project_root}/Cargo.toml"
readonly output_directory="${project_root}/dist"
readonly package_version="$(cargo pkgid --manifest-path "${manifest_path}" | sed -E 's/.*#([^@]*@)?//')"
readonly target_triple="$(rustc -vV | awk '/^host: / { print $2 }')"
readonly bundle_name="demux-${package_version}-${target_triple}"
readonly bundle_directory="${output_directory}/${bundle_name}"
readonly archive_path="${output_directory}/${bundle_name}.tar.gz"
readonly checksum_path="${archive_path}.sha256"
readonly binary_path="${project_root}/target/release/demux"
readonly desktop_file="${project_root}/packaging/linux/demux.desktop"
readonly icon_file="${project_root}/packaging/linux/demux.svg"

if [[ -z "${package_version}" || -z "${target_triple}" ]]; then
    echo "Could not determine Demux's version or Rust target triple." >&2
    exit 1
fi

for required_file in README.md LICENSE INSTALL.md packaging/linux/demux.desktop packaging/linux/demux.svg; do
    if [[ ! -f "${project_root}/${required_file}" ]]; then
        echo "Missing required release document: ${required_file}" >&2
        exit 1
    fi
done

if [[ -e "${bundle_directory}" || -e "${archive_path}" || -e "${checksum_path}" ]]; then
    echo "Release artifacts already exist for ${bundle_name}." >&2
    echo "Remove the existing bundle, archive, and checksum before rebuilding it." >&2
    exit 1
fi

cd "${project_root}"
cargo build --release --locked

if [[ ! -x "${binary_path}" ]]; then
    echo "Release binary was not created at ${binary_path}." >&2
    exit 1
fi

mkdir -p "${bundle_directory}"
install -m 755 "${binary_path}" "${bundle_directory}/demux"
install -m 644 README.md "${bundle_directory}/README.md"
install -m 644 INSTALL.md "${bundle_directory}/INSTALL.md"
install -m 644 LICENSE "${bundle_directory}/LICENSE"
install -D -m 644 "${desktop_file}" \
    "${bundle_directory}/share/applications/demux.desktop"
install -D -m 644 "${icon_file}" \
    "${bundle_directory}/share/icons/hicolor/scalable/apps/demux.svg"


tar -C "${output_directory}" -czf "${archive_path}" "${bundle_name}"
(
    cd "${output_directory}"
    sha256sum "${bundle_name}.tar.gz" >"${bundle_name}.tar.gz.sha256"
)

printf 'Created release bundle:\n  %s\n  %s\n' "${archive_path}" "${checksum_path}"
