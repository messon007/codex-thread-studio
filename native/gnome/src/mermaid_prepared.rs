//! Production prepared renderer: request, fonts and runtime share one identity.
use crate::{
    diagram::{self, Image},
    diagram_queue::Prepared,
    diagram_style::Style,
    mermaid_fonts::{self, Selection},
    mermaid_worker::protocol,
};
use std::{
    path::{Path, PathBuf},
    sync::{atomic::AtomicBool, Arc},
};

struct Mermaid {
    worker: PathBuf,
    request: protocol::Request,
    selection: Selection,
}
pub(crate) fn prepare(
    source: &str,
    dark: bool,
    style: &Style,
    cancel: Arc<AtomicBool>,
) -> Result<Box<dyn Prepared>, String> {
    let worker = std::env::current_exe()
        .map_err(|e| e.to_string())?
        .parent()
        .ok_or("Native application directory is unavailable")?
        .join("codex-thread-studio-mermaid");
    if !worker.is_file() {
        return Err("Bundled Mermaid renderer is unavailable. Build or install the complete native package.".into());
    }
    prepare_at(&worker, source, dark, style, cancel)
}
impl Prepared for Mermaid {
    fn environment(&self) -> &str {
        &self.selection.fingerprint
    }
    fn validate(&self) -> Result<(), String> {
        self.selection.validate_current()
    }
    fn render(&self, cancel: Arc<AtomicBool>) -> Result<Image, String> {
        let svg = self
            .selection
            .render(&self.worker, &self.request, cancel.clone())?;
        let math_labels = protocol::math_labels(&svg)?;
        diagram::outlined_image(svg, &cancel, math_labels)
    }
}

