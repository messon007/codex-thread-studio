# Native GNOME desktop check

Development branch: `feat/ubuntu-26-04-gnome-native`. The current optimized
application source is `ab21d4c`; its package manifest SHA-256 is
`38d4bdbafd45dafab62f8735475a3bb53a75740854dc4f3321eb3b79cf32aa0a`.
Later checkpoints reuse the same binaries. Automated evidence is listed in the
[acceptance review](gnome-native-acceptance.md) and
[current inventory](gnome-native-status.md).

## Start on the build machine

Use an Ubuntu 26.04 GNOME Wayland login. Close the other Studio/server instance,
because the native application shares its existing Studio profile and stores.
From the repository root:

```sh
./target/gnome-native-candidate-20261105-ime/codex-thread-studio-gnome
```

The native executable and gateway remain siblings in the qualified package.

## Install the portable bundle

Copy these two files from the build machine's `target/` into an empty working
directory on the Ubuntu 26.04 x86_64 machine:

- `codex-thread-studio-gnome-ubuntu-26.04-x86_64-ab21d4c.tar.gz`
- `codex-thread-studio-gnome-ubuntu-26.04-x86_64-ab21d4c.tar.gz.sha256`

The build-machine `target/` entries are symlinks to the external disk. Use
`cp -L` locally or `scp` to copy the archive bytes and checksum.

From that directory:

```sh
sha256sum -c codex-thread-studio-gnome-ubuntu-26.04-x86_64-ab21d4c.tar.gz.sha256
tar -xzf codex-thread-studio-gnome-ubuntu-26.04-x86_64-ab21d4c.tar.gz
cd release-package
bash ./install-runtime-deps.sh
bash ./install.sh
~/.local/bin/codex-thread-studio-gnome
```

The dependency installer targets Ubuntu 26.04 and uses apt. Its `--print` and
`--simulate` options list or preview the required packages. The normal installer
uses `~/.local`; the extracted README describes an alternate prefix. Installing
and running the bundle does not require Rust, Node, Python, Chrome or a separate
MarkNoria checkout. Configured Codex/OpenCode executables remain provider
prerequisites.

## Physical checks

Use a disposable project/session and a document copy for editing. Record each
check as Pass, Fail or Not run. These checks concern physical GNOME behavior;
the automated virtual-display results do not substitute for them.

| Check | Expected behavior |
| --- | --- |
| Launcher and foreground focus | Start from GNOME search and a launcher click. Minimize and reopen it. The existing window comes forward with working keyboard focus. |
| Notifications | Complete a test turn while another window is active, then click the actual GNOME notification. Studio comes forward at its source session; repeated activation does not create duplicate windows. |
| Keyboard and IME | Type Latin text, Chinese candidates and emoji using the normal keyboard/input engine. Candidate selection and cancellation preserve committed text. Enter/Ctrl+Enter during composition must not send a partial draft. |
| Pointer, layout and selection | Drag the split divider, scroll a large Markdown report, select/copy text and use table paging/column controls. Menus remain reachable and the composer keeps its draft. At narrow widths the document can occupy the pane; Close document restores the composer. |
| Real file dialogs | Open and cancel the GNOME file chooser; save an edit to the document copy. Cancel leaves the current view usable and no pending operation blocks closing it. |
| Physical monitors and appearance | Move/maximize the window on the monitors and scales normally used. Check dark/light appearance, larger text, CJK/emoji, menus and dialogs for clipping or lost focus. |
| Orca and speech | With the normal screen-reader setup, navigate the composer, menus and document controls by keyboard. Check announcements and actual speech clarity. |
| Resume and monitor changes | After a normal suspend/resume or monitor change, confirm that the window redraws, the draft/source position survives, and the connection can recover. |
| Configured providers | In the disposable session, send two ordinary messages with the configured provider. Each finishes and permits the next message without manual Stop. |

## Result record

Record Ubuntu and GNOME versions, `$XDG_SESSION_TYPE`, monitor scales, input
engine and the package manifest above. For a failure, include the action,
expected/actual behavior and the relevant terminal lines. For a large-document
failure, include its size and whether Preview, Source or Edit was active.
Keep Not run entries explicit. Passing this checklist is separate from publishing
the branch and running remote platform CI.
