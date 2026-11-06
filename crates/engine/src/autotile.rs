//! Auto-tiling terrains: paint "grass / water" or "dirt path" and the editor picks the edge
//! tiles. A terrain lives in a tileset (`TilesetDef::terrains`) and has one tile per
//! combination of its four neighbours (north, east, south, west) being the same terrain: 16
//! tiles. Which cells belong to a terrain is read from the tiles themselves (a cell whose tile
//! is one of the 16), so maps store nothing new and the game needs nothing to show them.
//! Terrains can lie on each other: a terrain *joins* others whose cells count as its own for
//! its edges (sand joins the shore water lying on it, so sand shows no grass edge there).

use serde::{Deserialize, Serialize};

use crate::map::{Map, Tile};

/// Neighbour bits of a terrain mask: that neighbour is the same terrain.
pub const NORTH: usize = 1;
pub const EAST: usize = 2;
pub const SOUTH: usize = 4;
pub const WEST: usize = 8;

/// An auto-tiling terrain of a tileset.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Terrain {
    pub name: String,
    /// Tile index (in the terrain's tileset) per neighbour mask (`NORTH | EAST | …`):
    /// `tiles[15]` is the inside, `tiles[0]` a cell on its own. `None` = not set (another
    /// tile of the terrain is used instead).
    pub tiles: [Option<u32>; 16],
    /// What erasing the terrain leaves on the cell (e.g. plain grass); `None` = no tile.
    #[serde(default)]
    pub base: Option<u32>,
    /// Names of other terrains of the tileset that lie on this one: their cells count as this
    /// terrain when its edges are chosen (sand joins the shore water inside it).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub joins: Vec<String>,
}

impl Terrain {
    /// Whether a tile of the tileset is one of the terrain's tiles.
    pub fn contains(&self, tile: u32) -> bool {
        self.tiles.contains(&Some(tile))
    }

    /// The tile for a neighbour mask; missing ones fall back to the inside, then to any tile.
    pub fn tile_for(&self, mask: usize) -> Option<u32> {
        self.tiles[mask & 15].or(self.tiles[15]).or_else(|| self.tiles.iter().flatten().next().copied())
    }

    /// Number of masks that have their own tile.
    pub fn tiles_set(&self) -> usize {
        self.tiles.iter().flatten().count()
    }

    /// Tile index of mask `k` in a 4×4 block laid out in mask order (row by row) whose
    /// top-left tile is `first`, in a tileset `columns` tiles wide.
    pub fn block_tile(first: u32, columns: u32, k: usize) -> u32 {
        first + (k as u32 / 4) * columns + k as u32 % 4
    }

    /// Sets all 16 tiles from such a block.
    pub fn fill_from_block(&mut self, first: u32, columns: u32) {
        for k in 0..16 {
            self.tiles[k] = Some(Self::block_tile(first, columns, k));
        }
    }
}

/// Neighbour mask of cell `(x, y)`: which of its four neighbours are `member`s.
pub fn mask(x: i32, y: i32, member: impl Fn(i32, i32) -> bool) -> usize {
    let mut m = 0;
    for (bit, dx, dy) in [(NORTH, 0, -1), (EAST, 1, 0), (SOUTH, 0, 1), (WEST, -1, 0)] {
        if member(x + dx, y + dy) {
            m |= bit;
        }
    }
    m
}

/// Which of a tileset's terrains a tile belongs to.
pub fn terrain_of(terrains: &[Terrain], tile: u32) -> Option<usize> {
    terrains.iter().position(|t| t.contains(tile))
}

/// Whether a tile counts as terrain `k` for its edges: one of its tiles, or of a terrain it
/// joins.
pub fn counts_as(terrains: &[Terrain], k: usize, tile: u32) -> bool {
    let Some(t) = terrains.get(k) else { return false };
    t.contains(tile) || terrains.iter().any(|o| t.joins.contains(&o.name) && o.contains(tile))
}

