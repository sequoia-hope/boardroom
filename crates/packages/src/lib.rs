//! The packages in `packages/` as the sources of boards' blocks (docs/one-system-spec.md, "Every
//! circuit has a source").
//!
//! A package names itself and its version in `manifest.toml`, and each of its layouts may carry
//! `areas.json`: the copper the block pours round the chip and the rule areas that say which
//! nets may cross where, in the layout's frame with nets named by the chip's pins. [`place`]
//! puts them on a board round one chip, as zones and `.kicad_dru` rules in the board's frame and
//! nets; [`compare`] finds each on the board (by name, or by shape where a board drew it by hand
//! under another name) and says how it differs; [`accept`] writes chosen ones into the board's
//! files. A board's pins, and the changes it declined, are in `<board>.blocks.json` beside it.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use kicad::{Keepout, NewZone, PadJoin};
use serde::{Deserialize, Serialize};

const MM: f64 = 1e-3;
/// How far a point may sit from where the source puts it and still be the same, in metres.
const SAME: f64 = 5e-6;

/// One package: `packages/<name>/`.
#[derive(Debug, Clone)]
pub struct Package {
    pub name: String,
    pub version: String,
    pub summary: String,
    pub dir: PathBuf,
    pub variants: Vec<Variant>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Variant {
    pub name: String,
    #[serde(default)]
    pub footprint: String,
    #[serde(default)]
    pub layouts: Vec<String>,
}

#[derive(Deserialize)]
struct Manifest {
    package: Head,
    #[serde(default)]
    variants: Vec<Variant>,
}

#[derive(Deserialize)]
struct Head {
    name: String,
    version: String,
    #[serde(default)]
    summary: String,
}

impl Package {
    pub fn load(dir: &Path) -> Result<Package, String> {
        let path = dir.join("manifest.toml");
        let text = std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        let m: Manifest = toml::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))?;
        Ok(Package { name: m.package.name, version: m.package.version, summary: m.package.summary, dir: dir.to_path_buf(), variants: m.variants })
    }

    /// The variant a layout is for.
    pub fn variant_of(&self, layout: &str) -> Option<&Variant> {
        self.variants.iter().find(|v| v.layouts.iter().any(|l| l == layout))
    }

    /// A variant's pins, number to name, from `ports.json`.
    pub fn pins(&self, variant: &str) -> Result<BTreeMap<String, String>, String> {
        let path = self.dir.join("ports.json");
        let v: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?).map_err(|e| e.to_string())?;
        let pins = v["variants"][variant]["pins"].as_object().ok_or(format!("{}: no pins for {variant}", path.display()))?;
        Ok(pins.iter().filter_map(|(n, p)| Some((n.clone(), p["name"].as_str()?.to_string()))).collect())
    }

    /// A layout's `areas.json`; empty when the layout has none.
    pub fn areas(&self, layout: &str) -> Result<Areas, String> {
        let path = self.dir.join("layouts").join(layout).join("areas.json");
        if !path.is_file() {
            return Ok(Areas::default());
        }
        serde_json::from_str(&std::fs::read_to_string(&path).map_err(|e| e.to_string())?).map_err(|e| format!("{}: {e}", path.display()))
    }
}

/// Every package under `root`, by name.
pub fn all(root: &Path) -> Result<Vec<Package>, String> {
    let mut dirs: Vec<PathBuf> = std::fs::read_dir(root)
        .map_err(|e| format!("{}: {e}", root.display()))?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.join("manifest.toml").is_file())
        .collect();
    dirs.sort();
    dirs.iter().map(|d| Package::load(d)).collect()
}

/// Where on the board, relative to the chip: its own side, the other outer layer, or every
/// inner layer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Side {
    Chip,
    Far,
    Inner,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(untagged)]
pub enum Sides {
    One(Side),
    Many(Vec<Side>),
}

impl Sides {
    fn list(&self) -> Vec<Side> {
        match self {
            Sides::One(s) => vec![*s],
            Sides::Many(v) => v.clone(),
        }
    }
}

/// Copper the block pours: `net` is a pin name.
#[derive(Debug, Clone, Deserialize)]
pub struct Pour {
    pub name: String,
    pub why: String,
    pub since: String,
    pub net: String,
    pub layers: Sides,
    /// With `inner`: only this many inner layers, the nearest the chip first.
    #[serde(default)]
    pub inner_nearest: Option<usize>,
    #[serde(default)]
    pub priority: u32,
    #[serde(default)]
    pub pads: PadJoin,
    #[serde(default)]
    pub clearance: Option<f64>,
    #[serde(default)]
    pub min_thickness: Option<f64>,
    #[serde(default)]
    pub thermal_gap: Option<f64>,
    #[serde(default)]
    pub thermal_bridge_width: Option<f64>,
    /// The pins whose copper alone may cut it (pin names): it is in sections where they cross
    /// it, and other nets' vias may pass through it so long as each section still joins its
    /// pins inside, by copper at least the fab's narrowest track for the stackup
    /// ([`Chip::sections`]).
    #[serde(default)]
    pub keep_joined: Option<Vec<String>>,
    pub polygon: Vec<[f64; 2]>,
}

