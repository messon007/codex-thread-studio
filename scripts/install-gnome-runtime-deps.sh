#!/usr/bin/env bash
set -euo pipefail

# System libraries are dynamically linked; document engines remain lazy.
packages=(
  libgtk-4-1 libadwaita-1-0 libgtksourceview-5-0 libvte-2.91-gtk4-0
  libwebkitgtk-6.0-4 libssl3t64 ca-certificates
  adwaita-icon-theme fontconfig fonts-dejavu-core fonts-noto-cjk
  shared-mime-info desktop-file-utils bubblewrap graphviz default-jre-headless
  speech-dispatcher speech-dispatcher-espeak-ng
)

if [[ $# -gt 1 ]]; then printf 'Expected at most one argument.\n' >&2; exit 2; fi
case ${1:-} in
  --print) printf '%s\n' "${packages[@]}"; exit 0 ;;
  --help|-h)
    printf 'Usage: %s [--print|--simulate]\nInstall native Studio runtime packages on Ubuntu 26.04.\n' "$0"
    exit 0 ;;
  ''|--simulate) ;;
  *) printf 'Unknown argument: %s\n' "$1" >&2; exit 2 ;;
esac
. /etc/os-release
if [[ ${ID:-} != ubuntu || ${VERSION_ID:-} != 26.04 ]]; then
  printf 'This runtime package list targets Ubuntu 26.04.\n' >&2
  exit 2
fi
if [[ ${1:-} == --simulate ]]; then
  exec apt-get --simulate install --no-install-recommends "${packages[@]}"
fi
if (( EUID == 0 )); then elevate=(); else elevate=(sudo); fi
"${elevate[@]}" apt-get update
"${elevate[@]}" apt-get install -y --no-install-recommends "${packages[@]}"
