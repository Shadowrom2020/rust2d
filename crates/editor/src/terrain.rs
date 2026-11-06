//! Auto-tiling in the editor: the Terrain map tool (palette, painting through the undo
//! history, picking) and the *Terrains* section of Database › Tilesets. The tile choice itself
//! is `rust2d_engine::autotile`.

use egui_macroquad::egui::{self, pos2, vec2, Color32, Rect, Sense, Stroke, StrokeKind, TextureId};
use rust2d_engine::autotile::{self, Terrain, EAST, NORTH, SOUTH, WEST};
use rust2d_engine::defs::TilesetDef;
use rust2d_engine::render::TilesetTex;

use crate::app::{Editor, Mode, Tool};
use crate::ui::{self, ph};

/// What a click into the Tilesets form's tile grid assigns while a terrain is edited.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TerrainPick {
    /// Index into the tileset's terrains.
    pub terrain: usize,
    /// 0–15 = the tile for that neighbour mask, 16 = the base tile ("erasing leaves").
    pub slot: usize,
    /// The next click is the top-left tile of a 4×4 block that fills all 16 slots.
    pub block: bool,
}

pub const BASE_SLOT: usize = 16;

/// UV rectangle of a tile in a tileset texture.
pub fn tile_uv(ts: &TilesetTex, tile_px: f32, i: u32) -> Rect {
    let (c, r) = (i % ts.columns, i / ts.columns);
    let (w, h) = (ts.image.width as f32, ts.image.height as f32);
    Rect::from_min_max(pos2(c as f32 * tile_px / w, r as f32 * tile_px / h), pos2((c + 1) as f32 * tile_px / w, (r + 1) as f32 * tile_px / h))
}

/// Small cross in the corner of a slot: arms towards the neighbours that are the same terrain.
fn paint_mask(p: &egui::Painter, r: Rect, mask: usize) {
    let c = r.min + vec2(9.0, 9.0);
    let col = Color32::from_rgb(255, 230, 90);
    p.rect_filled(Rect::from_center_size(c, vec2(14.0, 14.0)), 2.0, Color32::from_black_alpha(170));
    p.rect_filled(Rect::from_center_size(c, vec2(3.0, 3.0)), 0.0, col);
    for (bit, d) in [(NORTH, vec2(0.0, -5.0)), (EAST, vec2(5.0, 0.0)), (SOUTH, vec2(0.0, 5.0)), (WEST, vec2(-5.0, 0.0))] {
        if mask & bit != 0 {
            p.line_segment([c, c + d], Stroke::new(2.0_f32, col));
        }
    }
}

