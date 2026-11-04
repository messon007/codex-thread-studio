//! Diagram generation and rasterization on bounded Rust workers, without a DOM.
use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, OnceLock,
    },
    time::{Duration, Instant},
};

use resvg::{tiny_skia, usvg};

#[derive(Clone, Debug)]
pub struct Image {
    pub width: u32,
    pub height: u32,
    pub pixels: Arc<[u8]>,
    pub math: Option<crate::math::Metrics>,
    pub vector: Option<Arc<Vector>>,
}

/// Opaque, immutable vector output. Only Rust workers parse or rasterize it.
pub struct Vector {
    svg: Arc<str>,
    preview: (u32, u32),
    outlined: bool,
    math_labels: Option<usize>,
}
impl std::fmt::Debug for Vector {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Vector")
            .field("bytes", &self.svg.len())
            .field("preview", &self.preview)
            .finish_non_exhaustive()
    }
}
impl Vector {
    #[cfg(debug_assertions)]
    pub(crate) fn smoke_math_labels(&self) -> usize {
        self.math_labels
            .unwrap_or_else(|| self.svg.matches("class=\"pele-math\"").count())
    }
    pub fn heap_bytes(&self) -> usize {
        self.svg.len() + std::mem::size_of::<Self>()
    }
    pub fn dimensions(&self, factor: f64) -> Result<(u32, u32), String> {
        if !factor.is_finite() || factor < 1.0 || factor > 32.0 {
            return Err("Invalid vector raster scale".into());
        }
        let (w, h) = (self.preview.0 as f64, self.preview.1 as f64);
        let factor = factor
            .min(8192.0 / w)
            .min(8192.0 / h)
            .min((8.0 * 1024.0 * 1024.0 / (w * h)).sqrt());
        Ok((
            (w * factor).floor().max(1.0) as u32,
            (h * factor).floor().max(1.0) as u32,
        ))
    }
    pub fn rasterize(&self, factor: f64, cancel: &AtomicBool) -> Result<Image, String> {
        if cancel.load(Ordering::Relaxed) {
            return Err("Vector raster cancelled".into());
        }
        let dimensions = self.dimensions(factor)?;
        let tree = svg_tree_with_fonts(&self.svg, self.outlined)?;
        if cancel.load(Ordering::Relaxed) {
            return Err("Vector raster cancelled".into());
        }
        let scale = (dimensions.0 as f32 / tree.size().width())
            .min(dimensions.1 as f32 / tree.size().height());
        let image = raster_tree(&tree, dimensions, scale)?;
        if cancel.load(Ordering::Relaxed) {
            return Err("Vector raster cancelled".into());
        }
        Ok(image)
    }
}

#[cfg(test)]
pub fn render(kind: &str, source: &str, dark: bool) -> Result<Image, String> {
    render_cancellable(
        kind,
        source,
        dark,
        &Default::default(),
        Arc::new(AtomicBool::new(false)),
    )
}
pub fn render_cancellable(
    kind: &str,
    source: &str,
    dark: bool,
    style: &crate::diagram_style::Style,
    cancel: Arc<AtomicBool>,
) -> Result<Image, String> {
    if cancel.load(Ordering::Relaxed) {
        return Err("Diagram rendering cancelled".into());
    }
    if source.len() > 128 * 1024 {
        return Err("Diagram exceeds 128 KiB".into());
    }
    let svg = match kind {
        "mermaid" => mermaid_cancellable(source, dark, style, cancel.clone())?,
        "plantuml" | "d2" => external_cancellable(kind, source, dark, cancel.clone())?,
        "math" => crate::math::render(source, dark, cancel.clone())?,
        "math-inline" | "math-inline-display" => {
            crate::math::render_inline(source, dark, kind == "math-inline-display", cancel.clone())?
        }
        _ => return Err(format!("Unknown diagram format: {kind}")),
    };
    if cancel.load(Ordering::Relaxed) {
        return Err("Diagram rendering cancelled".into());
    }
    let mut image = rasterize(&svg)?;
    if matches!(kind, "math-inline" | "math-inline-display") {
        image.math = Some(crate::math::Metrics::from_svg(&svg)?);
    } else {
        if svg.len() > 4 * 1024 * 1024 {
            return Err("Diagram SVG exceeds 4 MiB".into());
        }
        image.vector = Some(Arc::new(Vector {
            svg: svg.into(),
            preview: (image.width, image.height),
            outlined: false,
            math_labels: None,
        }));
    }
    Ok(image)
}

