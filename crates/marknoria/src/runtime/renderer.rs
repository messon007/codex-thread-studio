use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    collections::{HashMap, VecDeque},
    env, fs, io,
    net::{Ipv4Addr, SocketAddrV4, TcpStream},
    path::PathBuf,
    process::{Command, Stdio},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};
use tungstenite::{Message, WebSocket, protocol::WebSocketConfig};

const CACHE_LIMIT: usize = 8 * 1024 * 1024;
const TIMEOUT: Duration = Duration::from_secs(20);
const IDLE: Duration = Duration::from_secs(5);

fn allowed_request(origin: &str, address: &str) -> bool {
    address.strip_prefix(origin).is_some_and(|path| {
        matches!(
            path,
            "/render.html"
                | "/engine.mjs"
                | "/vendor/purify.mjs"
                | "/vendor/mermaid.min.js"
                | "/vendor/plantuml.js"
                | "/vendor/plantuml-viz-global.js"
                | "/vendor/d2.js"
        )
    }) || address.starts_with(&format!("blob:{origin}/"))
        || address.starts_with("data:")
}

fn failure(message: impl ToString) -> io::Error {
    io::Error::other(message.to_string())
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Rendered {
    pub svg: String,
    pub elements: usize,
    pub render_ms: f64,
    pub cached: bool,
}

#[derive(Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Stats {
    pub renders: usize,
    pub hits: usize,
    pub recycles: usize,
    pub failures: usize,
    pub peak_rss_kib: usize,
    pub peak_pss_kib: usize,
    pub blocked_requests: usize,
}

pub struct Renderer {
    executable: Option<PathBuf>,
    worker: Option<Worker>,
    cache: HashMap<usize, Rendered>,
    order: VecDeque<usize>,
    cache_bytes: usize,
    pub stats: Stats,
}

impl Renderer {
    pub fn new() -> Self {
        Self {
            executable: None,
            worker: None,
            cache: HashMap::new(),
            order: VecDeque::new(),
            cache_bytes: 0,
            stats: Stats::default(),
        }
    }

    /// Configure a native renderer without changing the process environment.
    pub fn configure_executable(&mut self, executable: Option<PathBuf>) {
        if self.executable != executable {
            self.recycle();
            self.executable = executable;
        }
    }

    pub fn expire(&mut self) {
        if self
            .worker
            .as_ref()
            .is_some_and(|worker| worker.last_used.elapsed() >= IDLE)
        {
            self.recycle();
        }
    }

    fn recycle(&mut self) {
        if self.worker.take().is_some() {
            self.stats.recycles += 1;
        }
    }

    pub fn snapshot(&self) -> Value {
        json!({"stats":self.stats,"pid":self.worker.as_ref().map(|worker| worker.child.id()),"cacheBytes":self.cache_bytes,"mode":"isolated-chromium","idleSeconds":IDLE.as_secs()})
    }

