//! Isolated evaluator. Does not change the application renderer or its MSRV.
#[path = "../../../native/mermaid-worker/src/math_switch.rs"]
mod math_switch;
use merman::{OperationControl, RenderOutput, RenderRequest, Renderer, SvgRequest};
use serde_json::{Value, json};
use std::{
    io::{self, BufRead},
    sync::{Arc, OnceLock},
    time::{Duration, Instant},
};
fn raster(svg: &str, outlined: bool) -> Result<resvg::tiny_skia::Pixmap, String> {
    static FONTS: OnceLock<Arc<resvg::usvg::fontdb::Database>> = OnceLock::new();
    let fonts = if outlined {
        // The worker normalized all glyphs to paths with its measurement fonts.
        // The display-side rasterizer therefore needs no second font database.
        Arc::new(resvg::usvg::fontdb::Database::new())
    } else {
        FONTS
            .get_or_init(|| {
                let mut db = resvg::usvg::fontdb::Database::new();
                db.load_system_fonts();
                db.set_sans_serif_family("DejaVu Sans");
                db.set_serif_family("DejaVu Serif");
                db.set_monospace_family("DejaVu Sans Mono");
                Arc::new(db)
            })
            .clone()
    };
    // Match the production preview size/font policy. Neither evaluator grants
    // diagram SVG ambient file/network access. Node images are disabled here.
    let options = resvg::usvg::Options {
        font_family: "sans-serif".into(),
        fontdb: fonts,
        image_href_resolver: resvg::usvg::ImageHrefResolver {
            resolve_data: Box::new(|_, _, _| None),
            resolve_string: Box::new(|_, _| None),
        },
        ..Default::default()
    };
    let tree = resvg::usvg::Tree::from_str(svg, &options).map_err(|e| e.to_string())?;
    let size = tree.size();
    let scale = (2048.0 / size.width()).min(4096.0 / size.height()).min(2.0);
    let width = (size.width() * scale).ceil().max(1.0) as u32;
    let height = (size.height() * scale).ceil().max(1.0) as u32;
    if u64::from(width) * u64::from(height) > 8 * 1024 * 1024 {
        return Err("raster budget".into());
    }
    let mut image = resvg::tiny_skia::Pixmap::new(width, height).ok_or("raster allocation")?;
    resvg::render(
        &tree,
        resvg::tiny_skia::Transform::from_scale(scale, scale),
        &mut image.as_mut(),
    );
    Ok(image)
}
fn run(request: Value) -> Result<Value, String> {
    let source = request["source"].as_str().ok_or("source required")?;
    if source.len() > 128 * 1024 {
        return Err("source exceeds application limit".into());
    }
    let count = request["iterations"].as_u64().unwrap_or(3).clamp(1, 20);
    let native_fonts = request["nativeFonts"].as_bool().unwrap_or(false);
    let outlined = request["outlineText"].as_bool().unwrap_or(false);
    if outlined && !native_fonts {
        return Err("Text outlining requires explicit native measurement".into());
    }
    if outlined && request["rawSvg"].as_bool().unwrap_or(false) {
        return Err("Text outlining requires resvg-safe SVG".into());
    }
    // Enable semantic HTML/math labels, then lower them through resvg_safe.
    // Disabling htmlLabels renders literal $$...$$ even with a math renderer.
    let family = request["fontFamily"].as_str().unwrap_or(if native_fonts {
        "DejaVu Sans"
    } else {
        "sans-serif"
    });
    let config = json!({"securityLevel":"strict","fontSize":15,"fontFamily":family,"htmlLabels":true,
        "theme":"default","look":"classic",
        "flowchart":{"layout":"dagre","nodeSpacing":38,"rankSpacing":48,"diagramPadding":12,"padding":15,"curve":"rounded"}});
    let (mut render_us, mut raster_us, mut total_us) = (Vec::new(), Vec::new(), Vec::new());
    let mut svg = String::new();
    let mut capabilities = Vec::new();
    let mut selected_faces = Vec::new();
    let mut measurement = String::new();
    let mut layout_svg = String::new();
    let mut outline_us = Vec::new();
    let mut last = None;
    for _ in 0..count {
        let started = Instant::now();
        let control = OperationControl::new().with_deadline(Duration::from_secs(3));
        static NATIVE_FONTS: OnceLock<Result<merman::svg::export::NativeFontContext, String>> =
            OnceLock::new();
        let fonts = if native_fonts {
            Some(
                NATIVE_FONTS
                    .get_or_init(|| {
                        let files = request["fontFiles"].as_array();
                        let mut options = merman::svg::export::NativeFontOptions::default()
                            .with_system_fonts(files.is_none_or(|files| files.is_empty()));
                        if let Some(files) = files {
                            for file in files {
                                options = options.with_font_file(
                                    file.as_str().ok_or("fontFiles must contain paths")?,
                                );
                            }
                        }
                        options
                            .load(&control)
                            .map(|context| context.with_strict_glyph_coverage(true))
                            .map_err(|e| e.to_string())
                    })
                    .clone()?
                    .for_operation(&control),
            )
        } else {
            None
        };
        let mut environment = merman::SvgEnvironment::deterministic().with_compiled_math_renderer();
        if let Some(context) = &fonts {
            environment = environment
                .with_native_font_context(context.clone())
                .map_err(|e| e.to_string())?;
        }
        let mut pipeline = if request["rawSvg"].as_bool().unwrap_or(false) {
            merman::svg::SvgPipeline::parity()
        } else {
            merman::svg::SvgPipeline::resvg_safe()
        };
        if request["preferMathSwitches"].as_bool().unwrap_or(false) {
            pipeline.push_postprocessor(math_switch::PreferMathSwitches(control.clone()));
        }
        let target = SvgRequest {
            environment,
            pipeline: Some(pipeline),
            ..Default::default()
        };
        // The production Pele path constructs its engine per request. Include
        // the same lifetime here; only immutable/global font caches stay warm.
        let result = {
            let renderer = Renderer::new().with_engine(
                merman::Engine::new()
                    .with_site_config(merman::MermaidConfig::from_value(config.clone())),
            );
            renderer.render(RenderRequest::svg(source, control.clone(), target))
        }
        .map_err(|e| e.to_string())?;
        render_us.push(started.elapsed().as_micros() as u64);
        let RenderOutput::Svg(Some(output)) = result else {
            return Err("no SVG output".into());
        };
        svg = output.svg().to_string();
        capabilities = output
            .evidence()
            .required_capabilities()
            .iter()
            .map(|value| format!("{value:?}"))
            .collect::<Vec<_>>();
        selected_faces = output.evidence().native_font_faces().to_vec();
        measurement = format!("{:?}", output.evidence().measurement());
        if svg.len() > 4 * 1024 * 1024 {
            return Err("SVG exceeds application limit".into());
        }
        if outlined {
            let at = Instant::now();
            let context = fonts
                .as_ref()
                .ok_or("Missing native fonts")?
                .for_export(&control)
                .map_err(|e| e.to_string())?;
            let mut options = layout_svg::Options {
                image_href_resolver: layout_svg::ImageHrefResolver {
                    resolve_data: Box::new(|_, _, _| None),
                    resolve_string: Box::new(|_, _| None),
                },
                ..Default::default()
            };
            context.apply_to_usvg_options(&mut options);
            let tree = layout_svg::Tree::from_str(&svg, &options).map_err(|e| e.to_string())?;
            context.check().map_err(|e| e.to_string())?;
            control.checkpoint().map_err(|e| e.to_string())?;
            layout_svg = svg;
            svg = tree.to_string(&layout_svg::WriteOptions {
                preserve_text: false,
                ..Default::default()
            });
            if svg.contains("<text") {
                return Err("Outlined SVG still depends on fonts".into());
            }
            if svg.len() > 4 * 1024 * 1024 {
                return Err("Outlined SVG exceeds application limit".into());
            }
            outline_us.push(at.elapsed().as_micros() as u64);
        }
        let at = Instant::now();
        last = Some(raster(&svg, outlined)?);
        raster_us.push(at.elapsed().as_micros() as u64);
        total_us.push(started.elapsed().as_micros() as u64);
    }
    if let Some(path) = request["svgPath"].as_str() {
        std::fs::write(path, &svg).map_err(|e| e.to_string())?;
    }
    if outlined && let Some(path) = request["layoutSvgPath"].as_str() {
        std::fs::write(path, &layout_svg).map_err(|e| e.to_string())?;
    }
    let image = last.ok_or("no raster")?;
    if let Some(path) = request["pngPath"].as_str() {
        image.save_png(path).map_err(|e| e.to_string())?;
    }
    Ok(
        json!({"ok":true,"engine":"merman-0.9.3","nativeFontMeasurement":native_fonts,"outlinedText":outlined,"outlineUs":outline_us,"layoutSvgBytes":layout_svg.len(),"selectedFaces":selected_faces,"measurement":measurement,"requiredCapabilities":capabilities,"svgBytes":svg.len(),"renderUs":render_us,"rasterUs":raster_us,"totalUs":total_us,"width":image.width(),"height":image.height(),"foreignObjects":svg.matches("<foreignObject").count(),"textElements":svg.matches("<text").count()}),
    )
}
fn main() {
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
