//! An open map with undo/redo history and editing operations.

use std::collections::HashMap;
use std::time::Instant;

use rust2d_engine::entity::EntityInstance;
use rust2d_engine::events::Trigger;
use rust2d_engine::map::{EdgeLinks, Map, Tile, Zone};
use rust2d_engine::render::{Assets, Overview};

/// Everything on a map that isn't tile data.
#[derive(Clone, PartialEq)]
pub struct Objects {
    pub entities: Vec<EntityInstance>,
    pub zones: Vec<Zone>,
    pub edges: EdgeLinks,
    pub triggers: Vec<Trigger>,
    pub next_uid: u32,
    pub background: [u8; 3],
}

impl Objects {
    pub fn of(map: &Map) -> Self {
        Objects {
            entities: map.entities.clone(),
            zones: map.zones.clone(),
            edges: map.edges.clone(),
            triggers: map.triggers.clone(),
            next_uid: map.next_uid,
            background: map.background,
        }
    }
    fn apply(&self, map: &mut Map) {
        map.entities = self.entities.clone();
        map.zones = self.zones.clone();
        map.edges = self.edges.clone();
        map.triggers = self.triggers.clone();
        map.next_uid = self.next_uid;
        map.background = self.background;
    }
}

pub enum Edit {
    Tiles { layer: usize, changes: Vec<(u32, u32, Tile, Tile)> },
    Collision { changes: Vec<(u32, u32, u8, u8)> },
    Objects { before: Box<Objects>, after: Box<Objects>, tag: &'static str, time: Instant },
    /// Whole-map replacement (resize, layer add/remove).
    Map { before: Box<Map>, after: Box<Map> },
}

/// Tile changes being accumulated during a mouse drag.
#[derive(Default)]
pub struct Stroke {
    layer: usize,
    collision: bool,
    tiles: HashMap<(u32, u32), (Tile, Tile)>,
    col: HashMap<(u32, u32), (u8, u8)>,
}

pub struct MapDoc {
    pub map: Map,
    pub overview: Overview,
    /// History entries with an id each (ids are never reused).
    undo: Vec<(u64, Edit)>,
    redo: Vec<(u64, Edit)>,
    next_id: u64,
    /// History position of the saved file: `Some(id of the newest undo entry)` (`Some(None)`
    /// = empty history), or `None` when no undo/redo can get back to it.
    saved: Option<Option<u64>>,
    stroke: Option<Stroke>,
}

const MAX_HISTORY: usize = 200;

impl MapDoc {
    pub fn new(map: Map, assets: &Assets) -> Self {
        let overview = Overview::build(&map, assets);
        MapDoc { map, overview, undo: Vec::new(), redo: Vec::new(), next_id: 0, saved: Some(None), stroke: None }
    }

    pub fn rebuild_overview(&mut self, assets: &Assets) {
        self.overview = Overview::build(&self.map, assets);
    }

    fn push(&mut self, e: Edit) {
        self.next_id += 1;
        self.undo.push((self.next_id, e));
        if self.undo.len() > MAX_HISTORY {
            self.undo.remove(0);
            if self.saved == Some(None) {
                self.saved = None; // the saved state was before the dropped entry
            }
        }
        self.redo.clear();
    }

    /// History position right now.
    fn position(&self) -> Option<u64> {
        self.undo.last().map(|(id, _)| *id)
    }

    /// Whether the map differs from the saved file. Undoing back to the saved state (or
    /// redoing to it) makes it clean again.
    pub fn is_dirty(&self) -> bool {
        self.stroke.is_some() || self.saved != Some(self.position())
    }

    /// The map was written to (or freshly loaded from) disk.
    pub fn mark_saved(&mut self) {
        self.saved = Some(self.position());
    }

    /// The map was changed outside the undo history: only saving makes it clean again.
    pub fn mark_changed(&mut self) {
        self.saved = None;
    }