/// The nets (pin names) whose tracks, or vias, may enter an area; absent, the area says nothing
/// about them.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct Only {
    #[serde(default)]
    pub tracks: Option<Vec<String>>,
    #[serde(default)]
    pub vias: Option<Vec<String>>,
}

/// A rule area: what it bars outright (`keepout`: `tracks`, `vias`, `pads`, `copperpour`,
/// `footprints`) and which nets alone may enter it (`only`).
#[derive(Debug, Clone, Deserialize)]
pub struct Area {
    pub name: String,
    pub why: String,
    pub since: String,
    pub layers: Sides,
    /// With `inner`: only this many inner layers, the nearest the chip first.
    #[serde(default)]
    pub inner_nearest: Option<usize>,
    pub polygon: Vec<[f64; 2]>,
    #[serde(default)]
    pub only: Only,
    #[serde(default)]
    pub keepout: Vec<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct Areas {
    #[serde(default)]
    pub pours: Vec<Pour>,
    #[serde(default)]
    pub areas: Vec<Area>,
}

/// A block's areas placed round one chip of a board.
#[derive(Debug, Clone)]
pub struct Placed {
    pub chip: String,
    pub package: String,
    pub version: String,
    pub layout: String,
    pub entries: Vec<Entry>,
}

/// One pour or rule area, with the `.kicad_dru` rules that give it meaning.
#[derive(Debug, Clone)]
pub struct Entry {
    /// Its name in the source (`ring copper`); the zone is named `<chip> <name>`.
    pub name: String,
    pub why: String,
    pub since: String,
    pub zone: NewZone,
    /// `(name, text)` of each rule, the text a whole `(rule …)` with a comment line above.
    pub rules: Vec<(String, String)>,
    /// A pour's nets (on this board) that alone may cut it into sections, each section to stay
    /// joined inside ([`Pour::keep_joined`]).
    pub keep_joined: Option<Vec<String>>,
}

impl Entry {
    pub fn is_pour(&self) -> bool {
        self.zone.keepout.is_none()
    }
}

/// A transform from the layout's frame (mm) to the board's (m): `p ↦ o + R·(x, m·y)`.
#[derive(Debug, Clone, Copy)]
struct Frame {
    o: [f64; 2],
    c: f64,
    s: f64,
    mirror: f64,
    from: [f64; 2],
}

impl Frame {
    fn apply(&self, p: [f64; 2]) -> [f64; 2] {
        let (x, y) = (p[0] * MM - self.from[0], (p[1] * MM - self.from[1]) * self.mirror);
        [self.o[0] + x * self.c - y * self.s, self.o[1] + x * self.s + y * self.c]
    }
}

/// The rigid motion, mirrored or not, that takes `src` nearest to `dst`, and the farthest any
/// point then lies from its partner.
fn fit(src: &[[f64; 2]], dst: &[[f64; 2]]) -> (Frame, f64) {
    let n = src.len() as f64;
    let mean = |v: &[[f64; 2]]| [v.iter().map(|p| p[0]).sum::<f64>() / n, v.iter().map(|p| p[1]).sum::<f64>() / n];
    let (sm, dm) = (mean(src), mean(dst));
    let mut best: Option<(Frame, f64)> = None;
    for mirror in [1.0, -1.0] {
        let (mut a, mut b) = (0.0, 0.0);
        for (p, q) in src.iter().zip(dst) {
            let (x, y) = (p[0] - sm[0], (p[1] - sm[1]) * mirror);
            let (u, v) = (q[0] - dm[0], q[1] - dm[1]);
            a += x * u + y * v;
            b += x * v - y * u;
        }
        let th = b.atan2(a);
        let f = Frame { o: dm, c: th.cos(), s: th.sin(), mirror, from: sm };
        let err = src
            .iter()
            .zip(dst)
            .map(|(p, q)| {
                let r = f.apply([p[0] / MM, p[1] / MM]);
                (r[0] - q[0]).hypot(r[1] - q[1])
            })
            .fold(0.0, f64::max);
        if best.as_ref().is_none_or(|b| err < b.1) {
            best = Some((f, err));
        }
    }
    best.unwrap()
}

/// Pads of a footprint by number, only the numbers that one pad has.
fn single_pads(fp: &kicad::Footprint) -> BTreeMap<&str, [f64; 2]> {
    let mut count: BTreeMap<&str, usize> = BTreeMap::new();
    for p in &fp.pads {
        *count.entry(p.number.as_str()).or_default() += 1;
    }
    fp.pads.iter().filter(|p| !p.number.is_empty() && count[p.number.as_str()] == 1).map(|p| (p.number.as_str(), p.position)).collect()
}

/// `package`'s `layout` placed round the chip `chip` of `pcb` (read with
/// `kicad::parse_kicad_pcb`): the chip is matched pad for pad with the layout's fixture, so it
/// may sit on either side, turned any way.
pub fn place(package: &Package, layout: &str, pcb: &kicad::Pcb, chip: &str) -> Result<Placed, String> {
    let variant = package.variant_of(layout).ok_or(format!("{} has no layout {layout}", package.name))?;
    let pins = package.pins(&variant.name)?;
    let areas = package.areas(layout)?;
    let fixture_path = package.dir.join("layouts").join(layout).join("fixture.kicad_pcb");
    let fixture = kicad::parse_kicad_pcb(&std::fs::read_to_string(&fixture_path).map_err(|e| format!("{}: {e}", fixture_path.display()))?)
        .map_err(|e| format!("{}: {e}", fixture_path.display()))?;
    let fp = pcb.footprints.iter().find(|f| f.reference == chip).ok_or(format!("the board has no {chip}"))?;
    let ours = single_pads(fp);
    let theirs = fixture
        .footprints
        .iter()
        .max_by_key(|f| single_pads(f).keys().filter(|k| ours.contains_key(*k)).count())
        .ok_or("the layout's fixture has no parts")?;
    let theirs_pads = single_pads(theirs);
    let (mut src, mut dst) = (Vec::new(), Vec::new());
    for (n, p) in &theirs_pads {
        if let Some(q) = ours.get(n) {
            src.push([p[0] - theirs.at[0], p[1] - theirs.at[1]]);
            dst.push(*q);
        }
    }
    if src.len() < 3 {
        return Err(format!("{chip} shares too few pads with {}'s {layout} chip", package.name));
    }
    let (frame, err) = fit(&src, &dst);
    if err > 10e-6 {
        return Err(format!("{chip}'s pads sit {:.3} mm from where {}'s {layout} has them: not the same footprint", err / MM, package.name));
    }

    let chip_layer = if fp.side == kicad::Side::Front { "F.Cu" } else { "B.Cu" };
    let far = if chip_layer == "F.Cu" { "B.Cu" } else { "F.Cu" };
    // Inner layers, the nearest the chip first.
    let mut inner: Vec<String> = (1..pcb.copper_layers.saturating_sub(1)).map(|i| format!("In{i}.Cu")).collect();
    if chip_layer == "B.Cu" {
        inner.reverse();
    }
    let layers = |sides: &Sides, nearest: Option<usize>| -> Vec<String> {
        let mut out = Vec::new();
        for s in sides.list() {
            match s {
                Side::Chip => out.push(chip_layer.to_string()),
                Side::Far => out.push(far.to_string()),
                Side::Inner => out.extend(inner.iter().take(nearest.unwrap_or(usize::MAX)).cloned()),
            }
        }
        out.sort();
        out
    };
    // Each pin name's net on this board.
    let mut nets: BTreeMap<&str, BTreeSet<(u32, &str)>> = BTreeMap::new();
    for p in &fp.pads {
        if let (Some(name), Some(net)) = (pins.get(&p.number), &p.net) {
            if net.number != 0 {
                nets.entry(name.as_str()).or_default().insert((net.number, net.name.as_str()));
            }
        }
    }
    let net_of = |pin: &str| -> Result<(u32, String), String> {
        let found = nets.get(pin).ok_or(format!("no pin of {chip} named {pin} has a net"))?;
        if found.len() > 1 {
            return Err(format!("{chip}'s {pin} pins are on {} nets", found.len()));
        }
        let (n, name) = found.iter().next().unwrap();
        Ok((pcb.file_net(*n), name.to_string()))
    };
    let outline = |poly: &[[f64; 2]]| poly.iter().map(|p| frame.apply(*p)).collect::<Vec<_>>();
    let tag = format!("{}@{} {layout}", package.name, package.version);

    let mut entries = Vec::new();
    for p in &areas.pours {
        let (net, net_name) = net_of(&p.net)?;
        let keep_joined = match &p.keep_joined {
            Some(list) => Some(list.iter().map(|pin| net_of(pin).map(|n| n.1)).collect::<Result<Vec<_>, _>>()?),
            None => None,
        };
        entries.push(Entry {
            name: p.name.clone(),
            why: p.why.clone(),
            since: p.since.clone(),
            zone: NewZone {
                name: format!("{chip} {}", p.name),
                net,
                net_name,
                layers: layers(&p.layers, p.inner_nearest),
                outline: outline(&p.polygon),
                priority: p.priority,
                keepout: None,
                pads: p.pads,
                clearance: p.clearance.unwrap_or(0.0) * MM,
                min_thickness: p.min_thickness.unwrap_or(0.25) * MM,
                thermal_gap: p.thermal_gap.unwrap_or(0.5) * MM,
                thermal_bridge_width: p.thermal_bridge_width.unwrap_or(0.5) * MM,
                locked: true,
            },
            rules: Vec::new(),
            keep_joined,
        });
    }
    for a in &areas.areas {
        let bars = |w: &str| a.keepout.iter().any(|k| k == w);
        let zone_name = format!("{chip} {}", a.name);
        let mut rules = Vec::new();
        let named = |list: &Vec<String>| -> Result<Vec<String>, String> {
            let mut v = list.iter().map(|p| net_of(p).map(|n| n.1)).collect::<Result<Vec<_>, _>>()?;
            v.sort();
            v.dedup();
            Ok(v)
        };
        let tracks = a.only.tracks.as_ref().map(named).transpose()?;
        let vias = a.only.vias.as_ref().map(named).transpose()?;
        let rule = |suffix: &str, words: &str, allowed: &[String]| {
            let name = format!("{zone_name} foreign{suffix}");
            let cond = std::iter::once(format!("A.intersectsArea('{zone_name}')"))
                .chain(allowed.iter().map(|n| format!("A.NetName != '{}'", n.replace('\'', "\\'"))))
                .collect::<Vec<_>>()
                .join(" && ");
            (name.clone(), format!("# {tag}: {}\n(rule \"{name}\"\n  (condition \"{cond}\")\n  (constraint disallow {words}))", a.why))
        };
        match (&tracks, &vias) {
            (Some(t), Some(v)) if t == v => rules.push(rule("", "track via", t)),
            _ => {
                if let Some(t) = &tracks {
                    rules.push(rule(" tracks", "track", t));
                }
                if let Some(v) = &vias {
                    rules.push(rule(" vias", "via", v));
                }
            }
        }
        entries.push(Entry {
            name: a.name.clone(),
            why: a.why.clone(),
            since: a.since.clone(),
            zone: NewZone {
                name: zone_name,
                net: 0,
                net_name: String::new(),
                layers: layers(&a.layers, a.inner_nearest),
                outline: outline(&a.polygon),
                priority: 0,
                keepout: Some(Keepout {
                    tracks: bars("tracks"),
                    vias: bars("vias"),
                    pads: bars("pads"),
                    copperpour: bars("copperpour"),
                    footprints: bars("footprints"),
                }),
                pads: PadJoin::Reliefs,
                clearance: 0.0,
                min_thickness: 0.25 * MM,
                thermal_gap: 0.5 * MM,
                thermal_bridge_width: 0.5 * MM,
                locked: true,
            },
            rules,
            keep_joined: None,
        });
    }
    // An entry for layers the board does not have (inner layers on a two-layer board) is not
    // part of the block there.
    entries.retain(|e| !e.zone.layers.is_empty());
    Ok(Placed { chip: chip.to_string(), package: package.name.clone(), version: package.version.clone(), layout: layout.to_string(), entries })
}

/// How a board's copy of an entry stands against the source.
#[derive(Debug, Clone, PartialEq)]
pub enum State {
    Same,
    Missing,
    /// Each difference in plain words.
    Differs(Vec<String>),
}

/// An entry found on a board: its state, and the board's zones (indices into
/// `Copper::zones`) and rules (names) that are its copy.
#[derive(Debug, Clone)]
pub struct Found {
    pub state: State,
    pub zones: Vec<usize>,
    pub rules: Vec<String>,
}

/// The farthest a vertex of either outline lies from the other's vertices, in metres.
fn outline_gap(a: &[[f64; 2]], b: &[[f64; 2]]) -> f64 {
    let one = |a: &[[f64; 2]], b: &[[f64; 2]]| {
        a.iter().map(|p| b.iter().map(|q| (p[0] - q[0]).hypot(p[1] - q[1])).fold(f64::INFINITY, f64::min)).fold(0.0, f64::max)
    };
    one(a, b).max(one(b, a))
}

/// How much two outlines' bounding boxes overlap: their intersection over their union.
fn overlap(a: &[[f64; 2]], b: &[[f64; 2]]) -> f64 {
    let bounds = |p: &[[f64; 2]]| {
        p.iter().fold([f64::INFINITY, f64::INFINITY, f64::NEG_INFINITY, f64::NEG_INFINITY], |r, q| [r[0].min(q[0]), r[1].min(q[1]), r[2].max(q[0]), r[3].max(q[1])])
    };
    let (a, b) = (bounds(a), bounds(b));
    let area = |r: [f64; 4]| (r[2] - r[0]).max(0.0) * (r[3] - r[1]).max(0.0);
    let i = area([a[0].max(b[0]), a[1].max(b[1]), a[2].min(b[2]), a[3].min(b[3])]);
    let u = area(a) + area(b) - i;
    if u > 0.0 { i / u } else { 0.0 }
}

/// A rule's text reduced to what it means for an area: the area's name, the nets let through
/// and the words it disallows.
fn rule_meaning(text: &str) -> Option<(String, BTreeSet<String>, BTreeSet<String>)> {
    // The comment lines above it say nothing.
    let text = &text[text.find("(rule")?..];
    let area = text.split("intersectsArea('").nth(1)?.split('\'').next()?.to_string();
    let nets = text.split("NetName != '").skip(1).filter_map(|s| s.split('\'').next()).map(str::to_string).collect();
    let words = text.split("disallow").nth(1)?.split(')').next()?.split_whitespace().map(str::to_string).collect();
    Some((area, nets, words))
}

/// Each of `placed`'s entries as `zones` (the board's, from `kicad::read_copper`) and the rules
/// file `dru` have it.
pub fn compare(placed: &Placed, zones: &[kicad::Zone], dru: &str) -> Vec<Found> {
    let rules: Vec<(String, String)> = kicad::dru::rule_spans(dru).into_iter().map(|(n, s)| (n, dru[s].to_string())).collect();
    // Each entry's copy on the board: by name, else by the same shape, else (once every exact
    // match is taken) the zones of its kind on its layers whose bounds mostly overlap its own,
    // one per layer, the nearest first.
    const OVERLAP: f64 = 0.7;
    let kind_matches = |b: &kicad::Zone, z: &NewZone| b.footprint.is_none() && b.keepout.is_some() == z.keepout.is_some() && b.net_name == z.net_name && b.layers.iter().any(|l| z.layers.contains(l));
    let mut used = BTreeSet::new();
    let mut matched: Vec<(Vec<usize>, bool)> = Vec::new();
    for e in &placed.entries {
        let z = &e.zone;
        let mut mine: Vec<usize> = (0..zones.len()).filter(|&i| zones[i].name == z.name && !used.contains(&i)).collect();
        let by_name = !mine.is_empty();
        if !by_name {
            mine = (0..zones.len()).filter(|&i| !used.contains(&i) && kind_matches(&zones[i], z) && outline_gap(&zones[i].outline, &z.outline) < SAME).collect();
        }
        used.extend(mine.iter().copied());
        matched.push((mine, by_name));
    }
    for (e, (mine, _)) in placed.entries.iter().zip(matched.iter_mut()) {
        if mine.is_empty() {
            let mut near: Vec<(usize, f64)> = (0..zones.len())
                .filter(|&i| !used.contains(&i) && kind_matches(&zones[i], &e.zone))
                .map(|i| (i, overlap(&zones[i].outline, &e.zone.outline)))
                .filter(|&(_, o)| o > OVERLAP)
                .collect();
            near.sort_by(|a, b| b.1.total_cmp(&a.1));
            let mut layers = BTreeSet::new();
            for (i, _) in near {
                if zones[i].layers.iter().all(|l| layers.insert(l.clone())) {
                    mine.push(i);
                    used.insert(i);
                }
            }
        }
    }
    let mut out = Vec::new();
    for (e, (mine, by_name)) in placed.entries.iter().zip(matched) {
        let z = &e.zone;
        if mine.is_empty() {
            out.push(Found { state: State::Missing, zones: mine, rules: Vec::new() });
            continue;
        }
        let mut diff = Vec::new();
        let names: BTreeSet<&str> = mine.iter().map(|&i| zones[i].name.as_str()).collect();
        if !by_name {
            diff.push(format!("named {} on the board", names.iter().map(|n| format!("'{n}'")).collect::<Vec<_>>().join(", ")));
        }
        let have: BTreeSet<&str> = mine.iter().flat_map(|&i| zones[i].layers.iter().map(String::as_str)).collect();
        let want: BTreeSet<&str> = z.layers.iter().map(String::as_str).collect();
        let lacks: Vec<_> = want.difference(&have).collect();
        let extra: Vec<_> = have.difference(&want).collect();
        if !lacks.is_empty() {
            diff.push(format!("not on {}", lacks.iter().map(|s| s.to_string()).collect::<Vec<_>>().join(", ")));
        }
        if !extra.is_empty() {
            diff.push(format!("also on {}", extra.iter().map(|s| s.to_string()).collect::<Vec<_>>().join(", ")));
        }
        for &i in &mine {
            let b = &zones[i];
            if b.net_name != z.net_name {
                diff.push(format!("on net {} rather than {}", b.net_name, z.net_name));
            }
            let gap = outline_gap(&b.outline, &z.outline);
            if gap >= 0.5e-3 {
                diff.push("its outline is another shape".to_string());
            } else if gap >= SAME {
                diff.push(format!("its outline is up to {:.3} mm from the source's", gap / MM));
            }
            if b.keepout != z.keepout {
                let bars = |k: &Option<Keepout>| {
                    let k = k.unwrap_or_default();
                    let v: Vec<&str> = [(k.tracks, "tracks"), (k.vias, "vias"), (k.pads, "pads"), (k.copperpour, "pours"), (k.footprints, "footprints")]
                        .iter()
                        .filter(|x| x.0)
                        .map(|x| x.1)
                        .collect();
                    if v.is_empty() { "nothing".to_string() } else { v.join(", ") }
                };
                diff.push(format!("bars {} rather than {}", bars(&b.keepout), bars(&z.keepout)));
            }
            if z.keepout.is_none() {
                if b.priority != z.priority {
                    diff.push(format!("priority {} rather than {}", b.priority, z.priority));
                }
                let join = |b: &kicad::Zone| {
                    if b.no_pads {
                        PadJoin::None
                    } else if b.solid_pads {
                        PadJoin::Solid
                    } else if b.reliefs_tht_only {
                        PadJoin::ReliefsThroughHole
                    } else {
                        PadJoin::Reliefs
                    }
                };
                if join(b) != z.pads {
                    diff.push(format!("joins pads {:?} rather than {:?}", join(b), z.pads).to_lowercase());
                }
                for (what, was, is) in [("clearance", b.clearance, z.clearance), ("narrowest copper", b.min_thickness, z.min_thickness)] {
                    if (was - is).abs() > 1e-9 {
                        diff.push(format!("{what} {} mm rather than {} mm", kicad::mm(was), kicad::mm(is)));
                    }
                }
            }
        }
        // The rules that give an area its meaning, by name or by what they say about its zone.
        let mut found_rules = Vec::new();
        for (name, text) in &e.rules {
            let want = rule_meaning(text);
            let theirs = rules.iter().find(|(n, _)| n == name).or_else(|| {
                rules.iter().find(|(_, t)| match (rule_meaning(t), &want) {
                    (Some((area, nets, words)), Some((_, wn, ww))) => names.contains(area.as_str()) && &nets == wn && &words == ww,
                    _ => false,
                })
            });
            match theirs {
                None => diff.push(format!("rule '{name}' missing")),
                Some((n, t)) => {
                    found_rules.push(n.clone());
                    let (got, want) = (rule_meaning(t), want.clone());
                    match (got, want) {
                        (Some((_, gn, gw)), Some((_, wn, ww))) if gn == wn && gw == ww => {}
                        (Some((_, gn, gw)), Some((_, wn, ww))) => {
                            let let_in: Vec<_> = gn.difference(&wn).cloned().collect();
                            let shuts: Vec<_> = wn.difference(&gn).cloned().collect();
                            if !let_in.is_empty() {
                                diff.push(format!("rule '{n}' also lets {} through", let_in.join(", ")));
                            }
                            if !shuts.is_empty() {
                                diff.push(format!("rule '{n}' keeps {} out", shuts.join(", ")));
                            }
                            if gw != ww {
                                diff.push(format!("rule '{n}' disallows {} rather than {}", gw.into_iter().collect::<Vec<_>>().join(" "), ww.into_iter().collect::<Vec<_>>().join(" ")));
                            }
                        }
                        _ => diff.push(format!("rule '{n}' says something else")),
                    }
                    if n != name && !diff.iter().any(|d| d.starts_with("named")) {
                        diff.push(format!("its rule is named '{n}'"));
                    }
                }
            }
        }
        let mut seen = BTreeSet::new();
        diff.retain(|d| seen.insert(d.clone()));
        let state = if diff.is_empty() { State::Same } else { State::Differs(diff) };
        out.push(Found { state, zones: mine, rules: found_rules });
    }
    out
}

/// The board's files with the chosen entries written from the source: their copies on the
/// board (zones and rules, under whatever names) taken out, the source's put in. `text` is the
/// `.kicad_pcb`, `zones` its `read_copper` zones, `dru` its rules file (empty when it has none).
pub fn accept(text: &str, zones: &[kicad::Zone], dru: &str, placed: &Placed, found: &[Found], chosen: &[usize]) -> (String, String) {
    let mut edit = kicad::Edit::default();
    let mut remove_rules = Vec::new();
    let mut add_rules = Vec::new();
    for &i in chosen {
        let (e, f) = (&placed.entries[i], &found[i]);
        for &z in &f.zones {
            edit.remove.push(zones[z].span);
        }
        edit.zones.push(e.zone.clone());
        remove_rules.extend(f.rules.iter().cloned());
        for (name, text) in &e.rules {
            remove_rules.push(name.clone());
            add_rules.push(text.clone());
        }
    }
    (kicad::apply(text, &edit), kicad::dru::put_rules(dru, &remove_rules, &add_rules))
}

/// A board's pins: for each chip, the block it takes and what it declined.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct Pins {
    #[serde(flatten)]
    pub chips: BTreeMap<String, Pin>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Pin {
    pub package: String,
    /// The source's version the board last took everything from (or declined the rest of).
    pub version: String,
    pub layout: String,
    /// Entry name to the `since` version declined: asked again when the source changes it.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub declined: BTreeMap<String, String>,
}

/// `<dir>/<stem>.blocks.json` for `<dir>/<stem>.kicad_pcb`.
pub fn pins_path(board: &Path) -> PathBuf {
    board.with_extension("blocks.json")
}

impl Pins {
    pub fn load(board: &Path) -> Result<Pins, String> {
        let path = pins_path(board);
        if !path.is_file() {
            return Ok(Pins::default());
        }
        serde_json::from_str(&std::fs::read_to_string(&path).map_err(|e| e.to_string())?).map_err(|e| format!("{}: {e}", path.display()))
    }

