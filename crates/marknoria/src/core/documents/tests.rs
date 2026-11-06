use super::*;
use zip::write::SimpleFileOptions;
fn zipped(path: &Path, files: &[(&str, &[u8])]) {
    let mut out = zip::ZipWriter::new(File::create(path).unwrap());
    for (name, data) in files {
        out.start_file(
            *name,
            SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated),
        )
        .unwrap();
        out.write_all(data).unwrap();
    }
    out.finish().unwrap();
}
fn index(path: &Path) -> (tempfile::TempDir, Index) {
    let d = tempfile::tempdir().unwrap();
    let i = crate::core::build_index(path, d.path()).unwrap();
    (d, i)
}
#[test]
fn plain_and_structured_files_preserve_source_and_exact_formatted_token_mapping() {
    for name in [
        "settings.json",
        "config.yaml",
        "a.toml",
        "file.xml",
        "code.rs",
        "plain.txt",
        ".env",
        "Dockerfile",
        "unknown.extension",
    ] {
        let d = tempfile::tempdir().unwrap();
        let path = d.path().join(name);
        let raw = "\u{feff}{\"value\":1e+04,\"name\":\"中文😀\",\"literal\":\"<script>\"}\r\n";
        fs::write(&path, raw).unwrap();
        let (cache, i) = index(&path);
        assert_eq!(i.format, DocumentFormat::Text);
        assert_eq!(
            fs::read_to_string(cache.path().join("source.md")).unwrap(),
            raw
        );
        assert_eq!(i.revision, hash(raw.as_bytes()));
        for p in &i.pages {
            assert!(p.bytes <= crate::core::MAX_HTML);
            assert!(raw.is_char_boundary(p.start));
            assert!(raw.is_char_boundary(p.end));
        }
        let html = crate::core::read_page(cache.path(), &i, 0).unwrap();
        assert!(!html.contains("<script>"));
        assert!(html.contains("中文😀"));
        assert_eq!(
            crate::core::source::read_source(cache.path(), &i, None, None, false)
                .unwrap()
                .text,
            raw
        );
        crate::core::audit_tables(cache.path(), &i).unwrap();
    }
    let d = tempfile::tempdir().unwrap();
    let p = d.path().join("pretty.json");
    fs::write(&p, "{\"n\":1e+04,\"x\":9223372036854775807}").unwrap();
    let (c, i) = index(&p);
    let b = crate::core::blocks::read_blocks(c.path(), &i, 0).unwrap();
    assert!(
        matches!(&b[0],crate::core::blocks::Block::Code{text,..} if text.contains("1e+04")&&text.contains("9223372036854775807")&&text.contains('\n'))
    );
}

