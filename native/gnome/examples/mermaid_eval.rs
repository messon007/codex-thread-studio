//! JSON-lines probe of the retained Pele baseline pipeline, without GTK startup.
#![allow(dead_code)]
#[path = "../src/diagram.rs"]
mod diagram;
#[path = "../src/diagram_style.rs"]
mod diagram_style;
#[path = "../src/engine_process.rs"]
mod engine_process;
#[path = "../src/math.rs"]
mod math;
#[path = "../src/mermaid_math.rs"]
mod mermaid_math;
use serde_json::{json, Value};
use std::{
    io::{self, BufRead},
    sync::{atomic::AtomicBool, Arc},
    time::Instant,
};
fn run(request: Value) -> Result<Value, String> {
    let source = request["source"].as_str().ok_or("source required")?;
    let count = request["iterations"].as_u64().unwrap_or(3).clamp(1, 20);
    let mut times = Vec::new();
    let mut last = None;
    for _ in 0..count {
        let at = Instant::now();
        let image = diagram::render_cancellable(
            "mermaid",
            source,
            request["dark"].as_bool().unwrap_or(false),
            &Default::default(),
            Arc::new(AtomicBool::new(false)),
        )?;
        times.push(at.elapsed().as_micros() as u64);
        last = Some(image);
    }
    let image = last.ok_or("no raster")?;
    if let Some(path) = request["pngPath"].as_str() {
        let size =
            resvg::tiny_skia::IntSize::from_wh(image.width, image.height).ok_or("raster size")?;
        let pixmap = resvg::tiny_skia::Pixmap::from_vec(image.pixels.to_vec(), size)
            .ok_or("raster pixels")?;
        pixmap.save_png(path).map_err(|e| e.to_string())?;
    }
    Ok(
        json!({"ok":true,"engine":"baseline-pele-1.9.0","totalUs":times,"width":image.width,"height":image.height}),
    )
}
fn main() {
    // Production math spawns the current executable with this worker flag.
    // Keep that entrypoint too, or the resolver silently retains literal TeX.
    if std::env::args().nth(1).as_deref() == Some("--native-math-worker") {
        std::process::exit(math::worker_stdio());
    }
    for line in io::stdin().lock().lines() {
        let result = line
            .map_err(|e| e.to_string())
            .and_then(|s| serde_json::from_str(&s).map_err(|e| e.to_string()))
            .and_then(run);
        println!(
            "{}",
            result.unwrap_or_else(|error| json!({"ok":false,"error":error}))
        );
    }
}