    /// Drops undo/redo history (after edits made outside the history, e.g. id renames).
    pub fn clear_history(&mut self) {
        self.end_stroke();
        let clean = !self.is_dirty();
        self.undo.clear();
        self.redo.clear();
        self.saved = if clean { Some(None) } else { None };
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }
    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    // ------------------------------------------------------------ strokes

    pub fn begin_stroke(&mut self, layer: usize, collision: bool) {
        self.end_stroke();
        self.stroke = Some(Stroke { layer, collision, ..Default::default() });
    }

    pub fn end_stroke(&mut self) {
        let Some(s) = self.stroke.take() else { return };
        if s.collision {
            let changes: Vec<_> = s.col.into_iter().filter(|(_, (a, b))| a != b).map(|((x, y), (a, b))| (x, y, a, b)).collect();
            if !changes.is_empty() {
                self.push(Edit::Collision { changes });
            }
        } else {
            let changes: Vec<_> = s.tiles.into_iter().filter(|(_, (a, b))| a != b).map(|((x, y), (a, b))| (x, y, a, b)).collect();
            if !changes.is_empty() {
                self.push(Edit::Tiles { layer: s.layer, changes });
            }
        }
    }

    /// Sets a tile as part of the current stroke (starting one if needed).
    pub fn paint(&mut self, layer: usize, x: i32, y: i32, tile: Tile, assets: &Assets) {
        if !self.map.in_bounds(x, y) || layer >= self.map.layers.len() {
            return;
        }
        if self.stroke.is_none() {
            self.begin_stroke(layer, false);
        }
        let (x, y) = (x as u32, y as u32);
        let old = self.map.layers[layer].set(x, y, tile);
        if old != tile {
            self.overview.update_tile(&self.map, assets, x, y);
        }
        let s = self.stroke.as_mut().unwrap();
        s.tiles.entry((x, y)).or_insert((old, tile)).1 = tile;
    }

    pub fn paint_collision(&mut self, x: i32, y: i32, value: u8) {
        if !self.map.in_bounds(x, y) {
            return;
        }
        if self.stroke.is_none() {
            self.begin_stroke(0, true);
        }
        let i = (y as u32 * self.map.width + x as u32) as usize;
        let old = std::mem::replace(&mut self.map.collision[i], value);
        let s = self.stroke.as_mut().unwrap();
        s.col.entry((x as u32, y as u32)).or_insert((old, value)).1 = value;
    }

    /// Flood-fills the 4-connected region of identical tiles starting at `(x, y)`.
    /// `pattern(x, y)` gives the tile for each filled cell. Returns cells changed.
    pub fn flood_fill(&mut self, layer: usize, x: i32, y: i32, pattern: impl Fn(u32, u32) -> Tile, assets: &Assets) -> usize {
        if !self.map.in_bounds(x, y) {
            return 0;
        }
        let (w, h) = (self.map.width, self.map.height);
        let target = self.map.layers[layer].get(x as u32, y as u32);
        let mut seen = vec![false; (w * h) as usize];
        let mut stack = vec![(x as u32, y as u32)];
        let mut cells = Vec::new();
        while let Some((cx, cy)) = stack.pop() {
            let i = (cy * w + cx) as usize;
            if seen[i] || self.map.layers[layer].get(cx, cy) != target {
                continue;
            }
            seen[i] = true;
            cells.push((cx, cy));
            if cx > 0 {
                stack.push((cx - 1, cy));
            }
            if cy > 0 {
                stack.push((cx, cy - 1));
            }
            if cx + 1 < w {
                stack.push((cx + 1, cy));
            }
            if cy + 1 < h {
                stack.push((cx, cy + 1));
            }
        }
        self.begin_stroke(layer, false);
        for &(cx, cy) in &cells {
            self.paint(layer, cx as i32, cy as i32, pattern(cx, cy), assets);
        }
        self.end_stroke();
        cells.len()
    }

    // ------------------------------------------------------------ objects