/// The isolated Mermaid worker resolves every font and emits path-only SVG.
/// Parsing this output must not trigger a second system font scan, including
/// later inspector zooms. Font discovery for other engines remains unchanged.
pub(crate) fn outlined_image(
    svg: String,
    cancel: &AtomicBool,
    math_labels: usize,
) -> Result<Image, String> {
    if cancel.load(Ordering::Relaxed) {
        return Err("Diagram rendering cancelled".into());
    }
    if svg.len() > 4 * 1024 * 1024 {
        return Err("Diagram SVG exceeds 4 MiB".into());
    }
    let tree = svg_tree_with_fonts(&svg, true)?;
    let mut image = raster_preview(&tree)?;
    if cancel.load(Ordering::Relaxed) {
        return Err("Diagram rendering cancelled".into());
    }
    image.vector = Some(Arc::new(Vector {
        svg: svg.into(),
        preview: (image.width, image.height),
        outlined: true,
        math_labels: Some(math_labels),
    }));
    Ok(image)
}

fn executable(name: &str) -> Result<std::path::PathBuf, String> {
    std::env::var_os("PATH")
        .into_iter()
        .flat_map(|path| std::env::split_paths(&path).collect::<Vec<_>>())
        .map(|directory| directory.join(name))
        .find_map(|path| path.is_file().then(|| path.canonicalize().ok()).flatten())
        .ok_or_else(|| format!("Install {name} for native diagram support"))
}

#[cfg(test)]
fn external(kind: &str, source: &str, dark: bool) -> Result<String, String> {
    external_cancellable(kind, source, dark, Arc::new(AtomicBool::new(false)))
}
fn external_cancellable(
    kind: &str,
    source: &str,
    dark: bool,
    cancel: Arc<AtomicBool>,
) -> Result<String, String> {
    use std::process::Command;
    let mut command = Command::new(executable("bwrap")?);
    command.args([
        "--unshare-all",
        "--die-with-parent",
        "--new-session",
        "--clearenv",
        "--setenv",
        "PATH",
        "/usr/bin",
        "--setenv",
        "HOME",
        "/tmp",
        "--setenv",
        "LANG",
        "C.UTF-8",
        "--ro-bind",
        "/usr/lib",
        "/usr/lib",
        "--ro-bind",
        "/usr/bin",
        "/usr/bin",
        "--symlink",
        "usr/lib",
        "/lib",
        "--symlink",
        "usr/lib64",
        "/lib64",
        "--symlink",
        "usr/bin",
        "/bin",
        "--proc",
        "/proc",
        "--dev",
        "/dev",
        "--tmpfs",
        "/tmp",
        "--dir",
        "/work",
        "--chdir",
        "/work",
    ]);
    for directory in [
        "/usr/lib64",
        "/usr/share/fonts",
        "/usr/share/fontconfig",
        "/etc/fonts",
    ] {
        if std::path::Path::new(directory).exists() {
            command.args(["--ro-bind", directory, directory]);
        }
    }
    if kind == "plantuml" {
        let jar = std::env::var_os("STUDIO_GNOME_PLANTUML_JAR")
            .map(std::path::PathBuf::from)
            .or_else(|| {
                std::env::current_exe()
                    .ok()?
                    .parent()
                    .map(|path| path.join("plantuml.jar"))
            })
            .filter(|path| path.is_file())
            .ok_or("Install the native diagram runtimes with scripts/install-gnome-diagrams.sh")?;
        let java = executable("java")?;
        // Debian/OpenJDK links its built-in configuration into these system
        // directories. No user files or home directories enter the namespace.
        if let Ok(entries) = std::fs::read_dir("/etc") {
            for entry in entries.flatten() {
                if entry.file_name().to_string_lossy().starts_with("java-") && entry.path().is_dir()
                {
                    command.arg("--ro-bind").arg(entry.path()).arg(entry.path());
                }
            }
        }
        command
            .arg("--ro-bind")
            .arg(jar)
            .arg("/app/plantuml.jar")
            .arg("--")
            .arg(java)
            .args([
                "-Xmx256m",
                "-Djava.awt.headless=true",
                "-DPLANTUML_SECURITY_PROFILE=SANDBOX",
                "-Dplantuml.include.path=",
                "-jar",
                "/app/plantuml.jar",
                "-tsvg",
                "-pipe",
                "-nometadata",
                "-charset",
                "UTF-8",
            ]);
        if dark {
            command.arg("--dark-mode");
        }
    } else {
        let binary = std::env::var_os("STUDIO_GNOME_D2_BIN")
            .map(std::path::PathBuf::from)
            .or_else(|| {
                std::env::current_exe()
                    .ok()?
                    .parent()
                    .map(|path| path.join("d2"))
            })
            .filter(|path| path.is_file())
            .map(Ok)
            .unwrap_or_else(|| executable("d2"))?;
        command
            .args(["--setenv", "GOMEMLIMIT", "256MiB"])
            .arg("--ro-bind")
            .arg(binary)
            .arg("/app/d2")
            .arg("--")
            .args([
                "/app/d2",
                "--layout=dagre",
                "--bundle=false",
                "--timeout=8",
                "--no-xml-tag",
                "--theme",
            ])
            .arg(if dark { "200" } else { "0" })
            .args(["-", "-"]);
    }
    crate::engine_process::svg(
        &mut command,
        source.as_bytes().to_vec(),
        cancel,
        Duration::from_secs(10),
        4 * 1024 * 1024,
    )
    .map_err(|e| format!("{kind}: {e}"))
}

