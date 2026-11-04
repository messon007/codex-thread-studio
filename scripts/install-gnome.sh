#!/usr/bin/env bash
set -euo pipefail

script_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
root=$(cd -- "$script_dir/.." && pwd)
if [[ -f "$script_dir/SHA256SUMS" ]]; then
  default_bundle=$script_dir
else
  default_bundle=${CARGO_TARGET_DIR:-$root/target/gnome-native}/package
fi
if [[ $# -gt 2 ]]; then printf 'Usage: %s [BUNDLE_DIRECTORY [PREFIX]]\n' "$0" >&2; exit 2; fi
bundle=$(cd -- "${1:-$default_bundle}" && pwd)
prefix=${2:-"$HOME/.local"}
if [[ ! -f "$bundle/SHA256SUMS" ]]; then
  printf 'Bundle has no SHA256SUMS; rebuild it with the current native packager.\n' >&2; exit 2
fi
(cd -- "$bundle" && sha256sum --check --strict --quiet SHA256SUMS)
for executable in codex-thread-studio-gnome codex-thread-studio-gateway codex-thread-studio-mermaid; do
  if [[ ! -f "$bundle/$executable" || ! -x "$bundle/$executable" ]]; then
    printf 'Missing executable: %s\n' "$executable" >&2; exit 2
  fi
done
mkdir -p -- "$prefix/bin" "$prefix/libexec/codex-thread-studio-gnome" "$prefix/share/applications" "$prefix/share/icons/hicolor/128x128/apps" "$prefix/share/dbus-1/services"
prefix=$(cd -- "$prefix" && pwd)
managed="$prefix/libexec/codex-thread-studio-gnome"
if [[ -L "$managed" ]]; then printf 'Managed application directory is a symlink: %s\n' "$managed" >&2; exit 2; fi
checksum=$(sha256sum < "$bundle/SHA256SUMS")
release_id=${checksum%% *}
version_dir="$managed/$release_id"
staging=
trap 'if [[ -n "$staging" ]]; then rm -rf -- "$staging"; fi' EXIT
if [[ -d "$version_dir" ]]; then
  installed_checksum=$(sha256sum < "$version_dir/SHA256SUMS")
  if [[ ${installed_checksum%% *} != "$release_id" ]]; then
    printf 'Installed manifest differs: %s\n' "$version_dir" >&2; exit 2
  fi
  (cd -- "$version_dir" && sha256sum --check --strict --quiet SHA256SUMS)
else
  staging=$(mktemp -d "$managed/.install.XXXXXX")
  cp -R -- "$bundle/." "$staging/"
  (cd -- "$staging" && sha256sum --check --strict --quiet SHA256SUMS)
  chmod 755 "$staging"
  mv -T -- "$staging" "$version_dir"
  staging=
fi
# Versioned siblings keep a running process paired with its own gateway/engines.
# Only Studio's names enter bin; an existing d2/java/PDF installation is untouched.
for executable in codex-thread-studio-gnome codex-thread-studio-gateway codex-thread-studio-mermaid; do
  if [[ -d "$prefix/bin/$executable" && ! -L "$prefix/bin/$executable" ]]; then
    printf 'Executable destination is a directory: %s\n' "$executable" >&2; exit 2
  fi
  ln -sfn -- "../libexec/codex-thread-studio-gnome/$release_id/$executable" "$prefix/bin/$executable"
done
if [[ -d "$version_dir/licenses" ]]; then
  mkdir -p -- "$prefix/share/licenses/codex-thread-studio-gnome"
  cp -R -- "$version_dir/licenses/." "$prefix/share/licenses/codex-thread-studio-gnome/"
fi
install -m 644 "$version_dir/MERMAID-WORKER.json" "$prefix/share/licenses/codex-thread-studio-gnome/MERMAID-WORKER.json"
icon=io.github.messon007.codexthreadstudio.Gnome
install -m 644 "$version_dir/$icon.png" "$prefix/share/icons/hicolor/128x128/apps/$icon.png"
desktop="$prefix/share/applications/$icon.desktop"
# First quote the argument, then escape the file's string-value layer. Both
# desktop entries and D-Bus service files decode that layer before Exec parsing.
escape_exec_path() {
  escaped_path=$1
  escaped_path=${escaped_path//\\/\\\\}
  escaped_path=${escaped_path//\"/\\\"}
  escaped_path=${escaped_path//\$/\\\$}
  escaped_path=${escaped_path//\`/\\\`}
  escaped_path=${escaped_path//\\/\\\\}
  escaped_path=${escaped_path//$'\n'/\\n}
  escaped_path=${escaped_path//$'\r'/\\r}
  escaped_path=${escaped_path//$'\t'/\\t}
}
escape_exec_path "$prefix/bin/codex-thread-studio-gnome"
# Desktop Exec additionally expands percent field codes; D-Bus Exec does not.
command_path=${escaped_path//%/%%}
while IFS= read -r line; do
  # GLib checks the executable before expanding %% in its path. Keep a stable
  # executable name and pass the installation path as an argument to env.
  if [[ $line == Exec=* ]]; then printf 'Exec=/usr/bin/env -- "%s" %%F\n' "$command_path"; else printf '%s\n' "$line"; fi
done < "$version_dir/$icon.desktop" > "$desktop"
service_path=$escaped_path
while IFS= read -r line; do
  if [[ $line == Exec=* ]]; then printf 'Exec="%s" --gapplication-service\n' "$service_path"; else printf '%s\n' "$line"; fi
done < "$version_dir/$icon.service" > "$prefix/share/dbus-1/services/$icon.service"
if command -v update-desktop-database >/dev/null; then update-desktop-database "$prefix/share/applications"; fi
printf 'Installed native GNOME frontend and gateway into %s/bin\n' "$prefix"