    pub fn render(
        &mut self,
        id: usize,
        kind: &str,
        source: &str,
        origin: &str,
        cancelled: Arc<AtomicBool>,
    ) -> io::Result<Rendered> {
        self.expire();
        self.order.retain(|cached| *cached != id);
        if let Some(result) = self.cache.get(&id) {
            self.order.push_back(id);
            self.stats.hits += 1;
            let mut result = result.clone();
            result.cached = true;
            return Ok(result);
        }
        if self
            .worker
            .as_ref()
            .is_some_and(|worker| worker.kind != kind || worker.jobs >= 8)
        {
            self.recycle();
        }
        let started = Instant::now();
        if self.worker.is_none() {
            self.worker = Some(Worker::start(
                kind,
                origin,
                cancelled,
                self.executable.clone(),
            )?);
        }
        let worker = self.worker.as_mut().unwrap();
        worker.deadline = started + TIMEOUT;
        let expression = format!(
            "(async()=>{{const engine=await import('/engine.mjs');return engine.render({},{},{});}})()",
            serde_json::to_string(kind)?,
            serde_json::to_string(source)?,
            serde_json::to_string(&format!("backend-diagram-{}", worker.jobs))?
        );
        let result = worker.command(
            "Runtime.evaluate",
            json!({"expression":expression,"awaitPromise":true,"returnByValue":true}),
        );
        self.stats.peak_rss_kib = self.stats.peak_rss_kib.max(worker.peak_rss_kib);
        self.stats.peak_pss_kib = self.stats.peak_pss_kib.max(worker.peak_pss_kib);
        self.stats.blocked_requests += worker.blocked_requests;
        worker.blocked_requests = 0;
        worker.last_used = Instant::now();
        worker.jobs += 1;
        let result = result.and_then(|value| {
            if let Some(exception) = value.get("exceptionDetails") {
                return Err(failure(
                    exception
                        .pointer("/exception/description")
                        .and_then(Value::as_str)
                        .unwrap_or("Diagram rendering failed")
                        .chars()
                        .take(800)
                        .collect::<String>(),
                ));
            }
            let value = value
                .pointer("/result/value")
                .ok_or_else(|| failure("Renderer returned no result"))?;
            let svg = value["svg"]
                .as_str()
                .ok_or_else(|| failure("Renderer returned no SVG"))?
                .to_owned();
            let elements = value["elements"]
                .as_u64()
                .ok_or_else(|| failure("Renderer returned no element count"))?
                as usize;
            if svg.len() > 4 * 1048576 || elements > 20_000 {
                return Err(failure("Renderer output exceeds budget"));
            }
            Ok(Rendered {
                svg,
                elements,
                render_ms: started.elapsed().as_secs_f64() * 1000.0,
                cached: false,
            })
        });
        let result = match result {
            Ok(result) => result,
            Err(error) => {
                self.stats.failures += 1;
                self.recycle();
                return Err(error);
            }
        };
        self.stats.renders += 1;
        let bytes = result.svg.len();
        self.cache_bytes += bytes;
        self.cache.insert(id, result.clone());
        self.order.push_back(id);
        while self.cache_bytes > CACHE_LIMIT {
            if let Some(oldest) = self.order.pop_front()
                && let Some(entry) = self.cache.remove(&oldest)
            {
                self.cache_bytes -= entry.svg.len();
            }
        }
        Ok(result)
    }
}

struct Worker {
    #[cfg(not(windows))]
    child: std::process::Child,
    #[cfg(windows)]
    child: Box<dyn process_wrap::std::ChildWrapper>,
    _profile: tempfile::TempDir,
    socket: Option<WebSocket<TcpStream>>,
    session: String,
    origin: String,
    kind: String,
    next_id: u64,
    loaded: bool,
    deadline: Instant,
    cancelled: Arc<AtomicBool>,
    last_used: Instant,
    jobs: usize,
    peak_rss_kib: usize,
    peak_pss_kib: usize,
    blocked_requests: usize,
}

fn executable() -> io::Result<PathBuf> {
    if let Some(path) = env::var_os("READER_CHROMIUM") {
        return Ok(path.into());
    }
    for directory in env::split_paths(&env::var_os("PATH").unwrap_or_default()) {
        for name in [
            "chromium",
            "chromium-browser",
            "google-chrome",
            "google-chrome-stable",
            "chrome",
            "chromium.exe",
            "chrome.exe",
        ] {
            let path = directory.join(name);
            if path.is_file() {
                return Ok(path);
            }
        }
    }
    // GUI launchers on macOS/Windows do not inherit a terminal's PATH.
    #[allow(unused_mut)]
    let mut candidates = Vec::new();
    #[cfg(target_os = "macos")]
    {
        for root in [
            Some(PathBuf::from("/Applications")),
            env::var_os("HOME").map(|home| PathBuf::from(home).join("Applications")),
        ]
        .into_iter()
        .flatten()
        {
            candidates.push(root.join("Google Chrome.app/Contents/MacOS/Google Chrome"));
            candidates.push(root.join("Chromium.app/Contents/MacOS/Chromium"));
        }
    }
    #[cfg(windows)]
    for name in ["LOCALAPPDATA", "PROGRAMFILES", "PROGRAMFILES(X86)"] {
        if let Some(root) = env::var_os(name).map(PathBuf::from) {
            candidates.push(root.join("Google/Chrome/Application/chrome.exe"));
            candidates.push(root.join("Chromium/Application/chrome.exe"));
        }
    }
    if let Some(path) = candidates.into_iter().find(|path: &PathBuf| path.is_file()) {
        return Ok(path);
    }
    Err(failure(
        "Set READER_CHROMIUM to an installed Chromium/Chrome executable; no frontend fallback is used",
    ))
}