#[test]
fn large_json_streams_readable_pages_with_original_spelling_and_anchors() {
    let d = tempfile::tempdir().unwrap();
    let path = d.path().join("large.json");
    let raw = format!("[{}]", (0..16000).map(|n| format!(r#"{{"id":{n},"n":1e+04,"n":90071992547409931234,"text":"中文😀\u0061"}}"#)).collect::<Vec<_>>().join(","));
    assert!(raw.len() > structured::FORMAT_BYTES);
    fs::write(&path, &raw).unwrap();
    let (cache, i) = index(&path);
    let display = fs::read_to_string(cache.path().join("pages.txt")).unwrap();
    assert!(display.contains("\n    \"id\": 15999,"));
    assert_eq!(display.matches("1e+04").count(), 16000);
    assert_eq!(display.matches("90071992547409931234").count(), 16000);
    assert!(display.contains(r#""text": "中文😀\u0061""#));
    assert!(!cache.path().join("preview-note.txt").exists());
    for page in [0, i.pages.len() / 2, i.pages.len() - 1] {
        let text = crate::core::read_cached_page(cache.path(), &i, page).unwrap();
        for anchor in projection::read(cache.path(), &i, page, &text).unwrap() {
            assert_eq!(&text[anchor.display.clone()], &raw[anchor.original..anchor.original + anchor.display.len()]);
        }
        let html = crate::core::read_page(cache.path(), &i, page).unwrap();
        assert!(html.contains("data-source-start="));
        assert!(html.len() <= crate::core::MAX_HTML);
    }
    assert_eq!(fs::read_to_string(cache.path().join("source.md")).unwrap(), raw);
    assert_eq!(fs::read_to_string(path).unwrap(), raw);
}

#[test]
fn invalid_or_budget_limited_json_keeps_source_with_cached_feedback() {
    let d = tempfile::tempdir().unwrap();
    for (n, raw) in [
        "{\"a\": 1 \"b\": 2}".to_string(),
        "[1,]".to_string(),
        "[1,\u{00a0}2]".to_string(),
        "[\"line\nfeed\"]".to_string(),
        "{\"a\": [1, 2}".to_string(),
        format!("{}0{}", "[".repeat(129), "]".repeat(129)),
        format!("{{\"long\":\"{}\"}}", "\\u0061".repeat(50000)),
        format!("[{},]", "0,".repeat(150000)),
    ].into_iter().enumerate() {
        let path = d.path().join(format!("invalid{n}.json"));
        fs::write(&path, &raw).unwrap();
        let (cache, i) = index(&path);
        assert_eq!(fs::read_to_string(cache.path().join("pages.txt")).unwrap(), raw);
        let html = crate::core::read_page(cache.path(), &i, 0).unwrap();
        assert!(html.contains("preview shows original source:"));
        for page in 0..i.pages.len() {
            let text = crate::core::read_cached_page(cache.path(), &i, page).unwrap();
            for anchor in projection::read(cache.path(), &i, page, &text).unwrap() {
                assert_eq!(&text[anchor.display.clone()], &raw[anchor.original..anchor.original + anchor.display.len()]);
            }
        }
    }
}

#[test]
fn unformatted_source_pages_retain_line_anchors() {
    let d = tempfile::tempdir().unwrap();
    let path = d.path().join("lines.txt");
    fs::write(&path, "one\n中文😀\nthree\n").unwrap();
    let (cache, i) = index(&path);
    let text = crate::core::read_cached_page(cache.path(), &i, 0).unwrap();
    let anchors = projection::read(cache.path(), &i, 0, &text).unwrap();
    assert_eq!(anchors.iter().map(|a| a.line).collect::<Vec<_>>(), [1, 2, 3]);
    let html = crate::core::read_page(cache.path(), &i, 0).unwrap();
    assert!(html.contains("data-source-line=\"1\" data-source-end-line=\"1\""));
    assert!(html.contains("data-source-line=\"2\" data-source-end-line=\"2\""));
}

#[test]
fn jsonl_formats_beyond_the_whole_file_budget_and_packs_records_without_losing_source() {
    let d=tempfile::tempdir().unwrap();let path=d.path().join("records.JSONL");
    let raw=(0..12000).map(|n|format!("{{\"id\":{n},\"text\":\"中文😀\",\"nested\":{{\"x\":true}}}}\r\n")).collect::<String>();
    assert!(raw.len()>structured::FORMAT_BYTES);fs::write(&path,&raw).unwrap();
    let (cache,i)=index(&path);assert!(i.pages.len()<raw.len()/1000);
    let mut displayed=String::new();
    for (page,metadata) in i.pages.iter().enumerate(){assert!(metadata.bytes<=crate::core::MAX_HTML);assert!(metadata.start<=metadata.end&&metadata.end<=raw.len());
        let blocks=crate::core::blocks::read_blocks(cache.path(),&i,page).unwrap();
        for block in blocks{match block{crate::core::blocks::Block::Code{document:true,text,..}=>displayed.push_str(&text),other=>panic!("{other:?}")}}
    }
    assert!(displayed.contains("\n  \"id\": 11999,"));assert!(displayed.contains("\n    \"x\": true"));
    let records=serde_json::Deserializer::from_str(&displayed).into_iter::<serde_json::Value>().collect::<Result<Vec<_>,_>>().unwrap();assert_eq!(records.len(),12000);assert_eq!(records[11999]["id"],11999);
    assert_eq!(fs::read_to_string(cache.path().join("source.md")).unwrap(),raw);assert_eq!(fs::read_to_string(path).unwrap(),raw);
}

#[test]
fn dense_jsonl_caches_text_and_styles_only_requested_pages() {
    let d = tempfile::tempdir().unwrap();
    let path = d.path().join("dense.jsonl");
    let record = format!("{{\"values\":[{}],\"text\":\"<&中文😀\"}}\r\n", vec!["1";64].join(","));
    let raw = record.repeat(12000);
    fs::write(&path, &raw).unwrap();
    let (cache, index) = index(&path);
    assert!(index.text_pages);
    let display = fs::read_to_string(cache.path().join("pages.txt")).unwrap();
    assert!(display.len() <= raw.len() * 4);
    assert!(display.contains("\n    1,"));
    assert!(!cache.path().join("pages.html").exists());
    assert_eq!(serde_json::Deserializer::from_str(&display).into_iter::<serde_json::Value>().count(),12000);
    let mut reconstructed = String::new();
    for page in 0..index.pages.len() {
        assert!(index.pages[page].bytes <= TEXT_PAGE_BYTES);
        let blocks=crate::core::blocks::read_blocks(cache.path(), &index, page).unwrap();
        let crate::core::blocks::Block::Code { document:true,text,.. }=&blocks[0] else {panic!("expected document text")};
        reconstructed.push_str(text);
    }
    assert_eq!(reconstructed,display);
    for page in [0,index.pages.len()/2,index.pages.len()-1] {
        let html=crate::core::read_page(cache.path(),&index,page).unwrap();
        assert!(html.len()<=crate::core::MAX_HTML);
        assert!(html.contains("mn-code"));
        assert!(!html.contains("<&中文"));
    }
    assert_eq!(fs::read_to_string(cache.path().join("source.md")).unwrap(),raw);
}

#[test]
fn jsonl_indentation_expansion_falls_back_without_losing_rows_or_anchors() {
    let d=tempfile::tempdir().unwrap();let path=d.path().join("nested.ndjson");
    let record=format!("{}{{\"id\":1}}{}\r\n","[".repeat(60),"]".repeat(60));
    let raw=record.repeat(2000);fs::write(&path,&raw).unwrap();
    let(cache,index)=index(&path);
    let display=fs::read_to_string(cache.path().join("pages.txt")).unwrap();
    assert_eq!(display,raw);
    assert_eq!(index.pages.first().unwrap().start,0);
    assert_eq!(index.pages.last().unwrap().end,raw.len());
    for pair in index.pages.windows(2) { assert_eq!(pair[0].end,pair[1].start); }
    assert_eq!(serde_json::Deserializer::from_str(&display).into_iter::<serde_json::Value>().filter(|r|r.is_ok()).count(),2000);
}

#[test]
fn json_xml_and_code_cache_slices_do_not_become_independent_code_fences() {
    let d=tempfile::tempdir().unwrap();
    for (name,raw) in [
        ("tree.json",format!("{{\"items\":[{}]}}",(0..1200).map(|n|format!("{{\"id\":{n},\"name\":\"中文😀\"}}")).collect::<Vec<_>>().join(","))),
        ("tree.xml",format!("<root>{}</root>","<child><name>中文😀</name><value>1</value></child>".repeat(1200))),
        ("code.py","def check():\n    return '中文😀'\n\n".repeat(1200)),
    ] {
        let path=d.path().join(name);fs::write(&path,&raw).unwrap();let(cache,i)=index(&path);assert!(i.pages.len()>1);
        let expected=structured::Preview::prepare(name,&TextDocument::new(raw.clone()).unwrap()).map_or_else(||raw.clone(),|p|p.document.as_str().to_owned());
        let mut text=String::new();
        for page in 0..i.pages.len(){for block in crate::core::blocks::read_blocks(cache.path(),&i,page).unwrap(){match block{crate::core::blocks::Block::Code{document:true,text:s,..}=>text.push_str(&s),other=>panic!("{other:?}")}}}
        assert_eq!(text,expected,"{name}");assert_eq!(fs::read_to_string(path).unwrap(),raw);
    }
}
#[test]
fn tsv_keeps_multiline_records_and_queries_every_row() {
    let d = tempfile::tempdir().unwrap();
    let p = d.path().join("a.TSV");
    let raw = "ID\tValue\tNotes\r\n001\t12\t\"a\r\nb😀\"\r\n002\t3\tx\r\n";
    fs::write(&p, raw).unwrap();
    let (c, i) = index(&p);
    assert_eq!(i.format, DocumentFormat::Tsv);
    assert_eq!(i.tables[0].rows, 2);
    crate::core::audit_tables(c.path(), &i).unwrap();
    let result = crate::core::table_query::Queries::default()
        .run(
            c.path(),
            &i,
            0,
            "SELECT c1,c3,_line FROM t WHERE c2='12'",
            Arc::new(std::sync::atomic::AtomicBool::new(false)),
        )
        .unwrap();
    assert_eq!(
        result.rows[0],
        vec![
            Some("001".into()),
            Some("a\r\nb😀".into()),
            Some("2".into())
        ]
    );
    assert_eq!(fs::read_to_string(p).unwrap(), raw);
}
#[test]
fn static_html_is_inert_and_source_remains_exact() {
    let d = tempfile::tempdir().unwrap();
    let p = d.path().join("a.html");
    let raw = "<h1 onclick=\"bad()\">Title</h1><script>secret</script><style>bad</style><p>Hello <strong>中文😀</strong><img src=\"file:///etc/passwd\" onerror=\"bad()\" alt=\"local\"></p><a href=\"javascript:bad()\">unsafe</a><a href=\"https://example.com\">safe</a>";
    fs::write(&p, raw).unwrap();
    let (c, i) = index(&p);
    let html = crate::core::read_page(c.path(), &i, 0).unwrap();
    for forbidden in [
        "secret",
        "<script",
        "onclick",
        "onerror",
        "file:",
        "javascript:",
        "<style",
    ] {
        assert!(!html.contains(forbidden), "{html}");
    }
    assert!(html.contains("<strong>中文😀</strong>"));
    assert_eq!(
        crate::core::source::read_source(c.path(), &i, None, None, false)
            .unwrap()
            .text,
        raw
    );
}
#[test]
fn images_are_bounded_oriented_and_use_private_png_frames() {
    let d = tempfile::tempdir().unwrap();
    for ext in ["png", "jpg", "gif", "webp"] {
        let p = d.path().join(format!("a.{ext}"));
        let bitmap = image::RgbaImage::from_pixel(12, 8, image::Rgba([255, 0, 0, 255]));
        image::DynamicImage::ImageRgba8(bitmap).save(&p).unwrap();
        let original = fs::read(&p).unwrap();
        let (c, i) = index(&p);
        assert_eq!(i.format, DocumentFormat::Image);
        assert_eq!(i.original_bytes, Some(original.len()));
        let frame = asset(c.path(), &i, 0, 0).unwrap();
        let decoded = image::open(frame).unwrap();
        assert_eq!((decoded.width(), decoded.height()), (12, 8));
        assert_eq!(fs::read(p).unwrap(), original);
        assert!(asset(c.path(), &i, 1, 0).is_err());
    }
    let p = d.path().join("a.svg");
    fs::write(&p,"<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"40\" height=\"20\"><rect width=\"40\" height=\"20\" fill=\"red\"/><image href=\"file:///etc/passwd\" width=\"1\" height=\"1\"/></svg>").unwrap();
    let (c, i) = index(&p);
    assert!(asset(c.path(), &i, 0, 0).unwrap().is_file());
    assert_eq!((i.assets[0].width, i.assets[0].height), (40, 20));
    assert!(
        !fs::read_to_string(c.path().join("source.md"))
            .unwrap()
            .contains("passwd")
    );
}
#[test]
fn xlsx_streams_sparse_cells_without_executing_formulas_and_keeps_all_sheets() {
    let d = tempfile::tempdir().unwrap();
    let p = d.path().join("a.xlsx");
    let types=b"<Types xmlns=\"http://schemas.openxmlformats.org/package/2006/content-types\"><Override PartName=\"/xl/workbook.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml\"/></Types>";
    let workbook=b"<workbook xmlns=\"http://schemas.openxmlformats.org/spreadsheetml/2006/main\" xmlns:r=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships\"><sheets><sheet name=\"Data\" sheetId=\"1\" r:id=\"rId1\"/><sheet name=\"Empty\" sheetId=\"2\" r:id=\"rId2\"/></sheets></workbook>";
    let rel=b"<Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\"><Relationship Id=\"rId1\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet\" Target=\"worksheets/sheet1.xml\"/><Relationship Id=\"rId2\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet\" Target=\"worksheets/sheet2.xml\"/></Relationships>";
    let sheet=b"<worksheet xmlns=\"http://schemas.openxmlformats.org/spreadsheetml/2006/main\"><dimension ref=\"A1:XFD1048576\"/><sheetData><row r=\"1\"><c r=\"A1\" t=\"inlineStr\"><is><t>ID</t></is></c><c r=\"B1\" t=\"inlineStr\"><is><t>Value</t></is></c></row><row r=\"2\"><c r=\"A2\" t=\"inlineStr\"><is><t>001</t></is></c><c r=\"B2\"><v>12</v></c></row><row r=\"3\"><c r=\"A3\" t=\"inlineStr\"><is><t>002</t></is></c><c r=\"B3\"><f>B2+1</f><v>13</v></c></row></sheetData></worksheet>";
    let package_rels = b"<Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\"><Relationship Id=\"root\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument\" Target=\"xl/workbook.xml\"/></Relationships>";
    zipped(&p,&[("_rels/.rels",package_rels),("[Content_Types].xml",types),("xl/workbook.xml",workbook),("xl/_rels/workbook.xml.rels",rel),("xl/worksheets/sheet1.xml",sheet),("xl/worksheets/sheet2.xml",b"<worksheet xmlns=\"http://schemas.openxmlformats.org/spreadsheetml/2006/main\"><sheetData/></worksheet>")]);
    let original = fs::read(&p).unwrap();
    let (c, i) = index(&p);
    assert_eq!(i.tables.len(), 2);
    assert_eq!(i.tables[0].label.as_deref(), Some("Data"));
    assert_eq!(i.tables[0].rows, 2);
    assert_eq!(i.tables[1].rows, 0);
    crate::core::audit_tables(c.path(), &i).unwrap();
    let result = crate::core::table_query::Queries::default()
        .run(
            c.path(),
            &i,
            0,
            "SELECT SUM(CAST(c2 AS INTEGER)),MIN(c1),MAX(_line) FROM t",
            Arc::new(std::sync::atomic::AtomicBool::new(false)),
        )
        .unwrap();
    assert_eq!(
        result.rows[0],
        vec![Some("25".into()), Some("001".into()), Some("3".into())]
    );
    assert_eq!(fs::read(p).unwrap(), original);
}
#[test]
fn xlsx_dates_and_uncached_formulas_keep_studio_cell_text_semantics() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("types.xlsx");
    let types = br#"<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Override PartName="/xl/workbook.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml"/></Types>"#;
    let workbook = br#"<workbook xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"><sheets><sheet name="Types" sheetId="1" r:id="r1"/></sheets></workbook>"#;
    let relationships = br#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="r1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet" Target="worksheets/sheet1.xml"/></Relationships>"#;
    let package = br#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="root" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="xl/workbook.xml"/></Relationships>"#;
    let styles = br#"<styleSheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><cellXfs count="2"><xf numFmtId="0"/><xf numFmtId="22"/></cellXfs></styleSheet>"#;
    let sheet = br#"<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><sheetData><row r="1"><c r="A1" t="inlineStr"><is><t>Date</t></is></c><c r="B1" t="inlineStr"><is><t>Uncached</t></is></c><c r="C1" t="inlineStr"><is><t>Cached</t></is></c><c r="D1" t="inlineStr"><is><t>Code</t></is></c></row><row r="2"><c r="A2" s="1"><v>45293.12783564815</v></c><c r="B2"><f>HYPERLINK("https://forbidden.invalid/track","literal")</f></c><c r="C2"><f>1+1</f><v>2</v></c><c r="D2" t="inlineStr"><is><t>001</t></is></c></row></sheetData></worksheet>"#;
    zipped(&path, &[("_rels/.rels", package), ("[Content_Types].xml", types), ("xl/workbook.xml", workbook), ("xl/_rels/workbook.xml.rels", relationships), ("xl/styles.xml", styles), ("xl/worksheets/sheet1.xml", sheet)]);
    let original = fs::read(&path).unwrap();
    let (cache, index) = index(&path);
    let result = crate::core::table_query::Queries::default().run(cache.path(), &index, 0,
        "SELECT c1,c2,c3,c4 FROM t", Arc::new(std::sync::atomic::AtomicBool::new(false))).unwrap();
    assert_eq!(result.rows[0], [Some("2024-01-02T03:04:05.000Z".into()),
        Some("HYPERLINK(\"https://forbidden.invalid/track\",\"literal\")".into()), Some("2".into()), Some("001".into())]);
    crate::core::audit_tables(cache.path(), &index).unwrap();
    let display = crate::core::read_table(cache.path(), &index, 0, 0, &[0,1,2,3]).unwrap();
    assert!(display.html.contains("2024-01-02T03:04:05.000Z"));
    assert!(!display.html.contains("<a "));
    assert_eq!(fs::read(path).unwrap(), original);
}

#[test]
fn epub_follows_spine_and_rejects_traversal() {
    let d = tempfile::tempdir().unwrap();
    let p = d.path().join("book.epub");
    zipped(&p,&[("mimetype",b"application/epub+zip"),("META-INF/container.xml",b"<container><rootfiles><rootfile full-path=\"OPS/book.opf\"/></rootfiles></container>"),("OPS/book.opf",b"<package><manifest><item id=\"b\" href=\"second.xhtml\"/><item id=\"a\" href=\"first.xhtml\"/></manifest><spine><itemref idref=\"a\"/><itemref idref=\"b\"/></spine></package>"),("OPS/first.xhtml",b"<html><body><h1>First</h1><p>EPUB sentinel one</p><img src=\"missing.png\" alt=\"Missing cover\"/><img src=\"https://forbidden.invalid/a.png\" alt=\"External cover\"/><script>must-not-run</script></body></html>"),("OPS/second.xhtml",b"<html><body><h1>Second</h1><p>EPUB sentinel two</p></body></html>")]);
    let (c, i) = index(&p);
    assert_eq!(i.toc[0].label, "First");
    assert_eq!(i.toc[1].label, "Second");
    assert_eq!(
        crate::core::search(c.path(), &i, "sentinel two").unwrap().len(),
        1
    );
    assert!(
        !fs::read_to_string(c.path().join("pages.html"))
            .unwrap()
            .contains("must-not-run")
    );
    let p = d.path().join("unsafe.epub");
    zipped(&p, &[("../escape", b"unsafe")]);
    let cache = d.path().join("cache");
    assert!(crate::core::build_index(&p, &cache).is_err());
    assert!(!d.path().join("escape").exists());
}

#[test]
fn archive_names_with_invalid_utf8_are_rejected_without_panicking() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("invalid-name.epub");
    let name = "中文.txt";
    zipped(&path, &[(name, b"small public payload")]);
    let mut bytes = fs::read(&path).unwrap();
    let positions: Vec<_> = bytes.windows(name.len()).enumerate()
        .filter_map(|(at, part)| (part == name.as_bytes()).then_some(at)).collect();
    assert_eq!(positions.len(), 2, "Expected local and central UTF-8 names");
    for at in positions { bytes[at] = 0xff; }
    // Explicitly declare UTF-8 in both headers; CP437 is otherwise legal ZIP.
    let flags: Vec<_> = bytes.windows(4).enumerate().filter_map(|(at, magic)| {
        if magic == b"PK\x03\x04" { Some(at + 7) }
        else if magic == b"PK\x01\x02" { Some(at + 9) }
        else { None }
    }).collect();
    assert_eq!(flags.len(), 2);
    for at in flags { bytes[at] |= 0x08; }
    fs::write(&path, bytes).unwrap();
    let result = crate::core::build_index(&path, &root.path().join("cache"));
    assert!(result.is_err());
    assert!(!root.path().join(name).exists());
}

#[test]
fn unicode_epub_entry_names_remain_virtual_on_every_platform() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("unicode.epub");
    zipped(&path, &[
        ("META-INF/container.xml", b"<container><rootfiles><rootfile full-path=\"OPS/book.opf\"/></rootfiles></container>"),
        ("OPS/book.opf", "<package><manifest><item id=\"a\" href=\"é😀.xhtml\"/></manifest><spine><itemref idref=\"a\"/></spine></package>".as_bytes()),
        ("OPS/é😀.xhtml", b"<html><body><h1>Unicode chapter</h1><p>Unicode archive sentinel</p></body></html>"),
    ]);
    let (cache, index) = index(&path);
    assert_eq!(crate::core::search(cache.path(), &index, "archive sentinel").unwrap().len(), 1);
    assert!(!root.path().join("OPS").exists());
}
