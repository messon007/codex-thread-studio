use super::{AssetFrameInfo, AssetInfo, Reader, invalid};
use crate::core::Index;
use std::io;

impl Reader {
    /// Inspect one bounded projected asset before requesting its PNG frames.
    /// This reads immutable metadata only; it does not render a lazy PDF page or
    /// grant access to a sibling file, cache pathname or original container.
    pub fn asset_info(&self, revision: &str, id: usize) -> io::Result<AssetInfo> {
        self.check(revision)?;
        asset_info(&self.index, id)
    }
}

pub(crate) fn asset_info(index: &Index, id: usize) -> io::Result<AssetInfo> {
    let asset = index
        .assets
        .get(id)
        .ok_or_else(|| invalid("Image out of range"))?;
    if asset.frames.len() > 128 || asset.alt.len() > 1024 {
        return Err(invalid("Image metadata exceeds its frame/text budget"));
    }
    Ok(AssetInfo {
        revision: index.revision.clone(),
        id,
        alt: asset.alt.clone(),
        width: asset.width,
        height: asset.height,
        dimensions_exact: asset.pdf_page.is_none(),
        frames: asset
            .frames
            .iter()
            .enumerate()
            .map(|(frame, value)| AssetFrameInfo {
                frame,
                delay_ms: value.delay_ms,
            })
            .collect(),
        plays: asset.plays,
        pdf_page: asset
            .pdf_page
            .map(|page| {
                page.checked_add(1)
                    .ok_or_else(|| invalid("PDF page out of range"))
            })
            .transpose()?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::documents::{Asset, AssetFrame};

    #[test]
    fn lazy_pdf_metadata_preserves_page_precision_without_io_or_cache_paths() {
        let mut index = Index {
            revision: "a".repeat(64),
            ..Default::default()
        };
        index.assets.push(Asset {
            alt: "Page 42".into(),
            width: 1200,
            height: 1600,
            frames: vec![AssetFrame {
                file: "not-a-real-cache/private-path.png".into(),
                delay_ms: 100,
            }],
            plays: 1,
            pdf_page: Some(41),
        });
        let info = asset_info(&index, 0).unwrap();
        assert_eq!(info.pdf_page, Some(42));
        assert!(!info.dimensions_exact);
        assert!(
            !serde_json::to_string(&info)
                .unwrap()
                .contains("private-path")
        );
        let frame = index.assets[0].frames[0].clone();
        index.assets[0].frames.resize(129, frame);
        assert!(asset_info(&index, 0).is_err());
    }
}