/// The tile changes for painting (`erase` false) or erasing one cell of terrain `k` (of the
/// tileset's `terrains`) on `layer`: the cell gets terrain `k`'s tile for its neighbours, and
/// its four neighbours that belong to a terrain of the tileset are re-tiled (each with its own
/// terrain). `slot` is the map's tileset slot of that tileset. Cells outside the map count as
/// every terrain (terrains continue past the edge). Erasing changes only a cell of terrain `k`:
/// it gets the terrain's base tile (re-tiled if that is a tile of another terrain). Returns `(x, y, tile)` for every cell whose tile changes.
pub fn paint(map: &Map, layer: usize, slot: u16, terrains: &[Terrain], k: usize, (x, y): (i32, i32), erase: bool) -> Vec<(u32, u32, Tile)> {
    let (Some(l), Some(terrain)) = (map.layers.get(layer), terrains.get(k)) else { return Vec::new() };
    if !map.in_bounds(x, y) {
        return Vec::new();
    }
    let of_slot = |cx: i32, cy: i32| l.get(cx as u32, cy as u32).parts().filter(|&(s, _)| s == slot as usize).map(|(_, i)| i);
    if erase && !of_slot(x, y).is_some_and(|i| terrain.contains(i)) {
        return Vec::new();
    }
    let Some(placeholder) = terrain.tile_for(15) else { return Vec::new() };
    // The tile index each cell will have (the painted cell with any tile of its terrain).
    let virt = |cx: i32, cy: i32| if (cx, cy) == (x, y) { if erase { terrain.base } else { Some(placeholder) } } else { of_slot(cx, cy) };
    let mut out = Vec::new();
    for (cx, cy) in [(x, y), (x, y - 1), (x + 1, y), (x, y + 1), (x - 1, y)] {
        if !map.in_bounds(cx, cy) {
            continue;
        }
        // An erased cell gets the base tile, re-tiled when the base belongs to a terrain.
        let plain_base = erase && (cx, cy) == (x, y) && !terrain.base.is_some_and(|b| terrain_of(terrains, b).is_some());
        let new = if plain_base {
            terrain.base.map_or(Tile::EMPTY, |b| Tile::new(slot, b))
        } else {
            // The painted cell is terrain `k`; a neighbour whose tile several terrains share is
            // taken as `k` too when it is one of them (a tile should belong to one terrain).
            let kk = if (cx, cy) == (x, y) && !erase {
                k
            } else {
                let Some(i) = virt(cx, cy) else { continue };
                if terrain.contains(i) {
                    k
                } else {
                    let Some(o) = terrain_of(terrains, i) else { continue };
                    o
                }
            };
            let member = |nx: i32, ny: i32| !map.in_bounds(nx, ny) || virt(nx, ny).is_some_and(|i| counts_as(terrains, kk, i));
            match terrains[kk].tile_for(mask(cx, cy, member)) {
                Some(t) => Tile::new(slot, t),
                None => continue,
            }
        };
        if l.get(cx as u32, cy as u32) != new {
            out.push((cx as u32, cy as u32, new));
        }
    }
    out
}

/// Paints a terrain cell straight into a map (tests and tools; the editor goes through its
/// undo history instead).
pub fn paint_into(map: &mut Map, layer: usize, slot: u16, terrains: &[Terrain], k: usize, cell: (i32, i32), erase: bool) {
    for (cx, cy, t) in paint(map, layer, slot, terrains, k, cell, erase) {
        map.layers[layer].set(cx, cy, t);
    }
}

