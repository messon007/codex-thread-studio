//! The same native responsibility field is used by Router and session information.
pub fn view() -> gtk::TextView {
    let view = gtk::TextView::builder()
        .wrap_mode(gtk::WrapMode::WordChar)
        .top_margin(8)
        .bottom_margin(8)
        .left_margin(8)
        .right_margin(8)
        .build();
    crate::i18n_ui::accessible_label(&view, "Session responsibility");
    view
}
