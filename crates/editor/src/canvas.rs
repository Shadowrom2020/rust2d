//! The map viewport: camera, mouse tools and drawing.

use macroquad::prelude::*;
use rust2d_engine::entity::{EntityInstance, EntityKind};
use rust2d_engine::map::{CollisionOverride, Edge, Tile, Zone, ZoneKind};
use rust2d_engine::render::{self, OVERVIEW_THRESHOLD};

use crate::app::{Editor, Mode, Selection, Tool, ZoneTemplate};
use crate::doc::Objects;

/// Mouse state for one frame; can be injected by the editor's debug script.
#[derive(Clone, Copy, Default)]
pub struct Mouse {
    pub pos: Vec2,
    pub down: [bool; 3],
    pub pressed: [bool; 3],
    pub released: [bool; 3],
    pub wheel: f32,
}

thread_local! {
    static INJECTED: std::cell::RefCell<Option<Mouse>> = const { std::cell::RefCell::new(None) };
}

pub fn inject_mouse(m: Option<Mouse>) {
    INJECTED.with(|i| *i.borrow_mut() = m);
}

pub fn mouse_injected() -> bool {
    INJECTED.with(|i| i.borrow().is_some())
}

pub fn mouse() -> Mouse {
    if let Some(m) = INJECTED.with(|i| *i.borrow()) {
        return m;
    }
    let b = [MouseButton::Left, MouseButton::Right, MouseButton::Middle];
    Mouse {
        pos: Vec2::from(mouse_position()),
        down: b.map(is_mouse_button_down),
        pressed: b.map(is_mouse_button_pressed),
        released: b.map(is_mouse_button_released),
        wheel: mouse_wheel().1,
    }
}

const L: usize = 0;
const R: usize = 1;
const M: usize = 2;

pub enum Drag {
    Pan { last: Vec2 },
    Paint { last: (i32, i32) },
    Rect { start: (i32, i32) },
    MoveEntity { uid: u32, grab: Vec2, before: Box<Objects> },
    MoveZone { uid: u32, grab: (i32, i32), orig: (u32, u32), before: Box<Objects> },
    NewZone { start: (i32, i32) },
    /// Transition tool: exit / arrival area.
    LinkRect { start: (i32, i32) },
}

fn modifier_ctrl() -> bool {
    is_key_down(KeyCode::LeftControl) || is_key_down(KeyCode::RightControl)
}
fn modifier_shift() -> bool {
    is_key_down(KeyCode::LeftShift) || is_key_down(KeyCode::RightShift)
}

/// Integer tile coordinates covered by the line between two tiles.
fn line(a: (i32, i32), b: (i32, i32)) -> Vec<(i32, i32)> {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let n = dx.abs().max(dy.abs()).max(1);
    (0..=n)
        .map(|i| {
            let t = i as f32 / n as f32;
            ((a.0 as f32 + dx as f32 * t).round() as i32, (a.1 as f32 + dy as f32 * t).round() as i32)
        })
        .collect()
}

fn rect_of(a: (i32, i32), b: (i32, i32)) -> (i32, i32, i32, i32) {
    (a.0.min(b.0), a.1.min(b.1), (a.0 - b.0).abs() + 1, (a.1 - b.1).abs() + 1)
}

impl Editor {
    pub fn camera(&self) -> Camera2D {
        Camera2D {
            target: self.cam.center,
            zoom: vec2(2.0 * self.cam.zoom / screen_width(), 2.0 * self.cam.zoom / screen_height()),
            ..Default::default()
        }
    }

    /// Frame size and anchor used to draw and hit-test an entity.
    fn entity_rect(&self, e: &EntityInstance) -> Rect {
        let ts = self.ts();
        let sprite = match e.kind {
            EntityKind::Prop => self.project.db.prop(&e.def).map(|d| d.state_look(d.state_index(&e.state).unwrap_or(0)).0.clone()),
            _ => self.def_sprite(e.kind, &e.def),
        };
        let size = sprite.map_or(vec2(ts, ts), |s| {
            if e.kind == EntityKind::Character {
                // Characters: one frame of their sheet.
                self.assets.sheets.get(&s.sheet).map_or(vec2(ts, ts), |t| vec2(t.frame_w, t.frame_h))
            } else {
                self.assets.sprite_size(&s, ts)
            }
        });
        Rect::new(e.x - size.x / 2.0, e.y - size.y, size.x, size.y)
    }

    /// Collision box of an entity (props / characters), anchored bottom-center.
    fn entity_hitbox(&self, e: &EntityInstance) -> Option<Rect> {
        let db = &self.project.db;
        let hb = match e.kind {
            EntityKind::Prop => db.prop(&e.def).map(|d| d.hitbox)?,
            EntityKind::Character => db.character(&e.def).map(|d| d.hitbox)?,
            EntityKind::Item => return None,
        };
        Some(Rect::new(e.x - hb[0] / 2.0, e.y - hb[1], hb[0], hb[1]))
    }

    fn entity_at(&self, p: Vec2) -> Option<u32> {
        let doc = self.doc.as_ref()?;
        let mut hits: Vec<&EntityInstance> = doc.map.entities.iter().filter(|e| self.entity_rect(e).contains(p)).collect();
        hits.sort_by(|a, b| b.y.total_cmp(&a.y));
        hits.first().map(|e| e.uid)
    }

