//! Boardroom's one WASM module for the browser (docs/one-system-spec.md). Build it with
//! `web/scripts/wasm.sh`.
//!
//! - [`BoardView`]: a `.kicad_pcb` drawn as it is, for the Board tab.
//! - [`scene_board::Board`] (JS `SceneBoard`): a scene built item by item from JS, for views
//!   that hold their own model (Compose).

pub mod scene_board;

use scene::{Mesh, Scene};
use serde_json::json;
use wasm_bindgen::prelude::*;

/// Selected parts are ringed in this colour.
const HIGHLIGHT: scene::Rgba = [1.0, 1.0, 1.0, 0.95];

/// A board file, drawn. Coordinates are millimetres, y up.
#[wasm_bindgen]
pub struct BoardView {
    scene: Scene,
    bbox: [f32; 4],
    highlight: Vec<i32>,
}

impl Default for BoardView {
    fn default() -> BoardView {
        BoardView::new()
    }
}

#[wasm_bindgen]
impl BoardView {
    #[wasm_bindgen(constructor)]
    pub fn new() -> BoardView {
        let mut scene = Scene::new();
        scene.style.show_pours = true;
        BoardView { scene, bbox: [0.0; 4], highlight: Vec::new() }
    }

    /// Reads a board: its `.kicad_pcb`, `.kicad_pro` (`"{}"` for none) and `.kicad_dru` (empty
    /// for none). Layer visibility, the active layer and the pours switch carry over.
    pub fn load(&mut self, pcb: &str, pro: &str, dru: &str) -> Result<(), JsError> {
        let (board, mut scene) = render::load(pcb, pro, dru).map_err(|e| JsError::new(&e.to_string()))?;
        let hidden: Vec<String> = self.scene.layers().iter().filter(|l| !l.visible).map(|l| l.name.clone()).collect();
        for i in 0..scene.layers().len() {
            if hidden.contains(&scene.layers()[i].name) {
                scene.set_layer_visible(i, false);
            }
        }
        scene.style = self.scene.style.clone();
        scene.style.active_layer = scene.style.active_layer.min(scene.layers().len().saturating_sub(1));
        self.bbox = render::bbox(&board);
        self.scene = scene;
        Ok(())
    }

    /// `{"layers": [{name, hidden, color, active}], "traces": n, "vias": n}` as JSON.
    pub fn status(&self) -> String {
        let (traces, vias) = render::counts(&self.scene);
        let layers: Vec<_> = self
            .scene
            .layers()
            .iter()
            .enumerate()
            .map(|(i, l)| json!({"name": l.name, "hidden": !l.visible, "color": [l.color[0], l.color[1], l.color[2]], "active": i == self.scene.style.active_layer}))
            .collect();
        json!({"layers": layers, "traces": traces, "vias": vias}).to_string()
    }

    /// `[{"id": n, "name": "U1"}]` as JSON.
    pub fn parts(&self) -> String {
        let parts: Vec<_> = self.scene.parts().map(|(id, p)| json!({"id": id, "name": p.name})).collect();
        serde_json::Value::Array(parts).to_string()
    }

    /// A part's name, or nothing when there is no such part.
    pub fn part_name(&self, part: i32) -> String {
        self.scene.part(part).map(|p| p.name.clone()).unwrap_or_default()
    }

    /// The part under a point, or 0.
    pub fn part_at(&self, x: f32, y: f32) -> i32 {
        self.scene.part_at(x, y).unwrap_or(0)
    }

    /// The net of the copper nearest a point within `reach`, or nothing.
    pub fn net_name_at(&self, x: f32, y: f32, reach: f32) -> String {
        self.scene.net_at(x, y, reach).and_then(|n| self.scene.nets.get(&n).cloned()).unwrap_or_default()
    }

    pub fn bbox(&self) -> Vec<f32> {
        self.bbox.to_vec()
    }

    pub fn set_layer_hidden(&mut self, layer: usize, hidden: bool) {
        self.scene.set_layer_visible(layer, !hidden);
    }

    pub fn set_active_layer(&mut self, layer: usize) {
        self.scene.style.active_layer = layer.min(self.scene.layers().len().saturating_sub(1));
    }

    pub fn set_show_pours(&mut self, on: bool) {
        self.scene.style.show_pours = on;
    }

    /// The parts to ring.
    pub fn set_highlight(&mut self, parts: Vec<i32>) {
        self.highlight = parts;
    }

    /// The mesh to draw, for a screen pixel `px` millimetres wide: the layers whose bits are
    /// set in `mask`, then a ring round each highlighted part.
    pub fn mesh(&self, px: f32, mask: u32) -> Vec<f32> {
        let mut m = Mesh::new(px);
        self.scene.draw_into(&mut m, u64::from(mask));
        for b in self.highlight.iter().filter_map(|p| self.scene.part_box(*p)) {
            let pad = 3.0 * px;
            let ring = [b[0] - pad, b[1] - pad, b[2] + pad, b[1] - pad, b[2] + pad, b[3] + pad, b[0] - pad, b[3] + pad];
            m.ring(&ring, 1.25 * px, HIGHLIGHT);
        }
        m.into_vec()
    }
}
