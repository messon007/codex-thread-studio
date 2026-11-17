//! Temporary shared-reader layout fix tracked in MarkNoria #38.
//! Two exact assets are derived (#38 layout and #39 comment clicks); upstream sources and asset getter stay intact.
use axum::{http::header, routing::get, Router};

pub fn assets() -> Router {
    Router::new()
        .route(
            "/reader.mjs",
            get(|| async {
                (
                    [
                        (header::CONTENT_TYPE, "text/javascript"),
                        (header::CACHE_CONTROL, "no-store"),
                    ],
                    include_str!(concat!(env!("OUT_DIR"), "/reader-compat.mjs")),
                )
            }),
        )
        .route(
            "/comment-marks.mjs",
            get(|| async {
                (
                    [
                        (header::CONTENT_TYPE, "text/javascript"),
                        (header::CACHE_CONTROL, "no-store"),
                    ],
                    include_str!(concat!(env!("OUT_DIR"), "/comment-marks-compat.mjs")),
                )
            }),
        )
        .fallback_service(marknoria::web::reader_assets())
}

#[cfg(test)]
#[path = "../build/comment_compat.rs"]
mod comment_compat;
#[cfg(test)]
#[path = "../build/reader_compat.rs"]
mod reader_compat;

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        body::{to_bytes, Body},
        http::{Request, StatusCode},
    };
    use tower::ServiceExt;

    #[test]
    fn fixed_derivation_is_reversible_and_rejects_other_sources() {
        let original = include_bytes!("../../crates/marknoria/web/reader.mjs");
        let derived = reader_compat::derive(reader_compat::REVISION, original).unwrap();
        assert_eq!(reader_compat::restore(&derived).unwrap(), original);
        assert_eq!(
            derived,
            include_bytes!(concat!(env!("OUT_DIR"), "/reader-compat.mjs"))
        );
        assert!(reader_compat::derive("future-revision", original).is_err());
        let mut changed = original.to_vec();
        changed.push(b'\n');
        assert!(reader_compat::derive(reader_compat::REVISION, &changed).is_err());
    }

    #[test]
    fn comment_derivation_is_reversible_and_rejects_other_sources() {
        let original = include_bytes!("../../crates/marknoria/web/comment-marks.mjs");
        let derived = comment_compat::derive(reader_compat::REVISION, original).unwrap();
        assert_eq!(comment_compat::restore(&derived).unwrap(), original);
        assert_eq!(
            derived,
            include_bytes!(concat!(env!("OUT_DIR"), "/comment-marks-compat.mjs"))
        );
        assert!(comment_compat::derive("future-revision", original).is_err());
        let mut changed = original.to_vec();
        changed.push(b'\n');
        assert!(comment_compat::derive(reader_compat::REVISION, &changed).is_err());
    }

    #[tokio::test]
    async fn only_exact_reader_asset_changes_and_upstream_remains_available() {
        for path in [
            "/reader.mjs",
            "/comment-marks.mjs",
            "/table-cells.mjs",
            "/host-client.mjs",
            "/api/source",
            "/reader.mjs/extra",
        ] {
            let request = || Request::builder().uri(path).body(Body::empty()).unwrap();
            let original = marknoria::web::reader_assets()
                .oneshot(request())
                .await
                .unwrap();
            let derived = assets().oneshot(request()).await.unwrap();
            assert_eq!(original.status(), derived.status());
            if path == "/reader.mjs" {
                assert_eq!(derived.status(), StatusCode::OK);
            }
            let original = to_bytes(original.into_body(), 1024 * 1024).await.unwrap();
            let derived = to_bytes(derived.into_body(), 1024 * 1024).await.unwrap();
            if path == "/reader.mjs" {
                assert_eq!(reader_compat::hash(&original), reader_compat::SOURCE_SHA256);
                assert_eq!(reader_compat::hash(&derived), reader_compat::DERIVED_SHA256);
            } else if path == "/comment-marks.mjs" {
                assert_eq!(
                    reader_compat::hash(&original),
                    comment_compat::SOURCE_SHA256
                );
                assert_eq!(
                    reader_compat::hash(&derived),
                    comment_compat::DERIVED_SHA256
                );
            } else {
                assert_eq!(original, derived);
            }
        }
    }
}
