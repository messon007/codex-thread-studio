use super::*;
fn window(text: &str, start: usize, line: usize) -> crate::SourceWindow {
    crate::SourceWindow {
        start,
        end: start + text.len(),
        line,
        end_line: line + text.bytes().filter(|b| *b == b'\n').count()
            - usize::from(text.ends_with('\n')),
        text: text.into(),
        projection: vec![],
        references: vec![],
    }
}
fn parts() -> Vec<Fragment> {
    let header = "| 名称😀 | Value |\r\n|---|---|\r\n";
    let bodies = [
        "| 中文 | $x^2$ |\r\n| [link](../next.md) | **bold** |\r\n",
        "| a\\|b | ![Pixel](nested/pixel.svg) |\r\n| last | `code` |\r\n",
    ];
    let mut offset = 2000;
    let mut line = 100;
    bodies
        .iter()
        .enumerate()
        .map(|(index, body)| {
            let source = window(body, offset, line);
            offset += body.len();
            line += body.bytes().filter(|b| *b == b'\n').count();
            Fragment {
                page: 17 + index,
                document_id: "snapshot-doc".into(),
                revision: "snapshot-revision".into(),
                metadata: crate::FragmentMetadata {
                    start: source.start,
                    end: source.end,
                    line: source.line,
                    rows: 2,
                    row_start: 50 + index * 2,
                    table: Some(3),
                },
                source,
                header: Some(window(header, 100, 8)),
            }
        })
        .collect()
}
fn blocks(parts: &[Fragment]) -> Vec<crate::Block> {
    parts
        .iter()
        .flat_map(|part| crate::parse(part).unwrap())
        .collect()
}
#[test]
fn indexed_context_joins_once_and_preserves_exact_cells_and_containing_lines() {
    let input = parts();
    let expected = blocks(&input);
    let projected = project_context(&input).unwrap();
    assert_eq!(projected.objects.len(), 1);
    let object = &projected.objects[0];
    let ObjectKind::Table {
        id,
        row_start,
        header,
        rows,
    } = &object.kind
    else {
        panic!("missing joined table")
    };
    assert_eq!((*id, *row_start, rows.len()), (Some(3), 50, 4));
    let mut expected_rows = Vec::new();
    for block in expected {
        let BlockKind::Table {
            header: original,
            rows: records,
            ..
        } = block.kind
        else {
            unreachable!()
        };
        assert_eq!(&original, header);
        expected_rows.extend(records);
    }
    assert_eq!(*rows, expected_rows);
    assert_eq!(rows[0].source.line, 100);
    assert_eq!(rows[3].source.line, 103);
    assert_eq!(rows[0].cells[1].content.math.len(), 1);
    assert!(rows[1].cells[0].content.markup.contains("../next.md"));
    assert_eq!(rows[2].cells[0].content.plain, "a|b");
    assert_eq!(rows[2].cells[1].content.images.len(), 1);
    let selected = projected
        .select(0..projected.characters, BUFFER_BYTES)
        .unwrap();
    assert_eq!(selected.text.matches("名称😀").count(), 1);
    let source = selected.source.unwrap();
    assert_eq!((source.line, source.end_line), (8, 103));
    assert_eq!(source.start, 100);
    assert_eq!(source.end, input[1].source.end);
    let mut found = Vec::new();
    QuoteScanner::new(&selected.text)
        .unwrap()
        .feed(&projected, |range, _| {
            found.push(range);
            true
        });
    assert_eq!(found, vec![source]);
}
#[test]
fn exact_adjacent_indexed_replay_is_idempotent_but_local_tables_stay_separate() {
    let mut input = blocks(&parts());
    input.insert(1, input[0].clone());
    let projected = project_blocks(input).unwrap();
    assert_eq!(projected.objects.len(), 1);
    assert!(matches!(&projected.objects[0].kind,ObjectKind::Table{rows,..} if rows.len()==4));
    let mut input = blocks(&parts());
    for block in &mut input {
        if let BlockKind::Table { id, .. } = &mut block.kind {
            *id = None;
        }
    }
    assert_eq!(project_blocks(input).unwrap().objects.len(), 2);
    let mut input = blocks(&parts());
    if let BlockKind::Table { id, .. } = &mut input[1].kind {
        *id = Some(4);
    }
    assert_eq!(project_blocks(input).unwrap().objects.len(), 2);
}
#[test]
fn inconsistent_indexed_continuations_never_silently_reorder_or_drop_rows() {
    for offset in [51, 53, usize::MAX] {
        let mut input = blocks(&parts());
        if let BlockKind::Table { row_start, .. } = &mut input[1].kind {
            *row_start = offset;
        }
        assert!(project_blocks(input).is_err());
    }
    let mut input = blocks(&parts());
    if let BlockKind::Table { header, .. } = &mut input[1].kind {
        std::sync::Arc::make_mut(&mut header[0].content).plain = "another header".into();
    }
    assert!(project_blocks(input).is_err());
    let mut input = blocks(&parts());
    if let BlockKind::Table { rows, .. } = &mut input[1].kind {
        rows[0].source.start = 1;
    }
    assert!(project_blocks(input).is_err());
    let mut input = blocks(&parts());
    let paragraph = crate::parse_text("Between.").unwrap().remove(0);
    input.insert(1, paragraph);
    assert!(project_blocks(input).is_err());
    let mut input = blocks(&parts());
    // A block can be byte-identical but belong to a different list/quote scope.
    let continuation = input.remove(1);
    input.push(crate::Block {
        source: continuation.source.clone(),
        kind: BlockKind::Quote(vec![continuation]),
    });
    assert!(project_blocks(input).is_err());
}
#[test]
fn context_identity_order_and_source_budgets_are_checked_before_projection() {
    for change in 0..5 {
        let mut input = parts();
        match change {
            0 => input[1].document_id = "other".into(),
            1 => input[1].revision = "new".into(),
            2 => input[1].page = input[0].page,
            3 => input[1].source = window("rewound", 0, 1),
            _ => {
                input[0].document_id.clear();
                input[1].document_id.clear();
            }
        }
        assert!(project_context(&input).is_err());
    }
    let mut too_many = vec![parts().remove(0); CONTEXT_FRAGMENTS + 1];
    for (index, part) in too_many.iter_mut().enumerate() {
        part.page = index;
    }
    assert!(project_context(&too_many).is_err());
    let mut too_large = vec![parts().remove(0); 9];
    for (index, part) in too_large.iter_mut().enumerate() {
        part.page = index;
        part.source = window(&"x".repeat(256 * 1024), index * 256 * 1024, 1);
    }
    assert!(project_context(&too_large).is_err());
    let mut too_many_rows = vec![parts().remove(0); 2];
    for (index, part) in too_many_rows.iter_mut().enumerate() {
        part.page = index;
        part.metadata.table = None;
        part.header = None;
        part.source = window(
            &"word\n\n".repeat(SEGMENTS / 2 + 1),
            index * 100_000,
            index * 10_000 + 1,
        );
    }
    assert!(project_context(&too_many_rows).is_err());
    assert_eq!(project_context(&[]).unwrap().characters, 0);
}
#[test]
fn table_work_and_header_only_alternatives_keep_explicit_bounds() {
    let mut input = blocks(&parts());
    if let BlockKind::Table { header, rows, .. } = &mut input[0].kind {
        rows.clear();
        header.truncate(1);
        std::sync::Arc::make_mut(&mut header[0].content).plain = "x".repeat(BUFFER_BYTES + 1);
    }
    input.truncate(1);
    assert!(project_blocks(input).is_err());
    let mut input = blocks(&parts());
    if let BlockKind::Table { header, rows, .. } = &mut input[0].kind {
        let cell = header[0].clone();
        header.clear();
        rows.truncate(1);
        rows[0].cells = vec![cell; tables::CELLS + 1];
    }
    input.truncate(1);
    assert!(project_blocks(input).is_err());
}
