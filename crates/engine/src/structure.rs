//! Structures: multi-tile assets (houses, wall segments, rooms…) that the editor places on a
//! map in one click, and captures from a map area. See `defs::StructureDef`.

use crate::defs::StructureDef;
use crate::entity::EntityInstance;
use crate::map::{CollisionOverride, Map, Tile};
use crate::project::TILE_SIZE;

impl StructureDef {
    /// Layer tile value: empty the map's tile here (0 keeps it).
    pub const CLEAR: u32 = 1;
    /// Collision value: reset the map's override to "use the tileset" (0 keeps it).
    pub const COLLISION_AUTO: u8 = 3;

    /// Layer tile at a cell (0 = keep, `CLEAR`, or an encoded tile).
    pub fn tile(&self, layer: usize, x: u32, y: u32) -> u32 {
        self.layers.get(layer).and_then(|l| l.get((y * self.w + x) as usize)).copied().unwrap_or(0)
    }

    /// `(tileset id, tile index)` of an encoded layer value (`None` for keep / clear).
    pub fn tile_source(&self, v: u32) -> Option<(&str, u32)> {
        if v == Self::CLEAR {
            return None;
        }
        let (slot, index) = Tile(v).parts()?;
        Some((self.tilesets.get(slot)?.as_str(), index))
    }

    /// Slot of a tileset in this structure (added if it isn't used yet).
    pub fn tileset_slot(&mut self, id: &str) -> u16 {
        match self.tilesets.iter().position(|t| t == id) {
            Some(i) => i as u16,
            None => {
                self.tilesets.push(id.to_string());
                (self.tilesets.len() - 1) as u16
            }
        }
    }

    /// Sets a layer cell (0 = keep, `CLEAR`, or an encoded tile); adds layers as needed.
    pub fn set_tile(&mut self, layer: usize, x: u32, y: u32, v: u32) {
        if x >= self.w || y >= self.h {
            return;
        }
        let n = (self.w * self.h) as usize;
        while self.layers.len() <= layer {
            self.layers.push(vec![0; n]);
        }
        self.layers[layer].resize(n, 0);
        self.layers[layer][(y * self.w + x) as usize] = v;
    }

    /// Collision override at a cell (0 = keep).
    pub fn collision_at(&self, x: u32, y: u32) -> u8 {
        self.collision.get((y * self.w + x) as usize).copied().unwrap_or(0)
    }

    pub fn set_collision(&mut self, x: u32, y: u32, c: u8) {
        if x >= self.w || y >= self.h {
            return;
        }
        self.collision.resize((self.w * self.h) as usize, 0);
        self.collision[(y * self.w + x) as usize] = c;
    }

    /// Changes the size, keeping the content at the top-left (entities outside are dropped).
    pub fn resize(&mut self, w: u32, h: u32) {
        let (w, h) = (w.clamp(1, 256), h.clamp(1, 256));
        let (ow, oh) = (self.w, self.h);
        let remap = |old: &Vec<u8>| -> Vec<u8> {
            let mut out = vec![0; (w * h) as usize];
            for y in 0..h.min(oh) {
                for x in 0..w.min(ow) {
                    out[(y * w + x) as usize] = old.get((y * ow + x) as usize).copied().unwrap_or(0);
                }
            }
            out
        };
        for l in &mut self.layers {
            let mut out = vec![0; (w * h) as usize];
            for y in 0..h.min(oh) {
                for x in 0..w.min(ow) {
                    out[(y * w + x) as usize] = l.get((y * ow + x) as usize).copied().unwrap_or(0);
                }
            }
            *l = out;
        }
        if !self.collision.is_empty() {
            self.collision = remap(&self.collision);
        }
        let ts = TILE_SIZE as f32;
        self.entities.retain(|e| e.x < w as f32 * ts && e.y < h as f32 * ts);
        self.w = w;
        self.h = h;
    }