    pub fn save(&self, board: &Path) -> Result<(), String> {
        let path = pins_path(board);
        std::fs::write(&path, serde_json::to_string_pretty(self).unwrap() + "\n").map_err(|e| format!("{}: {e}", path.display()))
    }
}

impl Pin {
    /// Whether an entry's difference is one the board declined at its current version.
    pub fn declines(&self, e: &Entry) -> bool {
        self.declined.get(&e.name) == Some(&e.since)
    }
}

/// The entries still to be accepted or declined.
pub fn outstanding(pin: &Pin, placed: &Placed, found: &[Found]) -> Vec<usize> {
    (0..placed.entries.len()).filter(|&i| found[i].state != State::Same && !pin.declines(&placed.entries[i])).collect()
}

/// A board's files as the blocks see them.
pub struct BoardFiles {
    pub path: PathBuf,
    pub text: String,
    pub pcb: kicad::Pcb,
    pub zones: Vec<kicad::Zone>,
    /// The `.kicad_dru` beside the board; empty when there is none.
    pub dru: String,
}

impl BoardFiles {
    pub fn load(path: &Path) -> Result<BoardFiles, String> {
        let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
        let (pcb, copper) = kicad::parse_board(&text).map_err(|e| format!("{}: {e}", path.display()))?;
        let dru = std::fs::read_to_string(path.with_extension("kicad_dru")).unwrap_or_default();
        Ok(BoardFiles { path: path.to_path_buf(), text, pcb, zones: copper.zones, dru })
    }

