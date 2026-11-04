use std::{io::Read, sync::OnceLock, time::Instant};
fn latex(source: &str) -> Result<String, String> {
    static FONT: OnceLock<latex_rust::MathFont> = OnceLock::new();
    let font = FONT.get_or_init(|| latex_rust::MathFont::stix_two_math().unwrap());
    let mut options = latex_rust::SvgOptions::new();
    options.font_size_pt = latex_rust::Dim::from_i64(18);
    options.display = true;
    latex_rust::latex_to_svg(source, font, &options).map_err(|e| e.to_string())
}
fn ratex(source: &str) -> Result<String, String> {
    let nodes = ratex_parser::parse(source).map_err(|e| e.to_string())?;
    let options = ratex_layout::LayoutOptions::default();
    let tree = ratex_layout::layout(&nodes, &options);
    ratex_layout::validate_layout(&tree)?;
    let display = ratex_layout::to_display_list(&tree);
    Ok(ratex_svg::render_to_svg(
        &display,
        &ratex_svg::SvgOptions {
            font_size: 24.0,
            padding: 8.0,
            embed_glyphs: true,
            ..Default::default()
        },
    ))
}
fn main() {
    let args: Vec<_> = std::env::args().collect();
    let mut input = String::new();
    std::io::stdin().read_to_string(&mut input).unwrap();
    let v: serde_json::Value = serde_json::from_str(&input).unwrap();
    let source = v["source"].as_str().unwrap();
    let engine: fn(&str) -> Result<String, String> = if args[1] == "ratex" { ratex } else { latex };
    let start = Instant::now();
    let result = engine(source);
    let cold = start.elapsed().as_micros();
    let svg = match result {
        Ok(svg) => svg,
        Err(error) => {
            println!(
                "{}",
                serde_json::json!({"ok":false,"coldUs":cold,"error":error})
            );
            return;
        }
    };
    let start = Instant::now();
    for _ in 0..20 {
        std::hint::black_box(engine(source).unwrap());
    }
    let warm = start.elapsed().as_micros() / 20;
    let tree = resvg::usvg::Tree::from_str(&svg, &resvg::usvg::Options::default()).unwrap();
    let size = tree.size().to_int_size();
    let (width, height) = (size.width(), size.height());
    if u64::from(width) * u64::from(height) > 8 * 1024 * 1024 {
        println!("{}", serde_json::json!({"ok":false,"error":"pixel limit"}));
        return;
    }
    let mut pixels = resvg::tiny_skia::Pixmap::new(width, height).unwrap();
    pixels.fill(resvg::tiny_skia::Color::WHITE);
    resvg::render(
        &tree,
        resvg::tiny_skia::Transform::identity(),
        &mut pixels.as_mut(),
    );
    let visible = pixels
        .data()
        .chunks_exact(4)
        .filter(|p| p[0] < 250 || p[1] < 250 || p[2] < 250)
        .count();
    if let Some(path) = args.get(2) {
        pixels.save_png(path).unwrap();
        std::fs::write(format!("{path}.svg"), &svg).unwrap();
    }
    println!(
        "{}",
        serde_json::json!({"ok":true,"coldUs":cold,"warmUs":warm,"svgBytes":svg.len(),"hasText":svg.contains("<text"),"width":width,"height":height,"visiblePixels":visible})
    );
}
