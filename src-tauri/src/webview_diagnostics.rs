//! Failure-only diagnostics: never log credentials, resource contents or full URLs.
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};
use webkit2gtk::{NetworkError, WebResourceExt, WebView, WebViewExt};

static CLOSE_REQUESTED: AtomicBool = AtomicBool::new(false);

fn now_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
}

pub fn window_event(event: &str) {
    if event == "close-requested" {
        CLOSE_REQUESTED.store(true, Ordering::Relaxed);
    }
    eprintln!(
        "Studio window lifecycle: time_ms={} event={event}",
        now_ms()
    );
}

fn resource_origin(uri: &str) -> String {
    match url::Url::parse(uri) {
        Ok(url) if matches!(url.scheme(), "http" | "https") => url.origin().ascii_serialization(),
        Ok(_) => "<non-http>".into(),
        Err(_) => "<invalid>".into(),
    }
}

pub fn install(view: &WebView, role: &'static str) {
    view.connect_resource_load_started(move |_, resource, _| {
        resource.connect_failed(move |resource, error| {
            eprintln!(
                "Studio WebView resource failed: role={role} time_ms={} close_requested={} origin={} domain={} network_error={:?}",
                now_ms(), CLOSE_REQUESTED.load(Ordering::Relaxed),
                resource_origin(resource.uri().as_deref().unwrap_or_default()),
                error.domain().as_str(), error.kind::<NetworkError>()
            );
        });
    });
    view.connect_web_process_terminated(move |_, reason| {
        eprintln!(
            "Studio WebView process terminated: role={role} time_ms={} close_requested={} reason={reason:?}",
            now_ms(),
            CLOSE_REQUESTED.load(Ordering::Relaxed)
        );
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn failure_diagnostics_never_expose_url_secrets_or_document_paths() {
        assert_eq!(
            resource_origin(
                "http://user:password@127.0.0.1:1234/private/document?token=secret#credentials"
            ),
            "http://127.0.0.1:1234"
        );
        assert_eq!(
            resource_origin("https://example.com/private?key=secret"),
            "https://example.com"
        );
        assert_eq!(resource_origin("file:///private/document.md"), "<non-http>");
        assert_eq!(resource_origin("data:text/plain,secret"), "<non-http>");
        assert_eq!(resource_origin("not a URL"), "<invalid>");
    }
}
