//! A board as a picture. [`view`] turns the board model into a [`scene::Scene`]: its copper,
//! pads, vias, outline, rule areas and poured zones, with each part named so a click can pick
//! it. The scene is in millimetres with y up (a KiCad board runs y down), as the renderer in
//! `js/renderer.js` draws by default.
//!
//! The board model does not know how it is drawn; this crate does. Stage crates that want to
//! show their own results emit `scene` data themselves and never depend on this crate
//! (docs/one-system-spec.md, "Dependency rules").

use board::{Board, Item as BoardItem, Kind, Nm, Point, Shape};
use kicad::Copper;
use scene::{AreaKind, Fill, Geom, Item, Prim, Scene};

/// What [`load`] could not read.
#[derive(Debug)]
pub struct LoadError(pub String);

impl std::fmt::Display for LoadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for LoadError {}

/// The board in `pcb` (a `.kicad_pcb`'s text) under the rules in `pro` (its `.kicad_pro`'s
/// text) and `dru` (its `.kicad_dru`'s, or nothing), and its picture.
pub fn load(pcb: &str, pro: &str, dru: &str) -> Result<(Board, Scene), LoadError> {
    let (model, copper) = kicad::parse_board(pcb).map_err(|e| LoadError(e.to_string()))?;
    let mut rules = kicad::Rules::parse(pro).map_err(|e| LoadError(e.to_string()))?;
    if !dru.is_empty() {
        rules.apply_custom(dru, "board.kicad_dru");
    }
    let board = Board::new(&model, &copper, rules);
    let scene = view(&board, &copper);
    Ok((board, scene))
}

fn mm(v: Nm) -> f32 {
    (v as f64 * 1e-6) as f32
}

/// Millimetres, y up.
fn xy(p: Point) -> [f32; 2] {
    [mm(p.x), -mm(p.y)]
}

fn flat(points: &[Point]) -> Vec<f32> {
    points.iter().flat_map(|p| xy(*p)).collect()
}

/// A shape as the scene draws it: a disc, or a polygon (a capsule pad as its outline).
fn prim(shape: &Shape) -> Prim {
    match shape {
        Shape::Capsule { a, b, r } if a == b => {
            let c = xy(*a);
            Prim::Circle([c[0], c[1], mm(*r)])
        }
        Shape::Capsule { a, b, r } => {
            let base = ((b.y - a.y) as f64).atan2((b.x - a.x) as f64);
            let mut pts = Vec::new();
            for (c, from) in [(*b, base - std::f64::consts::FRAC_PI_2), (*a, base + std::f64::consts::FRAC_PI_2)] {
                for k in 0..=8 {
                    let t = from + std::f64::consts::PI * k as f64 / 8.0;
                    pts.push(Point::new(c.x + (*r as f64 * t.cos()) as i64, c.y + (*r as f64 * t.sin()) as i64));
                }
            }
            Prim::Poly(flat(&pts))
        }
        Shape::Polygon(p) => Prim::Poly(flat(p)),
    }
}

fn layers_of(board: &Board, item: &BoardItem) -> Vec<usize> {
    (0..board.layers.len()).filter(|l| item.on(*l)).collect()
}

/// The scene's part for a footprint: 0 is "no part", so footprints count from 1.
fn part_of(footprint: usize) -> i32 {
    footprint as i32 + 1
}

fn item(board: &Board, it: &BoardItem) -> Option<Item> {
    let layers = layers_of(board, it);
    let net = it.net as i32;
    let drill = it.hole.map(|h| mm(h.1));
    Some(match &it.kind {
        Kind::Track => {
            let Shape::Capsule { a, b, r } = &it.shape else { return None };
            Item::new(net, 0, Geom::Track { layer: *layers.first()?, width: 2.0 * mm(*r), points: flat(&[*a, *b]) })
        }
        Kind::Via => {
            let Shape::Capsule { a, .. } = &it.shape else { return None };
            Item::new(net, 0, Geom::Via { at: xy(*a), shape: Some(prim(&it.shape)), drill })
        }
        Kind::Pad { footprint, .. } => {
            let b = it.shape.bbox();
            let at = xy(Point::new((b.min.x + b.max.x) / 2, (b.min.y + b.max.y) / 2));
            let shape = prim(&it.shape);
            Item::new(net, part_of(*footprint), Geom::Pad { at, pads: layers.iter().map(|l| (*l, shape.clone())).collect(), drill })
        }
        Kind::Pour { .. } | Kind::Keepout { .. } => {
            let Shape::Polygon(p) = &it.shape else { return None };
            let (kind, part) = match &it.kind {
                Kind::Pour { .. } => (AreaKind::Pour, 0),
                Kind::Keepout { footprint, .. } => (AreaKind::Keepout, footprint.map_or(0, part_of)),
                _ => unreachable!(),
            };
            Item::new(net, part, Geom::Area { kind, tiles: layers.iter().map(|l| (*l, flat(p))).collect() })
        }
        // Room kept for a pour's spoke, not copper: nothing to show.
        Kind::Spoke { .. } => return None,
    })
}

