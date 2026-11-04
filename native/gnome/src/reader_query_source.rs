//! Query entry for the existing structured Source/Preview/Edit workspace.
//! Uses the document actor's ordinary read-only open/index/close operations.
use crate::{
    document::{self, Background, Reply, Request},
    reader_query::Context,
    reader_query_ui::Jq,
};
use adw::prelude::*;
use gtk::glib;
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};
use tokio::sync::{mpsc, oneshot};

// 1..=3 belong to visible readers, 4 to conversation diagrams, 101..=103
// to continuous reader diagrams and 9901 to inline math.
const KEY: u64 = 5;

#[derive(Clone, PartialEq, Eq)]
pub struct Target {
    pub root: String,
    pub path: String,
    pub hash: String,
}

pub struct Launcher {
    button: gtk::Button,
    parent: adw::ApplicationWindow,
    toast: adw::ToastOverlay,
    sender: mpsc::Sender<document::Message>,
    target: RefCell<Option<Target>>,
    generation: Cell<u64>,
    busy: Cell<bool>,
    enabled: Cell<bool>,
    query: RefCell<Option<Rc<Jq>>>,
}

async fn control(
    sender: &mpsc::Sender<document::Message>,
    generation: u64,
    operation: Background,
) -> Result<Reply, String> {
    let (reply, receive) = oneshot::channel();
    sender
        .try_send(document::Message {
            key: KEY,
            generation,
            serial: 0,
            request: Request::Background { operation, reply },
        })
        .map_err(|_| "Reader query could not be opened".to_string())?;
    receive
        .await
        .map_err(|_| "Reader query was disconnected".to_string())?
}

impl Launcher {
    pub fn new(
        parent: &adw::ApplicationWindow,
        toast: &adw::ToastOverlay,
        sender: mpsc::Sender<document::Message>,
        button: &gtk::Button,
    ) -> Rc<Self> {
        let ui = Rc::new(Self {
            button: button.clone(),
            parent: parent.clone(),
            toast: toast.clone(),
            sender,
            target: RefCell::default(),
            generation: Cell::new(0),
            busy: Cell::new(false),
            enabled: Cell::new(false),
            query: RefCell::default(),
        });
        let weak = Rc::downgrade(&ui);
        button.connect_clicked(move |_| {
            if let Some(ui) = weak.upgrade() {
                ui.open();
            }
        });
        ui
    }
    pub fn sync(&self, target: Option<Target>, dirty: bool, reading: bool) {
        if *self.target.borrow() != target
            || (dirty
                && (self.busy.get()
                    || self
                        .query
                        .borrow()
                        .as_ref()
                        .is_some_and(|query| !query.is_closed())))
        {
            self.cancel();
            self.target.replace(target);
        }
        let visible = self.target.borrow().is_some();
        self.enabled.set(visible && !dirty && !reading);
        self.button.set_visible(visible);
        self.button
            .set_sensitive(self.enabled.get() && !self.busy.get());
    }
    fn cancel(&self) {
        let previous = self.generation.get();
        self.generation.set(previous.wrapping_add(1));
        self.busy.set(false);
        if let Some(query) = self.query.borrow_mut().take() {
            query.close();
        }
        if previous > 0 {
            let sender = self.sender.clone();
            glib::MainContext::default().spawn_local(async move {
                let _ = control(&sender, previous, Background::Close).await;
            });
        }
    }
    fn open(self: &Rc<Self>) {
        if self.busy.get() || !self.enabled.get() {
            return;
        }
        if let Some(query) = self
            .query
            .borrow()
            .as_ref()
            .filter(|query| !query.is_closed())
        {
            query.present();
            return;
        }
        let Some(target) = self.target.borrow().clone() else {
            return;
        };
        let generation = self.generation.get().wrapping_add(1);
        self.generation.set(generation);
        self.busy.set(true);
        self.button.set_sensitive(false);
        let weak = Rc::downgrade(self);
        let sender = self.sender.clone();
        glib::MainContext::default().spawn_local(async move {
            let result = async {
                match control(
                    &sender,
                    generation,
                    Background::Open {
                        root: target.root.clone(),
                        path: target.path.clone(),
                    },
                )
                .await?
                {
                    Reply::Open {
                        read_only: true, ..
                    } => {}
                    _ => return Err("Reader query did not open a read-only snapshot".to_string()),
                }
                // The shared indexer owns a 60-second supervision budget.
                // Allow that existing budget to finish before timing out UI
                // preparation, rather than rejecting a still-valid large file.
                let deadline = std::time::Instant::now() + std::time::Duration::from_secs(65);
                while std::time::Instant::now() < deadline {
                    if weak
                        .upgrade()
                        .is_none_or(|ui| ui.generation.get() != generation)
                    {
                        return Err("Query source changed".into());
                    }
                    match control(&sender, generation, Background::Index).await? {
                        Reply::Index(Some(index)) => return Ok(index),
                        Reply::Index(None) => {
                            glib::timeout_future(std::time::Duration::from_millis(50)).await
                        }
                        _ => return Err("Reader query document metadata was missing".into()),
                    }
                }
                Err("Reader query indexing timed out".into())
            }
            .await;
            let Some(ui) = weak
                .upgrade()
                .filter(|ui| ui.generation.get() == generation)
            else {
                let _ = control(&sender, generation, Background::Close).await;
                return;
            };
            ui.busy.set(false);
            ui.button.set_sensitive(ui.enabled.get());
            match result {
                Ok(index) => {
                    let query = Jq::new(
                        &ui.parent,
                        sender,
                        Context {
                            key: KEY,
                            generation,
                            revision: index.revision.clone(),
                        },
                        &index.name,
                    );
                    ui.query.replace(Some(query));
                }
                Err(error) => {
                    let error: String = error.chars().take(2048).collect();
                    ui.toast.add_toast(
                        adw::Toast::builder()
                            .title(&error)
                            .use_markup(false)
                            .build(),
                    );
                    let _ = control(&sender, generation, Background::Close).await;
                }
            }
        });
    }
}
