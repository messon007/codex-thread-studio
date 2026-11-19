//! Stdio MCP transport only: no GUI, Codex credentials, scheduler or session ownership.
use serde_json::{json, Value};
use std::{
    fs::File,
    io::{BufRead, Read, Write},
    path::{Path, PathBuf},
    time::Duration,
};

type Result<T> = std::result::Result<T, String>;
const MAX_MESSAGE: usize = 1024 * 1024;
fn err(e: impl std::fmt::Display) -> String {
    e.to_string()
}

fn string<'a>(v: &'a Value, key: &str) -> Result<&'a str> {
    v[key]
        .as_str()
        .filter(|s| !s.is_empty())
        .ok_or_else(|| format!("Missing {key}"))
}

fn endpoint(descriptor: &Value, backend: &str) -> Result<(url::Url, String)> {
    let mut url = url::Url::parse(string(descriptor, "origin")?).map_err(err)?;
    if url.scheme() != "http"
        || !matches!(url.host_str(), Some("127.0.0.1" | "localhost" | "[::1]"))
        || !url.username().is_empty()
        || url.password().is_some()
        || url.path() != "/"
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return Err("Teams runtime must be a plain loopback HTTP origin".into());
    }
    url.path_segments_mut()
        .map_err(|_| "Invalid Teams origin")?
        .extend(["teams", "mcp", backend]);
    let token = string(&descriptor["registrations"][backend], "token")?.to_owned();
    Ok((url, token))
}

fn read_descriptor(path: &Path) -> Result<Value> {
    let mut bytes = Vec::new();
    File::open(path)
        .map_err(err)?
        .take((4 * MAX_MESSAGE + 1) as u64)
        .read_to_end(&mut bytes)
        .map_err(err)?;
    if bytes.len() > 4 * MAX_MESSAGE {
        return Err("Teams descriptor exceeds limit".into());
    }
    serde_json::from_slice(&bytes).map_err(err)
}

fn read_message(input: &mut impl BufRead) -> Result<Option<Value>> {
    let mut bytes = Vec::new();
    // Limit allocation before reading an untrusted line, not after it.
    let read = input
        .take((MAX_MESSAGE + 1) as u64)
        .read_until(b'\n', &mut bytes)
        .map_err(err)?;
    if read == 0 {
        return Ok(None);
    }
    if bytes.len() > MAX_MESSAGE {
        return Err("MCP request exceeds limit".into());
    }
    serde_json::from_slice(&bytes).map(Some).map_err(err)
}

fn bridge(path: &Path, backend: &str) -> Result<()> {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(err)?;
    let client = reqwest::Client::builder()
        .no_proxy()
        .redirect(reqwest::redirect::Policy::none())
        .timeout(Duration::from_secs(20))
        .build()
        .map_err(err)?;
    let mut input = std::io::stdin().lock();
    let mut output = std::io::stdout().lock();
    while let Some(value) = read_message(&mut input)? {
        // Re-read for every call: the same helper can survive a host restart.
        let response = runtime.block_on(async {
            let (url, token) = endpoint(&read_descriptor(path)?, backend)?;
            let mut response = client
                .post(url)
                .bearer_auth(token)
                .json(&value)
                .send()
                .await
                .map_err(err)?;
            if response.status() == reqwest::StatusCode::NO_CONTENT {
                return Ok(None);
            }
            if !response.status().is_success() {
                return Err(format!("Teams host returned HTTP {}", response.status()));
            }
            let mut bytes = Vec::new();
            while let Some(chunk) = response.chunk().await.map_err(err)? {
                if bytes.len() + chunk.len() > MAX_MESSAGE {
                    return Err("MCP response exceeds limit".into());
                }
                bytes.extend_from_slice(&chunk);
            }
            serde_json::from_slice::<Value>(&bytes)
                .map(Some)
                .map_err(err)
        });
        if let Some(id) = value.get("id") {
            let response = match response {
                Ok(v) => v,
                Err(e) => {
                    Some(json!({"jsonrpc":"2.0","id":id,"error":{"code":-32603,"message":e}}))
                }
            };
            if let Some(response) = response {
                writeln!(output, "{response}").map_err(err)?;
                output.flush().map_err(err)?;
            }
        }
    }
    Ok(())
}

fn run(args: &[String]) -> Result<()> {
    if args == ["--version"] {
        println!("studio-teams-mcp {}", env!("CARGO_PKG_VERSION"));
        return Ok(());
    }
    if args == ["--help"] {
        println!("studio-teams-mcp --runtime-file ABSOLUTE_PATH --backend BACKEND\nStdio transport for a running Studio Teams host. Does not start Studio or Codex.");
        return Ok(());
    }
    if args.len() != 4
        || args[0] != "--runtime-file"
        || args[2] != "--backend"
        || args[3].is_empty()
    {
        return Err(
            "Usage: studio-teams-mcp --runtime-file ABSOLUTE_PATH --backend BACKEND".into(),
        );
    }
    let path = PathBuf::from(&args[1]);
    if !path.is_absolute() {
        return Err("Runtime descriptor must be absolute".into());
    }
    bridge(&path, &args[3])
}

fn main() {
    if let Err(error) = run(&std::env::args().skip(1).collect::<Vec<_>>()) {
        eprintln!("Teams MCP: {error}");
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn origin_is_loopback_only_and_backend_is_a_single_path_segment() {
        let mut d = json!({"origin":"http://127.0.0.1:1234", "registrations":{"test/name":{"token":"fixture"}}});
        assert_eq!(
            endpoint(&d, "test/name").unwrap().0.path(),
            "/teams/mcp/test%2Fname"
        );
        for origin in [
            "https://127.0.0.1:1234",
            "http://example.com",
            "http://user@localhost",
            "http://localhost/other",
            "http://localhost/?x=1",
            "http://localhost/#x",
        ] {
            d["origin"] = json!(origin);
            assert!(endpoint(&d, "test/name").is_err(), "{origin}");
        }
        d["origin"] = json!("http://[::1]:1234");
        assert!(endpoint(&d, "test/name").is_ok());
        assert!(endpoint(&d, "missing").is_err());
    }
    #[test]
    fn input_limit_is_enforced_before_allocating_the_full_line() {
        let data = vec![b'x'; MAX_MESSAGE * 2];
        let mut input = std::io::Cursor::new(data);
        assert!(read_message(&mut input)
            .unwrap_err()
            .contains("exceeds limit"));
        assert_eq!(input.position(), (MAX_MESSAGE + 1) as u64);
        assert!(read_message(&mut std::io::Cursor::new(b""))
            .unwrap()
            .is_none());
    }
}
