use super::*;
fn sample() -> Projection {
    let mut blocks = crate::parse_text(
        "Before **中文😀**.\n\n| H | V |\n|---|---|\n| partial | x |\n\nAfter.\n",
    )
    .unwrap();
    for block in &mut blocks {
        if let BlockKind::Table { id, .. } = &mut block.kind {
            *id = Some(4);
        }
    }
    let last = blocks.last_mut().unwrap();
    last.source = SourceRange {
        start: 600,
        end: 606,
        line: 60,
        end_line: 60,
    };
    project_blocks(blocks).unwrap()
}
fn table() -> TableText {
    TableText {
        id: 4,
        text: "H\tV\npartial\tx\nlast\tvalue".into(),
        source: SourceRange {
            start: sample().segments[1].source.start,
            end: 500,
            line: 3,
            end_line: 50,
        },
    }
}
#[test]
fn whole_table_resolution_keeps_surrounding_unicode_and_original_source_extent() {
    let p = sample();
    let range = 0..p.characters;
    let request = p.selection_request(range.clone(), 1024).unwrap();
    assert_eq!(request.tables, vec![4]);
    let base = "Before 中文😀.\n\n\n\nAfter.";
    assert_eq!(request.remaining, 1024 - base.len());
    let selected = p.select_resolved(range, 1024, &[table()]).unwrap();
    assert_eq!(
        selected.text,
        format!("Before 中文😀.\n\n{}\n\nAfter.", table().text)
    );
    assert_eq!(
        selected.source.unwrap(),
        SourceRange {
            start: 0,
            end: 606,
            line: 1,
            end_line: 60
        }
    );
    let object = &p.objects[0];
    assert_eq!(
        p.select_resolved(object.characters.clone(), 1024, &[table()])
            .unwrap()
            .source,
        Some(table().source)
    );
    assert!(p
        .selection_request(0..object.characters.start, 1024)
        .unwrap()
        .tables
        .is_empty());
}
#[test]
fn resolved_selection_never_accepts_missing_foreign_or_partial_tables_or_truncation() {
    let p = sample();
    let range = 0..p.characters;
    assert!(p.select_resolved(range.clone(), 1024, &[]).is_err());
    let mut wrong = table();
    wrong.id = 9;
    assert!(p.select_resolved(range.clone(), 1024, &[wrong]).is_err());
    assert!(p
        .select_resolved(range.clone(), 1024, &[table(), table()])
        .is_err());
    let mut short = table();
    short.source.end = 21;
    assert!(p.select_resolved(range.clone(), 1024, &[short]).is_err());
    assert!(p.select_resolved(range.clone(), 10, &[table()]).is_err());
    let mut long = table();
    long.text = "x".repeat(1025);
    assert!(p.select_resolved(range, 1024, &[long]).is_err());
    // Invalid selection bounds are data, not a range to iterate.
    let reversed = std::ops::Range { start: 2, end: 1 };
    assert!(p.selection_request(reversed, 1024).is_err());
}
#[test]
fn full_table_quotes_are_atomic_and_large_skips_do_not_bridge_neighbors() {
    use crate::quote::{Anchor, Kind, Locator, Query, Status};
    let before = project_blocks(crate::parse_text("Before.").unwrap()).unwrap();
    let mut blocks = crate::parse_text("After.").unwrap();
    blocks[0].source = SourceRange {
        start: 600,
        end: 606,
        line: 60,
        end_line: 60,
    };
    let after = project_blocks(blocks).unwrap();
    let locate = |text: &str, skip: bool| {
        let mut locator = Locator::new(
            Query {
                quote: text.into(),
                anchor: Anchor {
                    text_kind: Kind::Flow,
                    ..Anchor::default()
                },
            },
            "revision",
        )
        .unwrap();
        locator.flow(&before);
        if skip {
            locator.flow_boundary();
        } else {
            locator.flow_table(table());
        }
        locator.flow(&after);
        locator.finish()
    };
    let full = format!("Before.\n\n{}\n\nAfter.", table().text);
    assert_eq!(locate(&full, false).status, Status::Unique);
    assert_eq!(locate("partial", false).status, Status::Missing);
    assert_eq!(locate("Before.\n\nAfter.", true).status, Status::Missing);
    assert_eq!(locate("After.", true).status, Status::Unique);
}