fn process_memory(root: u32) -> (usize, usize) {
    let mut pending = vec![root];
    let mut total = 0;
    let mut proportional = 0;
    while let Some(pid) = pending.pop() {
        if let Ok(children) = fs::read_to_string(format!("/proc/{pid}/task/{pid}/children")) {
            pending.extend(
                children
                    .split_whitespace()
                    .filter_map(|value| value.parse::<u32>().ok()),
            );
        }
        if let Ok(status) = fs::read_to_string(format!("/proc/{pid}/status")) {
            let resident = status
                .lines()
                .find_map(|line| {
                    line.strip_prefix("VmRSS:")?
                        .split_whitespace()
                        .next()?
                        .parse::<usize>()
                        .ok()
                })
                .unwrap_or(0);
            total += resident;
            let pss = fs::read_to_string(format!("/proc/{pid}/smaps_rollup"))
                .ok()
                .and_then(|memory| {
                    memory.lines().find_map(|line| {
                        line.strip_prefix("Pss:")?
                            .split_whitespace()
                            .next()?
                            .parse::<usize>()
                            .ok()
                    })
                });
            proportional += pss.unwrap_or(resident);
        }
    }
    (total, proportional)
}

impl Worker {
    fn start(
        kind: &str,
        origin: &str,
        cancelled: Arc<AtomicBool>,
        configured: Option<PathBuf>,
    ) -> io::Result<Self> {
        let origin = origin.trim_end_matches('/');
        let executable = match configured {
            Some(path) => path,
            None => executable()?,
        };
        let profile = crate::runtime::cache::temporary_directory("md-svg-")?;
        #[cfg(unix)]
        let profile_path = profile.path().canonicalize()?;
        #[cfg(not(unix))]
        let profile_path = profile.path().to_path_buf();
        let mut command = Command::new(executable);
        command
            .env_clear()
            .env("HOME", &profile_path)
            .env("XDG_CONFIG_HOME", profile_path.join("config"))
            .env("XDG_CACHE_HOME", profile_path.join("cache"));
        for name in [
            "PATH",
            "LANG",
            "LC_ALL",
            "LC_CTYPE",
            "LD_LIBRARY_PATH",
            "XDG_RUNTIME_DIR",
            "CODEX_SANDBOX_NETWORK_DISABLED",
            "TZ",
            "FONTCONFIG_PATH",
            "FONTCONFIG_FILE",
            "CHROME_DEVEL_SANDBOX",
            "SYSTEMROOT",
            "WINDIR",
        ] {
            if let Some(value) = env::var_os(name) {
                command.env(name, value);
            }
        }
        command
            .args([
                "--headless",
                "--remote-debugging-address=127.0.0.1",
                "--remote-debugging-port=0",
                "--no-first-run",
                "--no-default-browser-check",
                "--disable-background-networking",
                "--disable-background-timer-throttling",
                "--disable-renderer-backgrounding",
                "--disable-backgrounding-occluded-windows",
                "--disable-component-update",
                "--disable-sync",
                "--disable-extensions",
                "--disable-breakpad",
                "--disable-dev-shm-usage",
                "--disable-gpu",
                "--metrics-recording-only",
                "--window-size=1500,950",
                // Headless/testing profile: never touch the user's real
                // keychain (macOS shows a blocking prompt otherwise).
                "--use-mock-keychain",
                "--password-store=basic",
            ])
            .arg(format!("--user-data-dir={}", profile_path.display()))
            .arg("about:blank")
            .env("TMPDIR", &profile_path)
            .env("TMP", &profile_path)
            .env("TEMP", &profile_path)
            .env("APPDATA", profile_path.join("appdata"))
            .env("LOCALAPPDATA", profile_path.join("localappdata"))
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        if env::var_os("READER_CHROMIUM_NO_SANDBOX").is_some_and(|value| value == "1") {
            command.arg("--no-sandbox");
        }
        if env::var_os("READER_RENDER_TRACE").is_some() {
            command
                .stderr(Stdio::inherit())
                .arg("--enable-logging=stderr");
        }
        #[cfg(unix)]
        {
            use std::os::unix::process::CommandExt;
            if env::var_os("MARKNORIA_NATIVE_WORKER_GROUP").is_none() {
                command.process_group(0);
            }
        }
        #[cfg(not(windows))]
        let child = command.spawn()?;
        #[cfg(windows)]
        let child = {
            let mut command = process_wrap::std::CommandWrap::from(command);
            let mut flags = process_wrap::std::CreationFlags(Default::default());
            flags.0.0 = 0x08000000;
            command.wrap(flags).wrap(process_wrap::std::JobObject);
            command.spawn()?
        };
        let mut worker = Self {
            child,
            _profile: profile,
            socket: None,
            session: String::new(),
            origin: origin.into(),
            kind: kind.into(),
            next_id: 0,
            loaded: false,
            // Cold Chromium starts (first launch, macOS Gatekeeper checks)
            // can take well over 8s; startup gets its own generous budget.
            deadline: Instant::now() + Duration::from_secs(30),
            cancelled,
            last_used: Instant::now(),
            jobs: 0,
            peak_rss_kib: 0,
            peak_pss_kib: 0,
            blocked_requests: 0,
        };
        let address = loop {
            worker.check()?;
            if let Ok(address) =
                fs::read_to_string(worker._profile.path().join("DevToolsActivePort"))
            {
                break address;
            }
            if worker.child.try_wait()?.is_some() {
                return Err(failure(format!(
                    "Chromium failed to start; check its installation, OS sandbox support and cache quota at {}. On Unix, try a shorter disk-backed READER_CACHE_DIR if the temporary path is long.",
                    worker._profile.path().display()
                )));
            }
            std::thread::sleep(Duration::from_millis(20));
        };
        let mut lines = address.lines();
        let port = lines
            .next()
            .ok_or_else(|| failure("Missing DevTools port"))?
            .parse::<u16>()
            .map_err(failure)?;
        let path = lines
            .next()
            .ok_or_else(|| failure("Missing DevTools endpoint"))?;
        let stream = TcpStream::connect_timeout(
            &SocketAddrV4::new(Ipv4Addr::LOCALHOST, port).into(),
            Duration::from_secs(1),
        )?;
        // The DevTools websocket handshake can exceed the steady-state read
        // budget on a cold Chromium start (observed on macOS); give the
        // handshake its own budget, then tighten for CDP traffic.
        stream.set_read_timeout(Some(Duration::from_secs(2)))?;
        stream.set_write_timeout(Some(Duration::from_secs(2)))?;
        let (mut socket, _) =
            tungstenite::client(format!("ws://127.0.0.1:{port}{path}"), stream).map_err(failure)?;
        socket
            .get_ref()
            .set_read_timeout(Some(Duration::from_millis(100)))?;
        socket.set_config(|config| {
            *config = WebSocketConfig::default()
                .max_message_size(Some(8 * 1048576))
                .max_frame_size(Some(8 * 1048576));
        });
        worker.socket = Some(socket);
        let target = worker.command("Target.createTarget", json!({"url":"about:blank"}))?;
        let attached = worker.command(
            "Target.attachToTarget",
            json!({"targetId":target["targetId"],"flatten":true}),
        )?;
        worker.session = attached["sessionId"]
            .as_str()
            .ok_or_else(|| failure("Missing DevTools session"))?
            .into();
        worker.command("Page.enable", json!({}))?;
        if env::var_os("READER_RENDER_TRACE").is_some() {
            worker.command("Network.enable", json!({}))?;
        }
        // Keep network isolation identical for native and server rendering.
        worker.command("Fetch.enable", json!({"patterns":[{"urlPattern":"*"}]}))?;
        worker.loaded = false;
        worker.command(
            "Page.navigate",
            json!({"url":format!("{origin}/render.html")}),
        )?;
        while !worker.loaded {
            worker
                .receive()
                .map_err(|error| failure(format!("Renderer page load: {error}")))?;
        }
        worker.command("Page.bringToFront", json!({}))?;
        Ok(worker)
    }

