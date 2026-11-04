#!/usr/bin/env bash
set -euo pipefail
root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
if [[ $# -ne 1 ]]; then
  printf 'Usage: %s NEW_COMPONENT_DIRECTORY\n' "$0" >&2
  exit 2
fi
output=$1
if [[ -e $output ]]; then
  printf 'Preserve existing component directory; choose a new output: %s\n' "$output" >&2
  exit 2
fi
target=${CARGO_TARGET_DIR:-"$root/target/gnome-native/mermaid"}
python3 "$root/scripts/cargo-gnome-mermaid.py" build --release --locked --target-dir "$target"
metadata=$(mktemp "${TMPDIR:-/tmp}/studio-mermaid-metadata.XXXXXX")
trap 'rm -f -- "$metadata"' EXIT
host=$(rustc -vV | sed -n 's/^host: //p')
python3 "$root/scripts/cargo-gnome-mermaid.py" metadata --locked --offline \
  --format-version 1 --filter-platform "$host" > "$metadata"
python3 "$root/scripts/package-gnome-mermaid.py" --metadata "$metadata" \
  --binary "$target/release/codex-thread-studio-mermaid" --output "$output"