    /// Everything under a point for the Select tool: entities whose sprite covers it or that
    /// stand on its tile (front-most first), then visible zones containing it (smallest first).
    fn objects_at(&self, p: Vec2) -> Vec<Selection> {
        let Some(doc) = self.doc.as_ref() else { return Vec::new() };
        let ts = self.ts();
        let tile = ((p.x / ts).floor() as i32, (p.y / ts).floor() as i32);
        let feet = |e: &EntityInstance| ((e.x / ts).floor() as i32, ((e.y - 0.5) / ts).floor() as i32);
        let mut ents: Vec<&EntityInstance> = doc.map.entities.iter().filter(|e| self.entity_rect(e).contains(p)).collect();
        ents.sort_by(|a, b| b.y.total_cmp(&a.y));
        for e in doc.map.entities.iter().filter(|e| feet(e) == tile) {
            if !ents.iter().any(|x| x.uid == e.uid) {
                ents.push(e);
            }
        }
        let mut out: Vec<Selection> = ents.iter().map(|e| Selection::Entity(e.uid)).collect();
        if self.view.zones {
            let mut zones: Vec<_> = doc.map.zones.iter().filter(|z| z.pixel_rect(ts).contains(p)).collect();
            zones.sort_by_key(|z| z.w * z.h);
            out.extend(zones.iter().map(|z| Selection::Zone(z.uid)));
        }
        out
    }

    /// Starts moving the selected entity / zone with the mouse.
    fn grab_selection(&mut self, tile: (i32, i32), world: Vec2) {
        let Some(doc) = self.doc.as_ref() else { return };
        let before = Box::new(Objects::of(&doc.map));
        self.drag = match self.sel {
            Selection::Entity(uid) => doc.map.entity(uid).map(|e| Drag::MoveEntity { uid, grab: world - vec2(e.x, e.y), before }),
            Selection::Zone(uid) => doc.map.zones.iter().find(|z| z.uid == uid).map(|z| Drag::MoveZone { uid, grab: tile, orig: (z.x, z.y), before }),
            Selection::None => None,
        };
    }

    /// Whether the mouse moved far enough since the press to count as a drag (4 screen px).
    fn dragged(&self, world: Vec2) -> bool {
        (world - self.press_world).length() * self.cam.zoom >= 4.0
    }

    fn zone_at(&self, p: Vec2) -> Option<u32> {
        let doc = self.doc.as_ref()?;
        let ts = self.ts();
        doc.map
            .zones
            .iter()
            .filter(|z| z.pixel_rect(ts).contains(p))
            .min_by_key(|z| z.w * z.h)
            .map(|z| z.uid)
    }

    /// Screen rect of the minimap, if shown.
    pub fn minimap_rect(&self) -> Option<Rect> {
        let doc = self.doc.as_ref()?;
        if !self.view.minimap || !self.canvas_rect.is_positive() {
            return None;
        }
        let max = 180.0f32;
        let s = (max / doc.map.width as f32).min(max / doc.map.height as f32);
        let (w, h) = (doc.map.width as f32 * s, doc.map.height as f32 * s);
        Some(Rect::new(self.canvas_rect.max.x - w - 10.0, self.canvas_rect.max.y - h - 10.0, w, h))
    }

    // ------------------------------------------------------------ input

    pub fn update_canvas(&mut self) {
        if self.mode != Mode::Map || !self.project_open {
            return;
        }
        self.shortcuts();
        let Some(_) = &self.doc else { return };
        let cam = self.camera();
        let ms = mouse();
        let mouse = ms.pos;
        let world = cam.screen_to_world(mouse);
        let ts = self.ts();
        self.hover_world = world;
        let tile = ((world.x / ts).floor() as i32, (world.y / ts).floor() as i32);
        let over_minimap = self.minimap_rect().is_some_and(|r| r.contains(mouse));
        let over_ui = ((self.pointer_over_ui && !mouse_injected()) || over_minimap) && self.drag.is_none();
        self.hover_tile = (!over_ui).then_some(tile);

        // Keyboard panning.
        if !self.keyboard_busy {
            let mut d = Vec2::ZERO;
            if is_key_down(KeyCode::Left) {
                d.x -= 1.0;
            }
            if is_key_down(KeyCode::Right) {
                d.x += 1.0;
            }
            if is_key_down(KeyCode::Up) {
                d.y -= 1.0;
            }
            if is_key_down(KeyCode::Down) {
                d.y += 1.0;
            }
            self.cam.center += d * 600.0 * get_frame_time() / self.cam.zoom;
        }

        if !over_ui {
            let wheel = ms.wheel;
            if wheel != 0.0 {
                let factor = if wheel > 0.0 { 1.15 } else { 1.0 / 1.15 };
                let new_zoom = (self.cam.zoom * factor).clamp(0.02, 16.0);
                // Keep the point under the cursor fixed.
                self.cam.center = world + (self.cam.center - world) * (self.cam.zoom / new_zoom);
                self.cam.zoom = new_zoom;
            }
        }

        // Mouse buttons.
        let pan_button = ms.pressed[M] || (ms.pressed[L] && is_key_down(KeyCode::Space));
        if !over_ui && pan_button {
            self.drag = Some(Drag::Pan { last: mouse });
            return;
        }
        if let Some(Drag::Pan { last }) = &mut self.drag {
            self.cam.center -= (mouse - *last) / self.cam.zoom;
            *last = mouse;
            if !ms.down[M] && !ms.down[L] {
                self.drag = None;
            }
            return;
        }

        // Editing a patrol route: clicks place / move / remove waypoints instead.
        if self.route_edit.is_some() && self.route_edit_mouse(tile, &ms, over_ui) {
            return;
        }
        if !over_ui && ms.pressed[L] {
            self.mouse_down(tile, world);
        } else if !over_ui && ms.pressed[R] {
            self.right_click(tile);
        }
        if self.drag.is_some() {
            if ms.down[L] {
                self.mouse_drag(tile, world);
            } else {
                self.mouse_up(tile);
            }
        }
        // Collision tool also paints with the right button.
        if self.tool == Tool::Collision && !over_ui && ms.down[R] {
            if let Some(doc) = &mut self.doc {
                doc.paint_collision(tile.0, tile.1, CollisionOverride::Walkable as u8);
            }
        } else if self.tool == Tool::Collision && ms.released[R] {
            if let Some(doc) = &mut self.doc {
                doc.end_stroke();
            }
        }
    }

