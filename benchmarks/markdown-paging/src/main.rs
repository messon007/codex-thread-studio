use comrak::{Arena, nodes::NodeValue};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::{
    env,
    fs::{self, File},
    io::{BufWriter, Write},
    path::Path,
    time::Instant,
};
use studio_markdown_paging_bench::{
    Heading, Lines, PageMeta, cached_window, options, outline, paginate, render_page,
    render_window, scratch,
};

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Cache {
    toc: Vec<Heading>,
    pages: Vec<PageMeta>,
    targets: Vec<usize>,
}

fn memory(field: &str) -> Option<usize> {
    fs::read_to_string("/proc/self/status")
        .ok()?
        .lines()
        .find_map(|line| {
            line.strip_prefix(field)
                .and_then(|value| value.split_whitespace().next()?.parse::<usize>().ok())
        })
}

fn elapsed(start: Instant) -> f64 {
    start.elapsed().as_secs_f64() * 1000.0
}

fn main() {
    let arguments: Vec<_> = env::args().collect();
    let directory = Path::new(&arguments[2]);
    if arguments[1] == "cache" {
        let start = Instant::now();
        let index = fs::read(directory.join("index.json")).unwrap();
        let cache: Cache = serde_json::from_slice(&index).unwrap();
        let index_ms = elapsed(start);
        let mut file = File::open(directory.join("pages.html")).unwrap();
        let mut windows = Vec::new();
        for target in &cache.targets {
            let start = Instant::now();
            let window = cached_window(
                &cache.pages,
                &mut file,
                &cache.toc,
                *target,
                2,
                *target == 0,
            );
            let bytes = serde_json::to_vec(&window).unwrap();
            windows.push(json!({ "targetPage": target, "ms": elapsed(start), "responseBytes": bytes.len(), "pages": window.pages.len() }));
        }
        println!(
            "{}",
            json!({ "indexReadMs": index_ms, "totalMs": elapsed(start), "windows": windows, "rssKiB": memory("VmRSS:"), "peakRssKiB": memory("VmHWM:") })
        );
        return;
    }
    fs::create_dir_all(directory).unwrap();
    let target_bytes: usize = arguments
        .get(3)
        .map_or(65_536, |value| value.parse().unwrap());
    let row_limit: usize = arguments.get(4).map_or(200, |value| value.parse().unwrap());
    let start = Instant::now();
    let source = fs::read_to_string(&arguments[1]).unwrap();
    let read_ms = elapsed(start);
    let rss_read = memory("VmRSS:");
    let arena = Arena::new();
    let options = options();
    let phase = Instant::now();
    let root = comrak::parse_document(&arena, &source, &options);
    let parse_ms = elapsed(phase);
    let rss_parse = memory("VmRSS:");
    let phase = Instant::now();
    let lines = Lines::new(&source);
    let mut pages = paginate(root, &lines, target_bytes, row_limit);
    let toc = outline(root, &lines, &pages);
    let index_ms = elapsed(phase);
    let toc_ready_ms = elapsed(start);
    let rss_index = memory("VmRSS:");
    let scratch = scratch(&arena);
    let phase = Instant::now();
    let initial = render_window(&pages, scratch, &options, &toc, 0, 2, true);
    let first_render_ms = elapsed(phase);
    let phase = Instant::now();
    let initial_bytes = serde_json::to_vec(&initial).unwrap();
    let first_serialize_ms = elapsed(phase);
    let first_ready_ms = elapsed(start);
    fs::write(directory.join("initial.json"), &initial_bytes).unwrap();
    let largest_table = root
        .children()
        .filter(|node| matches!(node.data().value, NodeValue::Table(_)))
        .max_by_key(|node| node.children().count());
    let table_line = largest_table
        .and_then(|node| node.children().nth(node.children().count() / 2))
        .map(|node| node.data().sourcepos.start.line)
        .unwrap_or(1);
    let table_page = pages
        .partition_point(|page| page.meta.start_line <= table_line)
        .saturating_sub(1);
    let targets = vec![
        0,
        toc.get(toc.len() / 2)
            .map_or(pages.len() / 2, |heading| heading.page),
        table_page,
        pages.len() - 1,
    ];
    let mut jumps = Vec::new();
    for (index, target) in targets.iter().enumerate().skip(1) {
        let phase = Instant::now();
        let window = render_window(&pages, scratch, &options, &toc, *target, 2, false);
        let bytes = serde_json::to_vec(&window).unwrap();
        jumps.push(json!({ "targetPage": target, "ms": elapsed(phase), "responseBytes": bytes.len(), "htmlBytes": window.pages.iter().map(|page| page.html.len()).sum::<usize>(), "pages": window.pages.len(), "startLine": window.pages.first().unwrap().meta.start_line, "endLine": window.pages.last().unwrap().meta.end_line }));
        fs::write(directory.join(format!("jump-{index}.json")), bytes).unwrap();
    }
    let nodes = root.descendants().count();
    let html_blocks = root
        .descendants()
        .filter(|node| matches!(node.data().value, NodeValue::HtmlBlock(_)))
        .count();
    let phase = Instant::now();
    let mut file = BufWriter::new(File::create(directory.join("pages.html")).unwrap());
    let mut offset = 0;
    let mut max_html = 0;
    for page in &mut pages {
        let html = render_page(page, scratch, &options);
        page.meta.html_offset = offset;
        page.meta.html_bytes = html.len();
        offset += html.len() as u64;
        max_html = max_html.max(html.len());
        file.write_all(html.as_bytes()).unwrap();
    }
    file.flush().unwrap();
    let cache_build_ms = elapsed(phase);
    let phase = Instant::now();
    let cache = Cache {
        toc: toc.clone(),
        pages: pages.iter().map(|page| page.meta.clone()).collect(),
        targets,
    };
    let bytes = serde_json::to_vec(&cache).unwrap();
    fs::write(directory.join("index.json"), &bytes).unwrap();
    let cache_index_ms = elapsed(phase);
    let tables = root
        .children()
        .filter(|node| matches!(node.data().value, NodeValue::Table(_)))
        .count();
    let largest_table_rows =
        largest_table.map_or(0, |node| node.children().count().saturating_sub(1));
    println!(
        "{}",
        json!({ "sourceBytes": source.len(), "targetPageBytes": target_bytes, "tableRowLimit": row_limit, "pages": pages.len(), "headings": toc.len(), "nodes": nodes, "tables": tables, "largestTableRows": largest_table_rows, "htmlBlocks": html_blocks, "rawPlantumlPresent": source.to_ascii_lowercase().contains("@startuml"), "oversizedPages": pages.iter().filter(|page| page.meta.source_bytes > target_bytes).count(), "readMs": read_ms, "parseMs": parse_ms, "indexMs": index_ms, "tocReadyMs": toc_ready_ms, "firstRenderMs": first_render_ms, "firstSerializeMs": first_serialize_ms, "firstReadyMs": first_ready_ms, "firstResponseBytes": initial_bytes.len(), "firstPages": initial.pages.len(), "firstHtmlBytes": initial.pages.iter().map(|page| page.html.len()).sum::<usize>(), "jumps": jumps, "cacheBuildMs": cache_build_ms, "cacheIndexMs": cache_index_ms, "cacheHtmlBytes": offset, "cacheIndexBytes": bytes.len(), "maxPageHtmlBytes": max_html, "rssAfterReadKiB": rss_read, "rssAfterParseKiB": rss_parse, "rssAfterIndexKiB": rss_index, "rssFinalKiB": memory("VmRSS:"), "peakRssKiB": memory("VmHWM:") })
    );
}