/// The *Terrains* section of the Tilesets form. Clicks into the tile grid below it are
/// handled by `grid_click` while `pick` is set.
pub fn terrains_form(ui: &mut egui::Ui, d: &mut TilesetDef, tex: TextureId, ts: &TilesetTex, tile_px: f32, pick: &mut Option<TerrainPick>) {
    ui.heading("Terrains (auto-tiling)");
    ui.weak(
        "A terrain has 16 tiles, one for each combination of neighbours that are the same terrain (north, east, south, west: the yellow arms). \
         The Terrain tool (A) on the map picks the right one while you paint. Drawn as a 4×4 block in this order, one click fills all 16.",
    );
    let mut remove = None;
    let uv = |i: u32| tile_uv(ts, tile_px, i);
    for k in 0..d.terrains.len() {
        ui.push_id(("terrain", k), |ui| {
            let editing = pick.is_some_and(|p| p.terrain == k);
            ui.horizontal(|ui| {
                if ui.radio(editing, "").on_hover_text("Edit this terrain's tiles").clicked() {
                    *pick = if editing { None } else { Some(TerrainPick { terrain: k, slot: 0, block: false }) };
                }
                let old = d.terrains[k].name.clone();
                if ui.add(egui::TextEdit::singleline(&mut d.terrains[k].name).desired_width(140.0).hint_text("name")).changed() {
                    // Joins refer to terrains by name.
                    let new = d.terrains[k].name.clone();
                    for j in d.terrains.iter_mut().flat_map(|t| t.joins.iter_mut()).filter(|j| **j == old) {
                        *j = new.clone();
                    }
                }
                let n = d.terrains[k].tiles_set();
                if n < 16 {
                    ui.colored_label(Color32::from_rgb(240, 190, 90), format!("{n}/16 tiles"));
                } else {
                    ui.weak("16/16 tiles");
                }
                if ui.small_button(ph::TRASH).on_hover_text("Delete this terrain (painted tiles stay)").clicked() {
                    remove = Some(k);
                }
            });
            if !pick.is_some_and(|p| p.terrain == k) {
                return;
            }
            // Other terrains lying on this one count as it for its edges.
            let others: Vec<String> = d.terrains.iter().enumerate().filter(|(i, _)| *i != k).map(|(_, o)| o.name.clone()).collect();
            if !others.is_empty() {
                ui.horizontal_wrapped(|ui| {
                    ui.label("Joins").on_hover_text("Cells of these terrains count as this terrain for its edges: terrains that lie on it (sand joins the shore water inside it), so no edge is drawn between them");
                    let joins = &mut d.terrains[k].joins;
                    for o in &others {
                        let mut on = joins.contains(o);
                        if ui.checkbox(&mut on, o).changed() {
                            if on {
                                joins.push(o.clone());
                            } else {
                                joins.retain(|j| j != o);
                            }
                        }
                    }
                });
            }
            let t = &mut d.terrains[k];
            let p = pick.as_mut().unwrap();
            ui.horizontal(|ui| {
                // 4×4 slots in mask order.
                ui.vertical(|ui| {
                    let cell = 40.0;
                    let (rect, resp) = ui.allocate_exact_size(vec2(cell * 4.0, cell * 4.0), Sense::click());
                    let painter = ui.painter_at(rect);
                    for s in 0..16 {
                        let r = Rect::from_min_size(rect.min + vec2((s % 4) as f32 * cell, (s / 4) as f32 * cell), vec2(cell, cell)).shrink(1.0);
                        painter.rect_filled(r, 0.0, Color32::from_gray(35));
                        if let Some(i) = t.tiles[s] {
                            painter.image(tex, r, uv(i), Color32::WHITE);
                        }
                        paint_mask(&painter, r, s);
                        if !p.block && p.slot == s {
                            painter.rect_stroke(r, 0.0, Stroke::new(2.0_f32, Color32::from_rgb(90, 200, 255)), StrokeKind::Inside);
                        }
                    }
                    if let Some(pos) = resp.clicked().then(|| resp.interact_pointer_pos()).flatten() {
                        let rel = pos - rect.min;
                        p.slot = ((rel.y / cell) as usize).min(3) * 4 + ((rel.x / cell) as usize).min(3);
                        p.block = false;
                    }
                    ui.horizontal(|ui| {
                        let (r, resp) = ui.allocate_exact_size(vec2(36.0, 36.0), Sense::click());
                        ui.painter().rect_filled(r, 0.0, Color32::from_gray(35));
                        if let Some(b) = t.base {
                            ui.painter().image(tex, r, uv(b), Color32::WHITE);
                        }
                        if !p.block && p.slot == BASE_SLOT {
                            ui.painter().rect_stroke(r, 0.0, Stroke::new(2.0_f32, Color32::from_rgb(90, 200, 255)), StrokeKind::Inside);
                        }
                        if resp.on_hover_text("Click, then click a tile in the grid below").clicked() {
                            p.slot = BASE_SLOT;
                            p.block = false;
                        }
                        ui.label("Erasing leaves");
                        if t.base.is_some() && ui.small_button(ph::X).on_hover_text("Erasing leaves no tile").clicked() {
                            t.base = None;
                        }
                    });
                });
                ui.vertical(|ui| {
                    if ui.selectable_label(p.block, ui::t(ph::GRID_NINE, "Fill from a 4×4 block")).on_hover_text("Then click the top-left tile of a 4×4 block drawn in the slots' order").clicked() {
                        p.block = !p.block;
                    }
                    if ui.button(ui::t(ph::BROOM, "Clear the tiles")).clicked() {
                        t.tiles = [None; 16];
                    }
                    ui.weak("Preview:");
                    preview(ui, t, tex, &uv);
                });
            });
        });
    }
    if let Some(k) = remove {
        let gone = d.terrains.remove(k).name;
        for t in &mut d.terrains {
            t.joins.retain(|j| *j != gone);
        }
        *pick = None;
    }
    if ui.button(ui::t(ph::PLUS, "Terrain")).on_hover_text("A new auto-tiling terrain of this tileset").clicked() {
        let n = d.terrains.len() + 1;
        d.terrains.push(Terrain { name: format!("Terrain {n}"), ..Default::default() });
        *pick = Some(TerrainPick { terrain: d.terrains.len() - 1, slot: 0, block: true });
    }
}