#[cfg(test)]
fn mermaid(source: &str, dark: bool) -> Result<String, String> {
    mermaid_cancellable(
        source,
        dark,
        &Default::default(),
        Arc::new(AtomicBool::new(false)),
    )
}
fn mermaid_cancellable(
    source: &str,
    dark: bool,
    style: &crate::diagram_style::Style,
    cancel: Arc<AtomicBool>,
) -> Result<String, String> {
    use rquickjs::{Context, Function, Runtime};
    style.validate()?;
    let runtime = Runtime::new().map_err(|e| e.to_string())?;
    runtime.set_memory_limit(128 * 1024 * 1024);
    runtime.set_max_stack_size(512 * 1024);
    let deadline = Instant::now() + Duration::from_secs(3);
    let math_cancel = cancel.clone();
    runtime.set_interrupt_handler(Some(Box::new(move || {
        cancel.load(Ordering::Relaxed) || Instant::now() > deadline
    })));
    let context = Context::full(&runtime).map_err(|e| e.to_string())?;
    context.with(|ctx| -> Result<String, String> {
        let error = |e: rquickjs::Error| {
            if e.is_exception() {
                format!("Mermaid: {:?}", ctx.catch())
            } else {
                format!("Mermaid: {e}")
            }
        };
        let render_cancel = math_cancel.clone();
        let math = std::cell::RefCell::new(crate::mermaid_math::Resolver::new(
            move |source: &str| {
                crate::math::render_inline_until(source, dark, render_cancel.clone(), deadline)
            },
            math_cancel,
            deadline,
        ));
        let resolver = Function::new(ctx.clone(), move |source: String, size: f64| {
            math.borrow_mut()
                .resolve(&source, size)
                .map_err(|e| rquickjs::Error::new_from_js_message("math label", "SVG", e))
        })
        .map_err(error)?;
        ctx.globals()
            .set("nativeMermaidMath", resolver)
            .map_err(error)?;
        ctx.eval::<(), _>(include_str!("../vendor/pele/pele.js"))
            .map_err(error)?;
        ctx.eval::<(), _>(
            r#"
globalThis.nativeMermaid = function(source, dark, profile) {
  const options = JSON.parse(profile);
  if (pele.detectType(source)!=='flowchart') delete options.padding;
  const result = pele.render(source, {...options,
    math:(tex,size)=>JSON.parse(nativeMermaidMath(tex,size)),
    responsive:false,links:false,images:false,limit:131072,outputLimit:4194304,maxEdges:4096});
  const values = dark ? {'--pele-bg':'#202020','--pele-fg':'#f6f5f4','--pele-muted':'#b4b4b4',
    '--pele-line':'#deddda','--pele-surface':'#353535','--pele-surface-alt':'#414141',
    '--pele-border':'#9a9996','--pele-accent':'#78aeed'} : {};
  // Split generated inline CSS without treating XML entities or quoted font
  // names as declaration boundaries. Keep the original XML escaping intact.
  function declarations(style) {
    const result=[];let part='',quote='',escaped=false,depth=0;
    for (const token of style.match(/&(?:[a-zA-Z]+|#\d+|#x[\da-fA-F]+);|[\s\S]/g)||[]) {
      const char=token==='&quot;'?'"':token==='&apos;'?"'":token;
      if (!quote && !depth && char===';') {result.push(part);part='';continue;}
      part+=token;
      if (escaped) {escaped=false;continue;}
      if (char==='\\') {escaped=true;continue;}
      if (quote) {if (char===quote) quote='';}
      else if (char==='"'||char==="'") quote=char;
      else if (char==='(') depth++;
      else if (char===')') depth=Math.max(0,depth-1);
    }
    if (part) result.push(part);
    return result;
  }
  // SVG engines support concrete presentation attributes. Resolve Pele's theme
  // variables in attribute values only; diagram label text stays untouched.
  const rootStyle = result.svg.match(/^<svg[^>]*\bstyle="([^"]*)"/);
  if (rootStyle) for (const declaration of declarations(rootStyle[1])) {
    const colon = declaration.indexOf(':');
    if (colon > 0 && declaration.trim().startsWith('--'))
      values[declaration.slice(0,colon).trim()] = declaration.slice(colon+1).trim();
  }
  function resolve(value) {
    for (let count=0;count<16 && value.includes('var(');count++) {
      const next=value.replace(/var\(\s*(--[\w-]+)\s*(?:,\s*([^()]*))?\)/g,
        (_,name,fallback)=>values[name] || fallback || 'inherit');
      if (next===value) break;
      value=next;
    }
    return value;
  }
  return result.svg.replace(/([\w:-]+)="([^"]*)"/g,(_,name,value)=> {
    value=resolve(value);
    // usvg's CSS parser stops at custom property declarations. Once their
    // references have been resolved, remove these declarations so ordinary
    // inherited font/color declarations are parsed as well.
    if (name==='style') value=declarations(value).filter(d=>d.trim()&&!/^\s*--/.test(d)).join(';');
    return name+'="'+value+'"';
  });
};
"#,
        )
        .map_err(error)?;
        let function: Function = ctx.globals().get("nativeMermaid").map_err(error)?;
        let profile = serde_json::to_string(&style.options()).map_err(|e| e.to_string())?;
        let svg: String = function.call((source, dark, profile)).map_err(error)?;
        if svg.len() > 4 * 1024 * 1024 {
            return Err("Rendered SVG exceeds 4 MiB".into());
        }
        Ok(svg)
    })
}

