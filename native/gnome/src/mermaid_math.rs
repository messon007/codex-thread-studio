//! Pele's SVG math resolver; runs only inside the bounded diagram worker.
//! Native glyph outlines keep browser/MathML/ambient resource access unnecessary.
use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::Instant,
};

const JOBS: usize = 32;
const BYTES: usize = 2 * 1024 * 1024;
type Formula = Option<(String, crate::math::Metrics)>;
pub struct Resolver<R> {
    render: R,
    cancel: Arc<AtomicBool>,
    deadline: Instant,
    formulas: HashMap<String, Formula>,
    bytes: usize,
}
impl<R: FnMut(&str) -> Result<String, String>> Resolver<R> {
    pub fn new(render: R, cancel: Arc<AtomicBool>, deadline: Instant) -> Self {
        Self {
            render,
            cancel,
            deadline,
            formulas: HashMap::new(),
            bytes: 0,
        }
    }
    fn current(&self) -> Result<(), String> {
        if self.cancel.load(Ordering::Relaxed) || Instant::now() >= self.deadline {
            Err("Mermaid math deadline exceeded or rendering cancelled".into())
        } else {
            Ok(())
        }
    }
    pub fn resolve(&mut self, source: &str, size: f64) -> Result<String, String> {
        self.current()?;
        if source.is_empty() {
            return Ok("null".into());
        }
        if source.len() > crate::math::SOURCE_LIMIT
            || !size.is_finite()
            || size <= 0.
            || size > 256.
        {
            return Err("Mermaid math source or font size exceeds its budget".into());
        }
        if !self.formulas.contains_key(source) {
            if self.formulas.len() >= JOBS {
                return Err("Mermaid exceeds 32 distinct math labels; split the diagram".into());
            }
            let rendered = (self.render)(source);
            self.current()?;
            let formula = match rendered {
                Ok(svg) => {
                    let metrics = crate::math::Metrics::from_svg(&svg)?;
                    self.bytes = self.bytes.saturating_add(svg.len());
                    if self.bytes > BYTES {
                        return Err("Mermaid math outlines exceed 2 MiB".into());
                    }
                    Some((svg, metrics))
                }
                // Unsupported TeX remains escaped literal label text in Pele.
                Err(_) => None,
            };
            self.formulas.insert(source.to_owned(), formula);
        }
        let Some((svg, metrics)) = self.formulas.get(source).unwrap() else {
            return Ok("null".into());
        };
        let width = metrics.width * size / metrics.em;
        let height = metrics.height * size / metrics.em;
        if width > 32768. || height > 32768. {
            return Err("Mermaid math dimensions exceed their budget".into());
        }
        // The trusted RaTeX envelope contains only namespace, viewBox, dimensions
        // and our metrics. Keep its glyph body and original coordinate system;
        // pt dimensions must not override Pele's pixel-sized label box.
        let body = svg
            .split_once('>')
            .and_then(|(_, body)| body.strip_suffix("</svg>"))
            .ok_or("Unexpected native math SVG envelope")?;
        let scaled = format!(
            r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {} {}" width="{width}" height="{height}">{body}</svg>"#,
            metrics.width, metrics.height
        );
        serde_json::to_string(&serde_json::json!({"svg":scaled,"width":width,"height":height}))
            .map_err(|e| e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{cell::Cell, time::Duration};
    fn outline() -> String {
        r#"<svg data-studio-math="48 24 16 24" xmlns="http://www.w3.org/2000/svg" viewBox="0 0 48 24" width="48pt" height="24pt"><path d="M1 1H40V20Z" fill="black"/></svg>"#.into()
    }
    #[test]
    fn scales_native_outlines_and_reuses_source_across_font_sizes() {
        let calls = Cell::new(0);
        let mut resolver = Resolver::new(
            |_| {
                calls.set(calls.get() + 1);
                Ok(outline())
            },
            Arc::new(AtomicBool::new(false)),
            Instant::now() + Duration::from_secs(3),
        );
        for size in [12., 16., 24., 48.] {
            let result: serde_json::Value =
                serde_json::from_str(&resolver.resolve("x^2", size).unwrap()).unwrap();
            assert_eq!(result["width"].as_f64(), Some(size * 2.));
            assert_eq!(result["height"].as_f64(), Some(size));
            let svg = result["svg"].as_str().unwrap();
            assert!(
                svg.contains("<path")
                    && svg.contains("viewBox=\"0 0 48 24\"")
                    && !svg.contains("pt\"")
            );
        }
        assert_eq!(calls.get(), 1);
    }
    #[test]
    fn unsupported_formula_stays_literal_without_repeated_worker_calls() {
        let calls = Cell::new(0);
        let mut resolver = Resolver::new(
            |_| {
                calls.set(calls.get() + 1);
                Err("unsupported".into())
            },
            Arc::new(AtomicBool::new(false)),
            Instant::now() + Duration::from_secs(3),
        );
        assert_eq!(resolver.resolve("\\unknown", 16.).unwrap(), "null");
        assert_eq!(resolver.resolve("\\unknown", 24.).unwrap(), "null");
        assert_eq!(calls.get(), 1);
    }
    #[test]
    fn distinct_work_and_cancellation_are_bounded_before_dispatch() {
        let calls = Cell::new(0);
        let cancel = Arc::new(AtomicBool::new(false));
        let mut resolver = Resolver::new(
            |_| {
                calls.set(calls.get() + 1);
                Ok(outline())
            },
            cancel.clone(),
            Instant::now() + Duration::from_secs(3),
        );
        for i in 0..32 {
            resolver.resolve(&format!("x_{i}"), 16.).unwrap();
        }
        assert!(resolver
            .resolve("extra", 16.)
            .unwrap_err()
            .contains("32 distinct"));
        assert_eq!(calls.get(), 32);
        cancel.store(true, Ordering::Relaxed);
        assert!(resolver
            .resolve("x_0", 16.)
            .unwrap_err()
            .contains("cancelled"));
    }
    #[test]
    fn stale_results_and_excessive_output_are_not_admitted() {
        let cancel = Arc::new(AtomicBool::new(false));
        let token = cancel.clone();
        let mut resolver = Resolver::new(
            move |_| {
                token.store(true, Ordering::Relaxed);
                Ok(outline())
            },
            cancel,
            Instant::now() + Duration::from_secs(3),
        );
        assert!(resolver.resolve("x", 16.).is_err());
        let mut resolver = Resolver::new(
            |_| Ok(outline().replace("<path", &format!("<!--{}--><path", "x".repeat(BYTES)))),
            Arc::new(AtomicBool::new(false)),
            Instant::now() + Duration::from_secs(3),
        );
        assert!(resolver.resolve("x", 16.).unwrap_err().contains("2 MiB"));
        let mut resolver = Resolver::new(
            |_| panic!("expired job started"),
            Arc::new(AtomicBool::new(false)),
            Instant::now(),
        );
        assert!(resolver.resolve("x", 16.).unwrap_err().contains("deadline"));
    }
    #[test]
    fn invalid_sizes_and_oversized_source_never_start_a_worker() {
        let mut resolver = Resolver::new(
            |_| panic!("invalid label started a worker"),
            Arc::new(AtomicBool::new(false)),
            Instant::now() + Duration::from_secs(3),
        );
        for size in [0., -1., f64::NAN, f64::INFINITY, 257.] {
            assert!(resolver.resolve("x", size).is_err());
        }
        assert!(resolver
            .resolve(&"x".repeat(crate::math::SOURCE_LIMIT + 1), 16.)
            .is_err());
        assert_eq!(resolver.resolve("", 16.).unwrap(), "null");
    }
}