    fn check(&mut self) -> io::Result<()> {
        if self.cancelled.load(Ordering::Relaxed) {
            return Err(failure("Diagram renderer cancelled"));
        }
        if Instant::now() >= self.deadline {
            return Err(failure("Diagram renderer exceeded its time budget"));
        }
        let (rss, pss) = process_memory(self.child.id());
        self.peak_rss_kib = self.peak_rss_kib.max(rss);
        self.peak_pss_kib = self.peak_pss_kib.max(pss);
        if self.peak_pss_kib > 1024 * 1024 {
            return Err(failure(
                "Diagram process tree exceeded the sampled Linux 1 GiB PSS budget (RSS fallback if unavailable)",
            ));
        }
        Ok(())
    }

    fn send(&mut self, method: &str, params: Value, session: Option<String>) -> io::Result<u64> {
        self.next_id += 1;
        if env::var_os("READER_RENDER_TRACE").is_some() {
            eprintln!("Diagram CDP send {} {method}", self.next_id);
        }
        let mut message = json!({"id":self.next_id,"method":method,"params":params});
        if let Some(session) = session {
            message["sessionId"] = session.into();
        }
        self.socket
            .as_mut()
            .ok_or_else(|| failure("Renderer disconnected"))?
            .send(Message::Text(message.to_string().into()))
            .map_err(failure)?;
        Ok(self.next_id)
    }