/// A little sample map painted with the terrain (outside the sample = not the terrain).
fn preview(ui: &mut egui::Ui, t: &Terrain, tex: TextureId, uv: &dyn Fn(u32) -> Rect) {
    const SHAPE: [&str; 6] = ["........", ".###..#.", ".####.#.", "..#.###.", ".##...#.", "........"];
    let cell = 22.0;
    let (rect, _) = ui.allocate_exact_size(vec2(cell * 8.0, cell * 6.0), Sense::hover());
    let p = ui.painter_at(rect);
    p.rect_filled(rect, 0.0, Color32::from_gray(25));
    let member = |x: i32, y: i32| SHAPE.get(y as usize).and_then(|r| r.as_bytes().get(x as usize)).is_some_and(|&c| c == b'#') && x >= 0 && y >= 0;
    for y in 0..6 {
        for x in 0..8 {
            let r = Rect::from_min_size(rect.min + vec2(x as f32 * cell, y as f32 * cell), vec2(cell, cell));
            let tile = if member(x, y) { t.tile_for(autotile::mask(x, y, member)) } else { t.base };
            if let Some(i) = tile {
                p.image(tex, r, uv(i), Color32::WHITE);
            }
        }
    }
}

/// A click on tile `i` of the Tilesets form's grid while a terrain is edited. Returns false
/// if the click was not used.
pub fn grid_click(d: &mut TilesetDef, ts: &TilesetTex, pick: &mut TerrainPick, i: u32) -> bool {
    let Some(t) = d.terrains.get_mut(pick.terrain) else { return false };
    if pick.block {
        let rows = ts.count.div_ceil(ts.columns);
        let (col, row) = (i % ts.columns, i / ts.columns);
        if col + 4 > ts.columns || row + 4 > rows {
            return true;
        }
        t.fill_from_block(i, ts.columns);
        pick.block = false;
    } else if pick.slot == BASE_SLOT {
        t.base = Some(i);
    } else {
        t.tiles[pick.slot] = Some(i);
        // On to the next slot, so a whole terrain is set up in 16 clicks.
        pick.slot = (pick.slot + 1).min(15);
    }
    true
}

impl Editor {
    /// The armed terrain: (tileset id, terrain).
    pub fn active_terrain(&self) -> Option<(String, Terrain)> {
        let (id, k) = self.terrain.as_ref()?;
        let t = self.project.db.tileset(id)?.terrains.get(*k)?;
        Some((id.clone(), t.clone()))
    }

    /// Arms a terrain by name (editor script `terrain NAME`).
    pub fn select_terrain_named(&mut self, name: &str) -> bool {
        let found = self.project.db.tilesets.iter().find_map(|d| d.terrains.iter().position(|t| t.name.eq_ignore_ascii_case(name)).map(|k| (d.id.clone(), k)));
        let ok = found.is_some();
        if ok {
            self.terrain = found;
            self.tool = Tool::Terrain;
        }
        ok
    }

