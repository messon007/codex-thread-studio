#!/usr/bin/env bash
set -euo pipefail

# Assemble an already-built pair, including alternate shared Cargo target dirs.
root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
if [[ $# -lt 3 || $# -gt 4 ]]; then
  printf 'Usage: %s NATIVE_BINARY GATEWAY_BINARY OUTPUT_DIRECTORY [MERMAID_COMPONENT]\n' "$0" >&2
  exit 2
fi
native_binary=$1
gateway_binary=$2
bundle=$3
component=${4:-${STUDIO_GNOME_MERMAID_COMPONENT:-}}
if [[ -z $component ]]; then
  printf 'Provide the bundled Mermaid component as argument 4 or STUDIO_GNOME_MERMAID_COMPONENT.\n' >&2
  exit 2
fi
python3 "$root/scripts/verify-gnome-mermaid.py" "$component"
for executable in "$native_binary" "$gateway_binary"; do
  if [[ ! -f $executable || ! -x $executable ]]; then
    printf 'Missing executable: %s\n' "$executable" >&2
    exit 2
  fi
done
mkdir -p -- "$bundle"
python3 "$root/scripts/verify-gnome-mermaid.py" "$component" --install-to "$bundle"
install -m 755 "$native_binary" "$bundle/codex-thread-studio-gnome"
install -m 755 "$gateway_binary" "$bundle/codex-thread-studio-gateway"
install -m 644 "$root/src-tauri/icons/128x128.png" "$bundle/io.github.messon007.codexthreadstudio.Gnome.png"
install -m 644 "$root/native/gnome/data/io.github.messon007.codexthreadstudio.Gnome.desktop" "$bundle/"
install -m 644 "$root/native/gnome/data/io.github.messon007.codexthreadstudio.Gnome.service" "$bundle/"
for component in epub-cfi cssparser html5ever pele ssip-client sse-stream data-url memchr unicase syntect two-face onig onig-sys tls structured html-preview; do
  directory=$component
  if [[ $component == epub-cfi ]]; then directory=epub; fi
  mkdir -p -- "$bundle/licenses/$component"
  for notice in LICENSE UPSTREAM.md; do
    install -m 644 "$root/native/gnome/vendor/$directory/$notice" "$bundle/licenses/$component/$notice"
  done
done
install -m 644 "$root/native/gnome/vendor/two-face/ACKNOWLEDGEMENTS.md" "$bundle/licenses/two-face/ACKNOWLEDGEMENTS.md"
mkdir -p -- "$bundle/licenses/onig-sys/oniguruma"
install -m 644 "$root/native/gnome/vendor/onig-sys/oniguruma/COPYING" "$bundle/licenses/onig-sys/oniguruma/COPYING"
mkdir -p -- "$bundle/licenses/i18n"
install -m 644 "$root/LICENSE" "$bundle/licenses/i18n/LICENSE"
install -m 644 "$root/native/gnome/vendor/i18n/UPSTREAM.md" "$bundle/licenses/i18n/UPSTREAM.md"
mkdir -p -- "$bundle/licenses/ratex"
for notice in LICENSE OFL.txt FONT_NOTICE.txt UPSTREAM.md; do
  install -m 644 "$root/native/gnome/vendor/ratex/$notice" "$bundle/licenses/ratex/$notice"
done
if [[ ${STUDIO_GNOME_SKIP_DIAGRAM_DOWNLOAD:-0} != 1 ]]; then
  bash "$root/scripts/install-gnome-diagrams.sh" "$bundle"
fi
if [[ ${STUDIO_GNOME_SKIP_PDF_DOWNLOAD:-0} != 1 ]]; then
  bash "$root/scripts/install-gnome-pdf.sh" "$bundle"
fi
install -m 755 "$root/scripts/install-gnome.sh" "$bundle/install.sh"
install -m 755 "$root/scripts/install-gnome-runtime-deps.sh" "$bundle/install-runtime-deps.sh"
install -m 644 "$root/native/gnome/data/PACKAGE-README.txt" "$bundle/README.txt"
bash "$root/scripts/package-gnome-checksums.sh" "$bundle"
printf '\nNative GNOME package: %s/codex-thread-studio-gnome\n' "$bundle"
