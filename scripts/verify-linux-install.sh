#!/usr/bin/env bash
set -euo pipefail

if (($# != 1)) || [[ "$1" == '-h' || "$1" == '--help' ]]; then
    printf 'Usage: scripts/verify-linux-install.sh PATH_TO_DEB\n' >&2
    exit $(( $# == 1 ? 0 : 2 ))
fi

package=$(realpath "$1")
if [[ ! -f "$package" ]]; then
    printf 'error: package does not exist: %s\n' "$package" >&2
    exit 1
fi
if ! command -v docker >/dev/null 2>&1; then
    printf 'error: Docker is required for clean Ubuntu installation checks\n' >&2
    exit 1
fi

docker run --rm --pull=missing \
    --volume "$package:/tmp/demux.deb:ro" \
    ubuntu:22.04 \
    bash -ceu '
        export DEBIAN_FRONTEND=noninteractive
        apt-get update
        apt-get install --yes --no-install-recommends /tmp/demux.deb

        # Build a lower-version copy so apt exercises a real upgrade path.
        dpkg-deb --extract /tmp/demux.deb /tmp/demux-old
        dpkg-deb --control /tmp/demux.deb /tmp/demux-old/DEBIAN
        sed -i "s/^Version: .*/Version: 0.0.0/" /tmp/demux-old/DEBIAN/control
        dpkg-deb --build --root-owner-group /tmp/demux-old /tmp/demux-old.deb >/dev/null
        apt-get install --yes --allow-downgrades /tmp/demux-old.deb
        apt-get install --yes /tmp/demux.deb

        dpkg-query --showformat="\${Status}\n" --show demux \
            | grep -Fq "install ok installed"
        test -x /usr/bin/demux
        test -f /usr/share/applications/demux.desktop
        test -f /usr/share/metainfo/io.github.arapsum.Demux.metainfo.xml

        mkdir -p /root/.config/demux /root/.local/state/demux/logs
        printf "retained across package removal\n" > /root/.config/demux/settings.json
        printf "retained across package removal\n" > /root/.local/state/demux/logs/install-check.log
        apt-get remove --yes demux

        test ! -e /usr/bin/demux
        test -f /root/.config/demux/settings.json
        test -f /root/.local/state/demux/logs/install-check.log
        printf "Clean install, package contents, removal, and data-retention checks passed.\n"
    '