    fn receive(&mut self) -> io::Result<Value> {
        loop {
            self.check()?;
            match self.socket.as_mut().unwrap().read() {
                Ok(Message::Text(text)) => {
                    let value: Value = serde_json::from_str(&text)?;
                    if env::var_os("READER_RENDER_TRACE").is_some() {
                        eprintln!(
                            "Diagram CDP receive id={} method={}",
                            value["id"], value["method"]
                        );
                    }
                    match value["method"].as_str() {
                        Some("Page.loadEventFired") => self.loaded = true,
                        Some("Fetch.requestPaused") => {
                            let address = value
                                .pointer("/params/request/url")
                                .and_then(Value::as_str)
                                .unwrap_or("");
                            let allowed = allowed_request(&self.origin, address);
                            if !allowed {
                                self.blocked_requests += 1;
                            }
                            let params = if allowed {
                                json!({"requestId":value["params"]["requestId"]})
                            } else {
                                json!({"requestId":value["params"]["requestId"],"errorReason":"BlockedByClient"})
                            };
                            self.send(
                                if allowed {
                                    "Fetch.continueRequest"
                                } else {
                                    "Fetch.failRequest"
                                },
                                params,
                                value["sessionId"].as_str().map(str::to_owned),
                            )?;
                        }
                        _ => {}
                    }
                    return Ok(value);
                }
                Ok(Message::Close(_)) => return Err(failure("Renderer disconnected")),
                Ok(_) => {}
                Err(tungstenite::Error::Io(error))
                    if matches!(
                        error.kind(),
                        io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut
                    ) => {}
                Err(error) => return Err(failure(error)),
            }
        }
    }

    fn command(&mut self, method: &str, params: Value) -> io::Result<Value> {
        let session = (!self.session.is_empty()).then(|| self.session.clone());
        let id = self.send(method, params, session)?;
        loop {
            let response = self.receive().map_err(|error| {
                failure(format!(
                    "{method}: {error}; blocked requests: {}",
                    self.blocked_requests
                ))
            })?;
            if response["id"].as_u64() == Some(id) {
                if let Some(error) = response.get("error") {
                    return Err(failure(error));
                }
                return Ok(response["result"].clone());
            }
        }
    }
}