    fn shortcuts(&mut self) {
        if self.keyboard_busy {
            return;
        }
        let ctrl = modifier_ctrl();
        let shift = modifier_shift();
        if ctrl {
            if is_key_pressed(KeyCode::S) {
                self.save_all();
            }
            if is_key_pressed(KeyCode::Z) && !shift {
                if let Some(d) = &mut self.doc {
                    d.undo(&self.assets);
                }
            }
            if is_key_pressed(KeyCode::Y) || (is_key_pressed(KeyCode::Z) && shift) {
                if let Some(d) = &mut self.doc {
                    d.redo(&self.assets);
                }
            }
            if is_key_pressed(KeyCode::D) {
                self.duplicate_selection();
            }
            if is_key_pressed(KeyCode::M) {
                self.quick.open = true;
                self.quick.query.clear();
            }
            if is_key_pressed(KeyCode::PageUp) {
                self.cycle_map(-1);
            }
            if is_key_pressed(KeyCode::PageDown) {
                self.cycle_map(1);
            }
            return;
        }
        let tools = [
            (KeyCode::V, Tool::Select),
            (KeyCode::B, Tool::Brush),
            (KeyCode::R, Tool::Rect),
            (KeyCode::F, Tool::Fill),
            (KeyCode::E, Tool::Erase),
            (KeyCode::I, Tool::Pick),
            (KeyCode::C, Tool::Collision),
            (KeyCode::N, Tool::Entity),
            (KeyCode::Z, Tool::Zone),
            (KeyCode::T, Tool::Link),
        ];
        for (k, t) in tools {
            if is_key_pressed(k) {
                self.tool = t;
            }
        }
        if is_key_pressed(KeyCode::G) {
            self.view.grid = !self.view.grid;
        }
        if is_key_pressed(KeyCode::Home) {
            self.fit_view();
        }
        if is_key_pressed(KeyCode::F5) {
            self.play(true);
        }
        if is_key_pressed(KeyCode::F6) {
            self.play(false);
        }
        if is_key_pressed(KeyCode::Delete) {
            self.delete_selection();
        }
        if is_key_pressed(KeyCode::Escape) && self.route_edit.is_some() {
            self.route_edit = None;
            self.route_drag = None;
            self.set_status("Patrol route done");
            return;
        }
        if is_key_pressed(KeyCode::Escape) && self.link.is_some() {
            self.link = None;
            self.set_status("Transition cancelled");
            return;
        }
        if is_key_pressed(KeyCode::Escape) {
            // Esc: deselect, stop placing, and otherwise go back to the Select tool.
            self.sel = Selection::None;
            if self.tool == Tool::Entity && !self.place_def.is_empty() {
                self.disarm();
            } else {
                self.tool = Tool::Select;
            }
        }
        if self.tool == Tool::Entity {
            if is_key_pressed(KeyCode::RightBracket) || is_key_pressed(KeyCode::PageDown) {
                self.cycle_place(1);
                self.palette_scroll_to_armed = true;
            }
            if is_key_pressed(KeyCode::LeftBracket) || is_key_pressed(KeyCode::PageUp) {
                self.cycle_place(-1);
                self.palette_scroll_to_armed = true;
            }
        }
        let nums = [KeyCode::Key1, KeyCode::Key2, KeyCode::Key3, KeyCode::Key4, KeyCode::Key5, KeyCode::Key6, KeyCode::Key7, KeyCode::Key8, KeyCode::Key9];
        let layers = self.doc.as_ref().map_or(0, |d| d.map.layers.len());
        for (i, k) in nums.iter().enumerate() {
            if is_key_pressed(*k) && i < layers {
                self.layer = i;
            }
        }
    }

    fn paint_stamp(&mut self, tx: i32, ty: i32, erase: bool) {
        let (w, h) = (self.brush.w, self.brush.h);
        for dy in 0..h {
            for dx in 0..w {
                let t = if erase { Tile::EMPTY } else { self.stamp_tile(dx, dy) };
                if let Some(doc) = &mut self.doc {
                    doc.paint(self.layer, tx + dx as i32, ty + dy as i32, t, &self.assets);
                }
            }
        }
    }

    fn pick(&mut self, tile: (i32, i32)) {
        let Some(doc) = &self.doc else { return };
        if !doc.map.in_bounds(tile.0, tile.1) {
            return;
        }
        // Top-most visible non-empty tile, preferring the active layer.
        let order = std::iter::once(self.layer).chain((0..doc.map.layers.len()).rev());
        for li in order {
            let Some(l) = doc.map.layers.get(li) else { continue };
            if let Some((slot, idx)) = l.get(tile.0 as u32, tile.1 as u32).parts() {
                let Some(id) = doc.map.tilesets.get(slot) else { continue };
                let Some(ts) = self.assets.tilesets.get(id) else { continue };
                self.brush.tileset = id.clone();
                self.brush.x = idx % ts.columns;
                self.brush.y = idx / ts.columns;
                self.brush.w = 1;
                self.brush.h = 1;
                self.layer = li;
                if self.tool == Tool::Pick {
                    self.tool = Tool::Brush;
                }
                return;
            }
        }
    }

    fn right_click(&mut self, tile: (i32, i32)) {
        match self.tool {
            t if t.paints_tiles() => self.pick(tile),
            // Right-click clears the placing cursor (and the selection).
            Tool::Entity => {
                self.disarm();
                self.sel = Selection::None;
            }
            Tool::Zone | Tool::Select => self.sel = Selection::None,
            _ => {}
        }
    }

