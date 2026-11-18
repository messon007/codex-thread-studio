//! Stateless Studio policy adapter. Document authorization stays in the caller.
use axum::{http::header, routing::get, Router};
pub fn assets() -> Router {
    Router::new()
        .route(
            "/comment-marks.mjs",
            get(|| async {
                (
                    [
                        (header::CONTENT_TYPE, "text/javascript"),
                        (header::CACHE_CONTROL, "no-store"),
                    ],
                    include_str!(concat!(env!("OUT_DIR"), "/studio-comment-marks.mjs")),
                )
            }),
        )
        .fallback_service(marknoria::web::reader_assets())
}
#[cfg(test)]
#[path = "../build/comment_policy.rs"]
mod policy;
#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        body::{to_bytes, Body},
        http::Request,
    };
    use tower::ServiceExt;
    #[test]
    fn policy_refuses_unreviewed_versions_or_assets() {
        let source = include_bytes!("../../crates/marknoria/web/comment-marks.mjs");
        assert_eq!(
            policy::derive(policy::REVISION, source).unwrap(),
            include_bytes!(concat!(env!("OUT_DIR"), "/studio-comment-marks.mjs"))
        );
        assert!(policy::derive("future-version", source).is_err());
        let mut changed = source.to_vec();
        changed.push(b'\n');
        assert!(policy::derive(policy::REVISION, &changed).is_err());
    }
    #[tokio::test]
    async fn policy_changes_only_comment_module_and_adds_no_document_routes() {
        for path in [
            "/comment-marks.mjs",
            "/reader.mjs",
            "/host-client.mjs",
            "/api/source",
            "/api/open",
            "/comment-marks.mjs/extra",
        ] {
            let request = || Request::builder().uri(path).body(Body::empty()).unwrap();
            let original = marknoria::web::reader_assets()
                .oneshot(request())
                .await
                .unwrap();
            let response = assets().oneshot(request()).await.unwrap();
            assert_eq!(response.status(), original.status());
            let original = to_bytes(original.into_body(), 1024 * 1024).await.unwrap();
            let response = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
            if path == "/comment-marks.mjs" {
                assert_eq!(
                    response.as_ref(),
                    policy::derive(policy::REVISION, &original).unwrap()
                );
            } else {
                assert_eq!(response, original);
            }
        }
    }
}