    /// Tidies up after editing: drops tilesets no tile uses (renumbering the tiles), trailing
    /// layers without anything to do and an all-"keep" collision grid.
    pub fn prune(&mut self) {
        let mut used = vec![false; self.tilesets.len()];
        for v in self.layers.iter().flatten().filter(|&&v| v != Self::CLEAR) {
            if let Some((slot, _)) = Tile(*v).parts() {
                if let Some(u) = used.get_mut(slot) {
                    *u = true;
                }
            }
        }
        if used.iter().any(|u| !u) {
            let mut new_slot = Vec::with_capacity(used.len());
            let mut next = 0u16;
            for &u in &used {
                new_slot.push(next);
                if u {
                    next += 1;
                }
            }
            for v in self.layers.iter_mut().flatten() {
                if *v == Self::CLEAR {
                    continue;
                }
                if let Some((slot, index)) = Tile(*v).parts() {
                    *v = Tile::new(new_slot[slot], index).0;
                }
            }
            let mut i = 0;
            self.tilesets.retain(|_| {
                i += 1;
                used[i - 1]
            });
        }
        while self.layers.last().is_some_and(|l| l.iter().all(|&v| v == 0)) {
            self.layers.pop();
        }
        if self.collision.iter().all(|&c| c == 0) {
            self.collision.clear();
        }
    }

    /// Captures a rectangle of `map` (all layers, collision overrides and the entities whose
    /// feet are inside). Cells with any tile belong to the structure: there, empty upper
    /// layers clear the map when placed; cells without tiles keep whatever the map has.
    /// `ground`: include the ground layer (off for things that sit on any ground).
    pub fn capture(map: &Map, id: &str, name: &str, rect: (u32, u32, u32, u32), ground: bool) -> StructureDef {
        let (x0, y0, w, h) = rect;
        let mut d = StructureDef { id: id.into(), name: name.into(), w, h, ..Default::default() };
        let first = if ground { 0 } else { 1 };
        let mut slots: Vec<Option<u16>> = vec![None; map.tilesets.len()];
        let mut layers = vec![vec![0u32; (w * h) as usize]; map.layers.len()];
        let mut used = vec![false; (w * h) as usize];
        for (li, layer) in map.layers.iter().enumerate().skip(first) {
            for y in 0..h {
                for x in 0..w {
                    let (mx, my) = (x0 + x, y0 + y);
                    if !map.in_bounds(mx as i32, my as i32) {
                        continue;
                    }
                    let Some((slot, index)) = layer.get(mx, my).parts() else { continue };
                    let Some(ts) = map.tilesets.get(slot) else { continue };
                    let s = *slots[slot].get_or_insert_with(|| {
                        d.tilesets.push(ts.clone());
                        (d.tilesets.len() - 1) as u16
                    });
                    layers[li][(y * w + x) as usize] = Tile::new(s, index).0;
                    used[(y * w + x) as usize] = true;
                }
            }
        }
        // Inside the footprint, empty upper layers clear the map (e.g. trees inside a room).
        for (li, layer) in layers.iter_mut().enumerate().skip(1) {
            for (i, v) in layer.iter_mut().enumerate() {
                if used[i] && *v == 0 && li >= first {
                    *v = Self::CLEAR;
                }
            }
        }
        // Drop trailing layers that neither set nor clear anything.
        while layers.len() > 1 && layers.last().is_some_and(|l| l.iter().all(|&v| v == 0)) {
            layers.pop();
        }
        d.layers = layers;
        let mut col = vec![0u8; (w * h) as usize];
        for y in 0..h {
            for x in 0..w {
                let (mx, my) = (x0 + x, y0 + y);
                if !map.in_bounds(mx as i32, my as i32) || !used[(y * w + x) as usize] {
                    continue;
                }
                col[(y * w + x) as usize] = match map.collision_at(mx, my) {
                    CollisionOverride::Auto => Self::COLLISION_AUTO,
                    c => c as u8,
                };
            }
        }
        if col.iter().any(|&c| c != 0 && c != Self::COLLISION_AUTO) {
            d.collision = col;
        }
        let ts = TILE_SIZE as f32;
        let (left, top) = (x0 as f32 * ts, y0 as f32 * ts);
        let (right, bottom) = (left + w as f32 * ts, top + h as f32 * ts);
        for e in &map.entities {
            if e.x >= left && e.x < right && e.y >= top && e.y < bottom {
                let mut c = e.clone();
                c.uid = 0;
                c.x -= left;
                c.y -= top;
                d.entities.push(c);
            }
        }
        d
    }

