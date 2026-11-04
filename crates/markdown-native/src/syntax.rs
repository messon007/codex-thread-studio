//! Worker-side code colors for continuous native text flow. Text is never
//! rewritten or discarded when a language or highlighting budget is unsupported.
use crate::flow::{Projection, Style as FlowStyle};
use std::{
    collections::HashSet,
    ops::Range,
    sync::{
        atomic::{AtomicBool, Ordering},
        LazyLock,
    },
    time::{Duration, Instant},
};
use syntect::{
    highlighting::{FontStyle, HighlightIterator, HighlightState, Highlighter, Style, ThemeSet},
    parsing::{ParseState, ScopeStack, SyntaxSet},
    util::LinesWithEndings,
};

pub const CODE_BYTES: usize = 64 * 1024;
pub const PAGE_BYTES: usize = 256 * 1024;
pub const LINE_BYTES: usize = 4 * 1024;
pub const RUNS: usize = 8192;
pub const PALETTE: usize = 128;
pub const WORK_TIME: Duration = Duration::from_millis(500);
static SYNTAXES: LazyLock<SyntaxSet> = LazyLock::new(two_face::syntax::extra_newlines);
static THEMES: LazyLock<ThemeSet> = LazyLock::new(ThemeSet::load_defaults);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Paint {
    pub foreground: [u8; 4],
    pub background: [u8; 4],
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
}
impl From<Style> for Paint {
    fn from(value: Style) -> Self {
        let rgba = |c: syntect::highlighting::Color| [c.r, c.g, c.b, c.a];
        Self {
            foreground: rgba(value.foreground),
            background: rgba(value.background),
            bold: value.font_style.contains(FontStyle::BOLD),
            italic: value.font_style.contains(FontStyle::ITALIC),
            underline: value.font_style.contains(FontStyle::UNDERLINE),
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Run {
    pub characters: Range<usize>,
    pub paint: Paint,
}
#[derive(Clone, Debug)]
pub struct Colors {
    pub base: Paint,
    pub runs: Vec<Run>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Reason {
    UnknownLanguage,
    CodeBytes,
    PageBytes,
    LongLine,
    Runs,
    Palette,
    Cancelled,
    Deadline,
    Engine,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Status {
    Styled,
    Plain(Reason),
}
#[derive(Clone, Debug)]
pub struct Block {
    pub characters: Range<usize>,
    /// Original logical line starts in projected Unicode character coordinates.
    /// A final newline does not create an additional numbered source line.
    pub lines: Vec<usize>,
    pub language: String,
    pub status: Status,
    pub light: Option<Colors>,
    pub dark: Option<Colors>,
}
#[derive(Clone, Debug, Default)]
pub struct Page {
    pub blocks: Vec<Block>,
    /// Prepared even when a code block exceeds the syntax budget. A toolkit
    /// can keep complete plain code readable without running a theme engine.
    pub base_light: Option<Paint>,
    pub base_dark: Option<Paint>,
}
impl Page {
    pub fn heap_bytes(&self) -> usize {
        self.blocks.capacity() * std::mem::size_of::<Block>()
            + self
                .blocks
                .iter()
                .map(|b| {
                    b.language.capacity()
                        + b.lines.capacity() * std::mem::size_of::<usize>()
                        + b.light
                            .iter()
                            .chain(b.dark.iter())
                            .map(|c| c.runs.capacity() * std::mem::size_of::<Run>())
                            .sum::<usize>()
                })
                .sum::<usize>()
    }
}
#[derive(Default)]
struct Budget {
    runs: usize,
    palette: HashSet<Paint>,
}
fn admit(paint: Paint, budget: &mut Budget) -> Result<(), Reason> {
    if !budget.palette.contains(&paint) && budget.palette.len() >= PALETTE {
        return Err(Reason::Palette);
    }
    budget.palette.insert(paint);
    Ok(())
}
fn append(
    colors: &mut Colors,
    paint: Paint,
    range: Range<usize>,
    budget: &mut Budget,
) -> Result<(), Reason> {
    if paint == colors.base || range.is_empty() {
        return Ok(());
    }
    if let Some(last) = colors
        .runs
        .last_mut()
        .filter(|last| last.paint == paint && last.characters.end == range.start)
    {
        last.characters.end = range.end;
        return Ok(());
    }
    if budget.runs >= RUNS {
        return Err(Reason::Runs);
    }
    admit(paint, budget)?;
    budget.runs += 1;
    colors.runs.push(Run {
        characters: range,
        paint,
    });
    Ok(())
}
fn check(cancel: &AtomicBool, started: Instant) -> Result<(), Reason> {
    if cancel.load(Ordering::Relaxed) {
        Err(Reason::Cancelled)
    } else if started.elapsed() >= WORK_TIME {
        Err(Reason::Deadline)
    } else {
        Ok(())
    }
}
fn token(language: &str) -> &str {
    match language.split_whitespace().next().unwrap_or("") {
        "bash" | "shell" => "sh",
        "yml" => "yaml",
        "c++" => "cpp",
        "c#" => "cs",
        value => value,
    }
}
fn colorize(
    source: &str,
    language: &str,
    offset: usize,
    budget: &Budget,
    cancel: &AtomicBool,
    started: Instant,
) -> Result<(Colors, Colors, Budget), Reason> {
    check(cancel, started)?;
    if source.len() > CODE_BYTES {
        return Err(Reason::CodeBytes);
    }
    if LinesWithEndings::from(source).any(|line| line.len() > LINE_BYTES) {
        return Err(Reason::LongLine);
    }
    let token = token(language);
    if token.is_empty() {
        return Err(Reason::UnknownLanguage);
    }
    let syntax = SYNTAXES
        .find_syntax_by_token(token)
        .ok_or(Reason::UnknownLanguage)?;
    let light = Highlighter::new(&THEMES.themes["InspiredGitHub"]);
    let dark = Highlighter::new(&THEMES.themes["base16-ocean.dark"]);
    let mut paints = Budget {
        runs: budget.runs,
        palette: budget.palette.clone(),
    };
    let mut colors = [
        Colors {
            base: light.get_default().into(),
            runs: Vec::new(),
        },
        Colors {
            base: dark.get_default().into(),
            runs: Vec::new(),
        },
    ];
    for set in &colors {
        admit(set.base, &mut paints)?;
    }
    let mut states = [
        HighlightState::new(&light, ScopeStack::new()),
        HighlightState::new(&dark, ScopeStack::new()),
    ];
    let mut parser = ParseState::new(syntax);
    let mut character = offset;
    for line in LinesWithEndings::from(source) {
        check(cancel, started)?;
        // One syntax parse, two theme projections; GTK never parses the code.
        let operations = parser
            .parse_line(line, &SYNTAXES)
            .map_err(|_| Reason::Engine)?;
        for (index, highlighter) in [&light, &dark].into_iter().enumerate() {
            check(cancel, started)?;
            let mut position = character;
            for (style, text) in
                HighlightIterator::new(&mut states[index], &operations, line, highlighter)
            {
                let end = position + text.chars().count();
                append(&mut colors[index], style.into(), position..end, &mut paints)?;
                position = end;
            }
        }
        character += line.chars().count();
    }
    let [light, dark] = colors;
    Ok((light, dark, paints))
}
fn boundary(text: &str, characters: usize) -> Option<usize> {
    text.char_indices()
        .map(|(byte, _)| byte)
        .chain(std::iter::once(text.len()))
        .nth(characters)
}
/// Run only on a bounded worker. Deadlines are cooperative between engine calls,
/// not preemption of a regex operation. Limits affect colors, never source text.
pub fn prepare(flow: &Projection, cancel: &AtomicBool) -> Result<Page, String> {
    let mut page = Page::default();
    let mut budget = Budget::default();
    let mut bytes = 0usize;
    let (mut previous_byte, mut previous_character) = (0, 0);
    let started = Instant::now();
    for span in &flow.spans {
        let FlowStyle::Code(language) = &span.style else {
            continue;
        };
        if span.characters.start < previous_character
            || span.characters.end < span.characters.start
            || span.characters.end > flow.characters
        {
            return Err("Invalid code-flow character range".into());
        }
        let start = previous_byte
            + boundary(
                &flow.buffer[previous_byte..],
                span.characters.start - previous_character,
            )
            .ok_or("Missing code-flow start")?;
        let end = start
            + boundary(&flow.buffer[start..], span.characters.len())
                .ok_or("Missing code-flow end")?;
        if page.base_light.is_none() && !cancel.load(Ordering::Relaxed) {
            page.base_light = Some(
                Highlighter::new(&THEMES.themes["InspiredGitHub"])
                    .get_default()
                    .into(),
            );
            page.base_dark = Some(
                Highlighter::new(&THEMES.themes["base16-ocean.dark"])
                    .get_default()
                    .into(),
            );
        }
        let source = &flow.buffer[start..end];
        let mut lines = Vec::new();
        if !source.is_empty() {
            lines.push(span.characters.start);
            for (index, character) in source.chars().enumerate() {
                let next = span.characters.start + index + 1;
                if character == '\n' && next < span.characters.end {
                    lines.push(next);
                }
            }
        }
        previous_byte = end;
        previous_character = span.characters.end;
        let result = if source.len().saturating_add(bytes) > PAGE_BYTES {
            Err(Reason::PageBytes)
        } else {
            bytes += source.len();
            colorize(
                source,
                language,
                span.characters.start,
                &budget,
                cancel,
                started,
            )
        };
        let (status, light, dark) = match result {
            Ok((light, dark, next)) => {
                budget = next;
                (Status::Styled, Some(light), Some(dark))
            }
            Err(reason) => (Status::Plain(reason), None, None),
        };
        page.blocks.push(Block {
            characters: span.characters.clone(),
            lines,
            language: language.clone(),
            status,
            light,
            dark,
        });
    }
    Ok(page)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn projection(source: &str) -> Projection {
        let fragment = crate::Fragment {
            page: 0,
            document_id: "syntax".into(),
            revision: "immutable".into(),
            metadata: crate::FragmentMetadata {
                start: 0,
                end: source.len(),
                line: 1,
                rows: 0,
                row_start: 0,
                table: None,
            },
            source: crate::SourceWindow {
                start: 0,
                end: source.len(),
                line: 1,
                end_line: source.lines().count(),
                text: source.into(),
                projection: vec![],
                references: vec![],
            },
            header: None,
        };
        crate::flow::project(&fragment).unwrap()
    }
    #[test]
    fn logical_code_lines_use_projected_unicode_offsets_without_final_phantom() {
        for newline in ["\n", "\r\n"] {
            let source = format!("```unknown-syntax{newline}中文😀{newline}{newline}last{newline}```{newline}{newline}```{newline}```{newline}");
            let flow = projection(&source);
            let page = prepare(&flow, &AtomicBool::new(false)).unwrap();
            let code = &page.blocks[0];
            assert_eq!(code.status, Status::Plain(Reason::UnknownLanguage));
            assert_eq!(code.lines, [0, 4, 5].map(|n| code.characters.start + n));
            assert!(page.blocks[1].lines.is_empty());
            assert_eq!(
                flow.select(code.characters.clone(), crate::flow::BUFFER_BYTES)
                    .unwrap()
                    .text,
                "中文😀\n\nlast\n"
            );
            assert!(page.heap_bytes() >= code.lines.capacity() * std::mem::size_of::<usize>());
        }
    }
    #[test]
    fn themes_preserve_unicode_code_offsets_and_original_copy() {
        let flow=projection("Before 中文😀 $x$.\r\n\r\n```rust\r\nlet value = \"中文😀 <script>\";\r\n// comment\r\n```\r\n\r\nAfter.\r\n");
        let page = prepare(&flow, &AtomicBool::new(false)).unwrap();
        let code = &page.blocks[0];
        assert_eq!(code.status, Status::Styled);
        assert_ne!(
            code.light.as_ref().unwrap().base,
            code.dark.as_ref().unwrap().base
        );
        assert!(code.light.as_ref().unwrap().runs.len() > 1);
        for theme in code.light.iter().chain(code.dark.iter()) {
            for run in &theme.runs {
                assert!(
                    run.characters.start >= code.characters.start
                        && run.characters.end <= code.characters.end
                );
                assert!(!flow
                    .buffer
                    .chars()
                    .skip(run.characters.start)
                    .take(run.characters.len())
                    .collect::<String>()
                    .is_empty());
            }
        }
        let text = |run: &Run| {
            flow.buffer
                .chars()
                .skip(run.characters.start)
                .take(run.characters.len())
                .collect::<String>()
        };
        assert!(code
            .light
            .as_ref()
            .unwrap()
            .runs
            .iter()
            .any(|run| text(run) == "let"));
        assert!(code
            .light
            .as_ref()
            .unwrap()
            .runs
            .iter()
            .any(|run| text(run).contains("中文😀 <script>")));
        assert_eq!(
            flow.select(code.characters.clone(), crate::flow::BUFFER_BYTES)
                .unwrap()
                .text,
            "let value = \"中文😀 <script>\";\n// comment\n"
        );
        let source = flow
            .select(code.characters.clone(), crate::flow::BUFFER_BYTES)
            .unwrap()
            .source
            .unwrap();
        assert_eq!((source.line, source.end_line), (3, 6));
    }
    #[test]
    fn common_syntaxes_and_aliases_have_colors_without_html_output() {
        for (language, code) in [
            ("js", "const value = \"hello\";\n"),
            ("ts", "const value: string = \"hello\";\n"),
            ("tsx", "const value = <p>Hello</p>;\n"),
            ("jsx", "const value = <p>Hello</p>;\n"),
            ("toml", "[section]\nkey = true\n"),
            ("dockerfile", "FROM ubuntu:26.04\nRUN echo hello\n"),
            ("py", "def main():\n    print(\"hello\")\n"),
            ("shell", "echo \"hello\"\n"),
            ("yml", "key: true\n"),
            ("sql", "SELECT name FROM data;\n"),
            ("cpp", "int main() { return 1; }\n"),
        ] {
            let flow = projection(&format!("```{language}\n{code}```\n"));
            let page = prepare(&flow, &AtomicBool::new(false)).unwrap();
            assert_eq!(page.blocks[0].status, Status::Styled, "{language}");
            assert!(
                !page.blocks[0].light.as_ref().unwrap().runs.is_empty(),
                "{language}"
            );
        }
    }
    #[test]
    fn unsupported_and_limited_coloring_keep_all_text_readable() {
        for (language, code, reason) in [
            (
                "not-a-syntax",
                "unchanged tail".into(),
                Reason::UnknownLanguage,
            ),
            ("rust", "x".repeat(CODE_BYTES + 1), Reason::CodeBytes),
            ("rust", "x".repeat(LINE_BYTES + 1), Reason::LongLine),
            ("rust", "let x=1;\n".repeat(4000), Reason::Runs),
        ] {
            let flow = projection(&format!("```{language}\n{code}\n```\n"));
            let page = prepare(&flow, &AtomicBool::new(false)).unwrap();
            if reason == Reason::Runs {
                // Either independent budget can finish a dense block first;
                // the source must remain complete on slow and fast machines.
                assert!(matches!(
                    page.blocks[0].status,
                    Status::Plain(Reason::Runs | Reason::Deadline)
                ));
            } else {
                assert_eq!(page.blocks[0].status, Status::Plain(reason));
            }
            assert!(page.blocks[0].light.is_none() && page.blocks[0].dark.is_none());
            assert!(page.base_light.is_some() && page.base_dark.is_some());
            assert_ne!(
                page.base_light.unwrap().background,
                page.base_dark.unwrap().background
            );
            assert!(flow
                .select(page.blocks[0].characters.clone(), crate::flow::BUFFER_BYTES)
                .unwrap()
                .text
                .contains(&code));
        }
    }
    #[test]
    fn cancellation_and_invalid_ranges_are_explicit() {
        let mut flow = projection("```rust\nlet x=1;\n```\n");
        let page = prepare(&flow, &AtomicBool::new(true)).unwrap();
        assert_eq!(page.blocks[0].status, Status::Plain(Reason::Cancelled));
        assert!(page.base_light.is_none() && page.base_dark.is_none());
        flow.spans
            .iter_mut()
            .find(|s| matches!(s.style, FlowStyle::Code(_)))
            .unwrap()
            .characters
            .end = usize::MAX;
        assert!(prepare(&flow, &AtomicBool::new(false)).is_err());
    }
}
