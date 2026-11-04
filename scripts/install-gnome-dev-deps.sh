#!/usr/bin/env bash
set -euo pipefail

# Build tooling plus the same runtime set shipped with the native bundle.
root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
mapfile -t runtime_packages < <(bash "$root/scripts/install-gnome-runtime-deps.sh" --print)
sudo apt-get update
sudo apt-get install -y build-essential pkg-config curl python3 patch libssl-dev libgtk-4-dev libadwaita-1-dev libgtksourceview-5-dev libvte-2.91-gtk4-dev libwebkitgtk-6.0-dev "${runtime_packages[@]}"
