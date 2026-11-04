#!/usr/bin/env bash
set -euo pipefail

# Package self-contained upstream engines beside the native executable. Native
# application code and Mermaid need no Node, Chromium, or separate MarkNoria repo.
root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
bundle=${1:-"${CARGO_TARGET_DIR:-$root/target/gnome-native}/package"}
mkdir -p -- "$bundle"
temporary=$(mktemp -d "${TMPDIR:-/tmp}/studio-native-diagrams.XXXXXX")
trap 'rm -rf -- "$temporary"' EXIT
case $(uname -m) in
  x86_64) architecture=amd64; d2_hash=5669ddc46b99e942cc96078f4a4e36d5e62103348f4c05179ede27802fdd87a9 ;;
  aarch64|arm64) architecture=arm64; d2_hash=ac2c028697199479acb321db1e3d68caee9f2ba492ed73caa3cd13f3829bf913 ;;
  *) printf 'Unsupported D2 architecture\n' >&2; exit 2 ;;
esac
curl --fail --location --silent --show-error --max-time 120 \
  https://github.com/plantuml/plantuml/releases/download/v1.2026.9/plantuml-mit-1.2026.9.jar \
  --output "$temporary/plantuml.jar"
printf '%s  %s\n' 835b2f2eb1d756da3f03bffdbd0baf252b4b858547a741ac8e972958a09c7f91 "$temporary/plantuml.jar" | sha256sum --check --status
curl --fail --location --silent --show-error --max-time 120 \
  "https://github.com/d2lang/d2/releases/download/v0.9.0/d2-v0.9.0-linux-$architecture.tar.gz" \
  --output "$temporary/d2.tar.gz"
printf '%s  %s\n' "$d2_hash" "$temporary/d2.tar.gz" | sha256sum --check --status
tar -xzf "$temporary/d2.tar.gz" -C "$temporary"
install -m 644 "$temporary/plantuml.jar" "$bundle/plantuml.jar"
install -m 755 "$temporary/d2-v0.9.0/bin/d2" "$bundle/d2"
mkdir -p -- "$bundle/licenses/d2" "$bundle/licenses/pele"
install -m 644 "$root/native/gnome/vendor/d2/LICENSE.txt" "$root/native/gnome/vendor/d2/THIRD_PARTY_NOTICES.txt" "$bundle/licenses/d2/"
install -m 644 "$root/native/gnome/vendor/pele/LICENSE" "$bundle/licenses/pele/"
printf 'D2 source: https://github.com/d2lang/d2/tree/v0.9.0\nPlantUML MIT source: https://github.com/plantuml/plantuml/tree/v1.2026.9\n' > "$bundle/licenses/SOURCES.txt"
printf 'Installed checked PlantUML MIT 1.2026.9 and D2 0.9.0 into %s\n' "$bundle"
printf 'Runtime system packages: bubblewrap graphviz default-jre-headless fonts-dejavu-core\n'