    fn mouse_down(&mut self, tile: (i32, i32), world: Vec2) {
        self.press_world = world;
        self.click_stack = None;
        let layer_ok = self.doc.as_ref().is_some_and(|d| self.layer < d.map.layers.len());
        match self.tool {
            Tool::Brush | Tool::Erase if layer_ok => {
                if let Some(doc) = &mut self.doc {
                    doc.begin_stroke(self.layer, false);
                }
                self.paint_stamp(tile.0, tile.1, self.tool == Tool::Erase);
                self.drag = Some(Drag::Paint { last: tile });
            }
            Tool::Rect if layer_ok => self.drag = Some(Drag::Rect { start: tile }),
            Tool::Fill if layer_ok => {
                let (bw, bh) = (self.brush.w, self.brush.h);
                let tiles: Vec<Vec<Tile>> = (0..bh).map(|dy| (0..bw).map(|dx| self.stamp_tile(dx, dy)).collect()).collect();
                let (ox, oy) = (tile.0, tile.1);
                let layer = self.layer;
                if let Some(doc) = &mut self.doc {
                    let n = doc.flood_fill(
                        layer,
                        tile.0,
                        tile.1,
                        |x, y| {
                            let dx = (x as i32 - ox).rem_euclid(bw as i32) as usize;
                            let dy = (y as i32 - oy).rem_euclid(bh as i32) as usize;
                            tiles[dy][dx]
                        },
                        &self.assets,
                    );
                    self.set_status(format!("Filled {n} tiles"));
                }
            }
            Tool::Pick => self.pick(tile),
            Tool::Select => {
                let stack = self.objects_at(world);
                let Some(&first) = stack.first() else {
                    self.sel = Selection::None;
                    return;
                };
                // Double-click on a portal: go to the other side.
                if let Selection::Zone(uid) = first {
                    let now = macroquad::time::get_time();
                    let again = self.last_portal_click.is_some_and(|(u, t)| u == uid && now - t < 0.4);
                    self.last_portal_click = Some((uid, now));
                    let portal = self.doc.as_ref().and_then(|d| d.map.zones.iter().find(|z| z.uid == uid)).and_then(|z| match &z.kind {
                        ZoneKind::Portal { target_map, target_spawn } => Some((target_map.clone(), target_spawn.clone())),
                        _ => None,
                    });
                    if let (true, Some((m, s))) = (again, portal) {
                        self.last_portal_click = None;
                        self.goto_map(&m, Some(&s));
                        return;
                    }
                }
                let already = stack.contains(&self.sel);
                if !already {
                    self.sel = first;
                    self.on_map_scroll = true;
                }
                self.click_stack = Some((stack, already));
                self.grab_selection(tile, world);
            }
            Tool::Collision => {
                let v = if modifier_shift() { CollisionOverride::Auto } else { CollisionOverride::Solid };
                if let Some(doc) = &mut self.doc {
                    doc.begin_stroke(0, true);
                    doc.paint_collision(tile.0, tile.1, v as u8);
                }
                self.drag = Some(Drag::Paint { last: tile });
            }
            Tool::Entity => {
                if let Some(uid) = self.entity_at(world) {
                    self.sel = Selection::Entity(uid);
                    let doc = self.doc.as_ref().unwrap();
                    let e = doc.map.entity(uid).unwrap();
                    self.drag = Some(Drag::MoveEntity { uid, grab: world - vec2(e.x, e.y), before: Box::new(Objects::of(&doc.map)) });
                    self.on_map_scroll = true;
                } else if self.place_def.is_empty() {
                    // Nothing armed: clicking empty space just deselects.
                    self.sel = Selection::None;
                } else if self.doc.as_ref().is_some_and(|d| d.map.in_bounds(tile.0, tile.1)) {
                    let wide = self.def_tiles_wide(self.place_kind, &self.place_def);
                    let pos = self.snap_entity(world, modifier_shift(), wide);
                    self.place_entity(pos);
                }
            }
            Tool::Link => self.drag = Some(Drag::LinkRect { start: tile }),
            Tool::Zone => {
                if let Some(uid) = self.zone_at(world) {
                    self.sel = Selection::Zone(uid);
                    let doc = self.doc.as_ref().unwrap();
                    let z = doc.map.zones.iter().find(|z| z.uid == uid).unwrap();
                    self.drag = Some(Drag::MoveZone { uid, grab: tile, orig: (z.x, z.y), before: Box::new(Objects::of(&doc.map)) });
                } else {
                    self.drag = Some(Drag::NewZone { start: tile });
                }
            }
            _ => {}
        }
    }

    fn mouse_drag(&mut self, tile: (i32, i32), world: Vec2) {
        let free = modifier_shift();
        match self.drag.take() {
            Some(Drag::Paint { last }) => {
                if tile != last {
                    for t in line(last, tile).into_iter().skip(1) {
                        if self.tool == Tool::Collision {
                            let v = if free { CollisionOverride::Auto } else { CollisionOverride::Solid };
                            if let Some(doc) = &mut self.doc {
                                doc.paint_collision(t.0, t.1, v as u8);
                            }
                        } else {
                            self.paint_stamp(t.0, t.1, self.tool == Tool::Erase);
                        }
                    }
                }
                self.drag = Some(Drag::Paint { last: tile });
            }
            Some(d @ (Drag::MoveEntity { .. } | Drag::MoveZone { .. })) if !self.dragged(world) => {
                // Not a drag yet: a plain click must not nudge anything.
                self.drag = Some(d);
            }
            Some(Drag::MoveEntity { uid, grab, before }) => {
                let wide = self.doc.as_ref().and_then(|d| d.map.entity(uid)).map_or(1, |e| self.def_tiles_wide(e.kind, &e.def));
                let target = self.snap_entity(world - grab, free, wide);
                if let Some(e) = self.doc.as_mut().and_then(|d| d.map.entities.iter_mut().find(|e| e.uid == uid)) {
                    e.x = target.x;
                    e.y = target.y;
                }
                self.drag = Some(Drag::MoveEntity { uid, grab, before });
            }
            Some(Drag::MoveZone { uid, grab, orig, before }) => {
                if let Some(doc) = &mut self.doc {
                    let (mw, mh) = (doc.map.width, doc.map.height);
                    if let Some(z) = doc.map.zones.iter_mut().find(|z| z.uid == uid) {
                        z.x = (orig.0 as i32 + tile.0 - grab.0).clamp(0, (mw - z.w.min(mw)) as i32) as u32;
                        z.y = (orig.1 as i32 + tile.1 - grab.1).clamp(0, (mh - z.h.min(mh)) as i32) as u32;
                    }
                }
                self.drag = Some(Drag::MoveZone { uid, grab, orig, before });
            }
            other => self.drag = other,
        }
    }