    /// Writes the board and its rules file.
    pub fn write(&self, text: &str, dru: &str) -> Result<(), String> {
        std::fs::write(&self.path, text).map_err(|e| format!("{}: {e}", self.path.display()))?;
        let rules = self.path.with_extension("kicad_dru");
        std::fs::write(&rules, dru).map_err(|e| format!("{}: {e}", rules.display()))
    }
}

impl BoardFiles {
    /// The board as the checker reads it, for [`Chip::sections`].
    pub fn checked(&self) -> Result<board::Board, String> {
        board::Board::load(&self.path).map(|(b, _)| b).map_err(|e| e.to_string())
    }
}

/// The narrowest track the fab builds on zone `zone`'s (first) layer of the board's stackup.
pub fn fab_min(board: &board::Board, pcb: &kicad::Pcb, zone: usize) -> board::Nm {
    let layer = board.zone_areas.iter().find(|z| z.zone == zone).map(|z| z.layers.trailing_zeros() as usize);
    let name = layer.and_then(|l| board.layers.get(l));
    let copper = name.and_then(|n| pcb.copper_thickness_m.iter().find(|(l, _)| l == n).map(|(_, t)| *t));
    board::fab::min_track(pcb.copper_layers, copper)
}

/// A section in plain words: its pins, and whether whole, broken (into which groups) or too
/// narrow, with its weakest link and what pinches it.
pub fn describe_section(board: &board::Board, s: &board::Section) -> String {
    let pin = |id: board::ItemId| match board.item(id).map(|i| &i.kind) {
        Some(board::Kind::Pad { footprint, number, .. }) => format!("{}.{number}", board.footprints[*footprint].reference),
        _ => "?".into(),
    };
    let item = |id: board::ItemId| {
        board.item(id).map_or("?".to_string(), |i| {
            let what = match &i.kind {
                board::Kind::Pad { .. } => format!("pad {}", pin(id)),
                board::Kind::Track => "track".into(),
                board::Kind::Via => "via".into(),
                board::Kind::Pour { .. } => "pour".into(),
                _ => "copper".into(),
            };
            format!("{what} of {}", board.net_name(i.net))
        })
    };
    let pins: Vec<String> = s.pins.iter().map(|&p| pin(p)).collect();
    let state = match (s.ok, s.groups.len()) {
        (true, _) => "whole".to_string(),
        (false, 1) => "too narrow".to_string(),
        (false, n) => format!("broken in {n}: {}", s.groups.iter().map(|g| g.iter().map(|&p| pin(p)).collect::<Vec<_>>().join("+")).collect::<Vec<_>>().join(" / ")),
    };
    let weakest = s.weakest.as_ref().map_or(String::new(), |n| {
        let by: Vec<String> = n.by.iter().map(|&(id, _)| item(id)).collect();
        format!(", weakest link {:.3} mm at ({:.2}, {:.2}){}", n.width as f64 / 1e6, n.at.x as f64 / 1e6, n.at.y as f64 / 1e6, if by.is_empty() { String::new() } else { format!(" by {}", by.join(", ")) })
    });
    format!("pins {} on {}: {state}{weakest}", pins.join(", "), board.layers[s.layer])
}

/// A board text's zones, as [`compare`] and [`accept`] take them.
pub fn zones_of(text: &str) -> Result<Vec<kicad::Zone>, String> {
    kicad::parse_board(text).map(|(_, c)| c.zones).map_err(|e| e.to_string())
}

/// One pinned chip of a board against its source.
pub struct Chip {
    pub pin: Pin,
    pub placed: Placed,
    pub found: Vec<Found>,
}

impl Chip {
    pub fn outstanding(&self) -> Vec<usize> {
        outstanding(&self.pin, &self.placed, &self.found)
    }