    /// Records a change to entities/zones/edges/triggers. Consecutive edits with the
    /// same tag within a second are merged (e.g. typing into a text field).
    pub fn record_objects(&mut self, before: Objects, tag: &'static str) {
        let after = Objects::of(&self.map);
        if after == before {
            return;
        }
        let now = Instant::now();
        let at_saved = self.saved == Some(self.position());
        if let Some((_, Edit::Objects { after: last_after, tag: last_tag, time, .. })) = self.undo.last_mut() {
            // Not into the saved entry: undo must still be able to get back to the saved state.
            if *last_tag == tag && now.duration_since(*time).as_secs_f32() < 1.0 && self.redo.is_empty() && !at_saved {
                **last_after = after;
                *time = now;
                return;
            }
        }
        self.push(Edit::Objects { before: Box::new(before), after: Box::new(after), tag, time: now });
    }

    /// Records a whole-map change (resize, layer operations).
    pub fn record_map(&mut self, before: Map, assets: &Assets) {
        let after = self.map.clone();
        self.push(Edit::Map { before: Box::new(before), after: Box::new(after) });
        self.rebuild_overview(assets);
    }

    // ------------------------------------------------------------ undo / redo

    fn apply(&mut self, e: &Edit, forward: bool, assets: &Assets) {
        match e {
            Edit::Tiles { layer, changes } => {
                for &(x, y, old, new) in changes {
                    self.map.layers[*layer].set(x, y, if forward { new } else { old });
                    self.overview.update_tile(&self.map, assets, x, y);
                }
            }
            Edit::Collision { changes } => {
                for &(x, y, old, new) in changes {
                    self.map.collision[(y * self.map.width + x) as usize] = if forward { new } else { old };
                }
            }
            Edit::Objects { before, after, .. } => (if forward { after } else { before }).apply(&mut self.map),
            Edit::Map { before, after } => {
                self.map = (if forward { after } else { before }).as_ref().clone();
                self.rebuild_overview(assets);
            }
        }
    }

    pub fn undo(&mut self, assets: &Assets) {
        self.end_stroke();
        if let Some((id, e)) = self.undo.pop() {
            self.apply(&e, false, assets);
            self.redo.push((id, e));
        }
    }