    fn mouse_up(&mut self, tile: (i32, i32)) {
        match self.drag.take() {
            Some(Drag::Paint { .. }) => {
                if let Some(doc) = &mut self.doc {
                    doc.end_stroke();
                }
            }
            Some(Drag::Rect { start }) => {
                let (x0, y0, w, h) = rect_of(start, tile);
                let erase = modifier_shift();
                if let Some(doc) = &mut self.doc {
                    doc.begin_stroke(self.layer, false);
                }
                for y in y0..y0 + h {
                    for x in x0..x0 + w {
                        let t = if erase { Tile::EMPTY } else { self.stamp_tile((x - x0) as u32, (y - y0) as u32) };
                        if let Some(doc) = &mut self.doc {
                            doc.paint(self.layer, x, y, t, &self.assets);
                        }
                    }
                }
                if let Some(doc) = &mut self.doc {
                    doc.end_stroke();
                }
            }
            Some(Drag::MoveEntity { before, .. }) | Some(Drag::MoveZone { before, .. }) => {
                if let Some(doc) = &mut self.doc {
                    doc.record_objects(*before, "move");
                }
                // A plain click on something already selected cycles through what's stacked there.
                if let Some((stack, true)) = self.click_stack.take() {
                    if !self.dragged(self.hover_world) && stack.len() > 1 {
                        let i = stack.iter().position(|s| *s == self.sel).map_or(0, |i| (i + 1) % stack.len());
                        self.sel = stack[i];
                        self.on_map_scroll = true;
                    }
                }
            }
            Some(Drag::LinkRect { start }) => {
                let Some(doc) = &self.doc else { return };
                let (mw, mh) = (doc.map.width as i32, doc.map.height as i32);
                let (x0, y0, w, h) = rect_of(start, tile);
                let (x1, y1) = ((x0 + w).min(mw), (y0 + h).min(mh));
                let (x0, y0) = (x0.max(0), y0.max(0));
                if x1 > x0 && y1 > y0 {
                    self.link_area((x0 as u32, y0 as u32, (x1 - x0) as u32, (y1 - y0) as u32));
                }
            }
            Some(Drag::NewZone { start }) => {
                let Some(doc) = &mut self.doc else { return };
                let (mw, mh) = (doc.map.width as i32, doc.map.height as i32);
                let (x0, y0, w, h) = rect_of(start, tile);
                let (x1, y1) = ((x0 + w).min(mw), (y0 + h).min(mh));
                let (x0, y0) = (x0.max(0), y0.max(0));
                if x1 <= x0 || y1 <= y0 {
                    return;
                }
                let before = Objects::of(&doc.map);
                let uid = doc.map.alloc_uid();
                let (kind, prefix) = match self.zone_template {
                    ZoneTemplate::Spawn => (ZoneKind::Spawn, "spawn"),
                    ZoneTemplate::Portal => (
                        ZoneKind::Portal { target_map: self.refs.maps.first().cloned().unwrap_or_default(), target_spawn: String::new() },
                        "portal",
                    ),
                    ZoneTemplate::Trigger => (ZoneKind::Trigger, "zone"),
                    ZoneTemplate::Spawner => (crate::app::default_spawner(&self.refs), "spawner"),
                    ZoneTemplate::Transition => (ZoneKind::Transition { target_map: String::new(), target_zone: String::new() }, "transition"),
                };
                doc.map.zones.push(Zone {
                    uid,
                    name: format!("{prefix}_{uid}"),
                    x: x0 as u32,
                    y: y0 as u32,
                    w: (x1 - x0) as u32,
                    h: (y1 - y0) as u32,
                    kind,
                    triggers: Vec::new(),
                    loot: None,
                });
                doc.record_objects(before, "zone");
                self.sel = Selection::Zone(uid);
                self.refresh_refs();
            }
            other => self.drag = other.filter(|_| false),
        }
    }

    // ------------------------------------------------------------ drawing

    pub fn draw_canvas(&mut self) {
        clear_background(Color::from_rgba(32, 34, 40, 255));
        if self.mode != Mode::Map || self.doc.is_none() {
            return;
        }
        let cam = self.camera();
        set_camera(&cam);
        let view = render::camera_view(&cam);
        let px = 1.0 / self.cam.zoom; // one screen pixel in world units
        let mut doc = self.doc.take().unwrap();
        self.draw_map(&mut doc, &cam, view, px);
        self.doc = Some(doc);
    }

