//! Bounded Fontconfig selection on the diagram CPU lane. No GTK font enumeration.
use crate::{
    byte_cache::{ByteCache, Weight},
    mermaid_worker::{
        self,
        protocol::{FontDefaults, Request},
    },
};
use sha2::{Digest, Sha256};
use std::io::Read;
use std::{
    collections::BTreeSet,
    fs,
    num::NonZeroUsize,
    path::{Path, PathBuf},
    process::Command,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex, OnceLock,
    },
    time::{Duration, Instant},
};
#[derive(Clone, Debug, PartialEq, Eq)]
struct Stamp {
    path: PathBuf,
    len: u64,
    modified: std::time::SystemTime,
    changed: (i64, i64),
    device: u64,
    inode: u64,
}
fn stamp(path: &Path) -> Result<Stamp, String> {
    use std::os::unix::fs::MetadataExt;
    let path = path.canonicalize().map_err(|e| e.to_string())?;
    let m = path.metadata().map_err(|e| e.to_string())?;
    if !m.is_file() {
        return Err("Selected font is not a regular file".into());
    }
    Ok(Stamp {
        path,
        len: m.len(),
        modified: m.modified().map_err(|e| e.to_string())?,
        changed: (m.ctime(), m.ctime_nsec()),
        device: m.dev(),
        inode: m.ino(),
    })
}
#[derive(Clone, Debug)]
pub struct Selection {
    pub files: Vec<PathBuf>,
    pub defaults: FontDefaults,
    pub fingerprint: String,
    pub cache_hit: bool,
    stamps: Vec<Stamp>,
    worker: Stamp,
    created: Instant,
}
impl Selection {
    pub fn validate_current(&self) -> Result<(), String> {
        if stamp(&self.worker.path).is_ok_and(|now| now == self.worker)
            && self
                .stamps
                .iter()
                .all(|s| stamp(&s.path).is_ok_and(|now| &now == s))
        {
            Ok(())
        } else {
            Err("Selected Mermaid font or worker changed; retry rendering".into())
        }
    }
    pub fn render(
        &self,
        worker: &Path,
        request: &Request,
        cancel: Arc<AtomicBool>,
    ) -> Result<String, String> {
        if worker.canonicalize().map_err(|e| e.to_string())? != self.worker.path {
            return Err("Selected Mermaid worker changed; retry rendering".into());
        }
        self.validate_current()?;
        let mut request = request.clone();
        request.font_defaults = Some(self.defaults.clone());
        let svg = mermaid_worker::render(&self.worker.path, &self.files, &request, cancel)?;
        self.validate_current()?;
        Ok(svg)
    }
}
impl Weight for Selection {
    fn weight(&self) -> usize {
        self.files
            .iter()
            .map(|p| p.as_os_str().len())
            .sum::<usize>()
            * 2
            + self.defaults.sans.len()
            + self.defaults.mono.len()
            + self.fingerprint.len()
            + self.stamps.len() * std::mem::size_of::<Stamp>()
            + self.worker.path.as_os_str().len()
            + std::mem::size_of::<Self>()
    }
}
static CACHE: OnceLock<Mutex<ByteCache<[u8; 32], Selection>>> = OnceLock::new();
fn current(cancel: &AtomicBool, until: Instant) -> Result<Duration, String> {
    if cancel.load(Ordering::Relaxed) {
        return Err("Mermaid font selection cancelled".into());
    }
    until
        .checked_duration_since(Instant::now())
        .filter(|d| !d.is_zero())
        .ok_or_else(|| "Mermaid font selection deadline exceeded".into())
}
fn pattern(stack: &str) -> Result<String, String> {
    let families = svgtypes::parse_font_families(stack).map_err(|e| e.to_string())?;
    if families.is_empty() || families.len() > 16 {
        return Err("Invalid Mermaid font stack".into());
    }
    Ok(families
        .into_iter()
        .map(|family| {
            let name = match family {
                svgtypes::FontFamily::Named(name) => name,
                _ => family.to_string(),
            };
            let name = if name == "system-ui" {
                "sans-serif".into()
            } else {
                name
            };
            name.chars()
                .flat_map(|c| {
                    if matches!(c, '\\' | '-' | ':' | ',') {
                        vec!['\\', c]
                    } else {
                        vec![c]
                    }
                })
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join(","))
}
struct Candidate {
    path: PathBuf,
    index: u32,
    family: String,
}
fn query(
    stack: &str,
    variant: &str,
    point: Option<u32>,
    cancel: Arc<AtomicBool>,
    until: Instant,
) -> Result<Candidate, String> {
    let mut p = format!("{}:{variant}:outline=true:scalable=true", pattern(stack)?);
    if let Some(c) = point {
        p.push_str(&format!(":charset={c:x}"));
    }
    let mut command = Command::new("/usr/bin/fc-match");
    command
        .env_remove("FC_DEBUG")
        .args(["-f", "%{file}\\n%{index}\\n%{family[0]}\\n", "--", &p]);
    let bytes = crate::engine_process::bytes(
        &mut command,
        Vec::new(),
        cancel.clone(),
        current(&cancel, until)?.min(Duration::from_secs(2)),
        32768,
    )?;
    let text = String::from_utf8(bytes).map_err(|_| "Fontconfig returned invalid UTF-8")?;
    let rows = text.lines().collect::<Vec<_>>();
    if rows.len() != 3
        || rows[0].len() > 8192
        || rows[2].is_empty()
        || rows[2].len() > 8192
        || rows[2].chars().any(char::is_control)
    {
        return Err("Fontconfig returned invalid font metadata".into());
    }
    Ok(Candidate {
        path: PathBuf::from(rows[0]),
        index: rows[1].parse::<u32>().map_err(|e| e.to_string())? & 0xffff,
        family: rows[2].into(),
    })
}
fn add(
    candidate: Candidate,
    files: &mut Vec<PathBuf>,
    stamps: &mut Vec<Stamp>,
    uncovered: &mut BTreeSet<u32>,
    hash: &mut Sha256,
    total: &mut u64,
    cancel: &AtomicBool,
    until: Instant,
) -> Result<String, String> {
    current(cancel, until)?;
    let stat = stamp(&candidate.path)?;
    if files.contains(&stat.path) {
        return Ok(candidate.family);
    }
    if files.len() >= crate::mermaid_worker::protocol::FONT_LIMIT
        || total.saturating_add(stat.len) > crate::mermaid_worker::protocol::FONT_BYTES
    {
        return Err(
            "Mermaid fonts exceed twenty-four files / 64 MiB; reduce the requested font families"
                .into(),
        );
    }
    let mut bytes = Vec::new();
    fs::File::open(&stat.path)
        .map_err(|e| e.to_string())?
        .take(stat.len + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() as u64 != stat.len || stamp(&stat.path)? != stat {
        return Err("Selected font changed while reading; retry rendering".into());
    }
    let faces = ttf_parser::fonts_in_collection(&bytes).unwrap_or(1);
    if faces > 128 || candidate.index >= faces {
        return Err("Selected font has invalid collection metadata".into());
    }
    for index in 0..faces {
        let face = ttf_parser::Face::parse(&bytes, index).map_err(|e| e.to_string())?;
        uncovered.retain(|c| char::from_u32(*c).is_some_and(|c| face.glyph_index(c).is_none()));
    }
    hash.update((bytes.len() as u64).to_le_bytes());
    hash.update(&bytes);
    hash.update(candidate.index.to_le_bytes());
    *total += stat.len;
    files.push(stat.path.clone());
    stamps.push(stat);
    current(cancel, until)?;
    Ok(candidate.family)
}
pub fn prepare(
    worker: &Path,
    request: &Request,
    cancel: Arc<AtomicBool>,
) -> Result<Selection, String> {
    request.validate()?;
    let until = Instant::now() + Duration::from_secs(4);
    current(&cancel, until)?;
    let worker = stamp(worker)?;
    let mut hasher = Sha256::new();
    hasher.update(serde_json::to_vec(request).map_err(|e| e.to_string())?);
    hasher.update(format!("{worker:?}"));
    let key: [u8; 32] = hasher.finalize().into();
    let cache = CACHE.get_or_init(|| {
        Mutex::new(ByteCache::new(
            NonZeroUsize::new(32).unwrap(),
            2 * 1024 * 1024,
        ))
    });
    if let Some(mut value) = cache
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .update(&key, |v| v.clone())
    {
        if value.created.elapsed() < Duration::from_secs(10) && value.validate_current().is_ok() {
            value.cache_hit = true;
            return Ok(value);
        }
    }
    let plan = mermaid_worker::font_plan(&worker.path, request, cancel.clone())?;
    let mut uncovered=plan.characters.into_iter().filter(|n|char::from_u32(*n).is_some_and(|c|!c.is_whitespace() && !c.is_control()) && !matches!(*n,0x200b..=0x200f|0x202a..=0x202e|0x2060..=0x206f|0xfe00..=0xfe0f|0xe0100..=0xe01ef)).collect::<BTreeSet<_>>();
    let (mut files, mut stamps, mut hash, mut total) = (Vec::new(), Vec::new(), Sha256::new(), 0);
    // A replacement engine must not reuse images made by an earlier binary,
    // even when it resolves exactly the same font files.
    hash.update(format!("{worker:?}"));
    let mut defaults = FontDefaults {
        sans: String::new(),
        mono: String::new(),
    };
    // Role aliases remain explicit even when both body and code faces are monospaced.
    for (stack, code) in [(&plan.primary, false), (&plan.mono, true)] {
        for (i, variant) in [
            "weight=regular:slant=roman",
            "weight=bold:slant=roman",
            "weight=regular:slant=italic",
            "weight=bold:slant=italic",
        ]
        .into_iter()
        .enumerate()
        {
            let family = add(
                query(stack, variant, None, cancel.clone(), until)?,
                &mut files,
                &mut stamps,
                &mut uncovered,
                &mut hash,
                &mut total,
                &cancel,
                until,
            )?;
            if i == 0 {
                if code {
                    defaults.mono = family;
                } else {
                    defaults.sans = family;
                }
            }
        }
    }
    for stack in plan
        .families
        .iter()
        .filter(|s| *s != &plan.primary && *s != &plan.mono)
    {
        for variant in [
            "weight=regular:slant=roman",
            "weight=bold:slant=roman",
            "weight=regular:slant=italic",
            "weight=bold:slant=italic",
        ] {
            add(
                query(stack, variant, None, cancel.clone(), until)?,
                &mut files,
                &mut stamps,
                &mut uncovered,
                &mut hash,
                &mut total,
                &cancel,
                until,
            )?;
        }
    }
    while let Some(point) = uncovered.first().copied() {
        let before = uncovered.len();
        let candidate = query(
            &plan.primary,
            "weight=regular:slant=roman",
            Some(point),
            cancel.clone(),
            until,
        )?;
        let family = add(
            candidate,
            &mut files,
            &mut stamps,
            &mut uncovered,
            &mut hash,
            &mut total,
            &cancel,
            until,
        )?;
        if uncovered.len() == before {
            return Err(format!("No installed outline font covers U+{point:04X}"));
        }
        add(
            query(
                &format!("\"{}\"", family.replace('"', "\\\"")),
                "weight=bold:slant=roman",
                None,
                cancel.clone(),
                until,
            )?,
            &mut files,
            &mut stamps,
            &mut uncovered,
            &mut hash,
            &mut total,
            &cancel,
            until,
        )?;
    }
    hash.update(serde_json::to_vec(&defaults).map_err(|e| e.to_string())?);
    let selection = Selection {
        files,
        defaults,
        fingerprint: format!(
            "fontconfig-v2/{}",
            hash.finalize()
                .iter()
                .map(|b| format!("{b:02x}"))
                .collect::<String>()
        ),
        stamps,
        worker,
        cache_hit: false,
        created: Instant::now(),
    };
    selection.validate_current()?;
    cache
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .insert(key, selection.clone());
    Ok(selection)
}
