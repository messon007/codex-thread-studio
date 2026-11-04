//! Display-free preparation measurements. Never opens a backend/profile or GTK.
#[cfg(target_os = "linux")]
mod probe {
    use codex_thread_studio_gnome::{run_math_worker_if_requested, text_flow};
    use serde_json::json;
    use std::{
        io::Read,
        sync::{atomic::AtomicBool, Arc},
        time::Instant,
    };
    use studio_markdown_native::{Fragment, FragmentMetadata, SourceWindow};

    fn fragment(text: String) -> Fragment {
        Fragment {
            page: 0,
            document_id: "admission-probe".into(),
            revision: "immutable-probe".into(),
            metadata: FragmentMetadata {
                start: 0,
                end: text.len(),
                line: 1,
                rows: 0,
                row_start: 0,
                table: None,
            },
            source: SourceWindow {
                start: 0,
                end: text.len(),
                line: 1,
                end_line: text.lines().count(),
                text,
                projection: vec![],
                references: vec![],
            },
            header: None,
        }
    }
    pub fn run() -> Result<(), Box<dyn std::error::Error>> {
        run_math_worker_if_requested();
        let mut cases = Vec::new();
        for kind in ["images", "diagrams", "tables", "math", "code"] {
            for count in [8, 32, 33, 64, 128] {
                let text = (0..count)
                    .map(|i| match kind {
                        "images" => format!("![Image {i}](image-{i}.svg)\n\n"),
                        "diagrams" => format!("```mermaid\nflowchart LR\nA{i}-->B{i}\n```\n\n"),
                        "tables" => {
                            format!(
                                "| Name | Value |\n|---|---|\n| {i} | 中文😀 |\n\nSection {i}.\n\n"
                            )
                        }
                        "math" => format!("Formula {i}: $x_{{{count},{i}}}^2$.\n\n"),
                        _ => format!("```rust\nlet value_{i} = \"中文😀\";\n```\n\n"),
                    })
                    .collect::<String>();
                cases.push((format!("{kind}-{count}"), text));
            }
        }
        if let Some(path) = std::env::args_os().nth(1) {
            let mut bytes = Vec::new();
            std::fs::File::open(path)?
                .take(65536)
                .read_to_end(&mut bytes)?;
            if let Some(last) = bytes.iter().rposition(|b| *b == b'\n') {
                bytes.truncate(last + 1);
            }
            cases.push((
                "real-report-prefix-through-complete-line-64KiB".into(),
                String::from_utf8(bytes)?,
            ));
        }
        let mut rows = Vec::new();
        for (case, text) in cases {
            let fragment = fragment(text);
            let started = Instant::now();
            let prepared =
                text_flow::prepare_lazy_diagrams(&fragment, Arc::new(AtomicBool::new(false)));
            let elapsed = started.elapsed().as_micros();
            rows.push(match prepared {
            Ok(p)=>json!({"case":case,"inputBytes":fragment.source.text.len(),"preparedUs":elapsed,"objects":p.document.objects.len(),"bufferBytes":p.document.buffer.len(),"rasterBytes":p.raster_bytes,"syntaxUs":p.syntax_us,"ok":true}),
            Err(error)=>json!({"case":case,"inputBytes":fragment.source.text.len(),"preparedUs":elapsed,"ok":false,"error":error}),
        });
        }
        println!(
            "{}",
            serde_json::to_string_pretty(
                &json!({"scope":"Rust preparation only; real file limited to first complete lines within 64KiB; no GTK/layout/transport","cases":rows})
            )?
        );
        Ok(())
    }
}
#[cfg(target_os = "linux")]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    probe::run()
}
#[cfg(not(target_os = "linux"))]
fn main() {
    eprintln!("Native text-flow admission probe requires Linux");
}