    fn draw_map(&mut self, doc: &mut crate::doc::MapDoc, cam: &Camera2D, view: Rect, px: f32) {
        let ts = self.ts();
        let map = &doc.map;
        let (mw, mh) = (map.width as f32 * ts, map.height as f32 * ts);
        let bg = map.background;
        draw_rectangle(0.0, 0.0, mw, mh, Color::from_rgba(bg[0], bg[1], bg[2], 255));

        let overview = render::visible_tiles(view, ts) > OVERVIEW_THRESHOLD;
        if overview {
            doc.overview.draw(&doc.map, ts);
        }
        let map = &doc.map;
        let sets = self.assets.map_tilesets(map);
        let tint = |i: usize| {
            if self.view.dim_layers && i != self.layer {
                Color::new(1.0, 1.0, 1.0, 0.35)
            } else {
                WHITE
            }
        };
        if !overview {
            for (i, l) in map.layers.iter().enumerate().filter(|(_, l)| l.visible && !l.above_entities) {
                render::draw_layer(map, l, &sets, ts, view, tint(i));
            }
        }

        // Entities (y-sorted).
        let mut ents: Vec<&EntityInstance> = map
            .entities
            .iter()
            .filter(|e| e.x > view.x - 64.0 && e.x < view.x + view.w + 64.0 && e.y > view.y - 64.0 && e.y < view.y + view.h + 64.0)
            .collect();
        ents.sort_by(|a, b| a.y.total_cmp(&b.y));
        if ents.len() < 5000 {
            for e in &ents {
                let col = if e.active { WHITE } else { Color::new(1.0, 1.0, 1.0, 0.4) };
                let pos = vec2(e.x, e.y);
                let db = &self.project.db;
                let drawn = match e.kind {
                    EntityKind::Prop => db.prop(&e.def).is_some_and(|d| self.assets.draw_sprite(d.state_look(d.state_index(&e.state).unwrap_or(0)).0, e.frame, pos, col)),
                    EntityKind::Character => db.character(&e.def).is_some_and(|d| {
                        let sheet = if e.sheet.is_empty() { &d.sheet } else { &e.sheet };
                        self.assets.draw_frame(sheet, e.facing as u32 * d.walk_frames.max(1), pos, col)
                    }),
                    EntityKind::Item => db.item(&e.def).is_some_and(|d| self.assets.draw_sprite(&d.icon, 0, pos, col)),
                };
                if !drawn {
                    draw_rectangle(e.x - ts / 2.0, e.y - ts, ts, ts, Color::new(1.0, 0.0, 1.0, 0.6));
                }
            }
        }

        if !overview {
            for (i, l) in map.layers.iter().enumerate().filter(|(_, l)| l.visible && l.above_entities) {
                let mut t = tint(i);
                // Overhead layers get see-through while editing something below them.
                if i != self.layer && self.tool.paints_tiles() {
                    t.a *= 0.6;
                }
                render::draw_layer(map, l, &sets, ts, view, t);
            }
        }

        // Visible tile range.
        let x0 = (view.x / ts).floor().max(0.0) as u32;
        let y0 = (view.y / ts).floor().max(0.0) as u32;
        let x1 = ((view.x + view.w) / ts).ceil().clamp(0.0, map.width as f32) as u32;
        let y1 = ((view.y + view.h) / ts).ceil().clamp(0.0, map.height as f32) as u32;
        let tile_px = ts * self.cam.zoom;

        if (self.view.collision || self.tool == Tool::Collision) && tile_px >= 3.0 {
            let is_solid = |slot: usize, i: u32| sets.get(slot).copied().flatten().is_some_and(|t| t.is_solid(i));
            for y in y0..y1 {
                for x in x0..x1 {
                    let over = map.collision_at(x, y);
                    let (fx, fy) = (x as f32 * ts, y as f32 * ts);
                    if map.tile_solid(x, y, &is_solid) {
                        let c = if over == CollisionOverride::Solid { Color::new(1.0, 0.1, 0.1, 0.55) } else { Color::new(1.0, 0.2, 0.2, 0.3) };
                        draw_rectangle(fx, fy, ts, ts, c);
                    } else if over == CollisionOverride::Walkable {
                        draw_rectangle(fx, fy, ts, ts, Color::new(0.2, 1.0, 0.3, 0.3));
                    }
                }
            }
        }

        if self.view.grid && tile_px >= 8.0 {
            let c = Color::new(1.0, 1.0, 1.0, 0.08);
            for x in x0..=x1 {
                draw_line(x as f32 * ts, y0 as f32 * ts, x as f32 * ts, y1 as f32 * ts, px, c);
            }
            for y in y0..=y1 {
                draw_line(x0 as f32 * ts, y as f32 * ts, x1 as f32 * ts, y as f32 * ts, px, c);
            }
        }

        if self.view.zones {
            for z in &map.zones {
                let r = z.pixel_rect(ts);
                let c = zone_color(&z.kind);
                draw_rectangle(r.x, r.y, r.w, r.h, Color::new(c.r, c.g, c.b, 0.22));
                let selected = self.sel == Selection::Zone(z.uid);
                draw_rectangle_lines(r.x, r.y, r.w, r.h, if selected { 3.0 * px } else { 1.5 * px }, if selected { YELLOW } else { c });
            }
            for e in Edge::ALL {
                if let Some(r) = map.edges.strip_rect(e, map.width, map.height, ts) {
                    draw_rectangle(r.x, r.y, r.w, r.h, Color::new(0.2, 1.0, 0.4, 0.18));
                }
            }
        }

        // Patrol route of the selected character: numbered waypoints joined by lines.
        if let Some(r) = match self.sel {
            Selection::Entity(uid) => map.entity(uid).and_then(|e| e.patrol.as_ref()),
            _ => None,
        } {
            let editing = self.route_edit.is_some();
            let col = if editing { Color::new(0.3, 0.95, 1.0, 1.0) } else { Color::new(0.3, 0.95, 1.0, 0.6) };
            let c = |p: (i32, i32)| vec2((p.0 as f32 + 0.5) * ts, (p.1 as f32 + 0.5) * ts);
            let n = r.points.len();
            let closes = r.mode == rust2d_engine::entity::PatrolMode::Loop && n > 2;
            for k in 0..n.saturating_sub(1) + usize::from(closes) {
                let (a, b) = (c(r.points[k]), c(r.points[(k + 1) % n]));
                draw_line(a.x, a.y, b.x, b.y, 2.0 * px, col);
                // Arrow head halfway, pointing the walking direction.
                let mid = a.lerp(b, 0.5);
                let d = (b - a).normalize_or_zero() * 6.0 * px;
                let nrm = vec2(-d.y, d.x);
                draw_triangle(mid + d, mid - d + nrm, mid - d - nrm, col);
            }
            for (k, p) in r.points.iter().enumerate() {
                let q = c(*p);
                draw_circle(q.x, q.y, 9.0 * px, Color::new(0.05, 0.2, 0.3, 0.9));
                draw_circle_lines(q.x, q.y, 9.0 * px, 1.5 * px, col);
                let t = (k + 1).to_string();
                let fs = (14.0 * px).max(8.0);
                let w = measure_text(&t, None, fs as u16, 1.0).width;
                draw_text(&t, q.x - w / 2.0, q.y + fs * 0.35, fs, WHITE);
            }
        }

        // Selection outline.
        if let Selection::Entity(uid) = self.sel {
            if let Some(e) = map.entity(uid) {
                let r = self.entity_rect(e);
                draw_rectangle_lines(r.x - px, r.y - px, r.w + 2.0 * px, r.h + 2.0 * px, 2.0 * px, YELLOW);
                // Collision box (what actually blocks movement).
                if let Some(hb) = self.entity_hitbox(e) {
                    draw_rectangle(hb.x, hb.y, hb.w, hb.h, Color::new(1.0, 0.5, 0.1, 0.25));
                    draw_rectangle_lines(hb.x, hb.y, hb.w, hb.h, 1.5 * px, ORANGE);
                }
            }
        }

        // Tool previews.
        if let Some((tx, ty)) = self.hover_tile {
            let (fx, fy) = (tx as f32 * ts, ty as f32 * ts);
            match (&self.drag, self.tool) {
                (Some(Drag::Rect { start }), _) => {
                    let (rx, ry, rw, rh) = rect_of(*start, (tx, ty));
                    draw_rectangle(rx as f32 * ts, ry as f32 * ts, rw as f32 * ts, rh as f32 * ts, Color::new(1.0, 1.0, 0.3, 0.2));
                    draw_rectangle_lines(rx as f32 * ts, ry as f32 * ts, rw as f32 * ts, rh as f32 * ts, 2.0 * px, YELLOW);
                }
                (Some(Drag::NewZone { start }), _) => {
                    let (rx, ry, rw, rh) = rect_of(*start, (tx, ty));
                    draw_rectangle_lines(rx as f32 * ts, ry as f32 * ts, rw as f32 * ts, rh as f32 * ts, 2.0 * px, SKYBLUE);
                }
                (Some(Drag::LinkRect { start }), _) => {
                    let (rx, ry, rw, rh) = rect_of(*start, (tx, ty));
                    draw_rectangle(rx as f32 * ts, ry as f32 * ts, rw as f32 * ts, rh as f32 * ts, Color::new(1.0, 0.6, 0.1, 0.25));
                    draw_rectangle_lines(rx as f32 * ts, ry as f32 * ts, rw as f32 * ts, rh as f32 * ts, 2.0 * px, ORANGE);
                }
                (None, Tool::Link) => {
                    draw_rectangle_lines(fx, fy, ts, ts, 2.0 * px, ORANGE);
                }
                (_, Tool::Brush | Tool::Rect | Tool::Fill) if self.drag.is_none() => {
                    if let Some(t) = self.assets.tilesets.get(&self.brush.tileset) {
                        let src = Rect::new(self.brush.x as f32 * ts, self.brush.y as f32 * ts, self.brush.w as f32 * ts, self.brush.h as f32 * ts);
                        draw_texture_ex(
                            &t.texture,
                            fx,
                            fy,
                            Color::new(1.0, 1.0, 1.0, 0.6),
                            DrawTextureParams { source: Some(src), ..Default::default() },
                        );
                        draw_rectangle_lines(fx, fy, src.w, src.h, 1.5 * px, YELLOW);
                    }
                }
                (_, Tool::Erase) => {
                    draw_rectangle_lines(fx, fy, self.brush.w as f32 * ts, self.brush.h as f32 * ts, 2.0 * px, RED);
                }
                (_, Tool::Entity) if self.drag.is_none() && !self.place_def.is_empty() => {
                    // Ghost of the definition about to be placed (unless hovering an entity).
                    let wide = self.def_tiles_wide(self.place_kind, &self.place_def);
                    let p = self.snap_entity(self.hover_world, modifier_shift(), wide);
                    if self.entity_at(self.hover_world).is_none() {
                        if let Some(s) = self.def_sprite(self.place_kind, &self.place_def) {
                            self.assets.draw_sprite(&s, 0, p, Color::new(1.0, 1.0, 1.0, 0.5));
                        }
                    }
                    draw_circle_lines(p.x, p.y, 3.0 * px, 1.5 * px, YELLOW);
                }
                (None, Tool::Select) => {
                    // Outline what a click would select.
                    let hover = self.objects_at(self.hover_world).into_iter().next();
                    let r = match hover {
                        Some(Selection::Entity(uid)) => map.entity(uid).map(|e| self.entity_rect(e)),
                        Some(Selection::Zone(uid)) => map.zones.iter().find(|z| z.uid == uid).map(|z| z.pixel_rect(ts)),
                        _ => None,
                    };
                    match r {
                        Some(r) if hover != Some(self.sel) => draw_rectangle_lines(r.x, r.y, r.w, r.h, 1.5 * px, Color::new(1.0, 1.0, 1.0, 0.8)),
                        Some(_) => {}
                        None => draw_rectangle_lines(fx, fy, ts, ts, px, Color::new(1.0, 1.0, 1.0, 0.25)),
                    }
                }
                (None, _) => draw_rectangle_lines(fx, fy, ts, ts, 1.5 * px, Color::new(1.0, 1.0, 1.0, 0.6)),
                _ => {}
            }
        }

        draw_rectangle_lines(0.0, 0.0, mw, mh, 2.0 * px, Color::new(1.0, 1.0, 1.0, 0.5));

        // Screen-space labels.
        set_default_camera();
        if self.view.zones {
            for z in &map.zones {
                let r = z.pixel_rect(ts);
                let p = cam.world_to_screen(vec2(r.x, r.y));
                let label = match &z.kind {
                    ZoneKind::Portal { target_map, target_spawn } => format!("{} -> {target_map}:{target_spawn}", z.name),
                    ZoneKind::Transition { target_map, target_zone } if target_map.is_empty() => format!("{} <-> (not linked)", z.name),
                    ZoneKind::Transition { target_map, target_zone } => format!("{} <-> {target_map}:{target_zone}", z.name),
                    ZoneKind::Spawner { character, count, respawn } => {
                        let r = match respawn {
                            rust2d_engine::map::Respawn::Never => "no respawn".to_string(),
                            rust2d_engine::map::Respawn::OnEnter => "respawn on enter".to_string(),
                            rust2d_engine::map::Respawn::After { seconds } => format!("respawn {seconds}s"),
                        };
                        format!("{} [{count}x {character}, {r}]", z.name)
                    }
                    _ => z.name.clone(),
                };
                // Transitions whose other end is missing or leads elsewhere are flagged.
                let broken = match &z.kind {
                    ZoneKind::Transition { target_map, target_zone } => !self.refs.transitions.get(target_map).is_some_and(|v| {
                        v.iter().any(|(n, m, b)| n == target_zone && *m == map.name && *b == z.name)
                    }),
                    _ => false,
                };
                if r.w * self.cam.zoom > 20.0 || self.sel == Selection::Zone(z.uid) || broken {
                    draw_text(&label, p.x + 3.0, p.y + 14.0, 16.0, if broken { Color::new(1.0, 0.5, 0.5, 1.0) } else { WHITE });
                }
            }
            for e in Edge::ALL {
                if let (Some(l), Some(r)) = (map.edges.get(e), map.edges.strip_rect(e, map.width, map.height, ts)) {
                    let c = cam.world_to_screen(vec2(r.x + r.w / 2.0, r.y + r.h / 2.0));
                    let s = format!("{} -> {} ({:+})", e.label(), l.target_map, l.offset);
                    let w = measure_text(&s, None, 18, 1.0).width;
                    draw_rectangle(c.x - w / 2.0 - 4.0, c.y - 14.0, w + 8.0, 20.0, Color::new(0.0, 0.3, 0.1, 0.8));
                    draw_text(&s, c.x - w / 2.0, c.y + 1.0, 18.0, WHITE);
                }
            }
        }
        if self.cam.zoom >= 1.5 {
            for e in ents.iter().filter(|e| !e.name.is_empty()) {
                let p = cam.world_to_screen(vec2(e.x, e.y));
                let w = measure_text(&e.name, None, 14, 1.0).width;
                draw_text(&e.name, p.x - w / 2.0, p.y + 12.0, 14.0, Color::new(1.0, 1.0, 0.6, 0.9));
            }
        }

        // Minimap.
        let mm = if self.view.minimap && self.canvas_rect.is_positive() {
            let max = 180.0f32;
            let s = (max / map.width as f32).min(max / map.height as f32);
            Some((s, map.width as f32 * s, map.height as f32 * s))
        } else {
            None
        };
        if let Some((s, w, h)) = mm {
            let (x, y) = (self.canvas_rect.max.x - w - 10.0, self.canvas_rect.max.y - h - 10.0);
            draw_rectangle(x - 2.0, y - 2.0, w + 4.0, h + 4.0, Color::new(0.0, 0.0, 0.0, 0.7));
            doc.overview.draw_in(Rect::new(x, y, w, h));
            let k = s / ts;
            let vr = Rect::new(x + view.x * k, y + view.y * k, view.w * k, view.h * k);
            draw_rectangle_lines(vr.x.max(x), vr.y.max(y), vr.w.min(w), vr.h.min(h), 1.5, YELLOW);
            // Click/drag on the minimap to move the camera.
            let m = mouse();
            if m.down[L] && Rect::new(x, y, w, h).contains(m.pos) && self.drag.is_none() {
                let m = m.pos;
                self.cam.center = vec2((m.x - x) / k, (m.y - y) / k);
            }
        }
    }
}