fn svg_tree(svg: &str) -> Result<usvg::Tree, String> {
    svg_tree_with_fonts(svg, false)
}
fn svg_tree_with_fonts(svg: &str, outlined: bool) -> Result<usvg::Tree, String> {
    static FONTS: OnceLock<Arc<usvg::fontdb::Database>> = OnceLock::new();
    let fonts = if outlined {
        Arc::new(usvg::fontdb::Database::new())
    } else {
        FONTS
            .get_or_init(|| {
                let mut db = usvg::fontdb::Database::new();
                db.load_system_fonts();
                db.set_sans_serif_family("DejaVu Sans");
                db.set_serif_family("DejaVu Serif");
                db.set_monospace_family("DejaVu Sans Mono");
                Arc::new(db)
            })
            .clone()
    };
    let resources = studio_svg_resources::Resources::default();
    let options = usvg::Options {
        font_family: "sans-serif".into(),
        fontdb: fonts,
        // An SVG never gains ambient file/network access through image hrefs.
        image_href_resolver: resources.resolver(),
        ..usvg::Options::default()
    };
    let tree = usvg::Tree::from_str(svg, &options).map_err(|e| format!("SVG: {e}"))?;
    resources.check()?;
    Ok(tree)
}
fn rasterize(svg: &str) -> Result<Image, String> {
    let tree = svg_tree(svg)?;
    raster_preview(&tree)
}
fn raster_preview(tree: &usvg::Tree) -> Result<Image, String> {
    let size = tree.size();
    let scale = (2048.0 / size.width()).min(4096.0 / size.height()).min(2.0);
    let width = (size.width() * scale).ceil().max(1.0) as u32;
    let height = (size.height() * scale).ceil().max(1.0) as u32;
    raster_tree(tree, (width, height), scale)
}
fn raster_tree(
    tree: &usvg::Tree,
    (width, height): (u32, u32),
    scale: f32,
) -> Result<Image, String> {
    if u64::from(width) * u64::from(height) > 8 * 1024 * 1024 {
        return Err("Diagram image exceeds its pixel budget".into());
    }
    let mut pixmap =
        tiny_skia::Pixmap::new(width, height).ok_or("Diagram image allocation failed")?;
    resvg::render(
        tree,
        tiny_skia::Transform::from_scale(scale, scale),
        &mut pixmap.as_mut(),
    );
    Ok(Image {
        width,
        height,
        pixels: pixmap.take().into(),
        math: None,
        vector: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn embedded_bitmap_survives_vector_zoom_and_resource_limits_are_reported() {
        let data = "iVBORw0KGgoAAAANSUhEUgAAAAIAAAABCAYAAAD0In+KAAAADklEQVR4nGO4o8H1H4QBEjMEGyP2UiEAAAAASUVORK5CYII=";
        let element =
            format!(r#"<image width="20" height="10" href="data:image/png;base64,{data}"/>"#);
        let svg = format!(
            r#"<svg xmlns="http://www.w3.org/2000/svg" width="20" height="10">{element}</svg>"#
        );
        let preview = rasterize(&svg).unwrap();
        let vector = Vector {
            svg: svg.clone().into(),
            preview: (preview.width, preview.height),
            outlined: false,
            math_labels: None,
        };
        let zoom = vector.rasterize(4.0, &AtomicBool::new(false)).unwrap();
        for image in [&preview, &zoom] {
            let center = ((image.height / 2 * image.width + image.width / 2) * 4) as usize;
            assert_eq!(&image.pixels[center..center + 4], &[220, 40, 10, 255]);
        }
        assert_eq!(&*vector.svg, svg);
        let excessive = format!(
            r#"<svg xmlns="http://www.w3.org/2000/svg" width="20" height="10">{}</svg>"#,
            element.repeat(65)
        );
        assert!(rasterize(&excessive).unwrap_err().contains("64 embedded"));
    }
    #[test]
    #[ignore = "Explicit D2 stage benchmark; requires the pinned D2 runtime and user namespaces"]
    fn profile_d2_vector_stages() {
        let at = Instant::now();
        let svg = external("d2", "chat -> reader: Native chat", true).unwrap();
        let engine_ms = at.elapsed().as_secs_f64() * 1000.0;
        let at = Instant::now();
        let tree = svg_tree(&svg).unwrap();
        let initial_parse_ms = at.elapsed().as_secs_f64() * 1000.0;
        let scale = (2048.0 / tree.size().width())
            .min(4096.0 / tree.size().height())
            .min(2.0);
        let preview = (
            (tree.size().width() * scale).ceil() as u32,
            (tree.size().height() * scale).ceil() as u32,
        );
        let vector = Vector {
            svg: svg.into(),
            preview,
            outlined: false,
            math_labels: None,
        };
        let mut samples = Vec::new();
        for factor in [1.0, 4.0, 4.0] {
            let at = Instant::now();
            let tree = svg_tree(&vector.svg).unwrap();
            let parse_ms = at.elapsed().as_secs_f64() * 1000.0;
            let dimensions = vector.dimensions(factor).unwrap();
            let scale = (dimensions.0 as f32 / tree.size().width())
                .min(dimensions.1 as f32 / tree.size().height());
            let at = Instant::now();
            let image = raster_tree(&tree, dimensions, scale).unwrap();
            let raster_ms = at.elapsed().as_secs_f64() * 1000.0;
            use std::hash::{Hash, Hasher};
            let mut digest = std::collections::hash_map::DefaultHasher::new();
            image.pixels.as_ref().hash(&mut digest);
            samples.push(serde_json::json!({"factor":factor,"pixels":[image.width,image.height],"parseMs":parse_ms,"rasterMs":raster_ms,"pixelsDigest":digest.finish()}));
        }
        println!(
            "{}",
            serde_json::json!({"debug":cfg!(debug_assertions),"engineMs":engine_ms,"initialParseMs":initial_parse_ms,"svgBytes":vector.svg.len(),"samples":samples})
        );
    }
    #[test]
    fn vector_zoom_recovers_fine_detail_and_enforces_pixel_limits() {
        let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" width="40" height="24"><path d="M10.2 2V22" stroke="black" stroke-width="0.1"/></svg>"#;
        let preview = rasterize(svg).unwrap();
        let vector = Vector {
            svg: svg.into(),
            preview: (preview.width, preview.height),
            outlined: false,
            math_labels: None,
        };
        let zoom = vector.rasterize(4.0, &AtomicBool::new(false)).unwrap();
        assert_eq!(
            (zoom.width, zoom.height),
            (preview.width * 4, preview.height * 4)
        );
        let alpha = |image: &Image| image.pixels.chunks_exact(4).map(|p| p[3]).max().unwrap();
        assert!(
            u16::from(alpha(&zoom)) > u16::from(alpha(&preview)) * 2,
            "Zoom merely enlarged preview pixels instead of recovering vector detail"
        );
        let (w, h) = vector.dimensions(32.0).unwrap();
        assert!(w <= 8192 && h <= 8192 && u64::from(w) * u64::from(h) <= 8 * 1024 * 1024);
        let large = Vector {
            svg: svg.into(),
            preview: (2048, 4096),
            outlined: false,
            math_labels: None,
        };
        let (w, h) = large.dimensions(32.0).unwrap();
        assert!(w <= 8192 && h <= 8192 && u64::from(w) * u64::from(h) <= 8 * 1024 * 1024);
        for scale in [f64::NAN, f64::INFINITY, 0.5, 33.0] {
            assert!(vector.dimensions(scale).is_err());
        }
        assert!(vector.rasterize(2.0, &AtomicBool::new(true)).is_err());
        assert!(!format!("{vector:?}").contains("<svg"));
    }
    #[test]
    fn saved_mermaid_defaults_change_real_svg_and_raster_without_source_rewrites() {
        let source = "flowchart LR\n A[Native 中文] --> B[Second node]\n A --> C[Third node]";
        let draw = |style: &crate::diagram_style::Style| {
            mermaid_cancellable(source, false, style, Arc::new(AtomicBool::new(false))).unwrap()
        };
        let small = crate::diagram_style::Style {
            font_size: 12,
            density: "compact".into(),
            curve: "linear".into(),
            ..Default::default()
        };
        let large = crate::diagram_style::Style {
            font_size: 20,
            density: "loose".into(),
            curve: "step".into(),
            font_family: "\"DejaVu Sans\",sans-serif".into(),
            ..Default::default()
        };
        let (a, b) = (draw(&small), draw(&large));
        let (ap, bp) = (rasterize(&a).unwrap(), rasterize(&b).unwrap());
        assert!(bp.width > ap.width && bp.height > ap.height);
        assert!(a.contains("Native 中文") && b.contains("Native 中文"));
        assert_ne!(ap.pixels, bp.pixels);
        let serif = draw(&crate::diagram_style::Style {
            font_family: "\"DejaVu Serif\",serif".into(),
            ..small.clone()
        });
        assert!(
            ap.pixels != rasterize(&serif).unwrap().pixels,
            "Font-family-only changes did not affect the real image: {serif}"
        );
        let with_mono = draw(&crate::diagram_style::Style {
            font_family: "\"DejaVu Serif\",serif".into(),
            font_family_mono: "\"DejaVu Sans Mono\",monospace".into(),
            ..small.clone()
        });
        assert!(
            rasterize(&serif).unwrap().pixels == rasterize(&with_mono).unwrap().pixels,
            "Quoted font names or XML entities broke inherited diagram styles: {with_mono}"
        );
        let curved = draw(&crate::diagram_style::Style {
            curve: "step".into(),
            ..small.clone()
        });
        assert_ne!(
            ap.pixels,
            rasterize(&curved).unwrap().pixels,
            "Curve-only changes did not affect the real image"
        );
        let directed =
            "%%{init: {\"flowchart\":{\"curve\":\"linear\"}}}%%\nflowchart LR\nA --> B\nA --> C";
        let first =
            mermaid_cancellable(directed, false, &small, Arc::new(AtomicBool::new(false))).unwrap();
        let second = mermaid_cancellable(
            directed,
            false,
            &crate::diagram_style::Style {
                font_size: 12,
                curve: "basis".into(),
                ..small.clone()
            },
            Arc::new(AtomicBool::new(false)),
        )
        .unwrap();
        assert_eq!(
            first, second,
            "Source directives no longer override global curve defaults"
        );
    }
    #[test]
    fn mermaid_runs_without_browser_apis_and_renders_text() {
        let svg = mermaid("flowchart LR\nA[中文] --> B[Native]", false).unwrap();
        assert!(svg.contains("中文") && !svg.contains("var("));
        let image = rasterize(&svg).unwrap();
        assert!(image.width > 100 && image.height > 30);
        assert!(image.pixels.chunks_exact(4).any(|pixel| pixel[3] > 0));
    }
    #[test]
    fn invalid_or_oversized_diagrams_are_reported() {
        assert!(render("mermaid", "not a diagram", false).is_err());
        assert!(render("mermaid", &"x".repeat(128 * 1024 + 1), false).is_err());
    }

    #[test]
    fn representative_mermaid_families_work_in_embedded_runtime() {
        for source in [
            "sequenceDiagram\nAlice->>Bob: Hello 中文",
            "classDiagram\nAnimal <|-- Duck\nAnimal: +String name",
            "stateDiagram-v2\n[*] --> Idle\nIdle --> Running",
            "erDiagram\nCUSTOMER ||--o{ ORDER : places",
            "pie title Results\n\"Pass\": 8\n\"Fail\": 2",
            "mindmap\n  root((Native))\n    Reader\n    Editor",
            "gantt\n  dateFormat YYYY-MM-DD\n  section Native\n  Reader :2026-10-24, 2d",
            "gitGraph\ncommit\nbranch native\ncheckout native\ncommit",
            "journey\n  title Native migration\n  section Reader\n  Open file: 5: User",
            "timeline\n  title Native\n  2026 : Reader : Editor",
            "xychart-beta\n  x-axis [A, B, C]\n  y-axis 0 --> 10\n  bar [3, 7, 5]",
            "quadrantChart\n  title Coverage\n  x-axis Low --> High\n  y-axis Low --> High\n  Reader: [0.7, 0.8]",
            "block-beta\n  columns 2\n  A B",
            "packet-beta\n  0-7: \"Header\"\n  8-15: \"Body\"",
            "sankey-beta\n\nSource,Target,10",
        ] {
            let svg=mermaid(source,false).unwrap_or_else(|error|panic!("{}: {error}",source.lines().next().unwrap()));
            assert!(svg.starts_with("<svg") && !svg.contains("var("));
        }
    }

    #[test]
    #[ignore = "requires the packaged diagram runtimes and user namespaces"]
    fn isolated_native_engines_render_and_cannot_import_user_files() {
        for (kind, source) in [
            ("plantuml", "@startuml\nAlice -> Bob: Native 中文\n@enduml"),
            ("d2", "reader -> editor: Native 中文"),
        ] {
            let image = render(kind, source, false).unwrap();
            assert!(image.width > 40 && image.height > 20);
        }
        let path = std::env::temp_dir().join(format!(
            "studio-native-diagram-secret-{}.d2",
            std::process::id()
        ));
        std::fs::write(&path, "PRIVATE_NATIVE_DIAGRAM_SENTINEL").unwrap();
        let d2 = external("d2", &format!("...@\"{}\"", path.display()), false);
        assert!(
            d2.as_ref()
                .is_err_and(|error| error.contains("no such file")),
            "{d2:?}"
        );
        let plant = external(
            "plantuml",
            &format!("@startuml\n!include {}\n@enduml", path.display()),
            false,
        );
        assert!(!plant
            .unwrap_or_default()
            .contains("PRIVATE_NATIVE_DIAGRAM_SENTINEL"));
        std::fs::remove_file(path).unwrap();
    }
}