/// The board as a scene. `copper` is the file's copper the board was built from, for pouring
/// its zones as a refill would (our fill, a preview of KiCad's).
pub fn view(board: &Board, copper: &Copper) -> Scene {
    let mut s = Scene::new();
    s.set_layers(&board.layers);
    for (i, name) in board.nets.iter().enumerate().filter(|(_, n)| !n.is_empty()) {
        s.nets.insert(i as i32, name.clone());
    }
    for (i, fp) in board.footprints.iter().enumerate() {
        s.set_part(part_of(i), &fp.reference);
    }
    let mut next = 0u32;
    for (id, it) in board.items() {
        next = next.max(id as u32 + 1);
        if let Some(item) = item(board, it) {
            s.insert(id as u32, item);
        }
    }
    // What the board model does not hold as items: the edge and each part's courtyard.
    if !board.outline.is_empty() {
        s.insert(next, Item::new(0, 0, Geom::Outline { polys: board.outline.iter().map(|ring| flat(ring)).collect() }));
        next += 1;
    }
    let back = board.layers.len().saturating_sub(1);
    for (i, fp) in board.footprints.iter().enumerate() {
        if let Some(c) = fp.courtyard.as_ref().filter(|c| c.len() >= 3) {
            let layer = if fp.back { back } else { 0 };
            s.insert(next, Item::new(0, part_of(i), Geom::Area { kind: AreaKind::Courtyard, tiles: vec![(layer, flat(c))] }));
            next += 1;
        }
    }
    let fills = board::fill_zones(board, copper)
        .iter()
        .flat_map(|f| {
            f.rects.iter().map(move |r| {
                let (x0, y0, x1, y1) = (mm(r[0]), -mm(r[3]), mm(r[2]), -mm(r[1]));
                Fill { net: f.net as i32, layer: f.layer, polygon: vec![x0, y0, x1, y0, x1, y1, x0, y1] }
            })
        })
        .collect();
    s.set_fills(fills);
    s
}

/// How many tracks and vias a scene holds.
pub fn counts(scene: &Scene) -> (usize, usize) {
    scene.items().fold((0, 0), |(t, v), (_, i)| match i.geom {
        Geom::Track { .. } => (t + 1, v),
        Geom::Via { .. } => (t, v + 1),
        _ => (t, v),
    })
}

/// The box of everything drawn, `[x0, y0, x1, y1]` in millimetres: the outline's when the
/// board has one, else its copper's.
pub fn bbox(board: &Board) -> [f32; 4] {
    let ring: Vec<Point> = board.outline.iter().flatten().copied().collect();
    let b = if ring.is_empty() {
        board::BBox::of(board.items().flat_map(|(_, i)| {
            let b = i.shape.bbox();
            [b.min, b.max]
        }))
    } else {
        board::BBox::of(ring.iter().copied())
    };
    [mm(b.min.x), -mm(b.max.y), mm(b.max.x), -mm(b.min.y)]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(name: &str) -> String {
        std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../../fixtures/").to_string() + name).unwrap()
    }

    #[test]
    fn the_earring_draws_its_parts_copper_outline_and_nets() {
        let (board, scene) = load(&fixture("earring.kicad_pcb"), &fixture("earring.kicad_pro"), "").unwrap();
        assert_eq!(scene.layers().len(), board.layers.len());
        assert_eq!(scene.parts().count(), board.footprints.len());
        let (tracks, vias) = counts(&scene);
        let in_board = |k: fn(&Kind) -> bool| board.items().filter(|(_, i)| k(&i.kind)).count();
        assert_eq!(tracks, in_board(|k| matches!(k, Kind::Track)));
        assert_eq!(vias, in_board(|k| matches!(k, Kind::Via)));
        assert!(tracks > 0 && vias > 0, "{tracks} tracks, {vias} vias");
        assert!(scene.items().any(|(_, i)| matches!(i.geom, Geom::Outline { .. })));
        assert!(scene.nets.values().any(|n| n == "GND"));
        let b = bbox(&board);
        assert!(b[2] > b[0] && b[3] > b[1]);
    }

    #[test]
    fn a_click_on_a_pad_picks_its_part_by_name() {
        let (board, scene) = load(&fixture("earring.kicad_pcb"), &fixture("earring.kicad_pro"), "").unwrap();
        let (_, pad) = board.items().find(|(_, i)| matches!(i.kind, Kind::Pad { .. })).unwrap();
        let Kind::Pad { footprint, .. } = &pad.kind else { unreachable!() };
        let b = pad.shape.bbox();
        let at = xy(Point::new((b.min.x + b.max.x) / 2, (b.min.y + b.max.y) / 2));
        let hits = scene.part_ids_at(at[0], at[1]);
        assert!(hits.contains(&part_of(*footprint)), "{hits:?}");
        assert_eq!(scene.part(part_of(*footprint)).unwrap().name, board.footprints[*footprint].reference);
    }

    #[test]
    fn a_file_that_is_not_a_board_says_so() {
        assert!(load("(not a board)", "{}", "").is_err());
    }
}
