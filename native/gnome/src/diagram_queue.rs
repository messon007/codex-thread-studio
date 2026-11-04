//! A separate, bounded render lane keeps document navigation free of chart work.
use crate::{
    byte_cache::{ByteCache, Weight},
    diagram::Image,
    document::{Message, Reply, Request, Response},
    protocol::Event,
};
use std::{
    collections::HashMap,
    num::NonZeroUsize,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
};
use tokio::{sync::mpsc, task::JoinSet};

type Cancel = Arc<AtomicBool>;
type Pending = Arc<Mutex<HashMap<(u64, u64, usize), Cancel>>>;
type Renderer = Arc<
    dyn Fn(&str, &str, bool, &crate::diagram_style::Style, Cancel) -> Result<Image, String>
        + Send
        + Sync,
>;
/// Preparation and validation run on the same bounded CPU lane as rendering.
/// The prepared object owns the exact external resources used by its renderer.
pub(crate) trait Prepared: Send {
    fn environment(&self) -> &str;
    fn validate(&self) -> Result<(), String>;
    fn render(&self, cancel: Cancel) -> Result<Image, String>;
}
pub(crate) type Preparer = Arc<
    dyn Fn(
            &str,
            &str,
            bool,
            &crate::diagram_style::Style,
            Cancel,
        ) -> Result<Box<dyn Prepared>, String>
        + Send
        + Sync,