    /// Paints (or with `erase` removes) the armed terrain at a tile, as part of the current
    /// stroke.
    pub fn paint_terrain_at(&mut self, tile: (i32, i32), erase: bool) {
        let Some((id, k)) = self.terrain.clone() else { return };
        let Some(terrains) = self.project.db.tileset(&id).map(|d| d.terrains.clone()) else { return };
        let layer = self.layer;
        if let Some(doc) = &mut self.doc {
            let slot = doc.map.tileset_slot(&id);
            for (x, y, new) in autotile::paint(&doc.map, layer, slot, &terrains, k, tile, erase) {
                doc.paint(layer, x as i32, y as i32, new, &self.assets);
            }
        }
    }

    /// Right-click with the Terrain tool: arms the terrain of the tile under the cursor.
    pub fn pick_terrain(&mut self, tile: (i32, i32)) {
        let Some(doc) = &self.doc else { return };
        if !doc.map.in_bounds(tile.0, tile.1) {
            return;
        }
        let order = std::iter::once(self.layer).chain((0..doc.map.layers.len()).rev());
        for li in order {
            let Some((slot, idx)) = doc.map.layers.get(li).and_then(|l| l.get(tile.0 as u32, tile.1 as u32).parts()) else { continue };
            let Some(id) = doc.map.tilesets.get(slot) else { continue };
            let Some(k) = self.project.db.tileset(id).and_then(|d| d.terrains.iter().position(|t| t.contains(idx))) else { continue };
            let name = self.project.db.tileset(id).map(|d| d.terrains[k].name.clone()).unwrap_or_default();
            self.terrain = Some((id.clone(), k));
            self.layer = li;
            self.set_status(format!("Terrain {name} (layer {})", li + 1));
            return;
        }
        self.set_status("No terrain under the cursor");
    }

    /// Left panel of the Terrain tool.
    pub fn terrain_palette(&mut self, ui: &mut egui::Ui) {
        ui.heading("Terrain");
        ui.label("Paint a terrain and its edge tiles follow the neighbours. Shift+click or drag erases it (the cell gets the terrain's base tile). Right-click picks the terrain under the cursor.");
        let layer = self.doc.as_ref().and_then(|d| d.map.layers.get(self.layer)).map_or("–".to_string(), |l| format!("{}. {}", self.layer + 1, l.name));
        ui.weak(format!("Paints on layer {layer} (keys 1–9)."));
        ui.separator();
        let tile_px = self.assets.tile_size;
        let mut arm = None;
        let mut any = false;
        for d in &self.project.db.tilesets {
            let (Some(tex), Some(ts)) = (self.tex.tilesets.get(&d.id), self.assets.tilesets.get(&d.id)) else { continue };
            for (k, t) in d.terrains.iter().enumerate() {
                any = true;
                let on = self.terrain.as_ref().is_some_and(|(id, i)| *id == d.id && *i == k);
                ui.horizontal(|ui| {
                    for m in [0, 15] {
                        let (r, _) = ui.allocate_exact_size(vec2(32.0, 32.0), Sense::hover());
                        if let Some(i) = t.tile_for(m) {
                            ui.painter().image(tex.id(), r, tile_uv(ts, tile_px, i), Color32::WHITE);
                        }
                    }
                    let mut label = format!("{}\n{}", t.name, d.id);
                    if t.tiles_set() < 16 {
                        label += &format!(" · {}/16 tiles", t.tiles_set());
                    }
                    if ui.selectable_label(on, label).clicked() {
                        arm = Some((d.id.clone(), k));
                    }
                });
            }
        }
        if let Some(a) = arm {
            self.terrain = Some(a);
        }
        if !any {
            ui.weak("No terrains yet. Define them in Database › Tilesets › Terrains.");
            if ui.button(ui::t(ph::GRID_FOUR, "Open Database › Tilesets")).clicked() {
                self.db.tab = crate::database::Tab::Tilesets;
                self.mode = Mode::Database;
            }
        } else if self.terrain.is_none() {
            ui.weak("Pick a terrain above.");
        }
    }
}
