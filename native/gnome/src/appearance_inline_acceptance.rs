//! Test actual GTK/Pango code runs, including reused cells and paged paragraphs.
use super::*;
use gtk::gio;

pub(super) struct Probe {
    short: gtk::Label,
    reused: gtk::Label,
    long: gtk::Widget,
    view: gtk::TextView,
    tag: gtk::TextTag,
    block: gtk::Box,
}
fn text_view(widget: &gtk::Widget) -> Option<gtk::TextView> {
    if let Some(view) = widget.downcast_ref::<gtk::TextView>() {
        return Some(view.clone());
    }
    let mut child = widget.first_child();
    while let Some(widget) = child {
        child = widget.next_sibling();
        if let Some(found) = text_view(&widget) {
            return Some(found);
        }
    }
    None
}
fn code_font(label: &gtk::Label, needle: &str) -> Result<gtk::pango::FontDescription, String> {
    let text = label.text();
    let index = text.find(needle).ok_or("Inline code text missing")? as i32;
    let attrs = label
        .layout()
        .attributes()
        .ok_or("Pango attributes missing")?;
    let mut iterator = attrs.iterator();
    loop {
        let (start, end) = iterator.range();
        if start <= index && index < end {
            return Ok(iterator.font().0);
        }
        if !iterator.next_style_change() {
            return Err("Inline code range missing".into());
        }
    }
}
impl Probe {
    pub fn select(&self) {
        let text = self.short.text();
        let offset = text.find("inline 中文😀").unwrap();
        let start = text[..offset].chars().count() as i32;
        self.short
            .select_region(start, start + "inline 中文😀".chars().count() as i32);
    }
    pub async fn new() -> Result<Self, String> {
        let content = gio::spawn_blocking(|| -> Result<studio_markdown_native::RichText, String> {
            let source = format!("text 中文😀 `inline 中文😀` **`bold-code`** *`italic-code`* [linked `link-code`](https://example.invalid/) literal &lt;b&gt; {} `tail-code`", "paged context ".repeat(1400));
            let blocks = studio_markdown_native::parse_text(&source)?;
            match blocks.into_iter().next().map(|b| b.kind) {
                Some(studio_markdown_native::BlockKind::Paragraph(content)) => Ok(content),
                _ => Err("Inline code fixture paragraph missing".into()),
            }
        }).await.map_err(|_| "Inline code fixture worker stopped")??;
        let short = crate::content_widgets::rich("text 中文😀 <tt>inline 中文😀</tt> <b><tt>bold-code</tt></b> <i><tt>italic-code</tt></i> <a href=\"https://example.invalid/\">linked <tt>link-code</tt></a> &lt;tt&gt;literal&lt;/tt&gt;", Rc::new(|_|{}));
        let reused = crate::content_widgets::rich("", Rc::new(|_| {}));
        reused.set_markup("cell 中文😀 <tt>cell-code</tt>");
        let long = crate::content_widgets::formatted_at(
            &content,
            Rc::new(|_| {}),
            Rc::new(Default::default()),
        );
        let view = text_view(&long).ok_or("Long paragraph did not use TextView")?;
        let tag = view
            .buffer()
            .tag_table()
            .lookup("studio-inline-code")
            .ok_or("Inline code tag missing")?;
        let block = gtk::Box::new(gtk::Orientation::Vertical, 0);
        block.set_margin_top(8);
        block.set_margin_bottom(8);
        block.append(&short);
        block.append(&reused);
        block.append(&long);
        watch_markdown_block(&block);
        Ok(Self {
            short,
            reused,
            long,
            view,
            tag,
            block,
        })
    }
    pub fn verify(&self, customized: bool) -> Result<(), String> {
        for (label, needle) in [(&self.short, "inline 中文😀"), (&self.reused, "cell-code")] {
            let font = code_font(label, needle)?;
            if customized {
                if font.family().as_deref() != Some("DejaVu Sans Mono,monospace")
                    || font.size() != 15 * gtk::pango::SCALE
                    || !font.is_size_absolute()
                    || font.weight() != gtk::pango::Weight::Semibold
                {
                    return Err(format!("Label inline code lost saved typography: {font}, family={:?}, size={}, absolute={}, weight={:?}",font.family(),font.size(),font.is_size_absolute(),font.weight()));
                }
            } else if !font
                .family()
                .is_some_and(|f| f.eq_ignore_ascii_case("monospace"))
                || font.size() != 0
            {
                return Err(format!(
                    "Default inline code did not inherit GTK size: {font}, family={:?}, size={}, absolute={}, explicit={:?}, markup={}",font.family(),font.size(),font.is_size_absolute(),label.attributes().map(|a|a.to_string()),label.label()
                ));
            }
        }
        let bold = code_font(&self.short, "bold-code")?;
        if bold.weight() != gtk::pango::Weight::Bold
            || code_font(&self.short, "italic-code")?.style() != gtk::pango::Style::Italic
        {
            return Err(format!(
                "Inline code typography removed nested emphasis: bold={bold}, italic={}, markup={}",
                code_font(&self.short, "italic-code")?,
                self.short.label()
            ));
        }
        let plain = code_font(&self.short, "text 中文")?;
        if plain.family().is_some() || plain.size() != 0 {
            return Err(format!(
                "Inline code typography leaked into normal text: {plain}, family={:?}, size={}",
                plain.family(),
                plain.size()
            ));
        }
        if customized {
            let font = self.tag.font_desc().ok_or("TextView code font missing")?;
            if font.family().as_deref() != Some("DejaVu Sans Mono,monospace")
                || font.size() != 15 * gtk::pango::SCALE
                || !font.is_size_absolute()
                || font.weight() != gtk::pango::Weight::Semibold
            {
                return Err(format!(
                    "Long paragraph lost saved inline typography: {font:?}"
                ));
            }
        } else if self.tag.family().as_deref() != Some("monospace")
            || self.tag.property::<bool>("size-set")
            || self.tag.property::<bool>("weight-set")
        {
            return Err("Long paragraph code font reset failed".into());
        }
        let buffer = self.view.buffer();
        let (start, end) = buffer
            .start_iter()
            .forward_search("inline 中文😀", gtk::TextSearchFlags::TEXT_ONLY, None)
            .ok_or("Long code text missing")?;
        buffer.select_range(&start, &end);
        if crate::content_widgets::selection(&self.long, 1024).as_deref() != Some("inline 中文😀")
        {
            return Err("Code typography changed its selected Unicode quote".into());
        }
        let (bold, _) = buffer
            .start_iter()
            .forward_search("bold-code", gtk::TextSearchFlags::TEXT_ONLY, None)
            .ok_or("Strong code missing")?;
        let strongest = bold
            .tags()
            .into_iter()
            .filter(|t| t.property::<bool>("weight-set"))
            .max_by_key(|t| t.priority())
            .ok_or("Strong code weight missing")?;
        if strongest.weight() != 700 {
            return Err("TextView code font overrode strong emphasis".into());
        }
        self.reused.set_text("plain reused cell");
        if self.reused.attributes().is_some() {
            return Err("Reused plain cell kept code styling".into());
        }
        self.reused.set_markup("cell 中文😀 <tt>cell-code</tt>");
        let font = code_font(&self.reused, "cell-code")?;
        if customized && font.size() != 15 * gtk::pango::SCALE {
            return Err("Rebound code cell lost typography".into());
        }
        Ok(())
    }
    pub fn selection_round_trip(&self, runtime: &Runtime) -> Result<(), String> {
        let binding = runtime
            .inline_labels
            .borrow()
            .iter()
            .filter_map(Weak::upgrade)
            .find(|b| b.widget.upgrade().as_ref() == Some(&self.short))
            .ok_or("Code label binding missing")?;
        let original = runtime
            .plan
            .borrow()
            .clone()
            .ok_or("Code font plan missing")?;
        let mut changed = original.clone();
        changed.code.as_mut().ok_or("Custom code font missing")?.1 += 1.0;
        self.select();
        // Check each synchronous style update. Moving between separate widgets
        // during the full test can independently change GTK primary selection.
        for plan in [
            &changed,
            &Plan {
                code: None,
                ..original.clone()
            },
            &original,
        ] {
            Runtime::inline_label(&binding, Some(plan));
            if crate::content_widgets::selection(self.short.upcast_ref(), 1024).as_deref()
                != Some("inline 中文😀")
            {
                return Err("Inline font change lost its selected Unicode quote".into());
            }
        }
        Ok(())
    }
    pub fn page(&self) -> adw::PreferencesPage {
        let page = adw::PreferencesPage::builder().title("Code sample").build();
        let group = adw::PreferencesGroup::builder()
            .title("Saved inline typography")
            .build();
        group.add(&self.block);
        page.add(&group);
        page
    }
    pub fn geometry(&self) -> Result<Value, String> {
        if !self.short.is_mapped() || self.short.width() == 0 || !self.view.is_mapped() {
            return Err("Markdown mode sample is not mapped".into());
        }
        let context = self.short.pango_context();
        let font = context.font_description().ok_or("Markdown font missing")?;
        let pixels = context
            .load_font(&font)
            .ok_or("Markdown font cannot be resolved")?
            .describe_with_absolute_size()
            .size() as f64
            / gtk::pango::SCALE as f64;
        let line = self
            .short
            .layout()
            .line_readonly(0)
            .ok_or("Markdown line missing")?
            .pixel_extents()
            .1
            .height();
        let buffer = self.view.buffer();
        let first = self.view.iter_location(&buffer.start_iter());
        let mut next = buffer.start_iter();
        if !self.view.forward_display_line(&mut next) {
            return Err("Long paragraph did not wrap to its second line".into());
        }
        let advance = self.view.iter_location(&next).y() - first.y();
        Ok(
            json!({"fontPixels":pixels,"labelLineHeight":line,"textViewLineHeight":first.height(),"textViewLineAdvance":advance,"top":self.block.margin_top(),"bottom":self.block.margin_bottom(),"pageText":buffer.text(&buffer.start_iter(),&buffer.end_iter(),false).to_string()}),
        )
    }
}
