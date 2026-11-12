//! Temporary shared-reader layout fix tracked in MarkNoria #38.
//! Only this exact asset is derived; upstream sources and asset getter stay intact.
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
        .fallback_service(marknoria::web::reader_assets())
}

#[cfg(test)]
#[path = "../build/reader_compat.rs"]
mod derivation;

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
        let derived = derivation::derive(derivation::REVISION, original).unwrap();
        assert_eq!(derivation::restore(&derived).unwrap(), original);
        assert_eq!(
            derived,
            include_bytes!(concat!(env!("OUT_DIR"), "/reader-compat.mjs"))
        );
        assert!(derivation::derive("future-revision", original).is_err());
        let mut changed = original.to_vec();
        changed.push(b'\n');
        assert!(derivation::derive(derivation::REVISION, &changed).is_err());
    }

    #[tokio::test]
    async fn only_exact_reader_asset_changes_and_upstream_remains_available() {
        for path in [
            "/reader.mjs",
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
                assert_eq!(derivation::hash(&original), derivation::SOURCE_SHA256);
                assert_eq!(derivation::hash(&derived), derivation::DERIVED_SHA256);
            } else {
                assert_eq!(original, derived);
            }
        }
    }
}
