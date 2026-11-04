//! Private-window acceptance of real math objects, dynamic actions and export.
use super::*;
use std::cell::{Cell, RefCell};
fn view(root: &gtk::Widget) -> Option<gtk::TextView> {
    if let Some(v) = root
        .downcast_ref::<gtk::TextView>()
        .filter(|v| v.is_mapped() && v.widget_name() == "studio-multiline-rich-text")
    {
        return Some(v.clone());
    }
    let mut child = root.first_child();
    while let Some(widget) = child {
        child = widget.next_sibling();
        if let Some(v) = view(&widget) {
            return Some(v);
        }
    }
    None
}
fn picture(root: &gtk::Widget) -> Option<crate::math_picture::MathPicture> {
    if let Some(v) = root.downcast_ref::<crate::math_picture::MathPicture>() {
        return Some(v.clone());
    }
    let mut child = root.first_child();
    while let Some(widget) = child {
        child = widget.next_sibling();
        if let Some(v) = picture(&widget) {
            return Some(v);
        }
    }
    None
}
fn point(
    view: &gtk::TextView,
    window: &adw::ApplicationWindow,
    offset: i32,
) -> Result<gtk::graphene::Point, String> {
    let rect = view.iter_location(&view.buffer().iter_at_offset(offset));
    let (x, y) = view.buffer_to_window_coords(
        gtk::TextWindowType::Widget,
        rect.x() + 1,
        rect.y() + rect.height() / 2,
    );
    view.compute_point(window, &gtk::graphene::Point::new(x as f32, y as f32))
        .ok_or("No native text coordinates".into())
}
pub async fn run(ui: &Rc<Ui>, directory: &std::path::Path) -> Result<Value, String> {
    let (content, original) = gio::spawn_blocking(|| {
        let source =
            "Before **bold 字😀** $\\frac{\r\na}{b}$ after.\nOutside literal \u{fffc} text.";
        let content = studio_markdown_native::parse_text(source)?
            .into_iter()
            .find_map(|b| match b.kind {
                studio_markdown_native::BlockKind::Paragraph(c) => Some(c),
                _ => None,
            })
            .ok_or("No math paragraph")?;
        let original = content.plain.clone();
        Ok::<_, String>((content, original))
    })
    .await
    .map_err(|_| "Paragraph fixture worker stopped")??;
    let reading = Rc::new(crate::content_widgets::ParagraphState::default());
    let paragraph =
        crate::content_widgets::formatted_at(&content, Rc::new(|_| {}), reading.clone());
    let captured = Rc::new(RefCell::new(None));
    let sink_capture = captured.clone();
    crate::selection_menu::bind(
        &paragraph,
        "paragraph-test",
        Rc::new(move |s| {
            sink_capture.replace(s);
        }),
    );
    let body = gtk::Box::new(gtk::Orientation::Vertical, 12);
    body.set_margin_start(24);
    body.set_margin_end(24);
    body.set_margin_top(20);
    body.set_margin_bottom(20);
    body.append(&paragraph);
    let sink = gtk::TextView::builder()
        .editable(true)
        .vexpand(true)
        .height_request(160)
        .wrap_mode(gtk::WrapMode::WordChar)
        .build();
    body.append(&sink);
    let old = ui.window.content().ok_or("No native root")?;
    gtk::prelude::GtkWindowExt::set_focus(&ui.window, None::<&gtk::Widget>);
    ui.window.set_content(Some(&body));
    let scheme = adw::StyleManager::default().color_scheme();
    let font = gtk::CssProvider::new();
    font.load_from_string("textview.studio-paragraph-font-test {font-size:28px;}");
    gtk::style_context_add_provider_for_display(
        &gtk::prelude::WidgetExt::display(&ui.window),
        &font,
        gtk::STYLE_PROVIDER_PRIORITY_USER + 1,
    );
    let result=async {
        until("multiline object",||view(&paragraph).is_some_and(|v|picture(v.upcast_ref()).is_some_and(|p|p.width()>0 && p.height()>0))).await?;
        let first=view(&paragraph).unwrap();let projected=crate::text_flow::quick_projection(&first).unwrap();
        if first.buffer().line_count()!=2 {return Err("Outside paragraph break was removed".into());}
        let quick=projected.quick.as_ref().unwrap();
        let after=original[..original.find("after.").unwrap()].chars().count();
        let after=quick.projected_range(after..after).unwrap().start;
        if first.iter_location(&first.buffer().start_iter()).y()!=first.iter_location(&first.buffer().iter_at_offset(after as i32)).y(){return Err("Formula hard break escaped the object".into());}
        let q=projected.objects[0].alternative.clone();let source=quick.source_range(projected.objects[0].characters.clone()).unwrap();
        reading.focus(0,source.clone());
        until("source search selection",||reading.target_visible(&q,&source)).await?;
        copy_view(directory,&first,&q).await?;
        first.activate_action("paragraph-test.capture",None).map_err(|e|e.to_string())?;
        if captured.borrow().as_deref()!=Some(q.as_str()){return Err("Dynamic paragraph action lost original text".into());}
        let mut current=first;
        for dark in [false,true]{
            let primary=current.primary_clipboard();primary.set_text("foreign paragraph primary");
            let old=current.clone();
            adw::StyleManager::default().set_color_scheme(if dark{adw::ColorScheme::ForceDark}else{adw::ColorScheme::ForceLight});
            until("themed paragraph",||view(&paragraph).is_some_and(|v|v!=old && crate::content_widgets::selection(v.upcast_ref(),8192).as_deref()==Some(q.as_str()))).await?;
            current=view(&paragraph).unwrap();copy_view(directory,&current,&q).await?;
            if primary.read_text_future().await.map_err(|e|e.to_string())?.as_deref()!=Some("foreign paragraph primary"){return Err("Theme restore reclaimed foreign primary selection".into());}
            crate::window_capture::capture(&ui.window,&directory.join(if dark{"rich-paragraph-dark.png"}else{"rich-paragraph-light.png"})).await?;
        }
        // Prime pointer focus on the private sink before selecting the source.
        crate::smoke_input::request(directory,json!({"action":"focus"})).await?;
        let bounds=sink.compute_bounds(&ui.window).ok_or("No private sink bounds")?;
        crate::smoke_input::request(directory,json!({"action":"pointer","x":bounds.x()+20.0,"y":bounds.y()+20.0})).await?;
        glib::timeout_future(Duration::from_millis(60)).await;
        current.grab_focus();let buffer=current.buffer();buffer.select_range(&buffer.start_iter(),&buffer.end_iter());
        let rect=current.iter_location(&buffer.start_iter());let(x,y)=current.buffer_to_window_coords(gtk::TextWindowType::Widget,rect.x()+2,rect.y()+rect.height()/2);
        let start=current.compute_point(&ui.window,&gtk::graphene::Point::new(x as f32,y as f32)).ok_or("No native drag start")?;
        let target=(bounds.x()+20.0,bounds.y()+20.0);
        let began=Rc::new(Cell::new(false));let controllers=current.observe_controllers();
        for i in 0..controllers.n_items(){if let Some(d)=controllers.item(i).and_downcast::<gtk::DragSource>(){let flag=began.clone();d.connect_drag_begin(move |_,_|flag.set(true));}}
        if ui.window.width()!=1600 ||ui.window.height()!=1000{return Err("Drag requires owned fullscreen monitor".into());}
        crate::smoke_input::request(directory,json!({"action":"pointer","x":start.x(),"y":start.y(),"dx":target.0-start.x(),"dy":target.1-start.y(),"hold_ms":160})).await?;
        until("native source-text drop",||sink.buffer().char_count()>0).await?;
        let text=sink.buffer().text(&sink.buffer().start_iter(),&sink.buffer().end_iter(),true);
        if !began.get() ||text.as_str()!=original{return Err(format!("Native drag changed original text: began={} text={text:?}",began.get()));}
        crate::window_capture::capture(&ui.window,&directory.join("rich-paragraph-drop.png")).await?;
        // Reuse real pointer events after DnD: a click clears selection, native
        // double/triple clicks still select words/lines, and a new selection
        // drag must not become an export of the previous selection.
        glib::timeout_future(Duration::from_millis(450)).await;
        current.grab_focus();buffer.select_range(&buffer.start_iter(),&buffer.end_iter());
        let at=point(&current,&ui.window,after as i32+1)?;
        crate::smoke_input::request(directory,json!({"action":"pointer","x":at.x(),"y":at.y()})).await?;
        until("single click clears selection",||buffer.selection_bounds().is_none()).await?;
        glib::timeout_future(Duration::from_millis(450)).await;
        crate::smoke_input::request(directory,json!({"action":"pointer","x":at.x(),"y":at.y(),"clicks":2})).await?;
        until("native double click word",||crate::content_widgets::selection(current.upcast_ref(),8192).as_deref()==Some("after")).await?;
        glib::timeout_future(Duration::from_millis(450)).await;
        crate::smoke_input::request(directory,json!({"action":"pointer","x":at.x(),"y":at.y(),"clicks":3})).await?;
        let line=original.split_once("\nOutside").unwrap().0;
        until("native triple click line",||crate::content_widgets::selection(current.upcast_ref(),8192).is_some_and(|s|s.trim_end_matches('\n')==line)).await?;
        let selected=crate::content_widgets::selection(current.upcast_ref(),8192).unwrap();
        copy_view(directory,&current,&selected).await?;
        glib::timeout_future(Duration::from_millis(450)).await;
        buffer.place_cursor(&buffer.start_iter());
        let from=point(&current,&ui.window,0)?;let to=point(&current,&ui.window,after as i32)?;
        began.set(false);
        crate::smoke_input::request(directory,json!({"action":"pointer","x":from.x(),"y":from.y(),"dx":to.x()-from.x(),"dy":to.y()-from.y()})).await?;
        let prefix=&original[..original.find("after.").unwrap()];
        until("new native selection across formula",||crate::content_widgets::selection(current.upcast_ref(),8192).as_deref()==Some(prefix)).await?;
        if began.get(){return Err("Text selection unexpectedly exported a drag".into());}
        copy_view(directory,&current,prefix).await?;
        let math=picture(current.upcast_ref()).ok_or("No math before font change")?;
        let old_width=math.width();
        current.add_css_class("studio-paragraph-font-test");
        until("math follows actual text font",||math.width()>old_width).await.map_err(|e|format!("{e}; old={old_width} current={} font={:?}",math.width(),current.pango_context().font_description()))?;
        if crate::content_widgets::selection(current.upcast_ref(),8192).as_deref()!=Some(prefix){return Err("Font change changed selection".into());}
        copy_view(directory,&current,prefix).await?;
        current.remove_css_class("studio-paragraph-font-test");
        until("math font restores",||math.width()==old_width).await?;
        // An unmapped paragraph withdraws only its own primary offer; retained
        // callbacks from the discarded view cannot capture or export text.
        current.primary_clipboard().set_text("foreign paragraph after unmap");
        body.remove(&paragraph);
        if crate::content_widgets::selection(current.upcast_ref(),8192).is_some(){return Err("Unmapped paragraph still exports selection".into());}
        captured.replace(None);
        current.activate_action("paragraph-test.capture",None).ok();
        if captured.borrow().is_some(){return Err("Unmapped paragraph captured stale text".into());}
        if current.primary_clipboard().read_text_future().await.map_err(|e|e.to_string())?.as_deref()!=Some("foreign paragraph after unmap"){return Err("Unmap cleared foreign primary".into());}
        body.prepend(&paragraph);
        for _ in 0..3 {
            // Yield just enough for admission, then invalidate that generation
            // while its worker/GTK reply is pending.
            glib::timeout_future(Duration::from_millis(1)).await;
            body.remove(&paragraph);
            body.prepend(&paragraph);
        }
        until("paragraph remaps",||view(&paragraph).is_some_and(|v|v!=current && picture(v.upcast_ref()).is_some())).await?;
        Ok::<_,String>(json!({"outsideBreak":true,"sourceSearch":true,"dynamicCapture":true,"themes":true,"foreignPrimaryPreserved":true,"nativeDragExact":true,"clickSelection":true,"fontSizing":true,"unmapRemap":true,"originalBytes":original.len()}))
    }.await;
    gtk::prelude::GtkWindowExt::set_focus(&ui.window, None::<&gtk::Widget>);
    ui.window.set_content(Some(&old));
    adw::StyleManager::default().set_color_scheme(scheme);
    gtk::style_context_remove_provider_for_display(
        &gtk::prelude::WidgetExt::display(&ui.window),
        &font,
    );
    result
}
