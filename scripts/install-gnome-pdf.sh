#!/usr/bin/env bash
set -euo pipefail

# Pinned upstream PDFium without V8/XFA. Runtime downloads occur at build time.
root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
bundle=${1:-"${CARGO_TARGET_DIR:-$root/target/gnome-native}/package"}
case $(uname -m) in
  x86_64) architecture=x64; expected=bdc60126399f39fe768566a18d4be055a7b3681bc9e248e8a6656b4f6cae0aa2 ;;
  aarch64|arm64) architecture=arm64; expected=042de247138f96341534cccf24ed21c2550a7245b936bb4a60d2340120000f05 ;;
  *) printf 'Unsupported PDFium architecture\n' >&2; exit 2 ;;
esac
temporary=$(mktemp -d "${TMPDIR:-/tmp}/studio-native-pdf.XXXXXX")
trap 'rm -rf -- "$temporary"' EXIT
curl --fail --location --silent --show-error --max-time 120 \
  "https://github.com/bblanchon/pdfium-binaries/releases/download/chromium/8111/pdfium-linux-$architecture.tgz" \
  --output "$temporary/pdfium.tgz"
printf '%s  %s\n' "$expected" "$temporary/pdfium.tgz" | sha256sum --check --status
tar -xzf "$temporary/pdfium.tgz" -C "$temporary"
mkdir -p -- "$bundle/licenses/pdfium"
install -m 644 "$temporary/lib/libpdfium.so" "$bundle/libpdfium.so"
cp -R -- "$temporary/licenses/." "$bundle/licenses/pdfium/"
printf 'PDFium 158.0.8111.0 (no V8): https://github.com/bblanchon/pdfium-binaries/releases/tag/chromium/8111\n' > "$bundle/licenses/pdfium/SOURCE.txt"
printf 'Installed checked PDFium 158.0.8111.0 into %s\n' "$bundle"
