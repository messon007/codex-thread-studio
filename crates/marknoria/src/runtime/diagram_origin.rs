//! Loopback origin serving the diagram engine assets for native frontends.
//!
//! The core's Chromium diagram renderer evaluates `import('/engine.mjs')`
//! against an origin; the server normally provides it. Native frontends have
//! no HTTP server, so this module spawns a tiny std-only one bound to an
//! ephemeral loopback port serving exactly the files `engine.mjs` loads.
//! The socket serves for the process lifetime and bounds each response by
//! the embedded asset size.

use std::{
    io::{BufReader, Write},
    net::{TcpListener, TcpStream},
    thread,
};

const ASSETS: &[(&str, &str, &str)] = &[
    (
        "/",
        "text/html; charset=utf-8",
        "<!doctype html><title>d</title>",
    ),
    // The renderer navigates to /render.html before evaluating engine.mjs.
    (
        "/render.html",
        "text/html; charset=utf-8",
        "<!doctype html><title>d</title>",
    ),
    (
        "/engine.mjs",
        "text/javascript",
        crate::runtime::assets::ENGINE,
    ),
    (
        "/vendor/purify.mjs",
        "text/javascript",
        crate::runtime::assets::PURIFY,
    ),
    (
        "/vendor/mermaid.min.js",
        "text/javascript",
        crate::runtime::assets::MERMAID,
    ),
    (
        "/vendor/plantuml.js",
        "text/javascript",
        crate::runtime::assets::PLANTUML,
    ),
    (
        "/vendor/plantuml-viz-global.js",
        "text/javascript",
        crate::runtime::assets::PLANTUML_VIZ,
    ),
    (
        "/vendor/d2.js",
        "text/javascript",
        crate::runtime::assets::D2,
    ),
];

/// Binds an ephemeral loopback port and serves the engine assets on a
/// background thread; returns the origin, e.g. `http://127.0.0.1:45678/`.
pub fn spawn() -> std::io::Result<String> {
    use std::sync::{Arc, Mutex, mpsc};
    let listener = TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0))?;
    let authority = listener.local_addr()?.to_string();
    let origin = format!("http://{authority}/");
    // Four connections plus eight queued sockets. Idle preconnects cannot
    // allocate an unbounded number of OS threads or header buffers.
    let (sender, receiver) = mpsc::sync_channel::<TcpStream>(8);
    let receiver = Arc::new(Mutex::new(receiver));
    for _ in 0..4 {
        let receiver = receiver.clone();
        let authority = authority.clone();
        thread::Builder::new()
            .name("mn-diagram-http".into())
            .spawn(move || {
                loop {
                    let stream = match receiver.lock() {
                        Ok(receiver) => receiver.recv(),
                        Err(_) => return,
                    };
                    match stream {
                        Ok(stream) => handle(stream, &authority),
                        Err(_) => break,
                    }
                }
            })?;
    }
    thread::Builder::new()
        .name("mn-diagram-origin".into())
        .spawn(move || {
            for stream in listener.incoming().flatten() {
                let _ = sender.try_send(stream);
            }
        })?;
    Ok(origin)
}

fn handle(mut stream: TcpStream, authority: &str) {
    use std::{
        io::Read,
        time::{Duration, Instant},
    };
    let timeout = Duration::from_secs(2);
    let _ = stream.set_read_timeout(Some(timeout));
    let _ = stream.set_write_timeout(Some(timeout));
    let deadline = Instant::now() + timeout;
    let mut reader = BufReader::new(match stream.try_clone() {
        Ok(stream) => stream,
        Err(_) => return,
    });
    let mut request = Vec::new();
    loop {
        if Instant::now() >= deadline || request.len() >= 8192 {
            return;
        }
        let mut byte = [0];
        if reader.read_exact(&mut byte).is_err() {
            return;
        }
        request.push(byte[0]);
        if request.ends_with(b"\r\n\r\n") {
            break;
        }
    }
    let Ok(request) = std::str::from_utf8(&request) else {
        return;
    };
    let mut lines = request.split("\r\n");
    let Some(first) = lines.next() else {
        return;
    };
    let parts: Vec<_> = first.split_whitespace().collect();
    if parts.len() != 3 || parts[0] != "GET" || parts[2] != "HTTP/1.1" {
        return;
    }
    let headers: Vec<_> = lines.filter_map(|line| line.split_once(':')).collect();
    if headers
        .iter()
        .filter(|(name, _)| name.eq_ignore_ascii_case("host"))
        .count()
        != 1
        || !headers
            .iter()
            .any(|(name, value)| name.eq_ignore_ascii_case("host") && value.trim() == authority)
        || headers.iter().any(|(name, value)| {
            name.eq_ignore_ascii_case("origin") && value.trim() != format!("http://{authority}")
        })
    {
        return;
    }
    let path = parts[1];
    let (status, content_type, body) = ASSETS
        .iter()
        .find(|(asset, _, _)| *asset == path)
        .map(|(_, kind, body)| ("200 OK", *kind, *body))
        .unwrap_or(("404 Not Found", "text/plain", "not found"));
    let _ = write!(
        stream,
        "HTTP/1.1 {status}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\nX-Content-Type-Options: nosniff\r\nContent-Security-Policy: default-src 'none'; script-src 'self' 'unsafe-eval' 'wasm-unsafe-eval'; style-src 'self' 'unsafe-inline'; img-src data: blob:; font-src 'self' data:; worker-src 'self' blob:; connect-src 'self'\r\n\r\n",
        body.len()
    );
    let _ = stream.write_all(body.as_bytes());
    let _ = stream.flush();
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn serves_only_engine_assets_with_a_valid_loopback_host() {
        let origin = spawn().unwrap();
        let host = origin.trim_start_matches("http://").trim_end_matches('/');
        let response = request(
            host,
            &format!("GET /engine.mjs HTTP/1.1\r\nHost: {host}\r\n\r\n"),
        );
        assert!(response.starts_with("HTTP/1.1 200"));
        assert!(response.ends_with(crate::runtime::assets::ENGINE));
        assert!(
            request(
                host,
                &format!("GET /api/document HTTP/1.1\r\nHost: {host}\r\n\r\n")
            )
            .starts_with("HTTP/1.1 404")
        );
        assert!(
            request(
                host,
                "GET /engine.mjs HTTP/1.1\r\nHost: evil.example\r\n\r\n"
            )
            .is_empty()
        );
        assert!(request(host, &format!("GET /engine.mjs HTTP/1.1\r\nHost: {host}\r\nOrigin: https://evil.example\r\n\r\n")).is_empty());
        assert!(
            request(
                host,
                &format!(
                    "GET /engine.mjs HTTP/1.1\r\nHost: {host}\r\nBig: {}\r\n\r\n",
                    "a".repeat(9000)
                )
            )
            .is_empty()
        );
    }
    fn request(host: &str, request: &str) -> String {
        use std::io::Read;
        let mut stream = TcpStream::connect(host).unwrap();
        stream
            .set_read_timeout(Some(std::time::Duration::from_secs(3)))
            .unwrap();
        stream.write_all(request.as_bytes()).unwrap();
        let mut response = String::new();
        let _ = stream.read_to_string(&mut response);
        response
    }
}
