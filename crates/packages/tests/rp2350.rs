//! The RP2350 block's areas placed on its own reference fixture: nothing there at first, every
//! entry written on accepting, the same as the source afterwards, and a decline held until the
//! source changes the entry.

use std::path::Path;

use packages::{accept, compare, place, zones_of, Package, Pin, State};

fn root() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../packages")
}

#[test]
fn every_package_manifest_reads() {
    let all = packages::all(&root()).unwrap();
    assert!(all.len() >= 10, "{} packages", all.len());
    let rp = all.iter().find(|p| p.name == "rp2350").unwrap();
    assert_eq!(rp.variant_of("qfn60-reference").unwrap().name, "RP2350A");
}

#[test]
fn the_rp2350_block_goes_onto_its_fixture_and_then_matches() {
    let package = Package::load(&root().join("rp2350")).unwrap();
    let path = root().join("rp2350/layouts/qfn60-reference/fixture.kicad_pcb");
    let text = std::fs::read_to_string(&path).unwrap();
    let (pcb, copper) = kicad::parse_board(&text).unwrap();
    let placed = place(&package, "qfn60-reference", &pcb, "U1").unwrap();
    // The fixture is Raspberry Pi's two-layer board: no inner layer for the switch node's cutout.
    assert_eq!(placed.entries.len(), 16);
    assert!(!placed.entries.iter().any(|e| e.name == "LX cutout"));

    // The fixture is the source's frame, its chip at (100, 100) on the front: the ring's copper
    // sits square round it there.
    let ring = placed.entries.iter().find(|e| e.name == "ring copper").unwrap();
    assert_eq!(ring.zone.layers, ["F.Cu"]);
    let xs: Vec<f64> = ring.zone.outline.iter().map(|p| p[0] * 1e3).collect();
    assert!(xs.iter().all(|x| (x - 97.68).abs() < 1e-6 || (x - 102.32).abs() < 1e-6), "{xs:?}");

    // Raspberry Pi's board has its own pours near the block's (the board the block was lifted
    // from widened them a little), and none of the rule areas.
    let found = compare(&placed, &copper.zones, "");
    let states: Vec<_> = found.iter().zip(&placed.entries).map(|(f, e)| format!("{}: {:?}", e.name, f.state)).collect();
    for (f, e) in found.iter().zip(&placed.entries) {
        match e.is_pour() {
            true => assert!(matches!(&f.state, State::Differs(d) if d[0].starts_with("named 'buck_U1_")), "{states:#?}"),
            false => assert_eq!(f.state, State::Missing, "{states:#?}"),
        }
    }

    let all: Vec<usize> = (0..placed.entries.len()).collect();
    let (text2, dru2) = accept(&text, &copper.zones, "", &placed, &found, &all);
    assert!(kicad::dru::RuleSet::parse(&dru2).unwrap().errors.is_empty());
    let zones2 = zones_of(&text2).unwrap();
    let again = compare(&placed, &zones2, &dru2);
    let left: Vec<_> = again.iter().zip(&placed.entries).filter(|(f, _)| f.state != State::Same).map(|(f, e)| format!("{}: {:?}", e.name, f.state)).collect();
    assert!(left.is_empty(), "{left:#?}");
    // Accepting again changes nothing.
    let (text3, dru3) = accept(&text2, &zones2, &dru2, &placed, &again, &all);
    assert_eq!(zones_of(&text3).unwrap().len(), zones2.len());
    assert_eq!(kicad::dru::rule_spans(&dru3).len(), kicad::dru::rule_spans(&dru2).len());

    // A decline holds for the version declined.
    let band = placed.entries.iter().find(|e| e.name == "band").unwrap();
    let mut pin = Pin { package: "rp2350".into(), version: String::new(), layout: "qfn60-reference".into(), declined: Default::default() };
    pin.declined.insert("band".into(), band.since.clone());
    assert!(pin.declines(band));
    pin.declined.insert("band".into(), "0.0.1".into());
    assert!(!pin.declines(band));
}

#[test]
fn a_chip_that_is_not_the_layouts_is_refused() {
    let package = Package::load(&root().join("rp2350")).unwrap();
    let text = std::fs::read_to_string(root().join("rp2350/layouts/qfn80-reference/fixture.kicad_pcb")).unwrap();
    let (pcb, _) = kicad::parse_board(&text).unwrap();
    let chip = pcb.footprints.iter().max_by_key(|f| f.pads.len()).unwrap().reference.clone();
    let err = place(&package, "qfn60-reference", &pcb, &chip).unwrap_err();
    assert!(err.contains("not the same footprint") || err.contains("too few pads"), "{err}");
}
