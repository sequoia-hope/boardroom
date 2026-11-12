//! Gate input records (`design/gates/phaseN.inputs.json`): recomputing file,
//! `Edge.Cuts`, linked-outline, and own-outline hashes at read time to derive
//! gate staleness and Gate 10's "built against" release summary.

use crate::links;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, HashMap};
use std::path::{Component, Path, PathBuf};
use std::sync::{LazyLock, Mutex};
use std::time::{Duration, SystemTime};

const MAX_CACHE_ENTRIES: usize = 8192;
/// Avoid caching files modified within the last 10 ms so rapid in-place writes
/// inside the same kernel mtime tick never serve a stale hash.
const CACHE_MIN_AGE: Duration = Duration::from_millis(10);

static HASH_CACHE: LazyLock<Mutex<HashMap<CacheKey, String>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum HashKind {
    Full,
    Aspect(Aspect),
}

/// A part of a `.kicad_pcb` a gate judged; a record keys it as `<path>#<aspect>`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Aspect {
    EdgeCuts,
    Placement,
    Copper,
}

impl Aspect {
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "edge_cuts" => Some(Self::EdgeCuts),
            "placement" => Some(Self::Placement),
            "copper" => Some(Self::Copper),
            _ => None,
        }
    }

    fn words(self) -> &'static str {
        match self {
            Self::EdgeCuts => "Edge.Cuts",
            Self::Placement => "placement",
            Self::Copper => "copper",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct CacheKey {
    path: PathBuf,
    mtime: SystemTime,
    size: u64,
    kind: HashKind,
}

#[derive(Debug, Clone, Deserialize)]
pub struct GateInputRecord {
    #[serde(default)]
    pub schema: u32,
    pub gate: u32,
    #[serde(default)]
    pub judged_at: String,
    #[serde(default)]
    pub skill: String,
    #[serde(default)]
    pub files: BTreeMap<String, String>,
    #[serde(default)]
    pub links: BTreeMap<String, RecordedLink>,
    #[serde(default)]
    pub own_interface: BTreeMap<String, RecordedInterface>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecordedLink {
    #[serde(default)]
    pub commit: String,
    #[serde(default)]
    pub revision: String,
    #[serde(default)]
    pub geometry_hash: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecordedInterface {
    #[serde(default)]
    pub revision: String,
    #[serde(default)]
    pub geometry_hash: String,
}

/// Gate 10's recorded release inputs ("built against"), surfaced on `project::Summary`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReleaseRecord {
    pub judged_at: String,
    pub skill: String,
    pub links: Vec<ReleaseLink>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReleaseLink {
    pub id: String,
    pub commit: String,
    pub revision: String,
    pub geometry_hash: String,
    /// Current `resolved.commit` in `design/links.toml`, or `None` if removed.
    pub resolved_commit: Option<String>,
}

#[derive(Debug, Clone)]
pub struct CurrentLink {
    pub commit: String,
    pub geometry_hash: String,
}

/// Reads `design/gates/phase{gate}.inputs.json`, if present and valid JSON.
pub fn read_record(root: &Path, gate: u32) -> Option<GateInputRecord> {
    let path = root.join(format!("design/gates/phase{gate}.inputs.json"));
    let text = std::fs::read_to_string(path).ok()?;
    let mut rec: GateInputRecord = serde_json::from_str(&text).ok()?;
    rec.gate = gate;
    Some(rec)
}

/// Reads each link's current `resolved.commit` (from `design/links.toml`) and accepted
/// `geometry_hash` (from `.boardroom/links/<id>/board-shape.json`, falling back to
/// `interface_hash` in `design/links.toml` if not yet materialized).
pub fn current_links(root: &Path) -> HashMap<String, CurrentLink> {
    let Ok(records) = links::read_links_file(root) else {
        return HashMap::new();
    };
    let mut out = HashMap::with_capacity(records.len());
    for rec in records {
        if !is_single_normal_segment(&rec.id) {
            continue;
        }
        let accepted = root
            .join(".boardroom/links")
            .join(&rec.id)
            .join("board-shape.json");
        let geometry_hash = read_manifest_geometry_hash(&accepted)
            .filter(|h| !h.is_empty())
            .unwrap_or_else(|| rec.interface_hash.clone());
        out.insert(
            rec.id,
            CurrentLink {
                commit: rec.resolved.commit,
                geometry_hash,
            },
        );
    }
    out
}

/// Builds Gate 10's "built against" release summary when `design/gates/phase10.inputs.json` exists.
pub fn read_release(
    root: &Path,
    cur_links: &HashMap<String, CurrentLink>,
) -> Option<ReleaseRecord> {
    let rec = read_record(root, 10)?;
    let links = rec
        .links
        .into_iter()
        .map(|(id, l)| {
            let resolved_commit = cur_links.get(&id).map(|c| c.commit.clone());
            ReleaseLink {
                id,
                commit: l.commit,
                revision: l.revision,
                geometry_hash: l.geometry_hash,
                resolved_commit,
            }
        })
        .collect();
    Some(ReleaseRecord {
        judged_at: rec.judged_at,
        skill: rec.skill,
        links,
    })
}

/// Returns the first plain-words reason `rec` no longer matches `root`, or `None` if clean.
pub fn stale_reason(
    root: &Path,
    canon_root: Option<&Path>,
    rec: &GateInputRecord,
    cur_links: &HashMap<String, CurrentLink>,
) -> Option<String> {
    let gate = rec.gate;

    for (key, expected_hash) in &rec.files {
        let (rel, kind) = match key.split_once('#') {
            Some((rel, aspect)) => match Aspect::parse(aspect) {
                Some(a) => (rel, HashKind::Aspect(a)),
                None => return Some(format!("{key} changed since gate {gate}")),
            },
            None => (key.as_str(), HashKind::Full),
        };
        if !is_safe_rel_path(rel) {
            return Some(format!("{rel} changed since gate {gate}"));
        }
        let full = root.join(rel);
        if !full.is_file() {
            return Some(format!("{rel} removed since gate {gate}"));
        }
        let what = match kind {
            HashKind::Aspect(a) => format!(" ({})", a.words()),
            HashKind::Full => String::new(),
        };
        match cached_hash(&full, canon_root, kind) {
            Some(actual) if actual == *expected_hash => {}
            _ => return Some(format!("{rel}{what} changed since gate {gate}")),
        }
    }

    for (id, expected_link) in &rec.links {
        let Some(cur) = cur_links.get(id) else {
            return Some(format!("linked outline {id} was removed since gate {gate}"));
        };
        // Compare `geometry_hash` — NOT `commit` or `revision` — so a quiet commit advance
        // (`NewRevisionNoInterfaceChange`) or a handover without geometry edits never marks
        // the gate stale. When `geometry_hash` does differ and the resolved commit moved,
        // name both short SHAs in the reason.
        if cur.geometry_hash != expected_link.geometry_hash {
            if !cur.commit.is_empty()
                && !expected_link.commit.is_empty()
                && cur.commit != expected_link.commit
            {
                return Some(format!(
                    "linked outline {id} moved to {} (was {})",
                    short_sha(&cur.commit),
                    short_sha(&expected_link.commit)
                ));
            }
            return Some(format!("linked outline {id} changed since gate {gate}"));
        }
    }

    for (board, expected_own) in &rec.own_interface {
        if !is_single_normal_segment(board) {
            return Some("own outline changed".into());
        }
        let manifest = root.join("interface").join(board).join("board-shape.json");
        let cur_geom = read_manifest_geometry_hash(&manifest);
        if cur_geom.as_deref() != Some(expected_own.geometry_hash.as_str()) {
            if rec.own_interface.len() > 1 {
                return Some(format!("own outline ({board}) changed"));
            }
            return Some("own outline changed".into());
        }
    }

    None
}

fn short_sha(sha: &str) -> &str {
    &sha[..sha.len().min(7)]
}

fn is_single_normal_segment(s: &str) -> bool {
    if s.is_empty() || s.contains('/') || s.contains('\\') || s.contains('\0') {
        return false;
    }
    let mut comps = Path::new(s).components();
    matches!(
        (comps.next(), comps.next()),
        (Some(Component::Normal(_)), None)
    )
}

fn is_safe_rel_path(rel: &str) -> bool {
    if rel.is_empty() || rel.starts_with('/') || rel.contains('\\') || rel.contains('\0') {
        return false;
    }
    Path::new(rel)
        .components()
        .all(|c| matches!(c, Component::Normal(_)))
}

fn read_manifest_geometry_hash(path: &Path) -> Option<String> {
    let text = std::fs::read_to_string(path).ok()?;
    let val: serde_json::Value = serde_json::from_str(&text).ok()?;
    val.get("geometry_hash")
        .and_then(serde_json::Value::as_str)
        .map(str::to_string)
}

fn cached_hash(path: &Path, canon_root: Option<&Path>, kind: HashKind) -> Option<String> {
    let meta = std::fs::metadata(path).ok()?;
    if !meta.is_file() {
        return None;
    }
    let canon = std::fs::canonicalize(path).ok()?;
    if let Some(root) = canon_root
        && !canon.starts_with(root)
    {
        return None;
    }
    let mtime = meta.modified().ok();
    let size = meta.len();

    if let Some(mtime) = mtime {
        let key = CacheKey {
            path: canon.clone(),
            mtime,
            size,
            kind,
        };
        if let Some(hit) = HASH_CACHE.lock().unwrap().get(&key).cloned() {
            return Some(hit);
        }
    }

    let computed = match kind {
        HashKind::Full => {
            let bytes = std::fs::read(&canon).ok()?;
            sha256_prefixed(&bytes)
        }
        HashKind::Aspect(aspect) => {
            let text = std::fs::read_to_string(&canon).ok()?;
            pcb_aspect_hash_text(&text, aspect).ok()?
        }
    };

    if let Some(mtime) = mtime
        && mtime.elapsed().is_ok_and(|age| age >= CACHE_MIN_AGE)
    {
        let mut cache = HASH_CACHE.lock().unwrap();
        if cache.len() >= MAX_CACHE_ENTRIES {
            cache.clear();
        }
        cache.insert(
            CacheKey {
                path: canon,
                mtime,
                size,
                kind,
            },
            computed.clone(),
        );
    }

    Some(computed)
}

pub fn sha256_prefixed(bytes: &[u8]) -> String {
    let mut h = Sha256::new();
    h.update(bytes);
    let digest = h.finalize();
    let mut out = String::with_capacity(7 + 64);
    out.push_str("sha256:");
    for b in digest {
        use std::fmt::Write as _;
        let _ = write!(&mut out, "{b:02x}");
    }
    out
}

// ------------------------------------- .kicad_pcb aspects (gates 4, 6, 7, 8)
//
// A record keys an aspect hash as `<path>#<aspect>`; a key without `#` is a
// sha256 of the file's bytes. Shared verbatim with
// `.claude/skills/pcb-design/scripts/gate_inputs.py` (`pcb_aspect_hash_text`):
//
// 1. Tokenize the `.kicad_pcb` UTF-8 text into S-expression lists and string
//    atoms. Whitespace (`' '`, `'\t'`, `'\n'`, `'\r'`) separates tokens outside
//    quotes; `"..."` unescapes `\n` -> LF, `\r` -> CR, `\t` -> TAB, and `\<c>`
//    -> `<c>`; unquoted atoms keep their raw text.
// 2. From the root `(kicad_pcb ...)` list, collect:
//    a) Every direct child list whose head atom starts with `"gr_"` and that
//       sits on `"Edge.Cuts"`, i.e. has a direct child `(layer "Edge.Cuts")`
//       (or `(layers ... "Edge.Cuts" ...)`).
//    b) Inside every direct `(footprint ...)` or `(module ...)` child, for each
//       direct child list whose head starts with `"fp_"` and sits on
//       `"Edge.Cuts"`, emit `("footprint_edge" <at_or_empty> <fp_item>)` where
//       `<at_or_empty>` is the footprint's first direct `(at ...)` child or
//       `("at")`. Footprints without `Edge.Cuts` items are ignored.
//    That set is the `edge_cuts` aspect. The `placement` aspect is instead, for
//    every direct `(footprint ...)`/`(module ...)` child, the list
//    `("footprint" <lib id or ""> <reference or ""> <first (at ...) or ("at")>
//    <first (layer ...) or ("layer")>)`; the reference is the value of
//    `(property "Reference" X)` or `(fp_text reference X)`. The `copper` aspect
//    is the placement items plus every direct child whose head is `segment`,
//    `arc`, `via` or `zone`, and every `gr_*` child whose `(layer X)` ends in
//    `.Cu`; inside those, sublists headed `filled_polygon` or `fill_segments`
//    are dropped too (a zone refill doesn't change copper intent).
// 3. Canonicalize each collected item:
//    - Recursively drop sublists whose head atom is `"uuid"` or `"tstamp"`.
//    - Format an atom bare when it is non-empty and every byte is in
//      `[A-Za-z0-9_.:+/-]`; otherwise quote it with `"..."` escaping `\`, `"`,
//      `\n`, `\r`, `\t`.
//    - Format a list as `(` + single-space-joined children + `)`.
// 4. Sort the canonicalized item strings by UTF-8 byte order, join them with
//    `"\n"`, and return `"sha256:" + hex(sha256(utf8_bytes))`.

#[derive(Debug, Clone)]
enum Sexpr {
    Atom(String),
    List(Vec<Sexpr>),
}

impl Sexpr {
    fn head(&self) -> Option<&str> {
        match self {
            Self::List(items) => match items.first() {
                Some(Self::Atom(s)) => Some(s.as_str()),
                _ => None,
            },
            Self::Atom(_) => None,
        }
    }
}

fn parse_sexpr(text: &str) -> Result<Sexpr, String> {
    let mut stack: Vec<Vec<Sexpr>> = vec![vec![]];
    let mut chars = text.char_indices().peekable();

    while let Some(&(idx, ch)) = chars.peek() {
        if matches!(ch, ' ' | '\t' | '\n' | '\r') {
            chars.next();
            continue;
        }
        if ch == '(' {
            chars.next();
            stack.push(vec![]);
        } else if ch == ')' {
            chars.next();
            if stack.len() == 1 {
                return Err(format!("unbalanced ')' at byte {idx}"));
            }
            let done = stack.pop().unwrap();
            stack.last_mut().unwrap().push(Sexpr::List(done));
        } else if ch == '"' {
            chars.next();
            let mut buf = String::new();
            let mut closed = false;
            while let Some((_, c)) = chars.next() {
                if c == '"' {
                    closed = true;
                    break;
                }
                if c == '\\' {
                    let Some((_, esc)) = chars.next() else {
                        return Err("unterminated escape in quoted string".into());
                    };
                    match esc {
                        'n' => buf.push('\n'),
                        'r' => buf.push('\r'),
                        't' => buf.push('\t'),
                        other => buf.push(other),
                    }
                } else {
                    buf.push(c);
                }
            }
            if !closed {
                return Err("unterminated quoted string".into());
            }
            stack.last_mut().unwrap().push(Sexpr::Atom(buf));
        } else {
            let start = idx;
            let mut end = text.len();
            while let Some(&(i, c)) = chars.peek() {
                if matches!(c, ' ' | '\t' | '\n' | '\r' | '(' | ')' | '"') {
                    end = i;
                    break;
                }
                chars.next();
            }
            stack
                .last_mut()
                .unwrap()
                .push(Sexpr::Atom(text[start..end].to_string()));
        }
    }

    if stack.len() != 1 {
        return Err("unbalanced '(' in S-expression".into());
    }
    stack[0]
        .drain(..)
        .next()
        .ok_or_else(|| "empty S-expression".into())
}

fn is_on_edge_cuts(items: &[Sexpr]) -> bool {
    items.iter().skip(1).any(|c| {
        let Sexpr::List(sub) = c else { return false };
        match c.head() {
            Some("layer") => matches!(sub.get(1), Some(Sexpr::Atom(s)) if s == "Edge.Cuts"),
            Some("layers") => sub
                .iter()
                .skip(1)
                .any(|a| matches!(a, Sexpr::Atom(s) if s == "Edge.Cuts" || s == "*.Cuts")),
            _ => false,
        }
    })
}

fn is_bare_atom_byte(b: u8) -> bool {
    b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-' | b'.' | b'+' | b':' | b'/')
}

fn canon_atom(s: &str) -> String {
    if !s.is_empty() && s.bytes().all(is_bare_atom_byte) {
        return s.to_string();
    }
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for ch in s.chars() {
        match ch {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            other => out.push(other),
        }
    }
    out.push('"');
    out
}

fn canon_sexpr(node: &Sexpr) -> String {
    match node {
        Sexpr::Atom(s) => canon_atom(s),
        Sexpr::List(children) => {
            let mut kept = Vec::with_capacity(children.len());
            for c in children {
                if matches!(c.head(), Some("uuid" | "tstamp")) {
                    continue;
                }
                kept.push(canon_sexpr(c));
            }
            format!("({})", kept.join(" "))
        }
    }
}

fn first_child(list: &[Sexpr], head: &str, default: &str) -> Sexpr {
    list.iter()
        .skip(1)
        .find(|sub| sub.head() == Some(head))
        .cloned()
        .unwrap_or_else(|| Sexpr::List(vec![Sexpr::Atom(default.into())]))
}

fn footprint_reference(fp: &[Sexpr]) -> String {
    for sub in fp.iter().skip(1) {
        let Sexpr::List(items) = sub else { continue };
        let (Some(Sexpr::Atom(key)), Some(Sexpr::Atom(val))) = (items.get(1), items.get(2)) else {
            continue;
        };
        match sub.head() {
            Some("property") if key == "Reference" => return val.clone(),
            Some("fp_text") if key == "reference" => return val.clone(),
            _ => {}
        }
    }
    String::new()
}

fn on_copper_layer(list: &[Sexpr]) -> bool {
    let Sexpr::List(layer) = first_child(list, "layer", "layer") else {
        return false;
    };
    matches!(layer.get(1), Some(Sexpr::Atom(l)) if l.ends_with(".Cu"))
}

fn drop_fills(node: &Sexpr) -> Sexpr {
    match node {
        Sexpr::Atom(_) => node.clone(),
        Sexpr::List(children) => Sexpr::List(
            children
                .iter()
                .filter(|c| !matches!(c.head(), Some("filled_polygon" | "fill_segments")))
                .map(drop_fills)
                .collect(),
        ),
    }
}

/// Computes one aspect's canonical hash (`"sha256:<hex>"`) from `.kicad_pcb` UTF-8 text.
pub fn pcb_aspect_hash_text(text: &str, aspect: Aspect) -> Result<String, String> {
    let Sexpr::List(children) = parse_sexpr(text)? else {
        return Err("expected top-level S-expression list".into());
    };
    let mut items = Vec::new();
    for child in children.iter().skip(1) {
        let Sexpr::List(list) = child else { continue };
        let Some(h) = child.head() else { continue };
        let is_footprint = h == "footprint" || h == "module";
        if aspect == Aspect::EdgeCuts {
            if h.starts_with("gr_") && is_on_edge_cuts(list) {
                items.push(canon_sexpr(child));
            } else if is_footprint {
                let at_node = first_child(list, "at", "at");
                for sub in list.iter().skip(1) {
                    let Sexpr::List(sub_list) = sub else { continue };
                    if sub.head().is_some_and(|sh| sh.starts_with("fp_"))
                        && is_on_edge_cuts(sub_list)
                    {
                        let wrapped = Sexpr::List(vec![
                            Sexpr::Atom("footprint_edge".into()),
                            at_node.clone(),
                            sub.clone(),
                        ]);
                        items.push(canon_sexpr(&wrapped));
                    }
                }
            }
            continue;
        }
        if is_footprint {
            let lib_id = match list.get(1) {
                Some(Sexpr::Atom(s)) => s.clone(),
                _ => String::new(),
            };
            let placed = Sexpr::List(vec![
                Sexpr::Atom("footprint".into()),
                Sexpr::Atom(lib_id),
                Sexpr::Atom(footprint_reference(list)),
                first_child(list, "at", "at"),
                first_child(list, "layer", "layer"),
            ]);
            items.push(canon_sexpr(&placed));
        } else if aspect == Aspect::Copper
            && (matches!(h, "segment" | "arc" | "via" | "zone")
                || (h.starts_with("gr_") && on_copper_layer(list)))
        {
            items.push(canon_sexpr(&drop_fills(child)));
        }
    }
    items.sort();
    let payload = items.join("\n");
    Ok(sha256_prefixed(payload.as_bytes()))
}

/// Computes the canonical `Edge.Cuts` hash (`"sha256:<hex>"`) from `.kicad_pcb` UTF-8 text.
pub fn edge_cuts_hash_text(text: &str) -> Result<String, String> {
    pcb_aspect_hash_text(text, Aspect::EdgeCuts)
}

/// Computes one aspect's canonical hash for a `.kicad_pcb` file.
pub fn pcb_aspect_hash(pcb_path: &Path, aspect: Aspect) -> Result<String, String> {
    let text =
        std::fs::read_to_string(pcb_path).map_err(|e| format!("{}: {e}", pcb_path.display()))?;
    pcb_aspect_hash_text(&text, aspect)
}

/// Computes the canonical `Edge.Cuts` hash (`"sha256:<hex>"`) for a `.kicad_pcb` file.
pub fn edge_cuts_hash(pcb_path: &Path) -> Result<String, String> {
    let text =
        std::fs::read_to_string(pcb_path).map_err(|e| format!("{}: {e}", pcb_path.display()))?;
    edge_cuts_hash_text(&text)
}
