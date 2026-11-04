#[cfg(target_os = "linux")]
fn main() -> gtk::glib::ExitCode {
    codex_thread_studio_gnome::run()
}

#[cfg(not(target_os = "linux"))]
fn main() {
    eprintln!("The native GNOME frontend requires Linux with GTK4 and libadwaita.");
    std::process::exit(2);
}
