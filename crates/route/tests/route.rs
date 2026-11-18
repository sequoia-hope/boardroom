//! A small four-layer board routed end to end: two pads of net A to join, a ground pour over
//! all of In1 that nothing says may be crossed.

use std::path::PathBuf;

const BOARD: &str = r#"(kicad_pcb (version 20240108) (generator "pcbnew") (general (thickness 1.6))
 (layers (0 "F.Cu" signal) (1 "In1.Cu" signal) (2 "In2.Cu" signal) (31 "B.Cu" signal) (44 "Edge.Cuts" user))
 (net 0 "") (net 1 "GND") (net 2 "A")
 (gr_rect (start 0 0) (end 20 20) (layer "Edge.Cuts") (width 0.1))
 (footprint "t:R" (layer "F.Cu") (at 0 0) (property "Reference" "R1") (property "Value" "")
   (pad "1" smd rect (at 4 10) (size 1 1) (layers "F.Cu") (net 2 "A")))
 (footprint "t:R" (layer "F.Cu") (at 0 0) (property "Reference" "R2") (property "Value" "")
   (pad "1" smd rect (at 16 10) (size 1 1) (layers "F.Cu") (net 2 "A")))
 (zone (net 1) (net_name "GND") (layer "In1.Cu") (hatch edge 0.5) (connect_pads (clearance 0.2)) (min_thickness 0.25)
   (fill yes (thermal_gap 0.5) (thermal_bridge_width 0.5))
   (polygon (pts (xy 0.5 0.5) (xy 19.5 0.5) (xy 19.5 19.5) (xy 0.5 19.5)))
   (filled_polygon (layer "In1.Cu") (pts (xy 0.5 0.5) (xy 19.5 0.5) (xy 19.5 19.5) (xy 0.5 19.5)))))
"#;

fn dir(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("route-test-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

#[test]
fn a_board_is_routed_and_its_report_names_the_solid_plane() {
    let d = dir("route");
    let pcb = d.join("t.kicad_pcb");
    std::fs::write(&pcb, BOARD).unwrap();
    let out = d.join("out");
    let report = route::route(&pcb, &out, None).unwrap();
    assert_eq!(report["open"], 0, "{report}");
    for f in ["routed.kicad_pcb", "bare.kicad_pcb", "wires.json", "report.json", "wire.F.Cu.svg", "plan.F.Cu.svg"] {
        assert!(out.join(f).exists(), "{f} written");
    }
    let solid = report["solid"].as_array().unwrap();
    assert_eq!(solid.len(), 1, "{report}");
    assert_eq!((solid[0]["layer"].as_str(), solid[0]["net"].as_str()), (Some("In1.Cu"), Some("GND")));
    // The routed board checks clean.
    let check = route::check(&out.join("routed.kicad_pcb")).unwrap();
    assert_eq!((check["faults"].as_u64(), check["open"].as_u64()), (Some(0), Some(0)), "{check}");
    // The bare one has the connection open.
    let bare = route::check(&out.join("bare.kicad_pcb")).unwrap();
    assert_eq!(bare["open"], 1, "{bare}");
    std::fs::remove_dir_all(&d).ok();
}
