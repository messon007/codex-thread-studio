//! Resumable Unicode prefixes for selections containing deferred indexed tables.
//! No toolkit offsets, full-table copy or original-source mutation is required.
use super::{ObjectKind, Projection, BUFFER_BYTES};
use std::{ops::Range, sync::Arc};

#[derive(Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TablePrefix {
    pub id: usize,
    pub text: String,
    pub complete: bool,
}
#[derive(Debug)]
pub struct PrefixResult {
    pub text: String,
    pub truncated: bool,
}
#[derive(Debug)]
pub enum PrefixStep {
    Table { id: usize, characters: usize },
    Complete(PrefixResult),
}
pub struct Prefix {
    source: Arc<Projection>,
    range: Range<usize>,
    position: usize,
    byte: usize,
    object: usize,
    remaining: usize,
    text: String,
    pending: Option<usize>,
    partial: bool,
    finished: bool,
}
impl Prefix {
    pub fn new(
        source: Arc<Projection>,
        range: Range<usize>,
        characters: usize,
    ) -> Result<Self, String> {
        if range.start > range.end || range.end > source.characters || characters > BUFFER_BYTES / 4
        {
            return Err("Invalid projected prefix range or budget".into());
        }
        let byte = source
            .buffer
            .char_indices()
            .nth(range.start)
            .map_or(source.buffer.len(), |(byte, _)| byte);
        let object = source
            .objects
            .partition_point(|o| o.characters.end <= range.start);
        Ok(Self {
            position: range.start,
            range,
            byte,
            object,
            remaining: characters,
            text: String::new(),
            pending: None,
            partial: false,
            finished: false,
            source,
        })
    }
    /// Call again with exactly the requested table's bounded prefix. Complete
    /// Copy/comment capture continues to use Projection::select_resolved instead.
    pub fn step(&mut self, table: Option<TablePrefix>) -> Result<PrefixStep, String> {
        if self.finished {
            return Err("Projected prefix was already consumed".into());
        }
        if let Some(id) = self.pending {
            let table = table.ok_or("Projected prefix needs its table response")?;
            let count = table.text.chars().count();
            if table.id != id
                || table.text.len() > self.remaining.saturating_mul(4)
                || count > self.remaining
                || (!table.complete && count != self.remaining)
            {
                return Err("Invalid projected table prefix response".into());
            }
            self.remaining -= count;
            self.text.push_str(&table.text);
            self.partial |= !table.complete;
            self.pending = None;
            self.advance_object()?;
        } else if table.is_some() {
            return Err("Unexpected projected table prefix response".into());
        }
        while self.position < self.range.end && self.remaining > 0 {
            if let Some(object) = self
                .source
                .objects
                .get(self.object)
                .filter(|o| o.characters.start == self.position)
            {
                if let ObjectKind::Table { id: Some(id), .. } = object.kind {
                    self.pending = Some(id);
                    return Ok(PrefixStep::Table {
                        id,
                        characters: self.remaining,
                    });
                }
                for character in object.alternative.chars() {
                    if self.remaining == 0 {
                        self.partial = true;
                        break;
                    }
                    self.text.push(character);
                    self.remaining -= 1;
                }
                self.advance_object()?;
            } else {
                let character = self
                    .source
                    .buffer
                    .get(self.byte..)
                    .ok_or("Invalid projected prefix byte position")?
                    .chars()
                    .next()
                    .ok_or("Incomplete projected prefix buffer")?;
                self.text.push(character);
                self.remaining -= 1;
                self.byte += character.len_utf8();
                self.position += 1;
            }
        }
        self.finished = true;
        Ok(PrefixStep::Complete(PrefixResult {
            text: std::mem::take(&mut self.text),
            truncated: self.partial || self.position < self.range.end,
        }))
    }
    fn advance_object(&mut self) -> Result<(), String> {
        if !self
            .source
            .buffer
            .get(self.byte..)
            .is_some_and(|s| s.starts_with(super::ANCHOR))
        {
            return Err("Projected prefix object has no source anchor".into());
        }
        self.byte += super::ANCHOR.len_utf8();
        self.position += 1;
        self.object += 1;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Fragment, FragmentMetadata, SourceWindow};
    fn projection() -> Arc<Projection> {
        let text="# Heading\n\n汉😀 $x^2$ literal \u{fffc}.\n\n| A | B |\n|---|---|\n| 1 | 2 |\n\nTail.\n";
        let fragment = Fragment {
            document_id: "prefix".into(),
            revision: "a".repeat(64),
            page: 0,
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
                end_line: 10,
                text: text.into(),
                projection: Vec::new(),
                references: Vec::new(),
            },
            header: None,
        };
        let mut projection = super::super::project(&fragment).unwrap();
        for object in &mut projection.objects {
            if let ObjectKind::Table { id, .. } = &mut object.kind {
                *id = Some(7);
            }
        }
        Arc::new(projection)
    }
    #[test]
    fn every_unicode_prefix_matches_complete_projection_without_raw_anchors() {
        let source = projection();
        let table = "A\tB\n1\t2\nExtra\t汉😀 row";
        let full = source
            .select_resolved(
                0..source.characters,
                BUFFER_BYTES,
                &[super::super::TableText {
                    id: 7,
                    text: table.into(),
                    source: crate::SourceRange {
                        start: 0,
                        end: 1000,
                        line: 1,
                        end_line: 100,
                    },
                }],
            )
            .unwrap()
            .text;
        for length in 0..=full.chars().count() + 1 {
            let mut prefix = Prefix::new(source.clone(), 0..source.characters, length).unwrap();
            let mut next = prefix.step(None).unwrap();
            loop {
                match next {
                    PrefixStep::Table { id, characters } => {
                        let text = table.chars().take(characters).collect::<String>();
                        next = prefix
                            .step(Some(TablePrefix {
                                id,
                                text,
                                complete: table.chars().count() <= characters,
                            }))
                            .unwrap();
                    }
                    PrefixStep::Complete(result) => {
                        assert_eq!(result.text, full.chars().take(length).collect::<String>());
                        assert_eq!(result.truncated, length < full.chars().count());
                        break;
                    }
                }
            }
        }
    }
    #[test]
    fn table_identity_and_partial_response_cannot_silently_skip_text() {
        let source = projection();
        let object = source
            .objects
            .iter()
            .find(|o| matches!(o.kind, ObjectKind::Table { .. }))
            .unwrap();
        let mut prefix = Prefix::new(
            source.clone(),
            object.characters.start..source.characters,
            10,
        )
        .unwrap();
        assert!(matches!(
            prefix.step(None).unwrap(),
            PrefixStep::Table {
                id: 7,
                characters: 10
            }
        ));
        assert!(prefix
            .step(Some(TablePrefix {
                id: 8,
                text: "wrong".into(),
                complete: true
            }))
            .is_err());
        assert!(prefix
            .step(Some(TablePrefix {
                id: 7,
                text: "short".into(),
                complete: false
            }))
            .is_err());
        assert!(Prefix::new(source.clone(), 0..source.characters, BUFFER_BYTES).is_err());
        assert!(Prefix::new(source.clone(), 0..source.characters + 1, 1).is_err());
    }
}
