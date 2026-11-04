Codex Thread Studio — native GNOME edition

Target: Ubuntu 26.04 GNOME, Wayland.

From this extracted directory:
  bash ./install-runtime-deps.sh
  bash ./install.sh
  ~/.local/bin/codex-thread-studio-gnome

The runtime dependency installer supports --print and --simulate. The application
links GTK4, libadwaita, GtkSourceView, VTE and WebKitGTK 6 at startup; WebKit content
processes start only for actual HTML, EPUB or browser content. No Rust compiler,
Node.js, Python, Chrome or separate MarkNoria checkout is needed to install or run
this bundle. Java/Graphviz and Bubblewrap support the packaged diagram engines.

SHA256SUMS detects incomplete or changed bundle files before installation. Obtain
the bundle from a trusted release source; this checksum is not a signature.

The installer defaults to ~/.local. Optional arguments are BUNDLE_DIRECTORY and
PREFIX. Application files and engines live in a versioned libexec directory;
only the three Studio executable links, launcher, icon and notices are installed
outside it. Existing versions are retained, so a running application keeps its
matching engines. Reinstalling the same bundle reuses its checked version.

The existing Tauri Studio and this application share the normal Studio profile.
Close the other Studio/server before opening this one. Markdown reading/editing
is native and paged; large-file edits save explicitly or when crossing pages.
