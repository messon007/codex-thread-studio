//! Read navigation annotations only inside the existing PDFium worker sandbox.
use crate::pdf_navigation::{web_url, Link, Rect, Target, MAX_LINKS, MAX_REGIONS};
use pdfium_render::prelude::*;
fn region(page: &PdfPage<'_>, rect: PdfRect) -> Option<Rect> {
    let coordinates = [
        rect.left().value,
        rect.right().value,
        rect.top().value,
        rect.bottom().value,
    ];
    if coordinates
        .iter()
        .any(|v| !v.is_finite() || v.abs() > 1_000_000.0)
    {
        return None;
    }
    let config = PdfRenderConfig::new().set_target_size(10000, 10000);
    let corners = [
        (rect.left(), rect.top()),
        (rect.right(), rect.top()),
        (rect.right(), rect.bottom()),
        (rect.left(), rect.bottom()),
    ];
    let mut left = 10000;
    let mut top = 10000;
    let mut right = 0;
    let mut bottom = 0;
    for (x, y) in corners {
        let (x, y) = page.points_to_pixels(x, y, &config).ok()?;
        left = left.min(x);
        top = top.min(y);
        right = right.max(x);
        bottom = bottom.max(y);
    }
    let (left, top, right, bottom) = (
        left.clamp(0, 10000),
        top.clamp(0, 10000),
        right.clamp(0, 10000),
        bottom.clamp(0, 10000),
    );
    let rect = Rect {
        x: left as f64 / 10000.0,
        y: top as f64 / 10000.0,
        width: (right - left) as f64 / 10000.0,
        height: (bottom - top) as f64 / 10000.0,
    };
    rect.valid().then_some(rect)
}
pub(super) fn navigation_links(page: &PdfPage<'_>, pages: usize) -> (Vec<Link>, bool, bool) {
    let mut output = Vec::new();
    let mut limited = false;
    let mut unavailable = false;
    let mut regions = 0;
    let annotations = page.annotations();
    limited |= annotations.len() > 2048;
    for index in 0..annotations.len().min(2048) {
        let annotation = match annotations.get(index) {
            Ok(annotation) => annotation,
            Err(_) => {
                unavailable = true;
                continue;
            }
        };
        let Some(link_annotation) = annotation.as_link_annotation() else {
            continue;
        };
        if output.len() >= MAX_LINKS {
            limited = true;
            break;
        }
        if annotation.is_hidden()
            || annotation.is_printable_but_not_viewable()
            || annotation.is_read_only()
        {
            continue;
        }
        if !annotation.is_rotatable() || !annotation.is_zoomable() {
            unavailable = true;
            continue;
        }
        let result = (|| -> Option<Link> {
            let link = link_annotation.link().ok()?;
            let target = if let Some(action) = link.action() {
                if let Some(local) = action.as_local_destination_action() {
                    let page =
                        usize::try_from(local.destination().ok()?.page_index().ok()?).ok()?;
                    if page >= pages {
                        return None;
                    }
                    Target::Page { page }
                } else if let Some(uri) = action.as_uri_action() {
                    Target::Url {
                        url: web_url(&uri.uri().ok()?)?,
                    }
                } else {
                    return None;
                }
            } else {
                let page = usize::try_from(link.destination()?.page_index().ok()?).ok()?;
                if page >= pages {
                    return None;
                }
                Target::Page { page }
            };
            let attachments = annotation.attachment_points();
            let mut rects = Vec::new();
            if attachments.is_empty() {
                rects.push(region(page, link.rect().ok()?)?);
            } else {
                if attachments.len() > 8 {
                    limited = true;
                }
                for quad in attachments.iter().take(8) {
                    rects.push(region(page, quad.to_rect())?);
                }
            }
            Some(Link { rects, target })
        })();
        if let Some(link) = result {
            if regions + link.rects.len() > MAX_REGIONS {
                limited = true;
                break;
            }
            regions += link.rects.len();
            output.push(link);
        } else {
            unavailable = true;
        }
    }
    (output, limited, unavailable)
}
