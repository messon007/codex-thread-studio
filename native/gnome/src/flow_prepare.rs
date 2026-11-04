//! GTK-independent, cancellable preparation for bounded native text flow.
use std::sync::{atomic::AtomicBool, Arc};
use studio_markdown_native::{
    flow::{self, ObjectKind},
    syntax, Fragment,
};

#[derive(Debug)]
pub struct Prepared {
    pub revision: String,
    pub document: Arc<flow::Projection>,
    pub images: Vec<Option<crate::diagram::Image>>,
    pub raster_bytes: usize,
    pub syntax: Arc<syntax::Page>,
    pub syntax_us: u128,
    pub render_us: u128,
    pub rendered_objects: usize,
    pub reused_objects: usize,
}
pub fn prepare(fragment: &Fragment) -> Result<Prepared, String> {
    prepare_cancellable(fragment, Arc::new(AtomicBool::new(false)))
}
pub fn prepare_cancellable(
    fragment: &Fragment,
    cancel: Arc<AtomicBool>,
) -> Result<Prepared, String> {
    if cancel.load(std::sync::atomic::Ordering::Relaxed) {
        return Err("Text-flow preparation cancelled".into());
    }
    prepare_projection(
        Arc::new(flow::project(fragment)?),
        fragment.revision.clone(),
        cancel,
        false,
    )
}
/// Prepare selectable document text without waiting for standalone diagrams.
/// The native owner supplies the shared asynchronous diagram widgets at mount.
pub fn prepare_lazy_diagrams(
    fragment: &Fragment,
    cancel: Arc<AtomicBool>,
) -> Result<Prepared, String> {
    prepare_lazy_context(std::slice::from_ref(fragment), cancel)
}
/// Prepare multiple bounded index fragments as one snapshot-owned native view.
pub fn prepare_lazy_context(
    fragments: &[Fragment],
    cancel: Arc<AtomicBool>,
) -> Result<Prepared, String> {
    if cancel.load(std::sync::atomic::Ordering::Relaxed) {
        return Err("Text-flow preparation cancelled".into());
    }
    prepare_projection_mode(
        Arc::new(flow::project_context(fragments)?),
        fragments
            .first()
            .map_or_else(String::new, |f| f.revision.clone()),
        cancel,
        false,
        true,
    )
}
pub(crate) fn prepare_projection(
    document: Arc<flow::Projection>,
    revision: String,
    cancel: Arc<AtomicBool>,
    dark: bool,
) -> Result<Prepared, String> {
    prepare_projection_mode(document, revision, cancel, dark, false)
}
fn prepare_projection_mode(
    document: Arc<flow::Projection>,
    revision: String,
    cancel: Arc<AtomicBool>,
    dark: bool,
    lazy_diagrams: bool,
) -> Result<Prepared, String> {
    let token = cancel.clone();
    prepare_with_renderer(document, revision, cancel, lazy_diagrams, |kind, source| {
        crate::diagram::render_cancellable(kind, source, dark, &Default::default(), token.clone())
    })
}
fn prepare_with_renderer(
    document: Arc<flow::Projection>,
    revision: String,
    cancel: Arc<AtomicBool>,
    lazy_diagrams: bool,
    mut render: impl FnMut(&str, &str) -> Result<crate::diagram::Image, String>,
) -> Result<Prepared, String> {
    if cancel.load(std::sync::atomic::Ordering::Relaxed) {
        return Err("Text-flow preparation cancelled".into());
    }
    let syntax_started = std::time::Instant::now();
    let syntax = Arc::new(syntax::prepare(&document, &cancel)?);
    let syntax_us = syntax_started.elapsed().as_micros();
    if cancel.load(std::sync::atomic::Ordering::Relaxed) {
        return Err("Text-flow preparation cancelled".into());
    }
    {
        let mut formula_jobs = std::collections::HashSet::new();
        let mut work = 0;
        for object in &document.objects {
            if cancel.load(std::sync::atomic::Ordering::Relaxed) {
                return Err("Text-flow preparation cancelled".into());
            }
            let eager = match &object.kind {
                // Deferred factories retain their separate queues and total
                // object budget. Each table page still needs preparation.
                ObjectKind::CodeHeader { .. } | ObjectKind::Image(_) => false,
                ObjectKind::Diagram { .. } => !lazy_diagrams,
                ObjectKind::Math { source, display } => {
                    formula_jobs.insert((source.as_str(), *display))
                }
                ObjectKind::Table { .. } => true,
            };
            work += usize::from(eager);
            if work > 32 {
                return Err("This preview needs more than 32 renders or table pages; open a smaller section or use Source".into());
            }
        }
    }
    let mut images = Vec::new();
    let mut raster_bytes = 0usize;
    let render_started = std::time::Instant::now();
    let mut rendered_objects = 0;
    let mut reused_objects = 0;
    // Only one context and one theme live here. Keys borrow its immutable source;
    // images share their existing Arc pixels. External diagram engines are not
    // memoized. The total object/raster budgets still count every occurrence.
    let mut formulas = std::collections::HashMap::new();
    for object in &document.objects {
        if cancel.load(std::sync::atomic::Ordering::Relaxed) {
            return Err("Text-flow preparation cancelled".into());
        }
        let image = match &object.kind {
            ObjectKind::CodeHeader { .. } => None,
            ObjectKind::Math { source, display } => {
                let key = (source.as_str(), *display);
                if let Some(image) = formulas.get(&key) {
                    reused_objects += 1;
                    Some(crate::diagram::Image::clone(image))
                } else {
                    rendered_objects += 1;
                    let image = render(
                        if *display {
                            "math-inline-display"
                        } else {
                            "math-inline"
                        },
                        source,
                    )?;
                    formulas.insert(key, image.clone());
                    Some(image)
                }
            }
            ObjectKind::Diagram { .. } if lazy_diagrams => None,
            ObjectKind::Diagram { kind, source } => {
                rendered_objects += 1;
                Some(render(kind, source)?)
            }
            ObjectKind::Image(_) => None,
            ObjectKind::Table { id, header, .. } => {
                // Snapshot-owned tables use the production renderer's virtual
                // column picker and bounded selected-column API. This widget
                // limit applies only to the standalone local-table factory.
                if id.is_none() && header.len() > 64 {
                    return Err("Text-flow experiment table exceeds 64 columns".into());
                }
                None
            }
        };
        raster_bytes = raster_bytes.saturating_add(image.as_ref().map_or(0, |i| i.pixels.len()));
        if raster_bytes > 32 * 1024 * 1024 {
            return Err("Text-flow experiment exceeds 32 MiB of raster data".into());
        }
        images.push(image);
    }
    if cancel.load(std::sync::atomic::Ordering::Relaxed) {
        return Err("Text-flow preparation cancelled".into());
    }
    Ok(Prepared {
        revision,
        document,
        images,
        raster_bytes,
        syntax,
        syntax_us,
        render_us: render_started.elapsed().as_micros(),
        rendered_objects,
        reused_objects,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn input(text: String) -> Fragment {
        serde_json::from_value(serde_json::json!({"page":0,"documentId":"owned","revision":"revision","metadata":{"start":0,"end":text.len(),"line":1,"rows":0,"rowStart":0,"table":null},"source":{"start":0,"end":text.len(),"line":1,"endLine":text.lines().count(),"text":text},"header":null})).unwrap()
    }
    #[test]
    fn repeated_formula_results_share_pixels_without_merging_source_or_display_style() {
        let mut projection =
            flow::project(&input("One $x$. Two $x$. Three $y$. Four $x$.\n".into())).unwrap();
        let ObjectKind::Math { display, .. } = &mut projection.objects[1].kind else {
            panic!("Formula missing")
        };
        *display = true;
        let mut calls = Vec::new();
        let prepared = prepare_with_renderer(
            Arc::new(projection),
            "revision".into(),
            Arc::new(AtomicBool::new(false)),
            true,
            |kind, source| {
                calls.push((kind.to_owned(), source.to_owned()));
                Ok(crate::diagram::Image {
                    width: 1,
                    height: 1,
                    pixels: Arc::from([calls.len() as u8, 0, 0, 255]),
                    math: None,
                    vector: None,
                })
            },
        )
        .unwrap();
        assert_eq!(
            calls,
            [
                ("math-inline".into(), "x".into()),
                ("math-inline-display".into(), "x".into()),
                ("math-inline".into(), "y".into())
            ]
        );
        assert_eq!((prepared.rendered_objects, prepared.reused_objects), (3, 1));
        let pixels = |i: usize| &prepared.images[i].as_ref().unwrap().pixels;
        assert!(Arc::ptr_eq(pixels(0), pixels(3)));
        assert!(!Arc::ptr_eq(pixels(0), pixels(1)));
        assert!(!Arc::ptr_eq(pixels(0), pixels(2)));
        assert_eq!(prepared.raster_bytes, 16);
        assert_eq!(
            prepared
                .document
                .select(0..prepared.document.characters, flow::BUFFER_BYTES)
                .unwrap()
                .text,
            "One $x$. Two $x$. Three $y$. Four $x$."
        );
    }
    #[test]
    fn cancellation_retires_a_partially_memoized_formula_context() {
        let projection = flow::project(&input("One $x$. Two $x$.\n".into())).unwrap();
        let cancel = Arc::new(AtomicBool::new(false));
        let token = cancel.clone();
        let mut calls = 0;
        let result = prepare_with_renderer(
            Arc::new(projection),
            "revision".into(),
            cancel,
            true,
            |_, _| {
                calls += 1;
                token.store(true, std::sync::atomic::Ordering::Relaxed);
                Ok(crate::diagram::Image {
                    width: 1,
                    height: 1,
                    pixels: Arc::from([0, 0, 0, 255]),
                    math: None,
                    vector: None,
                })
            },
        );
        assert!(result.unwrap_err().contains("cancelled"));
        assert_eq!(calls, 1);
    }
    #[test]
    fn deferred_content_keeps_the_total_object_limit_without_raster_work() {
        let text = "![Image](local.svg)\n\n```mermaid\nflowchart LR\nA-->B\n```\n\n".repeat(128);
        let fragment = input(text.clone());
        let prepared = prepare_lazy_diagrams(&fragment, Arc::new(AtomicBool::new(false))).unwrap();
        assert_eq!(prepared.document.objects.len(), 256);
        assert!(prepared.images.iter().all(Option::is_none));
        assert_eq!(prepared.raster_bytes, 0);
        assert!(prepare_lazy_diagrams(
            &input(text + "![Extra](extra.svg)\n"),
            Arc::new(AtomicBool::new(false))
        )
        .unwrap_err()
        .contains("object budget"));
    }
    #[test]
    fn eager_math_tables_and_eager_diagrams_still_require_bounded_contexts() {
        for text in [
            (0..33)
                .map(|n| format!("Formula $x_{{{n}}}$.\n\n"))
                .collect::<String>(),
            "| A | B |\n|---|---|\n| one | two |\n\nParagraph.\n\n".repeat(33),
        ] {
            assert!(
                prepare_lazy_diagrams(&input(text), Arc::new(AtomicBool::new(false)))
                    .unwrap_err()
                    .contains("more than 32 renders")
            );
        }
        assert!(prepare(&input(
            "```mermaid\nflowchart LR\nA-->B\n```\n\n".repeat(33)
        ))
        .unwrap_err()
        .contains("more than 32 renders"));
    }
    #[test]
    fn repeated_formulas_use_one_job_up_to_the_projection_object_limit() {
        for count in [64, 256] {
            let fragment = input("Formula $x^2$ 中文😀.\n\n".repeat(count));
            let projection = flow::project(&fragment).unwrap();
            let mut calls = 0;
            let prepared = prepare_with_renderer(
                Arc::new(projection),
                "revision".into(),
                Arc::new(AtomicBool::new(false)),
                true,
                |_, _| {
                    calls += 1;
                    Ok(crate::diagram::Image {
                        width: 1,
                        height: 1,
                        pixels: Arc::from([0, 0, 0, 255]),
                        math: None,
                        vector: None,
                    })
                },
            )
            .unwrap();
            assert_eq!(calls, 1);
            assert_eq!(
                (prepared.rendered_objects, prepared.reused_objects),
                (1, count - 1)
            );
            assert_eq!(prepared.images.len(), count);
            assert_eq!(prepared.raster_bytes, count * 4);
            assert!(prepared.images.iter().all(|i| Arc::ptr_eq(
                &i.as_ref().unwrap().pixels,
                &prepared.images[0].as_ref().unwrap().pixels
            )));
            assert_eq!(
                prepared
                    .document
                    .select(0..prepared.document.characters, flow::BUFFER_BYTES)
                    .unwrap()
                    .text
                    .matches("$x^2$")
                    .count(),
                count
            );
        }
        assert!(flow::project(&input("Formula $x^2$.\n\n".repeat(257)))
            .unwrap_err()
            .contains("object budget"));
    }
    #[test]
    fn reused_formula_occurrences_still_charge_the_full_raster_admission_budget() {
        let projection = flow::project(&input("Formula $x$.\n\n".repeat(64))).unwrap();
        let mut calls = 0;
        let result = prepare_with_renderer(
            Arc::new(projection),
            "revision".into(),
            Arc::new(AtomicBool::new(false)),
            true,
            |_, _| {
                calls += 1;
                Ok(crate::diagram::Image {
                    width: 1024,
                    height: 256,
                    pixels: vec![0; 1024 * 1024].into(),
                    math: None,
                    vector: None,
                })
            },
        );
        assert!(result.unwrap_err().contains("32 MiB"));
        assert_eq!(calls, 1);
    }
    #[test]
    fn distinct_formula_work_is_rejected_before_starting_any_renderer() {
        let text = (0..33)
            .map(|n| format!("Formula $x_{{{n}}}$.\n\n"))
            .collect::<String>();
        let projection = flow::project(&input(text)).unwrap();
        let result = prepare_with_renderer(
            Arc::new(projection),
            "revision".into(),
            Arc::new(AtomicBool::new(false)),
            true,
            |_, _| panic!("Work admission must precede rendering"),
        );
        assert!(result.unwrap_err().contains("more than 32 renders"));
    }
    #[test]
    fn display_and_text_styles_have_separate_jobs_at_the_work_boundary() {
        for sources in [16, 17] {
            let text = (0..sources)
                .map(|n| format!("Formula $x_{{{n}}}$.\n\n").repeat(3))
                .collect::<String>();
            let mut projection = flow::project(&input(text)).unwrap();
            for (index, object) in projection.objects.iter_mut().enumerate() {
                if index % 3 == 1 {
                    let ObjectKind::Math { display, .. } = &mut object.kind else {
                        panic!("Math object missing")
                    };
                    *display = true;
                }
            }
            let mut calls = 0;
            let result = prepare_with_renderer(
                Arc::new(projection),
                "revision".into(),
                Arc::new(AtomicBool::new(false)),
                true,
                |_, _| {
                    calls += 1;
                    Ok(crate::diagram::Image {
                        width: 1,
                        height: 1,
                        pixels: Arc::from([0, 0, 0, 255]),
                        math: None,
                        vector: None,
                    })
                },
            );
            if sources == 16 {
                let prepared = result.unwrap();
                assert_eq!(
                    (calls, prepared.rendered_objects, prepared.reused_objects),
                    (32, 32, 16)
                );
            } else {
                assert!(result.unwrap_err().contains("more than 32 renders"));
                assert_eq!(calls, 0);
            }
        }
    }
    #[test]
    fn native_control_headers_do_not_consume_the_raster_object_admission_budget() {
        let text = "```unknown-language\n中文😀\n```\n\n".repeat(64);
        let fragment:Fragment=serde_json::from_value(serde_json::json!({"page":0,"documentId":"owned","revision":"revision","metadata":{"start":0,"end":text.len(),"line":1,"rows":0,"rowStart":0,"table":null},"source":{"start":0,"end":text.len(),"line":1,"endLine":256,"text":text},"header":null})).unwrap();
        let prepared = prepare_lazy_diagrams(&fragment, Arc::new(AtomicBool::new(false))).unwrap();
        assert_eq!(prepared.document.objects.len(), 64);
        assert!(prepared.images.iter().all(Option::is_none));
        assert_eq!(prepared.raster_bytes, 0);
        assert_eq!(prepared.syntax.blocks.len(), 64);
    }
    #[test]
    fn wide_indexed_headers_use_owner_column_limits_but_local_tables_remain_bounded() {
        let names = (0..2000).map(|i| format!("column-{i}")).collect::<Vec<_>>();
        let text = format!(
            "|{}|\n|{}|\n|{}|\n",
            names.join("|"),
            vec!["---"; 2000].join("|"),
            vec!["value"; 2000].join("|")
        );
        let mut fragment:Fragment=serde_json::from_value(serde_json::json!({"page":0,"documentId":"owned","revision":"revision","metadata":{"start":0,"end":text.len(),"line":1,"rows":1,"rowStart":0,"table":0},"source":{"start":0,"end":text.len(),"line":1,"endLine":3,"text":text},"header":null})).unwrap();
        let prepared =
            prepare_lazy_context(&[fragment.clone()], Arc::new(AtomicBool::new(false))).unwrap();
        assert!(
            matches!(&prepared.document.objects[0].kind,ObjectKind::Table{id:Some(0),header,..} if header.len()==2000)
        );
        fragment.metadata.table = None;
        assert!(
            prepare_lazy_context(&[fragment], Arc::new(AtomicBool::new(false)))
                .unwrap_err()
                .contains("64 columns")
        );
    }
}
