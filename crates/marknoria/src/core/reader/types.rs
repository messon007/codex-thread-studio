use crate::core::document_format::DocumentFormat;
use serde::{Deserialize, Serialize};
use std::io;

pub const API_VERSION: u16 = 1;
pub const BRIDGE_VERSION: u16 = 1;
pub(crate) fn valid_revision(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|b| b.is_ascii_hexdigit())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Origin {
    File,
    Bytes,
    MemoryTable,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum View {
    #[default]
    Preview,
    Source,
    Edit,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Precision {
    Exact,
    Approximate,
    Draft,
}

/// PDF coordinates are normalized to the displayed page, top-left origin.
/// Rotation/crop are applied by the renderer before normalization.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PageRect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PdfRegion {
    pub page: usize,
    pub rects: Vec<PageRect>,
}

/// Browser viewport CSS pixels. The host adds the containing frame's offset;
/// these are never source coordinates or device pixels.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ViewportRect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

/// Source byte offsets, projection pages, tables/rows/columns are zero-based.
/// Lines and original PDF page numbers are one-based. UTF-16 offsets apply only
/// to the named projection, never implicitly to original UTF-8 source bytes.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum Location {
    Source {
        start_byte: usize,
        end_byte: usize,
    },
    Line {
        line: usize,
    },
    Projection {
        page: usize,
        start_utf16: usize,
        end_utf16: usize,
    },
    Cell {
        table: usize,
        row: usize,
        column: usize,
    },
    QueryCell {
        query_id: String,
        row: usize,
        column: usize,
    },
    Pdf {
        pages: Vec<PdfRegion>,
    },
    /// Preserve the full CFI. An indexed-page approximation must not replace it.
    Epub {
        cfi: String,
        #[serde(default)]
        href: Option<String>,
    },
    /// Navigation to an original archive chapter. Durable precise positions use
    /// Epub's full CFI after the renderer resolves this target.
    EpubChapter { href: String },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Selection {
    pub document_id: String,
    pub revision: String,
    pub quote: String,
    #[serde(default)]
    pub quote_truncated: bool,
    pub precision: Precision,
    pub location: Location,
    #[serde(default)]
    pub viewport_rect: Option<ViewportRect>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Budgets {
    pub source_bytes: u64,
    pub source_window_bytes: usize,
    pub edit_window_bytes: usize,
    pub page_html_bytes: usize,
    pub sql_result_bytes: usize,
    pub chart_points: usize,
    pub chart_bytes: usize,
    /// Actual document-specific jq output cap; distinct from the SQL cap.
    pub jq_result_bytes: usize,
    pub sql_bytes: usize,
    pub quote_bytes: usize,
    pub max_columns: usize,
    pub max_outline_items: usize,
    pub max_window_pages: usize,
}
impl Default for Budgets {
    fn default() -> Self {
        Self {
            source_bytes: crate::core::MAX_SOURCE,
            source_window_bytes: crate::core::source::SOURCE_BYTES,
            edit_window_bytes: crate::core::edit::MAX_PAGE,
            page_html_bytes: crate::core::MAX_HTML,
            sql_result_bytes: crate::core::table_query::RESULT_BYTES as usize,
            chart_points: crate::core::table_query::MAX_CHART_POINTS,
            chart_bytes: crate::core::MAX_HTML,
            jq_result_bytes: crate::core::jq_query::output_budget(0),
            sql_bytes: crate::core::table_query::SQL_BYTES,
            quote_bytes: 64 * 1024,
            max_columns: 256,
            max_outline_items: 256,
            max_window_pages: 5,
        }
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Capabilities {
    pub api_version: u16,
    pub preview: bool,
    pub source: bool,
    pub edit: bool,
    pub sql: bool,
    pub charts: bool,
    pub jq: bool,
    pub notes: bool,
    pub query_notes: bool,
    pub assets: bool,
    pub asset_info: bool,
    pub reference_info: bool,
    pub original_bytes: bool,
    pub html_policy: &'static str,
    pub dataset_scope: &'static str,
    pub budgets: Budgets,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AssetFrameInfo {
    pub frame: usize,
    pub delay_ms: u32,
}

/// Image metadata without internal cache filenames. Dimensions describe delivered
/// pixels: source resolution for accepted Web images, bounded previews for the
/// indexed profile, or an explicitly estimated lazy PDF raster.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AssetInfo {
    pub revision: String,
    pub id: usize,
    pub alt: String,
    pub width: u32,
    pub height: u32,
    /// Lazy raster PDF rasters retain an estimated layout size until rendered.
    pub dimensions_exact: bool,
    pub frames: Vec<AssetFrameInfo>,
    pub plays: u32,
    /// Original PDF page numbers are one-based in the public API.
    pub pdf_page: Option<usize>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Descriptor {
    pub renderer: Option<crate::core::DocumentRenderer>,
    pub epub_package: Option<String>,
    pub id: String,
    pub resource_id: String,
    pub origin: Origin,
    pub revision: String,
    pub name: String,
    pub format: DocumentFormat,
    pub language: String,
    pub file_bytes: usize,
    pub source_bytes: usize,
    pub source_lines: usize,
    pub projection_pages: usize,
    pub tables: usize,
    pub assets: usize,
    pub references: usize,
    pub outline_items: usize,
    pub capabilities: Capabilities,
    pub index_ms: f64,
    pub peak_rss_kib: Option<usize>,
    pub editing_revision: Option<String>,
    pub needs_refresh: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ReadingPosition {
    pub version: u16,
    pub resource_id: String,
    pub revision: String,
    pub view: View,
    pub location: Location,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Restoration {
    /// Retain the complete saved location/revision, especially an EPUB CFI.
    pub position: ReadingPosition,
    pub same_revision: bool,
    pub requires_renderer: bool,
}

fn invalid(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, message)
}
impl Location {
    /// Structural bounds only. Document revision/range checks and DOM-dependent
    /// PDF/CFI resolution are separate and must not claim a byte-exact mapping.
    pub fn validate(&self) -> io::Result<()> {
        let ok = match self {
            Self::Source {
                start_byte,
                end_byte,
            } => start_byte <= end_byte && *end_byte <= crate::core::MAX_SOURCE as usize,
            Self::Line { line } => *line > 0,
            Self::Projection {
                start_utf16,
                end_utf16,
                ..
            } => start_utf16 <= end_utf16 && end_utf16 - start_utf16 <= crate::core::edit::MAX_PAGE,
            Self::Cell { column, .. } => *column < 256,
            Self::QueryCell {
                query_id, column, ..
            } => uuid::Uuid::parse_str(query_id).is_ok() && *column < 512,
            Self::Pdf { pages } => {
                !pages.is_empty()
                    && pages.len() <= 64
                    && pages.iter().map(|p| p.rects.len()).sum::<usize>() <= 256
                    && pages.iter().all(|p| {
                        p.page > 0
                            && p.page <= 10000
                            && !p.rects.is_empty()
                            && p.rects.iter().all(|r| {
                                [r.x, r.y, r.width, r.height].iter().all(|v| v.is_finite())
                                    && r.x >= 0.0
                                    && r.y >= 0.0
                                    && r.width > 0.0
                                    && r.height > 0.0
                                    && r.x + r.width <= 1.000001
                                    && r.y + r.height <= 1.000001
                            })
                    })
            }
            Self::Epub { cfi, href } => {
                cfi.len() <= 8192
                    && cfi.starts_with("epubcfi(")
                    && cfi.ends_with(')')
                    && !cfi.contains(['\0', '\n', '\r'])
                    && href
                        .as_ref()
                        .is_none_or(|h| h.len() <= 8192 && !h.contains('\0'))
            }
            Self::EpubChapter { href } => !href.is_empty() && href.len() <= 8192
                && !href.starts_with('/') && !href.contains([':', '\\', '\0', '\r', '\n'])
                && href.split('#').next().is_some_and(|path| path.split('/').all(|part| !part.is_empty() && !matches!(part, "." | ".."))),
        };
        if ok {
            Ok(())
        } else {
            Err(invalid("Invalid or over-budget reading location"))
        }
    }
}

impl Selection {
    pub fn validate(&self) -> io::Result<()> {
        if self.document_id.len() > 256
            || self.document_id.is_empty()
            || !valid_revision(&self.revision)
            || self.quote.len() > Budgets::default().quote_bytes
            || self.quote.contains('\0')
        {
            return Err(invalid("Invalid or over-budget selection"));
        }
        if let Some(r) = &self.viewport_rect {
            if ![r.x, r.y, r.width, r.height].iter().all(|v| v.is_finite())
                || r.width < 0.0
                || r.height < 0.0
            {
                return Err(invalid("Invalid viewport rectangle"));
            }
        }
        self.location.validate()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn positions_round_trip_units_and_reject_invalid_geometry() {
        let cfi = Location::Epub {
            cfi: "epubcfi(/6/2[chapter]!/4/2/1:7)".into(),
            href: Some("chapter.xhtml#part".into()),
        };
        assert_eq!(
            serde_json::from_str::<Location>(&serde_json::to_string(&cfi).unwrap()).unwrap(),
            cfi
        );
        assert!(cfi.validate().is_ok());
        let bad = Location::Pdf {
            pages: vec![PdfRegion {
                page: 1,
                rects: vec![PageRect {
                    x: 0.9,
                    y: 0.0,
                    width: 0.5,
                    height: 0.5,
                }],
            }],
        };
        assert!(bad.validate().is_err());
        assert!(Location::Line { line: 0 }.validate().is_err());
        assert!(
            Location::Source {
                start_byte: 3,
                end_byte: 2
            }
            .validate()
            .is_err()
        );
    }
}
