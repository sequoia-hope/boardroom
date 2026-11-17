//! The workspace's dependency rules (docs/one-system-spec.md, "Dependency rules"), checked
//! against `cargo metadata`. A crate may depend only on crates in groups above it; the router
//! and the judge may depend only on Foundation and Understanding, so the router runs alone on
//! any board and the judge scores any board; the block generator does not rank with the judge.

use std::collections::BTreeMap;

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
enum Group {
    Foundation,
    Understanding,
    Judging,
    Composer,
    UltraRoute,
    Program,
}

fn group(name: &str) -> Option<Group> {
    use Group::*;
    Some(match name {
        // kicad-text is the GPL program the reader runs; only its tests use the reader.
        "kicad" | "kicad-text" | "board" | "check" | "shape" | "scene" => Foundation,
        "intent" | "recognise" | "library" | "interfaces" => Understanding,
        "examine" => Judging,
        "compose" | "pinsolve" | "floorplan" | "blockgen" => Composer,
        "plan" | "wire" | "route" | "reshape" => UltraRoute,
        "render" | "wasm" | "project" | "server" | "boardroom" | "boardroom-gtk" => Program,
        _ => return None,
    })
}

fn allowed(from: &str, to: &str) -> Result<(), String> {
    allowed_dep(from, to, None)
}

fn allowed_dep(from: &str, to: &str, kind: Option<&str>) -> Result<(), String> {
    use Group::*;
    let (f, t) = (group(from).unwrap(), group(to).unwrap());
    let ok = if to == "kicad-text" {
        false
    } else if from == "kicad-text" {
        to == "kicad" && kind == Some("dev")
    } else {
        match f {
            Foundation => match from {
                "scene" | "shape" | "kicad" => false,
                "board" => to == "kicad",
                "check" => to == "board" || to == "kicad",
                _ => false,
            },
            Understanding => t <= Understanding,
            Judging | UltraRoute => t <= Understanding || t == f,
            Composer => t <= Composer && !(from == "blockgen" && to == "examine"),
            Program => {
                if from == "render" {
                    t < Program
                } else {
                    true
                }
            }
        }
    };
    if ok { Ok(()) } else { Err(format!("{from} ({f:?}) may not depend on {to} ({t:?})")) }
}

#[test]
fn crates_depend_only_where_the_spec_allows() {
    let root = concat!(env!("CARGO_MANIFEST_DIR"), "/../../Cargo.toml");
    let out = std::process::Command::new(env!("CARGO"))
        .args(["metadata", "--format-version", "1", "--no-deps", "--manifest-path", root])
        .output()
        .expect("cargo metadata");
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let meta: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    let packages: BTreeMap<String, Vec<(String, Option<String>)>> = meta["packages"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| {
            let deps = p["dependencies"].as_array().unwrap().iter();
            // Only the workspace's own crates (path dependencies) are ruled on.
            let local = deps.filter(|d| d["path"].is_string()).map(|d| {
                (
                    d["name"].as_str().unwrap().to_string(),
                    d["kind"].as_str().map(str::to_string),
                )
            });
            (p["name"].as_str().unwrap().to_string(), local.collect())
        })
        .collect();
    let mut wrong = Vec::new();
    for (name, deps) in &packages {
        if group(name).is_none() {
            wrong.push(format!("{name} is in no group: add it to tests/layers.rs and the spec's layout"));
            continue;
        }
        for (dep, kind) in deps.iter().filter(|(d, _)| packages.contains_key(d)) {
            if let Err(e) = allowed_dep(name, dep, kind.as_deref()) {
                wrong.push(e);
            }
        }
    }
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}

#[test]
fn the_rules_themselves() {
    assert!(allowed("route", "board").is_ok());
    assert!(allowed("route", "recognise").is_ok());
    assert!(allowed("route", "plan").is_ok());
    assert!(allowed("route", "compose").is_err());
    assert!(allowed("route", "render").is_err());
    assert!(allowed("examine", "floorplan").is_err());
    assert!(allowed("board", "intent").is_err());
    assert!(allowed("blockgen", "examine").is_err());
    assert!(allowed("floorplan", "examine").is_ok());
    assert!(allowed("server", "route").is_ok());
}

#[test]
fn foundation_gpl_and_render_rules() {
    assert!(allowed("kicad", "kicad-text").is_err());
    assert!(allowed("kicad-text", "kicad").is_err());
    assert!(allowed_dep("kicad-text", "kicad", Some("dev")).is_ok());
    assert!(allowed("scene", "kicad").is_err());
    assert!(allowed("board", "scene").is_err());
    assert!(allowed("board", "kicad").is_ok());
    assert!(allowed("check", "board").is_ok());
    assert!(allowed("check", "kicad").is_ok());
    assert!(allowed("render", "server").is_err());
    assert!(allowed("boardroom-gtk", "project").is_ok());
}
