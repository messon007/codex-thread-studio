//! One bounded request per process. No GTK, JS, browser, network or file output.
mod font_plan;
#[cfg(test)]
mod math_islands_tests;
mod math_switch;
pub mod protocol;
use merman::{OperationControl, RenderOutput, RenderRequest, Renderer, SvgEnvironment, SvgRequest};
use protocol::{FONT_BYTES, FONT_LIMIT, INPUT_LIMIT, Request, SVG_LIMIT};
use std::{
    io::{Read, Write},
    path::PathBuf,
    time::Duration,
};

fn limits() -> Result<(), String> {
    #[cfg(target_os = "linux")]
    unsafe {
        for (kind, value) in [
            (libc::RLIMIT_AS, 1024 * 1024 * 1024),
            (libc::RLIMIT_DATA, 512 * 1024 * 1024),
            (libc::RLIMIT_CPU, 4),
            (libc::RLIMIT_CORE, 0),
            (libc::RLIMIT_FSIZE, 0),
            (libc::RLIMIT_NOFILE, 64),
            (libc::RLIMIT_NPROC, 0),
        ] {
            let limit = libc::rlimit {
                rlim_cur: value,
                rlim_max: value,
            };
            if libc::setrlimit(kind, &limit) != 0 {
                return Err("Cannot apply Mermaid worker resource limits".into());
            }
        }
        if libc::prctl(libc::PR_SET_NO_NEW_PRIVS, 1, 0, 0, 0) != 0 {
            return Err("Cannot restrict Mermaid worker privilege changes".into());
        }
    }
    Ok(())
}

fn fonts(args: impl Iterator<Item = std::ffi::OsString>) -> Result<Vec<PathBuf>, String> {
    let mut args = args;
    let mut paths = Vec::new();
    let mut total = 0u64;
    while let Some(flag) = args.next() {
        if flag != "--font" || paths.len() >= FONT_LIMIT {
            return Err("Expected at most twenty-four --font FILE arguments".into());
        }
        let path = PathBuf::from(args.next().ok_or("--font requires a file")?);
        let metadata = path
            .metadata()
            .map_err(|_| "Cannot read selected Mermaid font")?;
        if !metadata.is_file() {
            return Err("Selected Mermaid font is not a regular file".into());
        }
        total = total.saturating_add(metadata.len());
        if total > FONT_BYTES {
            return Err("Selected Mermaid fonts exceed 64 MiB".into());
        }
        paths.push(path);
    }
    if paths.is_empty() {
        return Err("Native Mermaid requires explicitly selected fonts".into());
    }
    Ok(paths)
}