    /// Places the structure with its top-left corner at tile `(x, y)`: sets tiles (registering
    /// the tilesets on the map), collision overrides and adds the entities with new uids.
    /// Cells outside the map are skipped. Returns what changed (for undo).
    pub fn place(&self, map: &mut Map, x: i32, y: i32) -> Placement {
        let mut out = Placement { tiles: vec![Vec::new(); map.layers.len()], ..Default::default() };
        let slots: Vec<u16> = self.tilesets.iter().map(|t| map.tileset_slot(t)).collect();
        for (li, layer) in self.layers.iter().enumerate().take(map.layers.len()) {
            for (i, &v) in layer.iter().enumerate() {
                if v == 0 {
                    continue;
                }
                let (mx, my) = (x + (i as u32 % self.w) as i32, y + (i as u32 / self.w) as i32);
                if !map.in_bounds(mx, my) {
                    continue;
                }
                let new = if v == Self::CLEAR {
                    Tile::EMPTY
                } else {
                    match Tile(v).parts() {
                        Some((slot, index)) if slot < slots.len() => Tile::new(slots[slot], index),
                        _ => continue,
                    }
                };
                let old = map.layers[li].set(mx as u32, my as u32, new);
                if old != new {
                    out.tiles[li].push((mx as u32, my as u32, old, new));
                }
            }
        }
        for (i, &c) in self.collision.iter().enumerate() {
            let (mx, my) = (x + (i as u32 % self.w) as i32, y + (i as u32 / self.w) as i32);
            if c == 0 || !map.in_bounds(mx, my) {
                continue;
            }
            let new = match c {
                1 => CollisionOverride::Solid,
                2 => CollisionOverride::Walkable,
                _ => CollisionOverride::Auto,
            };
            let old = map.set_collision(mx as u32, my as u32, new);
            if old != new {
                out.collision.push((mx as u32, my as u32, old as u8, new as u8));
            }
        }
        let ts = TILE_SIZE as f32;
        for e in &self.entities {
            let mut n: EntityInstance = e.clone();
            n.uid = map.alloc_uid();
            n.x += x as f32 * ts;
            n.y += y as f32 * ts;
            if map.in_bounds((n.x / ts) as i32, ((n.y - 1.0) / ts) as i32) {
                out.entities.push(n.uid);
                map.entities.push(n);
            }
        }
        out
    }
}

/// What placing a structure changed: per layer `(x, y, old, new)` tiles, collision bytes and
/// the uids of the added entities.
#[derive(Clone, Debug, Default)]
pub struct Placement {
    pub tiles: Vec<Vec<(u32, u32, Tile, Tile)>>,
    pub collision: Vec<(u32, u32, u8, u8)>,
    pub entities: Vec<u32>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entity::EntityKind;

