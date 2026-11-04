#!/usr/bin/env bash
set -euo pipefail

if [[ $# != 1 ]]; then printf 'Usage: %s BUNDLE_DIRECTORY\n' "$0" >&2; exit 2; fi
bundle=$(cd -- "$1" && pwd)
for executable in codex-thread-studio-gnome codex-thread-studio-gateway codex-thread-studio-mermaid; do
  if [[ ! -f "$bundle/$executable" || ! -x "$bundle/$executable" ]]; then
    printf 'Missing executable: %s\n' "$executable" >&2; exit 2
  fi
done
if [[ -n $(find "$bundle" -type l -print -quit) ]]; then
  printf 'Native bundles must contain regular files, not symlinks.\n' >&2; exit 2
fi
manifest=$(mktemp "$(dirname -- "$bundle")/.studio-checksums.XXXXXX")
trap 'rm -f -- "$manifest"' EXIT
(
  cd -- "$bundle"
  find . -type f ! -path './SHA256SUMS' -print0 |
    LC_ALL=C sort -z | xargs -0 sha256sum --binary
) > "$manifest"
chmod 644 "$manifest"
mv -- "$manifest" "$bundle/SHA256SUMS"
