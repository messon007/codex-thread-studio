use std::{env, fs, hint::black_box, time::Instant};

fn render(engine: &str, source: &str) -> String {
    match engine {
        "pulldown" => {
            let options = pulldown_cmark::Options::ENABLE_TABLES
                | pulldown_cmark::Options::ENABLE_STRIKETHROUGH
                | pulldown_cmark::Options::ENABLE_TASKLISTS;
            let events = pulldown_cmark::Parser::new_ext(source, options).map(|event| {
                if event == pulldown_cmark::Event::SoftBreak {
                    pulldown_cmark::Event::HardBreak
                } else {
                    event
                }
            });
            let mut html = String::new();
            pulldown_cmark::html::push_html(&mut html, events);
            html
        }
        "comrak" => {
            let mut options = comrak::Options::default();
            options.extension.table = true;
            options.extension.strikethrough = true;
            options.extension.tasklist = true;
            options.extension.autolink = true;
            options.render.hardbreaks = true;
            options.render.r#unsafe = true;
            comrak::markdown_to_html(source, &options)
        }
        _ => panic!("unknown engine: {engine}"),
    }
}

fn main() {
    let arguments: Vec<String> = env::args().collect();
    assert!(
        arguments.len() >= 4,
        "usage: bench ENGINE FILE render|bench [SAMPLES]"
    );
    let engine = &arguments[1];
    let source = fs::read_to_string(&arguments[2]).expect("read source");
    if arguments[3] == "render" {
        print!("{}", render(engine, &source));
        return;
    }
    assert_eq!(arguments[3], "bench");
    let samples: usize = arguments
        .get(4)
        .map_or(21, |value| value.parse().expect("samples"));
    assert!(samples > 0);
    let start = Instant::now();
    let output = black_box(render(engine, black_box(&source)));
    let first_ms = start.elapsed().as_secs_f64() * 1000.0;
    let output_bytes = output.len();
    drop(output);
    for _ in 0..5 {
        black_box(render(engine, black_box(&source)));
    }
    let mut times = Vec::with_capacity(samples);
    for _ in 0..samples {
        let start = Instant::now();
        let output = black_box(render(engine, black_box(&source)));
        times.push(start.elapsed().as_secs_f64() * 1000.0);
        assert_eq!(output.len(), output_bytes);
        drop(output);
    }
    println!(
        "{{\"firstMs\":{first_ms},\"inputBytes\":{},\"outputBytes\":{output_bytes},\"samplesMs\":{times:?},\"simdFeature\":{}}}",
        source.len(),
        cfg!(feature = "pulldown-simd")
    );
}

#[cfg(test)]
mod tests {
    use super::render;

    #[test]
    fn source_locations_preserve_utf8_and_crlf() {
        let source = "中文 😀\r\n\r\n## Heading\r\n";
        let heading_offset = source.find("## Heading").unwrap();
        let heading = pulldown_cmark::Parser::new(source)
            .into_offset_iter()
            .find(|(event, _)| {
                matches!(
                    event,
                    pulldown_cmark::Event::Start(pulldown_cmark::Tag::Heading { .. })
                )
            })
            .unwrap();
        assert_eq!(heading.1.start, heading_offset);
        assert!(source[heading.1].starts_with("## Heading"));
        let arena = comrak::Arena::new();
        let root = comrak::parse_document(&arena, source, &comrak::Options::default());
        let node = root
            .descendants()
            .find(|node| matches!(node.data().value, comrak::nodes::NodeValue::Heading(_)))
            .unwrap();
        assert_eq!(node.data().sourcepos.start.line, 3);
        assert_eq!(node.data().sourcepos.start.column, 1);
    }

    #[test]
    fn comparable_gfm_and_breaks() {
        for engine in ["pulldown", "comrak"] {
            let html = render(
                engine,
                "# 中文\n\nhello\nworld\n\n| A | B |\n| - | - |\n| a | b |\n\n- [x] ~~done~~\n\n```mermaid\ngraph TD; A-->B\n```\n\n<span>raw</span>\n",
            );
            for expected in [
                "<h1>中文</h1>",
                "<br",
                "<table>",
                "checked",
                "<del>done</del>",
                "language-mermaid",
                "<span>raw</span>",
            ] {
                assert!(html.contains(expected), "{engine}: missing {expected}");
            }
        }
    }
}