>;
struct Direct {
    renderer: Renderer,
    kind: String,
    source: String,
    dark: bool,
    style: crate::diagram_style::Style,
}
impl Prepared for Direct {
    fn environment(&self) -> &str {
        ""
    }
    fn validate(&self) -> Result<(), String> {
        Ok(())
    }
    fn render(&self, cancel: Cancel) -> Result<Image, String> {
        (self.renderer)(&self.kind, &self.source, self.dark, &self.style, cancel)
    }
}
struct Work {
    message: Message,
    cancel: Cancel,
}
#[derive(Clone, Hash, PartialEq, Eq)]
struct CacheKey {
    kind: String,
    source: String,
    dark: bool,
    style: crate::diagram_style::Style,
    environment: String,
}
struct Cached {
    image: Image,
    source_bytes: usize,
}
impl Weight for Cached {
    fn weight(&self) -> usize {
        self.image.pixels.len()
            + self.image.vector.as_ref().map_or(0, |v| v.heap_bytes())
            + self.source_bytes
            + std::mem::size_of::<Self>()
    }
}
pub struct Queue {
    input: mpsc::Sender<Work>,
    pending: Pending,
    tasks: JoinSet<()>,
}
impl Queue {
    pub fn new(output: async_channel::Sender<Event>) -> Self {
        Self::with_preparer(
            output,
            Arc::new(|kind, source, dark, style, cancel| {
                #[cfg(target_os = "linux")]
                if kind == "mermaid" {
                    return crate::mermaid_prepared::prepare(source, dark, style, cancel);
                }
                Ok(Box::new(Direct {
                    renderer: Arc::new(crate::diagram::render_cancellable),
                    kind: kind.into(),
                    source: source.into(),
                    dark,
                    style: style.clone(),
                }))
            }),
        )
    }
    pub(crate) fn with_renderer(output: async_channel::Sender<Event>, render: Renderer) -> Self {
        Self::with_preparer(
            output,
            Arc::new(move |kind, source, dark, style, _| {
                Ok(Box::new(Direct {
                    renderer: render.clone(),
                    kind: kind.into(),
                    source: source.into(),
                    dark,
                    style: style.clone(),
                }))
            }),
        )
    }
    pub(crate) fn with_preparer(output: async_channel::Sender<Event>, prepare: Preparer) -> Self {
        let (input, mut messages) = mpsc::channel::<Work>(8);
        let pending: Pending = Arc::default();
        let owned = pending.clone();
        let mut tasks = JoinSet::new();
        tasks.spawn(async move {
            let mut cache = image_cache();
            while let Some(work) = messages.recv().await {
                let Message {
                    key,
                    generation,
                    serial,
                    request,
                } = work.message;
                let Request::Diagram {
                    id,
                    kind,
                    source,
                    dark,
                    style,
                } = request
                else {
                    continue;
                };
                if work.cancel.load(Ordering::Relaxed) {
                    continue;
                }
                let style = if kind == "mermaid" {
                    style
                } else {
                    Default::default()
                };
                let mut cache_key = CacheKey {
                    kind,
                    source,
                    dark,
                    style,
                    environment: String::new(),
                };
                let preparer = prepare.clone();
                let cancel = work.cancel.clone();
                // Move the cache into this single CPU job. Resource preparation,
                // cache-hit validation and raster work never block Tokio or GTK.
                let result = crate::worker::compute(move || {
                    let image = cached_render(&mut cache, &mut cache_key, &preparer, cancel);
                    Ok((cache, image))
                })
                .await;
                let (returned_cache, image) = result.unwrap_or_else(|error| {
                    // A panicked worker retires its cache rather than reusing
                    // resources whose validation did not finish.
                    (image_cache(), Err(error))
                });
                cache = returned_cache;
                if work.cancel.load(Ordering::Relaxed) {
                    continue;
                }
                {
                    let mut pending = owned.lock().unwrap_or_else(|error| error.into_inner());
                    if pending
                        .get(&(key, generation, id))
                        .is_some_and(|token| Arc::ptr_eq(token, &work.cancel))
                    {
                        pending.remove(&(key, generation, id));
                    }
                }
                if output
                    .send(Event::Document(Response {
                        key,
                        generation,
                        serial,
                        kind: "diagram",
                        result: image.map(|image| Reply::Diagram { id, image }),
                    }))
                    .await
                    .is_err()
                {
                    break;
                }
            }
        });
        Self {
            input,
            pending,
            tasks,
        }
    }
    pub fn cancel_one(&self, key: u64, generation: u64, id: usize) {
        if let Some(token) = self
            .pending
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .remove(&(key, generation, id))
        {
            token.store(true, Ordering::Relaxed);
        }
    }
    pub fn cancel(&self, key: u64) {
        self.pending
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .retain(|(owner, _, _), token| {
                if *owner == key {
                    token.store(true, Ordering::Relaxed);
                    false
                } else {
                    true
                }
            });
    }
    pub fn cancel_generation(&self, key: u64, generation: u64) {
        self.pending
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .retain(|(owner, epoch, _), token| {
                if *owner == key && *epoch == generation {
                    token.store(true, Ordering::Relaxed);
                    false
                } else {
                    true
                }
            });
    }
    pub fn submit(&self, message: Message) -> Result<(), Message> {
        let Request::Diagram {
            id, source, style, ..
        } = &message.request
        else {
            return Err(message);
        };
        if style.validate().is_err() {
            return Err(message);
        }
        if source.len() > 128 * 1024 {
            return Err(message);
        }
        let identity = (message.key, message.generation, *id);
        let cancel = Arc::new(AtomicBool::new(false));
        if let Some(old) = self
            .pending
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .insert(identity, cancel.clone())
        {
            old.store(true, Ordering::Relaxed);
        }
        self.input
            .try_send(Work { message, cancel })
            .map_err(|error| {
                let work = error.into_inner();
                work.cancel.store(true, Ordering::Relaxed);
                self.pending
                    .lock()
                    .unwrap_or_else(|error| error.into_inner())
                    .remove(&identity);
                work.message
            })
    }
}
fn image_cache() -> ByteCache<CacheKey, Cached> {
    ByteCache::new(NonZeroUsize::new(32).unwrap(), 16 * 1024 * 1024)
}
fn cached_render(
    cache: &mut ByteCache<CacheKey, Cached>,
    key: &mut CacheKey,
    prepare: &Preparer,
    cancel: Cancel,
) -> Result<Image, String> {
    let current = || {
        if cancel.load(Ordering::Relaxed) {
            Err("Diagram rendering cancelled".to_owned())
        } else {
            Ok(())
        }
    };
    current()?;
    let prepared = prepare(&key.kind, &key.source, key.dark, &key.style, cancel.clone())?;
    // Environment identities are compact, versioned fingerprints, never files
    // or font bytes. Bound them independently of the source admission budget.
    if prepared.environment().len() > 512 {
        return Err("Diagram resource identity exceeds its budget".into());
    }
    key.environment = prepared.environment().into();
    current()?;
    prepared.validate()?;
    let (image, fresh) = if let Some(image) = cache.update(key, |cached| cached.image.clone()) {
        (image, false)
    } else {
        (prepared.render(cancel.clone())?, true)
    };
    current()?;
    prepared.validate()?;
    if fresh {
        cache.insert(
            key.clone(),
            Cached {
                image: image.clone(),
                source_bytes: key.source.len()
                    + key.kind.len()
                    + key.style.heap_bytes()
                    + key.environment.len(),
            },
        );
    }
    Ok(image)
}
impl Drop for Queue {
    fn drop(&mut self) {
        for token in self
            .pending
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .values()
        {
            token.store(true, Ordering::Relaxed);
        }
        self.tasks.abort_all();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicUsize;

    struct Resource {
        environment: String,
        epoch: usize,
        current: Arc<AtomicUsize>,
        renders: Arc<AtomicUsize>,
        validations: Arc<AtomicUsize>,
        change_during_render: Arc<AtomicBool>,
        forbidden_thread: std::thread::ThreadId,
    }
    impl Prepared for Resource {
        fn environment(&self) -> &str {
            &self.environment
        }
        fn validate(&self) -> Result<(), String> {
            assert_ne!(std::thread::current().id(), self.forbidden_thread);
            self.validations.fetch_add(1, Ordering::Relaxed);
            if self.current.load(Ordering::Relaxed) == self.epoch {
                Ok(())
            } else {
                Err("resource changed".into())
            }
        }
        fn render(&self, _: Cancel) -> Result<Image, String> {
            assert_ne!(std::thread::current().id(), self.forbidden_thread);
            self.renders.fetch_add(1, Ordering::Relaxed);
            if self.change_during_render.load(Ordering::Relaxed) {
                self.current.fetch_add(1, Ordering::Relaxed);
            }
            Ok(Image {
                width: 1,
                height: 1,
                pixels: Arc::from([self.epoch as u8, 0, 0, 255]),
                math: None,
                vector: None,
            })
        }
    }
    async fn result(events: &async_channel::Receiver<Event>) -> Result<Image, String> {
        let Event::Document(Response { result, .. }) =
            tokio::time::timeout(std::time::Duration::from_secs(2), events.recv())
                .await
                .unwrap()
                .unwrap()
        else {
            panic!("unexpected queue event")
        };
        result.map(|reply| {
            let Reply::Diagram { image, .. } = reply else {
                panic!("unexpected reply")
            };
            image
        })
    }
    #[tokio::test(flavor = "current_thread")]
    async fn prepared_resources_identify_and_validate_cached_images_on_cpu_lane() {
        let caller = std::thread::current().id();
        let epoch = Arc::new(AtomicUsize::new(1));
        let renders = Arc::new(AtomicUsize::new(0));
        let validations = Arc::new(AtomicUsize::new(0));
        let corrupt = Arc::new(AtomicBool::new(false));
        let prepare: Preparer = {
            let (epoch, renders, validations, corrupt) = (
                epoch.clone(),
                renders.clone(),
                validations.clone(),
                corrupt.clone(),
            );
            Arc::new(move |_, _, _, _, _| {
                assert_ne!(std::thread::current().id(), caller);
                let selected = epoch.load(Ordering::Relaxed);
                Ok(Box::new(Resource {
                    environment: format!("resource/{selected}"),
                    epoch: selected,
                    current: epoch.clone(),
                    renders: renders.clone(),
                    validations: validations.clone(),
                    change_during_render: corrupt.clone(),
                    forbidden_thread: caller,
                }))
            })
        };
        let (output, events) = async_channel::bounded(8);
        let queue = Queue::with_preparer(output, prepare);
        for (serial, selected) in [1, 1, 2, 1].into_iter().enumerate() {
            epoch.store(selected, Ordering::Relaxed);
            queue.submit(message(1, serial as u64, "same")).unwrap();
            assert_eq!(result(&events).await.unwrap().pixels[0], selected as u8);
        }
        assert_eq!(
            renders.load(Ordering::Relaxed),
            2,
            "resource replacement reused a stale image, or warm hits rendered twice"
        );
        assert_eq!(
            validations.load(Ordering::Relaxed),
            8,
            "cache hits skipped resource validation"
        );
        epoch.store(3, Ordering::Relaxed);
        corrupt.store(true, Ordering::Relaxed);
        queue.submit(message(1, 5, "same")).unwrap();
        assert_eq!(result(&events).await.unwrap_err(), "resource changed");
        epoch.store(3, Ordering::Relaxed);
        corrupt.store(false, Ordering::Relaxed);
        queue.submit(message(1, 6, "same")).unwrap();
        assert_eq!(result(&events).await.unwrap().pixels[0], 3);
        assert_eq!(
            renders.load(Ordering::Relaxed),
            4,
            "an image made with changed resources entered the cache"
        );
    }
    #[tokio::test(flavor = "current_thread")]
    async fn preparation_cancellation_leaves_runtime_and_replacement_request_live() {
        let entered = Arc::new(AtomicBool::new(false));
        let renders = Arc::new(AtomicUsize::new(0));
        struct Ready(Arc<AtomicUsize>);
        impl Prepared for Ready {
            fn environment(&self) -> &str {
                "ready"
            }
            fn validate(&self) -> Result<(), String> {
                Ok(())
            }
            fn render(&self, _: Cancel) -> Result<Image, String> {
                self.0.fetch_add(1, Ordering::Relaxed);
                Ok(Image {
                    width: 1,
                    height: 1,
                    pixels: Arc::from([9, 0, 0, 255]),
                    math: None,
                    vector: None,
                })
            }
        }
        let preparer: Preparer = {
            let (entered, renders) = (entered.clone(), renders.clone());
            Arc::new(move |_, source, _, _, cancel| {
                if source == "wait" {
                    entered.store(true, Ordering::Relaxed);
                    while !cancel.load(Ordering::Relaxed) {
                        std::thread::sleep(std::time::Duration::from_millis(2));
                    }
                    return Err("cancelled".into());
                }
                Ok(Box::new(Ready(renders.clone())))
            })
        };
        let (output, events) = async_channel::bounded(8);
        let queue = Queue::with_preparer(output, preparer);
        queue.submit(message(1, 1, "wait")).unwrap();
        tokio::time::timeout(std::time::Duration::from_secs(1), async {
            while !entered.load(Ordering::Relaxed) {
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
        queue.cancel_one(1, 1, 0);
        queue.submit(message(1, 2, "replacement")).unwrap();
        assert_eq!(result(&events).await.unwrap().pixels[0], 9);
        assert_eq!(renders.load(Ordering::Relaxed), 1);
        assert!(events.try_recv().is_err());
    }
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn raster_cache_distinguishes_mermaid_font_spacing_curve_style_and_layout() {
        use std::sync::atomic::AtomicUsize;
        let calls = Arc::new(AtomicUsize::new(0));
        let count = calls.clone();
        let (output, events) = async_channel::bounded(8);
        let queue = Queue::with_renderer(
            output,
            Arc::new(move |_, _, _, style, _| {
                count.fetch_add(1, Ordering::Relaxed);
                Ok(Image {
                    width: 1,
                    height: 1,
                    pixels: Arc::from([style.font_size, 0, 0, 255]),
                    math: None,
                    vector: None,
                })
            }),
        );
        let base = crate::diagram_style::Style::default();
        for (serial, style) in [
            base.clone(),
            crate::diagram_style::Style {
                font_size: 20,
                ..base.clone()
            },
            crate::diagram_style::Style {
                density: "loose".into(),
                ..base.clone()
            },
            crate::diagram_style::Style {
                curve: "linear".into(),
                ..base.clone()
            },
            crate::diagram_style::Style {
                style: "handDrawn".into(),
                ..base.clone()
            },
            crate::diagram_style::Style {
                layout: "elk".into(),
                ..base.clone()
            },
            base,
        ]
        .into_iter()
        .enumerate()
        {
            let mut message = message(1, serial as u64 + 1, "same");
            if let Request::Diagram {
                kind,
                style: target,
                ..
            } = &mut message.request
            {
                *kind = "mermaid".into();
                *target = style.clone();
            }
            queue.submit(message).unwrap();
            let event = tokio::time::timeout(std::time::Duration::from_secs(2), events.recv())
                .await
                .unwrap()
                .unwrap();
            let Event::Document(Response {
                result: Ok(Reply::Diagram { image, .. }),
                ..
            }) = event
            else {
                panic!("Diagram cache lost its result")
            };
            assert_eq!(image.pixels[0], style.font_size);
        }
        assert_eq!(
            calls.load(Ordering::Relaxed),
            6,
            "A different profile reused an old raster, or an equal profile missed the cache"
        );
    }
    fn message(key: u64, serial: u64, source: &str) -> Message {
        Message {
            key,
            generation: 1,
            serial,
            request: Request::Diagram {
                id: 0,
                kind: "test".into(),
                source: source.into(),
                dark: false,
                style: Default::default(),
            },
        }
    }
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn late_generation_cleanup_does_not_cancel_a_recreated_owner() {
        let (output, events) = async_channel::bounded(8);
        let entered = Arc::new(AtomicBool::new(false));
        let rendering = entered.clone();
        let queue = Queue::with_renderer(
            output,
            Arc::new(move |_, source, _, _, cancel| {
                if source == "old" {
                    rendering.store(true, Ordering::Relaxed);
                    while !cancel.load(Ordering::Relaxed) {
                        std::thread::sleep(std::time::Duration::from_millis(2));
                    }
                    return Err("cancelled".into());
                }
                Ok(Image {
                    width: 1,
                    height: 1,
                    pixels: Arc::from([7u8, 0, 0, 255]),
                    math: None,
                    vector: None,
                })
            }),
        );
        let mut old = message(5, 1, "old");
        old.generation = 11;
        queue.submit(old).unwrap();
        tokio::time::timeout(std::time::Duration::from_secs(1), async {
            while !entered.load(Ordering::Relaxed) {
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
        let mut new = message(5, 1, "new");
        new.generation = 12;
        queue.submit(new).unwrap();
        queue.cancel_one(5, 11, 0);
        queue.cancel_generation(5, 11);
        let event = tokio::time::timeout(std::time::Duration::from_secs(1), events.recv())
            .await
            .unwrap()
            .unwrap();
        let Event::Document(Response {
            generation: 12,
            result: Ok(Reply::Diagram { image, .. }),
            ..
        }) = event
        else {
            panic!("Late cleanup affected the replacement renderer")
        };
        assert_eq!(image.pixels.as_ref(), [7u8, 0, 0, 255]);
        assert!(events.try_recv().is_err());
    }
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn cancellation_cache_and_shutdown_are_bounded() {
        use std::{
            sync::atomic::AtomicUsize,
            time::{Duration, Instant},
        };
        let calls = Arc::new(AtomicUsize::new(0));
        let render_calls = calls.clone();
        let (output, events) = async_channel::bounded(8);
        let queue = Queue::with_renderer(
            output,
            Arc::new(move |_, source, _, _, cancel| {
                render_calls.fetch_add(1, Ordering::Relaxed);
                if source == "slow" {
                    let start = Instant::now();
                    while !cancel.load(Ordering::Relaxed)
                        && start.elapsed() < Duration::from_secs(3)
                    {
                        std::thread::sleep(Duration::from_millis(2));
                    }
                    return Err("cancelled".into());
                }
                Ok(Image {
                    width: 1,
                    height: 1,
                    pixels: Arc::from([0u8; 4]),
                    math: None,
                    vector: None,
                })
            }),
        );
        queue.submit(message(1, 1, "slow")).unwrap();
        tokio::time::timeout(Duration::from_secs(1), async {
            while calls.load(Ordering::Relaxed) == 0 {
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
        queue.cancel_one(1, 1, 99);
        assert!(!queue
            .pending
            .lock()
            .unwrap()
            .get(&(1, 1, 0))
            .unwrap()
            .load(Ordering::Relaxed));
        queue.cancel_one(1, 1, 0);
        queue.submit(message(2, 2, "same")).unwrap();
        let first = tokio::time::timeout(Duration::from_secs(1), events.recv())
            .await
            .unwrap()
            .unwrap();
        assert!(matches!(
            first,
            Event::Document(Response {
                key: 2,
                result: Ok(_),
                ..
            })
        ));
        queue.submit(message(3, 3, "same")).unwrap();
        let second = tokio::time::timeout(Duration::from_secs(1), events.recv())
            .await
            .unwrap()
            .unwrap();
        assert!(matches!(
            second,
            Event::Document(Response {
                key: 3,
                result: Ok(_),
                ..
            })
        ));
        assert_eq!(
            calls.load(Ordering::Relaxed),
            2,
            "identical diagrams were rendered twice"
        );
        queue.submit(message(4, 4, "slow")).unwrap();
        tokio::time::timeout(Duration::from_secs(1), async {
            while calls.load(Ordering::Relaxed) < 3 {
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
        let token = queue
            .pending
            .lock()
            .unwrap()
            .get(&(4, 1, 0))
            .unwrap()
            .clone();
        drop(queue);
        assert!(token.load(Ordering::Relaxed));
    }
}