/// Re-tiles a whole layer at once (generators): every cell `pick` assigns a terrain gets that
/// terrain's tile for its neighbours, where `member(k, x, y)` says whether a cell counts as
/// terrain `k`. Membership is decided on the layer as it was before (no order effects); cells
/// outside the map count as every terrain.
pub fn retile(map: &mut Map, layer: usize, slot: u16, terrains: &[Terrain], pick: impl Fn(&Map, i32, i32) -> Option<usize>, member: impl Fn(&Map, usize, i32, i32) -> bool) {
    let (w, h) = (map.width as i32, map.height as i32);
    let mut changes = Vec::new();
    for y in 0..h {
        for x in 0..w {
            let Some(k) = pick(map, x, y) else { continue };
            let m = mask(x, y, |nx, ny| !map.in_bounds(nx, ny) || member(map, k, nx, ny));
            if let Some(t) = terrains.get(k).and_then(|t| t.tile_for(m)) {
                changes.push((x as u32, y as u32, Tile::new(slot, t)));
            }
        }
    }
    for (x, y, t) in changes {
        map.layers[layer].set(x, y, t);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A terrain whose tile for mask `m` is `100 + m`, erasing to tile 7.
    fn terrain() -> Terrain {
        let mut t = Terrain { name: "pond".into(), base: Some(7), ..Default::default() };
        t.fill_from_block(100, 4);
        t
    }

    fn idx(map: &Map, x: u32, y: u32) -> Option<u32> {
        map.layers[0].get(x, y).parts().map(|p| p.1)
    }

    #[test]
    fn block_layout_and_fallbacks() {
        assert_eq!(Terrain::block_tile(40, 8, 0), 40);
        assert_eq!(Terrain::block_tile(40, 8, 5), 49);
        assert_eq!(Terrain::block_tile(40, 8, 15), 67);
        let mut t = terrain();
        assert_eq!((t.tile_for(NORTH | SOUTH), t.tiles_set()), (Some(105), 16));
        t.tiles[5] = None;
        assert_eq!(t.tile_for(5), Some(115));
        assert!(t.contains(100) && !t.contains(7));
        assert_eq!(Terrain::default().tile_for(3), None);
    }

    #[test]
    fn painting_picks_edges_and_updates_neighbours() {
        let mut map = Map::new("m", 6, 6);
        let slot = map.tileset_slot("terrain");
        let ts = [terrain()];
        // A single cell has no terrain neighbours.
        paint_into(&mut map, 0, slot, &ts, 0, (2, 2), false);
        assert_eq!(idx(&map, 2, 2), Some(100));
        // A second cell east of it: both get their joined edge.
        paint_into(&mut map, 0, slot, &ts, 0, (3, 2), false);
        assert_eq!((idx(&map, 2, 2), idx(&map, 3, 2)), (Some(100 + EAST as u32), Some(100 + WEST as u32)));
        // At the map edge the terrain continues outside.
        paint_into(&mut map, 0, slot, &ts, 0, (0, 0), false);
        assert_eq!(idx(&map, 0, 0), Some(100 + (NORTH | WEST) as u32));
        // Erasing leaves the base tile and fixes the neighbour; erasing other tiles does nothing.
        paint_into(&mut map, 0, slot, &ts, 0, (3, 2), true);
        assert_eq!((idx(&map, 2, 2), idx(&map, 3, 2)), (Some(100), Some(7)));
        assert!(paint(&map, 0, slot, &ts, 0, (3, 2), true).is_empty());
        assert!(paint(&map, 0, slot, &ts, 0, (9, 9), false).is_empty());
        // Tiles of another tileset slot are not the terrain.
        let other = map.tileset_slot("other");
        map.layers[0].set(2, 1, Tile::new(other, 100 + 15));
        paint_into(&mut map, 0, slot, &ts, 0, (2, 3), false);
        assert_eq!(idx(&map, 2, 2), Some(100 + SOUTH as u32));
    }

    #[test]
    fn joined_terrains_lie_on_each_other() {
        // Sand (tiles 100..) joins Shore (tiles 200..): sand next to shore water has no edge
        // there, and painting shore re-tiles the sand around it.
        let mut sand = Terrain { name: "Sand".into(), base: Some(7), joins: vec!["Shore".into()], ..Default::default() };
        sand.fill_from_block(100, 4);
        let mut shore = Terrain { name: "Shore".into(), base: Some(115), ..Default::default() };
        shore.fill_from_block(200, 4);
        let ts = [sand, shore];
        let mut map = Map::new("m", 7, 3);
        let slot = map.tileset_slot("t");
        for x in 1..6 {
            paint_into(&mut map, 0, slot, &ts, 0, (x, 1), false);
        }
        assert_eq!(idx(&map, 3, 1), Some(100 + (EAST | WEST) as u32));
        assert!(counts_as(&ts, 0, 203) && !counts_as(&ts, 1, 103));
        paint_into(&mut map, 0, slot, &ts, 1, (3, 1), false);
        // The shore cell has sand on both sides (not shore): its own single tile.
        assert_eq!(idx(&map, 3, 1), Some(200));
        // The sand still joins up with it.
        assert_eq!(idx(&map, 2, 1), Some(100 + (EAST | WEST) as u32));
        assert_eq!(terrain_of(&ts, 200), Some(1));
        // Erasing the shore leaves its base, a sand tile, re-tiled as sand.
        paint_into(&mut map, 0, slot, &ts, 1, (3, 1), true);
        assert_eq!(idx(&map, 3, 1), Some(100 + (EAST | WEST) as u32));
        // Whole-layer retiling with a custom membership.
        let mut m2 = Map::new("m2", 3, 1);
        retile(&mut m2, 0, slot, &ts, |_, x, _| (x == 1).then_some(0), |_, _, x, _| x == 1);
        assert_eq!(idx(&m2, 1, 0), Some(100 + (NORTH | SOUTH) as u32));
    }

    #[test]
    fn old_tileset_files_load_without_terrains() {
        let d: crate::defs::TilesetDef = ron::from_str(r#"(id: "t", image: "t.png", solid: [1], animations: [])"#).unwrap();
        assert!(d.terrains.is_empty());
        let mut d2 = d.clone();
        d2.terrains.push(terrain());
        let back: crate::defs::TilesetDef = ron::from_str(&ron::to_string(&d2).unwrap()).unwrap();
        assert_eq!(back.terrains, d2.terrains);
        assert!(!ron::to_string(&d).unwrap().contains("terrains"));
    }
}