    /// Each pour that must stay joined ([`Pour::keep_joined`]) as the board has it: its copy's
    /// sections, as only the nets let cut it would cut it, and whether each still joins its pins
    /// inside by copper at least the narrowest track the fab builds on that layer of the board's
    /// stackup ([`board::fab::min_track`]). `board` is the same board loaded for checking.
    pub fn sections(&self, board: &board::Board, pcb: &kicad::Pcb) -> Vec<(usize, Vec<board::Section>)> {
        let mut out = Vec::new();
        for (i, e) in self.placed.entries.iter().enumerate() {
            let Some(cut) = &e.keep_joined else { continue };
            let cut: Vec<u32> = cut.iter().filter_map(|n| board.nets.iter().position(|b| b == n)).map(|n| n as u32).collect();
            let mut all = Vec::new();
            for &z in &self.found[i].zones {
                let min_width = fab_min(board, pcb, z);
                all.extend(board::sections(board, z, &cut, min_width));
            }
            out.push((i, all));
        }
        out
    }

    /// Behind its source: an older version pinned, or changes neither accepted nor declined.
    pub fn behind(&self) -> bool {
        self.pin.version != self.placed.version || !self.outstanding().is_empty()
    }
}

/// A pour a board's pinned blocks keep joined ([`Pour::keep_joined`]).
pub struct KeptPour {
    /// The zone, as numbered among the file's, and its name.
    pub zone: usize,
    pub name: String,
    /// The nets that may cut it.
    pub cut_by: Vec<u32>,
    /// The fab's narrowest track on its layer.
    pub min_width: board::Nm,
}

/// Each pour the board's pinned blocks keep joined, for a router to keep whole
/// ([`board::Board::keep_joined`]). None when nothing is pinned.
pub fn kept_joined(path: &Path, board: &board::Board, root: &Path) -> Result<Vec<KeptPour>, String> {
    let pins = Pins::load(path)?;
    if pins.chips.is_empty() {
        return Ok(Vec::new());
    }
    let files = BoardFiles::load(path)?;
    let mut out = Vec::new();
    for chip in chips(&files, &pins, root)?.values() {
        for (i, e) in chip.placed.entries.iter().enumerate() {
            let Some(cut) = &e.keep_joined else { continue };
            let cut: Vec<u32> = cut.iter().filter_map(|n| board.nets.iter().position(|b| b == n)).map(|n| n as u32).collect();
            for &z in &chip.found[i].zones {
                let name = files.zones.get(z).map(|zone| zone.name.clone()).unwrap_or_default();
                out.push(KeptPour { zone: z, name, cut_by: cut.clone(), min_width: fab_min(board, &files.pcb, z) });
            }
        }
    }
    Ok(out)
}

/// Every chip `pins` names, placed from its package under `root` and compared with `board`.
pub fn chips(board: &BoardFiles, pins: &Pins, root: &Path) -> Result<BTreeMap<String, Chip>, String> {
    let mut out = BTreeMap::new();
    for (chip, pin) in &pins.chips {
        let package = Package::load(&root.join(&pin.package))?;
        let placed = place(&package, &pin.layout, &board.pcb, chip)?;
        let found = compare(&placed, &board.zones, &board.dru);
        out.insert(chip.clone(), Chip { pin: pin.clone(), placed, found });
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_fit_finds_a_mirror_and_a_turn() {
        let src = [[0.0, 0.0], [1e-3, 0.0], [0.0, 2e-3], [3e-3, 1e-3]];
        // Mirrored in y, turned 90 degrees, moved.
        let dst: Vec<[f64; 2]> = src.iter().map(|p| [5e-3 - (-p[1]), 7e-3 + p[0]]).collect();
        let (f, err) = fit(&src, &dst);
        assert!(err < 1e-12, "{err}");
        assert_eq!(f.mirror, -1.0);
    }

    #[test]
    fn rule_meaning_reads_area_nets_and_words() {
        let (a, n, w) = rule_meaning("(rule \"x\" (condition \"A.intersectsArea('U3 ring') && A.NetName != 'GND' && A.NetName != '+1V1'\") (constraint disallow track via))").unwrap();
        assert_eq!(a, "U3 ring");
        assert_eq!(n.into_iter().collect::<Vec<_>>(), ["+1V1", "GND"]);
        assert_eq!(w.into_iter().collect::<Vec<_>>(), ["track", "via"]);
    }
}
