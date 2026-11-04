//! GTK accounting probe; normal queue/engine behavior has separate coverage.
use super::*;
impl Renderer {
    pub fn smoke_preview_budget(&self) -> Result<(), String> {
        let owner = Self::new_bounded(self.sender.clone(), 998, 5);
        let first_root = owner.widget("mermaid", "flowchart LR\nA-->B", Styles::new());
        let second_root = owner.widget("mermaid", "flowchart LR\nC-->D", Styles::new());
        let get = |root: &gtk::Box| {
            owner
                .widgets
                .borrow()
                .values()
                .filter_map(Weak::upgrade)
                .find(|w| w.root.upgrade().as_ref() == Some(root))
                .unwrap()
        };
        let first = get(&first_root);
        let second = get(&second_root);
        let reply = |widget: &Diagram, serial: u64| {
            widget.serial.set(serial);
            owner.active.borrow_mut().insert(widget.id);
            owner.event(Response {
                key: 998,
                generation: owner.generation.get(),
                serial,
                kind: "diagram",
                result: Ok(Reply::Diagram {
                    id: widget.id,
                    image: crate::diagram::Image {
                        width: 1,
                        height: 1,
                        pixels: std::sync::Arc::from([200u8, 80, 60, 255]),
                        math: None,
                        vector: None,
                    },
                }),
            });
        };
        reply(&first, 1);
        reply(&second, 2);
        if first.bytes.get() != 4
            || second.bytes.get() != 0
            || second.picture.paintable().is_some()
            || !second.retry.is_visible()
        {
            return Err("Diagram preview budget admitted excessive pixels".into());
        }
        first.clear();
        reply(&second, 3);
        if second.bytes.get() != 4 || second.picture.paintable().is_none() {
            return Err("Released preview budget could not be reused".into());
        }
        owner.reset();
        if first.picture.paintable().is_some()
            || second.picture.paintable().is_some()
            || second.bytes.get() != 0
        {
            return Err("Reset retained native preview bytes".into());
        }
        let tall = Self::new_bounded(self.sender.clone(), 997, 4096);
        let root = tall.widget("mermaid", "flowchart LR\nA-->B", Styles::new());
        let item = tall
            .widgets
            .borrow()
            .values()
            .find_map(Weak::upgrade)
            .unwrap();
        item.serial.set(44);
        tall.event(Response {
            key: 997,
            generation: tall.generation.get(),
            serial: 44,
            kind: "diagram",
            result: Ok(Reply::Diagram {
                id: item.id,
                image: crate::diagram::Image {
                    width: 1,
                    height: 512,
                    pixels: std::sync::Arc::from(vec![0u8; 2048]),
                    math: None,
                    vector: None,
                },
            }),
        });
        let (minimum, natural, _, _) = item.picture.measure(gtk::Orientation::Vertical, 640);
        if item.picture.paintable().is_none() || minimum > 720 || natural > 720 {
            return Err("Tall diagram exceeded its native layout bound".into());
        }
        tall.reset();
        drop(root);
        Ok(())
    }
}
