//! Fresh-process/first/warm highlighting probe, using the production registry.
use std::{
    io::{self, Read, Write},
    time::Instant,
};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let language = std::env::args().nth(1).unwrap_or_else(|| "rust".into());
    let mut source = String::new();
    io::stdin().take(65537).read_to_string(&mut source)?;
    if source.len() > 65536 || language.len() > 64 {
        return Err("Probe input budget exceeded".into());
    }
    if source.is_empty() {
        source =
            "// 中文😀\r\nfn main() { let answer = 42; println!(\"hello <world>\"); }\r\n".into()
    }
    println!(
        "{}",
        serde_json::json!({"phase":"ready","grammarCount":marknoria_syntax::LANGUAGES.len()})
    );
    io::stdout().flush()?;
    let started = Instant::now();
    let first = marknoria::core::highlight::native(&language, &source, false);
    let first_ms = started.elapsed().as_secs_f64() * 1000.;
    let complete =
        first.is_empty() || first.iter().map(|r| r.text.as_str()).collect::<String>() == source;
    let started = Instant::now();
    for _ in 0..32 {
        std::hint::black_box(marknoria::core::highlight::native(
            &language, &source, false,
        ));
    }
    let warm_ms = started.elapsed().as_secs_f64() * 1000. / 32.;
    println!(
        "{}",
        serde_json::json!({"languages":marknoria_syntax::LANGUAGES,"language":language,"firstHighlightMs":first_ms,"warmHighlightMs":warm_ms,"sourceBytes":source.len(),"runs":first.len(),"fallback":first.is_empty(),"complete":complete})
    );
    Ok(())
}