    #[test]
    fn capture_and_place_roundtrip() {
        let mut src = Map::new("src", 10, 10);
        let (stone, grass) = (src.tileset_slot("city"), src.tileset_slot("terrain"));
        // A 3x2 "hut": walls on the detail layer, a door gap, floor on the ground layer.
        for x in 2..5 {
            src.layers[0].set(x, 3, Tile::new(grass, 6));
            src.layers[0].set(x, 4, Tile::new(grass, 6));
            src.layers[1].set(x, 3, Tile::new(stone, 8));
        }
        src.layers[1].set(3, 4, Tile::new(stone, 12));
        src.set_collision(3, 4, CollisionOverride::Walkable);
        let uid = src.alloc_uid();
        src.entities.push(EntityInstance::new(uid, EntityKind::Prop, "chest", 4.5 * 32.0, 5.0 * 32.0 - 1.0));
        let d = StructureDef::capture(&src, "hut", "Hut", (2, 3, 3, 2), true);
        assert_eq!((d.w, d.h), (3, 2));
        assert_eq!(d.tilesets, vec!["terrain".to_string(), "city".to_string()]);
        assert_eq!(d.tile_source(d.tile(1, 1, 1)), Some(("city", 12)));
        // Empty detail cells inside the footprint clear the map; the door is walkable.
        assert_eq!(d.tile(1, 0, 1), StructureDef::CLEAR);
        assert_eq!(d.collision[4], CollisionOverride::Walkable as u8);
        assert_eq!(d.entities.len(), 1);
        assert_eq!((d.entities[0].x, d.entities[0].y), (2.5 * 32.0, 2.0 * 32.0 - 1.0));

        // Place it on a map that uses the tilesets in another order, over a tree.
        let mut m = Map::new("dst", 8, 8);
        let other = m.tileset_slot("forest");
        m.layers[1].set(5, 6, Tile::new(other, 3));
        m.next_uid = 7;
        let p = d.place(&mut m, 4, 5);
        let city = m.tilesets.iter().position(|t| t == "city").unwrap() as u16;
        assert_eq!(m.layers[1].get(5, 6), Tile::new(city, 12));
        assert_eq!(m.layers[1].get(4, 6), Tile::EMPTY);
        assert_eq!(m.collision_at(5, 6), CollisionOverride::Walkable);
        assert_eq!(p.entities, vec![7]);
        assert_eq!((m.entities[0].x, m.entities[0].y), (6.5 * 32.0, 7.0 * 32.0 - 1.0));
        assert!(p.tiles[1].iter().any(|&(x, y, old, _)| (x, y) == (5, 6) && old == Tile::new(other, 3)));
        // Partly outside the map: only the cells inside are set, no panic.
        let p = d.place(&mut m, 6, 7);
        assert!(p.tiles[0].len() == 2 && p.entities.is_empty());
    }

    #[test]
    fn editing_helpers() {
        let mut d = StructureDef { id: "x".into(), w: 2, h: 2, ..Default::default() };
        let a = d.tileset_slot("a");
        let b = d.tileset_slot("b");
        d.set_tile(1, 1, 1, Tile::new(b, 7).0);
        d.set_tile(0, 0, 0, Tile::new(a, 3).0);
        d.set_tile(2, 0, 1, StructureDef::CLEAR);
        d.set_collision(1, 0, 2);
        assert_eq!(d.layers.len(), 3);
        d.resize(3, 1);
        assert_eq!((d.w, d.layers[0].len(), d.collision_at(1, 0)), (3, 3, 2));
        // The only "b" tile was cut off: "b" goes, layers that do nothing go.
        d.prune();
        assert_eq!(d.tilesets, vec!["a".to_string()]);
        assert_eq!(d.tile_source(d.tile(0, 0, 0)), Some(("a", 3)));
        assert_eq!(d.layers.len(), 1);
        // Removing "a" while "b" stays renumbers b's tiles.
        let mut d = StructureDef { id: "y".into(), w: 1, h: 1, ..Default::default() };
        let _ = d.tileset_slot("a");
        let b = d.tileset_slot("b");
        d.set_tile(0, 0, 0, Tile::new(b, 5).0);
        d.set_tile(1, 0, 0, StructureDef::CLEAR);
        d.prune();
        assert_eq!(d.tile(1, 0, 0), StructureDef::CLEAR);
        assert_eq!((d.tilesets.len(), d.tile_source(d.tile(0, 0, 0))), (1, Some(("b", 5))));
    }

    #[test]
    fn capture_without_ground_keeps_the_map_ground() {
        let mut src = Map::new("src", 4, 4);
        let s = src.tileset_slot("terrain");
        src.layers[0].set(1, 1, Tile::new(s, 0));
        src.layers[1].set(1, 1, Tile::new(s, 12));
        let d = StructureDef::capture(&src, "tree", "Tree", (1, 1, 1, 1), false);
        assert_eq!(d.tile(0, 0, 0), 0);
        assert_eq!(d.tile_source(d.tile(1, 0, 0)), Some(("terrain", 12)));
    }
}