pub(crate) fn prepare_at(
    worker: &Path,
    source: &str,
    dark: bool,
    style: &Style,
    cancel: Arc<AtomicBool>,
) -> Result<Box<dyn Prepared>, String> {
    style.validate()?;
    let worker = worker.canonicalize().map_err(|e| e.to_string())?;
    let request = protocol::Request {
        version: protocol::VERSION,
        source: source.into(),
        dark,
        style: protocol::Style {
            font_family: style.font_family.clone(),
            font_family_mono: style.font_family_mono.clone(),
            font_size: style.font_size,
            density: style.density.clone(),
            curve: style.curve.clone(),
            style: style.style.clone(),
            layout: style.layout.clone(),
        },
        font_defaults: None,
    };
    let selection = mermaid_fonts::prepare(&worker, &request, cancel)?;
    Ok(Box::new(Mermaid {
        worker,
        request,
        selection,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        diagram_queue::{Preparer, Queue},
        document::{Message, Reply, Request, Response},
        protocol::Event,
    };
    use std::{
        io::Write,
        sync::atomic::{AtomicUsize, Ordering},
    };

    struct Counted {
        inner: Box<dyn Prepared>,
        renders: Arc<AtomicUsize>,
    }
    impl Prepared for Counted {
        fn environment(&self) -> &str {
            self.inner.environment()
        }
        fn validate(&self) -> Result<(), String> {
            self.inner.validate()
        }
        fn render(&self, cancel: Arc<AtomicBool>) -> Result<Image, String> {
            self.renders.fetch_add(1, Ordering::Relaxed);
            self.inner.render(cancel)
        }
    }
    async fn image(queue: &Queue, events: &async_channel::Receiver<Event>, serial: u64) -> Image {
        queue.submit(Message {
            key: 90, generation: 1, serial,
            request: Request::Diagram {
                id: 0, kind: "mermaid".into(),
                source: "flowchart LR\nA[\"Native <b>bold</b> <i>italic</i> <code>code</code>\"] --> B[Reader]".into(),
                dark: false,
                style: Style { font_family: "DejaVu Sans".into(), font_family_mono: "DejaVu Sans".into(), ..Default::default() },
            },
        }).unwrap();
        let Event::Document(Response {
            result: Ok(Reply::Diagram { image, .. }),
            ..
        }) = tokio::time::timeout(std::time::Duration::from_secs(12), events.recv())
            .await
            .unwrap()
            .unwrap()
        else {
            panic!("prepared Mermaid queue did not produce an image")
        };
        image
    }
    #[tokio::test(flavor = "current_thread")]
    #[ignore = "explicit copied-font/worker fixture and real user namespaces; run scripts/test-gnome-mermaid-queue.py"]
    async fn real_prepared_resources_invalidate_image_cache_and_preserve_vector_zoom() {
        let root = PathBuf::from(
            std::env::var_os("STUDIO_GNOME_TEST_RESOURCE_ROOT").expect("owned fixture required"),
        )
        .canonicalize()
        .unwrap();
        assert_eq!(
            std::fs::read_to_string(root.join("FIXTURE")).unwrap(),
            "owned-mermaid-queue-fixture\n"
        );
        let font = root.join("fonts/DejaVuSans.ttf").canonicalize().unwrap();
        let worker = root.join("mermaid").canonicalize().unwrap();
        assert!(font.starts_with(&root) && worker.starts_with(&root));
        assert_eq!(
            PathBuf::from(std::env::var_os("FONTCONFIG_FILE").unwrap())
                .canonicalize()
                .unwrap(),
            root.join("fonts.conf")
        );
        let source_font = std::fs::metadata(&font).unwrap().len();
        let source_worker = std::fs::metadata(&worker).unwrap().len();
        let renders = Arc::new(AtomicUsize::new(0));
        let prepare: Preparer = {
            let (worker, renders) = (worker.clone(), renders.clone());
            Arc::new(move |kind, source, dark, style, cancel| {
                assert_eq!(kind, "mermaid");
                Ok(Box::new(Counted {
                    inner: prepare_at(&worker, source, dark, style, cancel)?,
                    renders: renders.clone(),
                }))
            })
        };
        let (output, events) = async_channel::bounded(8);
        let queue = Queue::with_preparer(output, prepare);
        let first = image(&queue, &events, 1).await;
        let warm = image(&queue, &events, 2).await;
        assert_eq!(first.pixels, warm.pixels);
        assert_eq!(renders.load(Ordering::Relaxed), 1);
        std::fs::OpenOptions::new()
            .append(true)
            .open(&font)
            .unwrap()
            .write_all(&[0; 16])
            .unwrap();
        let changed = image(&queue, &events, 3).await;
        assert_eq!(
            first.pixels, changed.pixels,
            "font padding must not change glyphs"
        );
        assert_eq!(
            renders.load(Ordering::Relaxed),
            2,
            "a changed font reused the old raster"
        );
        std::fs::OpenOptions::new()
            .write(true)
            .open(&font)
            .unwrap()
            .set_len(source_font)
            .unwrap();
        let restored = image(&queue, &events, 4).await;
        assert_eq!(first.pixels, restored.pixels);
        assert_eq!(
            renders.load(Ordering::Relaxed),
            2,
            "restored font identity missed its valid cached raster"
        );
        // These are private copied ELF bytes. A trailing pad preserves valid
        // code while replacing the runtime identity; never edit installed files.
        std::fs::OpenOptions::new()
            .append(true)
            .open(&worker)
            .unwrap()
            .write_all(&[0; 16])
            .unwrap();
        let replaced = image(&queue, &events, 5).await;
        assert_eq!(first.pixels, replaced.pixels);
        assert_eq!(
            renders.load(Ordering::Relaxed),
            3,
            "worker replacement reused an earlier runtime's raster"
        );
        std::fs::OpenOptions::new()
            .write(true)
            .open(&worker)
            .unwrap()
            .set_len(source_worker)
            .unwrap();
        let alias = root.join("worker-alias");
        let other = root.join("unselected-worker");
        std::fs::write(&other, "not an executable").unwrap();
        std::os::unix::fs::symlink(&worker, &alias).unwrap();
        let request = protocol::Request {
            version: protocol::VERSION,
            source: "flowchart LR\nA --> B".into(),
            dark: false,
            style: protocol::Style {
                font_family: "DejaVu Sans".into(),
                font_family_mono: "DejaVu Sans".into(),
                ..Default::default()
            },
            font_defaults: None,
        };
        let selected = mermaid_fonts::prepare(&alias, &request, Default::default()).unwrap();
        let captured = prepare_at(
            &alias,
            &request.source,
            false,
            &Style {
                font_family: "DejaVu Sans".into(),
                font_family_mono: "DejaVu Sans".into(),
                ..Default::default()
            },
            Default::default(),
        )
        .unwrap();
        std::fs::remove_file(&alias).unwrap();
        std::os::unix::fs::symlink(&other, &alias).unwrap();
        assert!(selected
            .render(&alias, &request, Default::default())
            .unwrap_err()
            .contains("worker changed"));
        // The prepared adapter owns its canonical runtime, not the mutable alias.
        assert!(captured.render(Default::default()).is_ok());
        let vector = first.vector.as_ref().unwrap();
        let zoom = vector.rasterize(4.0, &AtomicBool::new(false)).unwrap();
        assert!(zoom.width > first.width && zoom.height > first.height);
        assert!(zoom.pixels.chunks_exact(4).any(|p| p[3] > 0));
        let png = |image: &Image, name| {
            let size = resvg::tiny_skia::IntSize::from_wh(image.width, image.height).unwrap();
            resvg::tiny_skia::Pixmap::from_vec(image.pixels.to_vec(), size)
                .unwrap()
                .save_png(root.join(name))
                .unwrap();
        };
        png(&first, "preview.png");
        png(&zoom, "zoom.png");
        println!(
            "{}",
            serde_json::json!({"status":"passed", "preparations":5,"renders":renders.load(Ordering::Relaxed), "warmImageCacheReuse":true,"changedFontInvalidates":true,"restoredFontReusesImage":true,"changedWorkerInvalidates":true,"retargetedWorkerRejected":true,"capturedRuntimeStable":true,"preview":[first.width,first.height],"zoom":[zoom.width,zoom.height]})
        );
    }
}