fn zone_color(k: &ZoneKind) -> Color {
    match k {
        ZoneKind::Spawn => Color::new(0.3, 0.8, 1.0, 1.0),
        ZoneKind::Portal { .. } => Color::new(0.8, 0.4, 1.0, 1.0),
        ZoneKind::Transition { .. } => Color::new(1.0, 0.6, 0.1, 1.0),
        ZoneKind::Trigger => Color::new(1.0, 0.65, 0.2, 1.0),
        ZoneKind::Spawner { .. } => Color::new(1.0, 0.25, 0.3, 1.0),
    }
}

impl Editor {
    /// Mouse handling while a patrol route is edited. Returns true if the event was used.
    fn route_edit_mouse(&mut self, tile: (i32, i32), ms: &Mouse, over_ui: bool) -> bool {
        const L: usize = 0;
        const R: usize = 1;
        let Some(uid) = self.route_edit else { return false };
        let Some(doc) = &mut self.doc else { return false };
        // Editing ends when another object is selected or the character is gone.
        if self.sel != Selection::Entity(uid) || doc.map.entity(uid).is_none_or(|e| e.patrol.is_none()) {
            self.route_edit = None;
            self.route_drag = None;
            return false;
        }
        let inside = doc.map.in_bounds(tile.0, tile.1);
        fn points(doc: &mut crate::doc::MapDoc, uid: u32) -> Option<&mut Vec<(i32, i32)>> {
            doc.map.entities.iter_mut().find(|e| e.uid == uid).and_then(|e| e.patrol.as_mut()).map(|r| &mut r.points)
        }
        // Dragging a waypoint.
        if let Some((k, before)) = self.route_drag.take() {
            if ms.down[L] {
                if inside {
                    if let Some(p) = points(doc, uid).and_then(|p| p.get_mut(k)) {
                        *p = tile;
                    }
                }
                self.route_drag = Some((k, before));
            } else {
                doc.record_objects(before, "route");
            }
            return true;
        }
        if over_ui || !inside {
            return false;
        }
        if ms.pressed[L] {
            let before = crate::doc::Objects::of(&doc.map);
            let Some(pts) = points(doc, uid) else { return false };
            match pts.iter().position(|p| *p == tile) {
                // On a waypoint: drag it.
                Some(k) => self.route_drag = Some((k, before)),
                None => {
                    pts.push(tile);
                    doc.record_objects(before, "route");
                }
            }
            return true;
        }
        if ms.pressed[R] {
            let before = crate::doc::Objects::of(&doc.map);
            if let Some(pts) = points(doc, uid) {
                // The waypoint on this tile, else the last one.
                let k = pts.iter().rposition(|p| *p == tile).or(pts.len().checked_sub(1));
                if let Some(k) = k {
                    pts.remove(k);
                }
            }
            doc.record_objects(before, "route");
            return true;
        }
        false
    }
}
