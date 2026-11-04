#!/usr/bin/env bash
set -euo pipefail

root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
profile=release
flags=(--release)
if [[ ${1:-} == --debug ]]; then
  profile=debug
  flags=()
elif [[ $# -gt 0 ]]; then
  printf 'Usage: %s [--debug]\n' "$0" >&2
  exit 2
fi
output=${CARGO_TARGET_DIR:-"$root/target/gnome-native"}
mkdir -p -- "$output"
output=$(cd -- "$output" && pwd)

# The worker has an independent Rust >=1.95 graph; the frontend remains 1.92.
# A separately built, verified component supports building the parent with 1.92.
component=${STUDIO_GNOME_MERMAID_COMPONENT:-}
if [[ -z $component ]]; then
  staging=$(mktemp -d "$output/mermaid-component.XXXXXX")
  trap 'rm -rf -- "$staging"' EXIT
  component=$staging/component
  CARGO_TARGET_DIR="$output/mermaid" bash "$root/scripts/build-gnome-mermaid.sh" "$component"
fi
python3 "$root/scripts/verify-gnome-mermaid.py" "$component"

cargo build --manifest-path "$root/Cargo.toml" -p codex-thread-studio \
  --no-default-features --locked "${flags[@]}" --target-dir "$output/gateway"
cargo build --manifest-path "$root/native/gnome/Cargo.toml" \
  --locked "${flags[@]}" --target-dir "$output/frontend"

bash "$root/scripts/package-gnome.sh" \
  "$output/frontend/$profile/codex-thread-studio-gnome" \
  "$output/gateway/$profile/codex-thread-studio" "$output/package" "$component"