fn render(request: &Request, files: Vec<PathBuf>) -> Result<String, String> {
    request.validate()?;
    let control = OperationControl::new().with_deadline(Duration::from_secs(3));
    let mut options = merman::svg::export::NativeFontOptions::default().with_system_fonts(false);
    for file in files {
        options = options.with_font_file(file);
    }
    let mut fonts = options
        .load(&control)
        .map_err(|e| e.to_string())?
        .with_strict_glyph_coverage(true);
    if let Some(defaults) = &request.font_defaults {
        fonts = fonts
            .with_generic_families(&defaults.sans, &defaults.mono)
            .map_err(|e| e.to_string())?;
    }
    let mut resources = merman::svg::RenderResourcePolicy::default();
    for (kind, value) in [
        (
            merman::svg::ResourceLimitId::MaxSourceBytes,
            protocol::SOURCE_LIMIT,
        ),
        (merman::svg::ResourceLimitId::MaxModelItems, 8192),
        (merman::svg::ResourceLimitId::MaxSvgBytes, SVG_LIMIT),
        (merman::svg::ResourceLimitId::MaxSvgElements, 131_072),
    ] {
        resources = resources
            .with_limit(kind, value)
            .map_err(|e| e.to_string())?;
    }
    let environment = SvgEnvironment::deterministic()
        .with_compiled_math_renderer()
        .with_resource_policy(resources)
        .with_native_font_context(fonts.clone())
        .map_err(|e| e.to_string())?;
    let mut pipeline = merman::svg::SvgPipeline::resvg_safe();
    pipeline.push_postprocessor(math_switch::PreferMathSwitches(control.clone()));
    let renderer = Renderer::new().with_engine(
        merman::Engine::new().with_site_config(merman::MermaidConfig::from_value(request.config())),
    );
    let output = renderer
        .render(RenderRequest::svg(
            &request.source,
            control.clone(),
            SvgRequest {
                environment,
                pipeline: Some(pipeline),
                ..Default::default()
            },
        ))
        .map_err(|e| e.to_string())?;
    let RenderOutput::Svg(Some(output)) = output else {
        return Err("No Mermaid SVG output".into());
    };
    if output.svg().len() > SVG_LIMIT {
        return Err("Mermaid SVG exceeds 4 MiB".into());
    }
    let math_document = roxmltree::Document::parse_with_options(
        output.svg(),
        roxmltree::ParsingOptions {
            nodes_limit: 131_072,
            ..Default::default()
        },
    )
    .map_err(|e| e.to_string())?;
    let math_labels = math_document
        .descendants()
        .filter(|node| {
            node.has_tag_name(("http://www.w3.org/2000/svg", "svg"))
                && node.attribute("class").is_some_and(|classes| {
                    classes
                        .split_ascii_whitespace()
                        .any(|c| c == "merman-native-math-svg")
                })
        })
        .count();
    drop(math_document);
    let context = fonts.for_export(&control).map_err(|e| e.to_string())?;
    let mut options = layout_svg::Options {
        image_href_resolver: layout_svg::ImageHrefResolver {
            resolve_data: Box::new(|_, _, _| None),
            resolve_string: Box::new(|_, _| None),
        },
        ..Default::default()
    };
    context.apply_to_usvg_options(&mut options);
    let tree = layout_svg::Tree::from_str(output.svg(), &options).map_err(|e| e.to_string())?;
    context.check().map_err(|e| e.to_string())?;
    control.checkpoint().map_err(|e| e.to_string())?;
    let size = tree.size();
    if !size.width().is_finite()
        || !size.height().is_finite()
        || size.width() > 32768.
        || size.height() > 32768.
    {
        return Err("Mermaid SVG dimensions exceed their budget".into());
    }
    let svg = tree.to_string(&layout_svg::WriteOptions {
        preserve_text: false,
        ..Default::default()
    });
    let body = svg
        .strip_prefix("<svg ")
        .ok_or("Unexpected outlined SVG header")?;
    let svg = format!("{}{math_labels}\" {body}", protocol::SVG_HEADER);
    if svg.len() > SVG_LIMIT {
        return Err("Outlined Mermaid SVG exceeds 4 MiB".into());
    }
    let document = roxmltree::Document::parse_with_options(
        &svg,
        roxmltree::ParsingOptions {
            nodes_limit: 131_072,
            ..Default::default()
        },
    )
    .map_err(|e| e.to_string())?;
    if document
        .descendants()
        .any(|n| matches!(n.tag_name().name(), "text" | "foreignObject" | "image"))
    {
        return Err("Mermaid output retains a font or image dependency".into());
    }
    control.checkpoint().map_err(|e| e.to_string())?;
    Ok(svg)
}

fn main() {
    let result = (|| -> Result<(), String> {
        limits()?;
        let mut input = Vec::new();
        std::io::stdin()
            .take(INPUT_LIMIT as u64 + 1)
            .read_to_end(&mut input)
            .map_err(|e| e.to_string())?;
        if input.len() > INPUT_LIMIT {
            return Err("Mermaid request exceeds 800 KiB".into());
        }
        let request: Request =
            serde_json::from_slice(&input).map_err(|_| "Invalid native Mermaid request")?;
        request.validate()?;
        let args = std::env::args_os().skip(1).collect::<Vec<_>>();
        let output = if args == [std::ffi::OsString::from("--font-plan")] {
            let bytes =
                serde_json::to_vec(&font_plan::plan(&request)?).map_err(|e| e.to_string())?;
            if bytes.len() > 65536 {
                return Err("Mermaid font plan exceeds 64 KiB".into());
            }
            bytes
        } else {
            render(&request, fonts(args.into_iter())?)?.into_bytes()
        };
        std::io::stdout()
            .lock()
            .write_all(&output)
            .map_err(|e| e.to_string())?;
        Ok(())
    })();
    if let Err(error) = result {
        eprintln!(
            "Native Mermaid: {}",
            error.chars().take(2048).collect::<String>()
        );
        std::process::exit(1);
    }
}