    pub fn redo(&mut self, assets: &Assets) {
        self.end_stroke();
        if let Some((id, e)) = self.redo.pop() {
            self.apply(&e, true, assets);
            self.undo.push((id, e));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust2d_engine::entity::{EntityInstance, EntityKind};

    fn assets() -> Assets {
        Assets { tile_size: rust2d_engine::project::TILE_SIZE as f32, tilesets: HashMap::new(), sheets: HashMap::new(), errors: vec![] }
    }

    #[test]
    fn stroke_undo_redo() {
        let a = assets();
        let mut d = MapDoc::new(Map::new("t", 64, 64), &a);
        let t = Tile::new(0, 5);
        d.begin_stroke(0, false);
        for x in 0..40 {
            d.paint(0, x, 3, t, &a);
            d.paint(0, x, 3, t, &a); // repeated paint in one stroke
        }
        d.paint(0, -1, 3, t, &a); // out of bounds is ignored
        d.end_stroke();
        assert!(d.is_dirty() && d.can_undo());
        assert_eq!(d.map.layers[0].get(39, 3), t);
        d.undo(&a);
        assert_eq!(d.map.layers[0].get(39, 3), Tile::EMPTY);
        assert_eq!(d.map.layers[0].get(0, 3), Tile::EMPTY);
        d.redo(&a);
        assert_eq!(d.map.layers[0].get(0, 3), t);
        assert!(!d.can_redo());
    }

    #[test]
    fn flood_fill_is_bounded_by_region() {
        let a = assets();
        let mut d = MapDoc::new(Map::new("t", 20, 20), &a);
        let wall = Tile::new(0, 1);
        d.begin_stroke(0, false);
        for i in 0..20 {
            d.paint(0, 10, i, wall, &a);
        }
        d.end_stroke();
        let n = d.flood_fill(0, 2, 2, |_, _| Tile::new(0, 2), &a);
        assert_eq!(n, 10 * 20);
        assert_eq!(d.map.layers[0].get(15, 5), Tile::EMPTY);
        assert_eq!(d.map.layers[0].get(9, 19), Tile::new(0, 2));
        d.undo(&a);
        assert_eq!(d.map.layers[0].get(9, 19), Tile::EMPTY);
        assert_eq!(d.map.layers[0].get(10, 19), wall);
    }

    #[test]
    fn collision_and_object_edits() {
        let a = assets();
        let mut d = MapDoc::new(Map::new("t", 8, 8), &a);
        d.paint_collision(2, 2, 1);
        d.end_stroke();
        assert_eq!(d.map.collision[2 * 8 + 2], 1);

        let before = Objects::of(&d.map);
        let uid = d.map.alloc_uid();
        d.map.entities.push(EntityInstance::new(uid, EntityKind::Prop, "chest", 8.0, 15.0));
        d.record_objects(before, "place");
        // Two quick inspector edits merge into one undo step.
        for name in ["a", "ab"] {
            let before = Objects::of(&d.map);
            d.map.entities[0].name = name.into();
            d.record_objects(before, "inspector");
        }
        d.undo(&a);
        assert_eq!(d.map.entities[0].name, "");
        d.undo(&a);
        assert!(d.map.entities.is_empty());
        d.undo(&a);
        assert_eq!(d.map.collision[2 * 8 + 2], 0);
        // Unchanged snapshots don't create history.
        let before = Objects::of(&d.map);
        d.record_objects(before, "noop");
        d.redo(&a);
        assert_eq!(d.map.collision[2 * 8 + 2], 1);
    }

    #[test]
    fn dirty_follows_the_saved_history_position() {
        let a = assets();
        let mut d = MapDoc::new(Map::new("t", 8, 8), &a);
        assert!(!d.is_dirty());
        let paint = |d: &mut MapDoc, x| {
            d.paint(0, x, 0, Tile::new(0, 1), &a);
            d.end_stroke();
        };
        paint(&mut d, 1);
        assert!(d.is_dirty());
        d.undo(&a);
        assert!(!d.is_dirty(), "undo back to the loaded state is clean");
        d.redo(&a);
        assert!(d.is_dirty());
        d.mark_saved();
        paint(&mut d, 2);
        d.undo(&a);
        assert!(!d.is_dirty(), "undo back to the saved state is clean");
        d.undo(&a);
        assert!(d.is_dirty(), "undo past the saved state is a change");
        d.redo(&a);
        assert!(!d.is_dirty());
        // A new edit after undoing past the save drops the saved state from the redo list.
        d.undo(&a);
        paint(&mut d, 3);
        d.undo(&a);
        assert!(d.is_dirty());
        // Inspector edits merge, but never into the saved entry.
        let mut d = MapDoc::new(Map::new("t", 8, 8), &a);
        let uid = d.map.alloc_uid();
        let before = Objects::of(&d.map);
        d.map.entities.push(EntityInstance::new(uid, EntityKind::Prop, "chest", 8.0, 15.0));
        d.record_objects(before, "inspector");
        d.mark_saved();
        let before = Objects::of(&d.map);
        d.map.entities[0].name = "x".into();
        d.record_objects(before, "inspector");
        assert!(d.is_dirty());
        d.undo(&a);
        assert!(!d.is_dirty() && d.map.entities.len() == 1);
        // Changes outside the history stay dirty until saved.
        d.mark_changed();
        assert!(d.is_dirty());
        d.clear_history();
        assert!(d.is_dirty());
        d.mark_saved();
        d.clear_history();
        assert!(!d.is_dirty());
    }

    #[test]
    fn whole_map_edits() {
        let a = assets();
        let mut d = MapDoc::new(Map::new("t", 8, 8), &a);
        let before = d.map.clone();
        d.map.resize(30, 4);
        d.record_map(before, &a);
        assert_eq!(d.overview.image.width, 30);
        d.undo(&a);
        assert_eq!((d.map.width, d.overview.image.width), (8, 8));
    }
}
