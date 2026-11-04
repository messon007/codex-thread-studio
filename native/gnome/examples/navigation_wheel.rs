//! Exercise the production navigation coordinator with real private Wayland input.
//! Stdin controls the fixture; wheel events must come from the compositor.
#[cfg(target_os = "linux")]
#[path = "../src/list_navigation.rs"]
mod list_navigation;

#[cfg(target_os = "linux")]
fn main() -> gtk::glib::ExitCode {
    use gtk::{glib, prelude::*};
    use std::{
        cell::{Cell, RefCell},
        collections::HashMap,
        io::{self, BufRead, Write},
        rc::Rc,
    };
    fn report(value: serde_json::Value) {
        let mut output = io::stdout().lock();
        writeln!(output, "{value}").unwrap();
        output.flush().unwrap();
    }
    let app = gtk::Application::builder()
        .application_id("io.github.messon007.NavigationWheelFixture")
        .build();
    app.connect_activate(|app| {
        let rows = gtk::StringList::new(&[]);
        for i in 0..200 { rows.append(&format!("Row {i:03}")); }
        let labels: Rc<RefCell<HashMap<u32, glib::WeakRef<gtk::Label>>>> = Rc::default();
        let factory = gtk::SignalListItemFactory::new();
        factory.connect_setup(|_, object| {
            let item = object.downcast_ref::<gtk::ListItem>().unwrap();
            let label = gtk::Label::builder().height_request(48).xalign(0.0).build();
            item.set_child(Some(&label));
        });
        let bound = labels.clone();
        factory.connect_bind(move |_, object| {
            let item = object.downcast_ref::<gtk::ListItem>().unwrap();
            let label = item.child().unwrap().downcast::<gtk::Label>().unwrap();
            label.set_label(&format!("Row {:03}", item.position()));
            bound.borrow_mut().insert(item.position(), label.downgrade());
        });
        let view = gtk::ListView::new(Some(gtk::NoSelection::new(Some(rows))), Some(factory));
        let scroll = gtk::ScrolledWindow::builder().child(&view).build();
        let window = gtk::ApplicationWindow::builder().application(app).title("Native navigation wheel fixture").default_width(960).default_height(640).child(&scroll).build();
        let navigation = list_navigation::Navigation::new(&scroll);
        let count = Rc::new(Cell::new(0u32));
        let armed = Rc::new(Cell::new(false));
        let case = Rc::new(Cell::new(0u32));
        let (sender, receiver) = std::sync::mpsc::sync_channel::<String>(8);
        std::thread::spawn(move || {
            for line in io::stdin().lock().lines() {
                let Ok(line) = line else { break };
                if sender.send(line).is_err() { break }
            }
        });
        let weak_window = window.downgrade();
        glib::timeout_add_local(std::time::Duration::from_millis(2), move || {
            let Some(window) = weak_window.upgrade() else { return glib::ControlFlow::Break };
            for _ in 0..4 {
                let Ok(command) = receiver.try_recv() else { break };
                match command.as_str() {
                    "start" => {
                        count.set(0); armed.set(false); case.set(case.get()+1);
                        let identity = case.get();
                        let (count, armed, labels) = (count.clone(), armed.clone(), labels.clone());
                        let adjustment = scroll.vadjustment();
                        navigation.jump_start(&view, &scroll, 100, move || {
                            count.set(count.get()+1);
                            let anchor = labels.borrow().get(&100).and_then(|label| label.upgrade()).filter(|label| label.text()=="Row 100" && label.is_mapped());
                            report(serde_json::json!({"event":"probe","case":identity,"calls":count.get(),"offset":adjustment.value(),"mapped":anchor.is_some()}));
                            if armed.get() { anchor.map(|label| label.upcast()) } else { None }
                        });
                        report(serde_json::json!({"event":"started","case":identity}));
                    }
                    "arm" => {
                        // Make an actual correction observable in the positive control.
                        let adjustment=scroll.vadjustment();
                        adjustment.set_value(adjustment.value()+120.0);
                        armed.set(true);
                    }
                    "state" => {
                        let anchor_y = labels.borrow().get(&100).and_then(|label| label.upgrade()).filter(|label| label.text()=="Row 100").and_then(|label| label.compute_bounds(&scroll)).map(|bounds| bounds.y());
                        report(serde_json::json!({"event":"state","case":case.get(),"calls":count.get(),"offset":scroll.vadjustment().value(),"anchorY":anchor_y,"mapped":view.is_mapped()}));
                    }
                    "quit" => { window.close(); return glib::ControlFlow::Break; }
                    _ => panic!("Unknown fixture command"),
                }
            }
            glib::ControlFlow::Continue
        });
        window.present();
        window.maximize();
    });
    app.run_with_args::<&str>(&[])
}

#[cfg(not(target_os = "linux"))]
fn main() {
    panic!("This fixture requires GTK4 and Wayland on Linux");
}