impl Drop for Worker {
    fn drop(&mut self) {
        if let Some(socket) = &self.socket {
            let _ = socket
                .get_ref()
                .set_write_timeout(Some(Duration::from_millis(100)));
        }
        if self.socket.is_some() && self.send("Browser.close", json!({}), None).is_ok() {
            let deadline = Instant::now() + Duration::from_millis(500);
            while Instant::now() < deadline {
                if self.child.try_wait().ok().flatten().is_some() {
                    return;
                }
                std::thread::sleep(Duration::from_millis(20));
            }
        }
        #[cfg(unix)]
        unsafe {
            libc::kill(-(self.child.id() as libc::pid_t), libc::SIGKILL);
        }
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renderer_network_scope_excludes_private_apis_files_and_external_origins() {
        let origin = "http://127.0.0.1:12345";
        for suffix in [
            "/render.html",
            "/engine.mjs",
            "/vendor/mermaid.min.js",
            "/vendor/d2.js",
            "/vendor/plantuml.js",
        ] {
            assert!(allowed_request(origin, &format!("{origin}{suffix}")));
        }
        for address in [
            format!("{origin}/api/document"),
            format!("{origin}/api/diagram?id=0"),
            format!("{origin}/"),
            format!("{origin}.example/render.html"),
            format!("{origin}@example.com/render.html"),
            "https://example.com/diagram.svg".into(),
            "file:///etc/passwd".into(),
            "blob:https://example.com/id".into(),
        ] {
            assert!(!allowed_request(origin, &address));
        }
    }
}

/// Rasterizes a diagram SVG to PNG bytes for native widget display,
/// scaled down to `max_width` pixels when wider. Pure Rust (resvg) — no
/// browser involved.
#[cfg(feature = "diagram-png")]
pub fn rasterize(svg: &str, max_width: u32) -> io::Result<Vec<u8>> {
    use resvg::usvg;
    const MAX_SIDE: f32 = 4096.0;
    const MAX_PIXELS: f32 = 4_194_304.0;
    if svg.len() > CACHE_LIMIT {
        return Err(failure("Diagram SVG exceeds 8 MiB"));
    }
    let mut options = usvg::Options::default();
    // Load fonts once; never let SVG hrefs read arbitrary local files.
    static FONTS: std::sync::OnceLock<Arc<usvg::fontdb::Database>> = std::sync::OnceLock::new();
    options.fontdb = FONTS
        .get_or_init(|| {
            let mut fonts = usvg::fontdb::Database::new();
            fonts.load_system_fonts();
            Arc::new(fonts)
        })
        .clone();
    options.image_href_resolver.resolve_string = Box::new(|_, _| None);
    options.image_href_resolver.resolve_data = Box::new(|_, _, _| None);
    let tree = usvg::Tree::from_str(svg, &options)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidInput, error.to_string()))?;
    let size = tree.size();
    let scale = 1.0_f32
        .min(max_width.clamp(1, 4096) as f32 / size.width())
        .min(MAX_SIDE / size.height())
        .min((MAX_PIXELS / (size.width() * size.height())).sqrt());
    let transform = resvg::usvg::Transform::from_scale(scale, scale);
    let width = (size.width() * scale).ceil().max(1.0) as u32;
    let height = (size.height() * scale).ceil().max(1.0) as u32;
    let mut pixmap = resvg::tiny_skia::Pixmap::new(width, height)
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "Diagram has no size"))?;
    resvg::render(&tree, transform, &mut pixmap.as_mut());
    pixmap
        .encode_png()
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error.to_string()))
}

#[cfg(all(test, feature = "diagram-png"))]
mod raster_tests {
    #[test]
    fn bounds_extreme_svg_dimensions_before_allocating_pixels() {
        let png = super::rasterize(
            r##"<svg xmlns="http://www.w3.org/2000/svg" width="1000000" height="1000000"><rect width="1000000" height="1000000" fill="red"/></svg>"##, 800,
        ).unwrap();
        // PNG IHDR stores the actual allocated dimensions, big endian.
        let width = u32::from_be_bytes(png[16..20].try_into().unwrap());
        let height = u32::from_be_bytes(png[20..24].try_into().unwrap());
        assert!(width <= 800 && height <= 4096 && width * height <= 4_194_304);
    }

    #[test]
    fn rejects_svg_over_budget() {
        assert!(super::rasterize(&" ".repeat(super::CACHE_LIMIT + 1), 800).is_err());
    }
    #[test]
    fn rasterizes_a_small_svg_to_png() {
        let png = super::rasterize(
            r##"<svg xmlns="http://www.w3.org/2000/svg" width="40" height="20"><rect width="40" height="20" fill="#123456"/></svg>"##,
            800,
        )
        .unwrap();
        assert_eq!(&png[..4], b"\x89PNG");
        assert!(png.len() > 100);
    }

    #[test]
    fn rasterizes_svg_text_with_the_maintained_font_stack() {
        let png = super::rasterize(
            r##"<svg xmlns="http://www.w3.org/2000/svg" width="240" height="48"><rect width="240" height="48" fill="white"/><text x="8" y="34" font-family="DejaVu Sans, Liberation Sans, Noto Sans, Arial, Helvetica, sans-serif" font-size="24" fill="black">MarkNoria SVG</text></svg>"##,
            240,
        ).unwrap();
        let pixels = image::load_from_memory(&png).unwrap().to_rgba8();
        assert_eq!(pixels.dimensions(), (240, 48));
        assert!(pixels.pixels().filter(|pixel| pixel[0] < 100 && pixel[3] > 0).count() > 100,
            "SVG text disappeared after the font dependency update");
    }
}
