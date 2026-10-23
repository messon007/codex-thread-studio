use std::{env, fs, hint::black_box, time::Instant};
use studio_markdown_wasm_bench::{render_document_json, render_html};

fn main() {
    let arguments: Vec<String> = env::args().collect();
    assert!(
        arguments.len() >= 3,
        "usage: bench FILE html|document [SAMPLES]"
    );
    let source = fs::read_to_string(&arguments[1]).unwrap();
    let render = match arguments[2].as_str() {
        "html" => render_html,
        "document" => render_document_json,
        _ => panic!("unknown mode"),
    };
    if arguments.get(3).is_some_and(|value| value == "render") {
        print!("{}", render(&source));
        return;
    }
    let samples: usize = arguments.get(3).map_or(15, |value| value.parse().unwrap());
    let mut times = Vec::new();
    for index in 0..samples + 3 {
        let start = Instant::now();
        let output = black_box(render(black_box(&source)));
        let elapsed = start.elapsed().as_secs_f64() * 1000.0;
        black_box(output);
        if index >= 3 {
            times.push(elapsed);
        }
    }
    println!("{{\"samplesMs\":{times:?}}}");
}
