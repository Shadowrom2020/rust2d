//! Structures (multi-tile assets placed with one click in the editor): a basic set of houses,
//! walls, fences, rooms and outdoor features. Each one is painted on a small scratch map and
//! captured with `StructureDef::capture`, exactly as the editor's "New from map area" does.
//! Terrain-based ones go into the example project; city / dungeon / prop-based ones are
//! library-only extras of the starter pack.

use rustpg2d_engine::defs::StructureDef;
use rustpg2d_engine::entity::{EntityInstance, EntityKind};
use rustpg2d_engine::map::{CollisionOverride, Map, Tile};
use rustpg2d_engine::project::TILE_SIZE;

use crate::art::t;

/// Scratch map for painting one structure.
struct P {
    m: Map,
}

impl P {
    fn new(w: u32, h: u32) -> Self {
        P { m: Map::new("scratch", w, h) }
    }
    fn set(&mut self, layer: usize, ts: &str, x: i32, y: i32, tile: u32) {
        if self.m.in_bounds(x, y) {
            let s = self.m.tileset_slot(ts);
            self.m.layers[layer].set(x as u32, y as u32, Tile::new(s, tile));
        }
    }
    #[allow(clippy::too_many_arguments)] // layer, tileset, rectangle and tile read best inline
    fn fill(&mut self, layer: usize, ts: &str, x: i32, y: i32, w: i32, h: i32, tile: u32) {
        for yy in y..y + h {
            for xx in x..x + w {
                self.set(layer, ts, xx, yy, tile);
            }
        }
    }
    /// A walkable tile (door gap) in an otherwise solid row.
    fn walkable(&mut self, x: i32, y: i32) {
        self.m.set_collision(x as u32, y as u32, CollisionOverride::Walkable);
    }
    /// An entity standing on tile `(tx, ty)`; props `w` tiles wide are centered like the editor
    /// snaps them (odd widths on the tile center, even widths on its left edge).
    fn ent(&mut self, kind: EntityKind, def: &str, tx: i32, ty: i32, w: i32) -> &mut EntityInstance {
        let ts = TILE_SIZE as f32;
        let uid = self.m.alloc_uid();
        let x = if w % 2 == 0 { tx as f32 * ts } else { (tx as f32 + 0.5) * ts };
        self.m.entities.push(EntityInstance::new(uid, kind, def, x, (ty as f32 + 1.0) * ts - 1.0));
        self.m.entities.last_mut().unwrap()
    }
    fn prop(&mut self, def: &str, tx: i32, ty: i32, w: i32) {
        self.ent(EntityKind::Prop, def, tx, ty, w);
    }
    fn done(self, id: &str, name: &str, category: &str, ground: bool) -> StructureDef {
        let (w, h) = (self.m.width, self.m.height);
        let mut d = StructureDef::capture(&self.m, id, name, (0, 0, w, h), ground);
        d.category = category.into();
        d
    }
}

const TR: &str = "terrain";

/// Cottage like the village houses: roof, eaves, wall with windows and a walkable door.
fn cottage(w: i32) -> P {
    let mut p = P::new(w as u32, 5);
    p.fill(1, TR, 0, 0, w, 2, t::ROOF);
    p.fill(1, TR, 0, 2, w, 1, t::ROOF_EDGE);
    p.fill(1, TR, 0, 3, w, 2, t::HOUSE_WALL);
    p.set(1, TR, 1, 3, t::WINDOW);
    p.set(1, TR, w - 2, 3, t::WINDOW);
    p.set(1, TR, w / 2, 4, t::DOOR);
    p.walkable(w / 2, 4);
    p
}

/// Interior room: wall with windows at the top, black void around, floor and a door gap at the
/// bottom.
fn room(w: i32, h: i32, floor: u32, wall: u32) -> P {
    let mut p = P::new(w as u32, h as u32);
    p.fill(0, TR, 0, 0, w, h, t::VOID);
    p.fill(0, TR, 1, 2, w - 2, h - 3, floor);
    p.fill(1, TR, 1, 0, w - 2, 2, wall);
    p.fill(1, TR, 0, 0, 1, h, t::VOID);
    p.fill(1, TR, w - 1, 0, 1, h, t::VOID);
    p.fill(1, TR, 0, h - 1, w, 1, t::VOID);
    // Door at the bottom center.
    p.set(0, TR, w / 2, h - 1, floor);
    p.set(1, TR, w / 2, h - 1, t::DOOR);
    p.walkable(w / 2, h - 1);
    p
}

/// Terrain-based structures (for the example project; they also end up in the starter pack).
pub fn example_structures() -> Vec<StructureDef> {
    let mut out = Vec::new();

    // --- Buildings
    out.push(cottage(5).done("cottage_small", "Small cottage", "Buildings", false));
    out.push(cottage(7).done("cottage", "Cottage", "Buildings", false));
    let mut p = P::new(7, 6);
    for (x, tile) in [(0, t::ROOF), (1, t::ROOF)] {
        p.fill(1, TR, 0, x, 7, 1, tile);
    }
    p.fill(1, TR, 0, 2, 7, 1, t::ROOF_EDGE);
    p.fill(1, TR, 0, 3, 7, 2, t::HOUSE_WALL);
    p.set(1, TR, 1, 3, t::WINDOW);
    p.set(1, TR, 5, 3, t::WINDOW);
    p.set(1, TR, 3, 4, t::DOOR);
    p.walkable(3, 4);
    p.fill(0, TR, 0, 5, 7, 1, t::PATH);
    p.prop("sign", 2, 5, 1);
    p.prop("barrel", 5, 5, 1);
    out.push(p.done("shop_front", "Shop front (sign, barrel)", "Buildings", true));

    // --- Walls & fences
    let mut p = P::new(5, 1);
    p.fill(1, TR, 0, 0, 5, 1, t::STONE_WALL);
    out.push(p.done("stone_wall_h", "Stone wall, horizontal (5)", "Walls & fences", false));
    let mut p = P::new(1, 5);
    p.fill(1, TR, 0, 0, 1, 5, t::STONE_WALL);
    out.push(p.done("stone_wall_v", "Stone wall, vertical (5)", "Walls & fences", false));
    let mut p = P::new(7, 6);
    p.fill(1, TR, 0, 0, 7, 1, t::STONE_WALL);
    p.fill(1, TR, 0, 0, 1, 6, t::STONE_WALL);
    p.fill(1, TR, 6, 0, 1, 6, t::STONE_WALL);
    p.fill(1, TR, 0, 5, 3, 1, t::STONE_WALL);
    p.fill(1, TR, 4, 5, 3, 1, t::STONE_WALL);
    out.push(p.done("stone_yard", "Walled yard (open south)", "Walls & fences", false));
    let mut p = P::new(5, 1);
    p.fill(1, TR, 0, 0, 5, 1, t::FENCE);
    out.push(p.done("fence_h", "Fence, horizontal (5)", "Walls & fences", false));
    let mut p = P::new(1, 5);
    p.fill(1, TR, 0, 0, 1, 5, t::FENCE);
    out.push(p.done("fence_v", "Fence, vertical (5)", "Walls & fences", false));
    let mut p = P::new(6, 5);
    p.fill(0, TR, 1, 1, 4, 3, t::FLOWERS);
    p.fill(1, TR, 0, 0, 6, 1, t::FENCE);
    p.fill(1, TR, 0, 0, 1, 5, t::FENCE);
    p.fill(1, TR, 5, 0, 1, 5, t::FENCE);
    p.fill(1, TR, 0, 4, 2, 1, t::FENCE);
    p.fill(1, TR, 4, 4, 2, 1, t::FENCE);
    out.push(p.done("garden", "Fenced flower garden", "Walls & fences", true));

    // --- Rooms (for interior maps)
    let mut p = room(9, 8, t::WOOD_FLOOR, t::HOUSE_WALL);
    p.set(1, TR, 2, 1, t::WINDOW);
    p.set(1, TR, 6, 1, t::WINDOW);
    p.set(1, TR, 1, 2, t::BED);
    p.set(1, TR, 1, 3, t::BED_FOOT);
    p.fill(1, TR, 4, 2, 2, 1, t::SHELF);
    p.set(1, TR, 7, 4, t::TABLE);
    p.fill(0, TR, 3, 4, 3, 2, t::RUG);
    out.push(p.done("room_bedroom", "Bedroom (bed, shelf, table)", "Rooms", true));
    let mut p = room(7, 6, t::STONE_FLOOR, t::STONE_WALL);
    p.prop("barrel", 1, 2, 1);
    p.prop("barrel", 1, 3, 1);
    p.prop("chest", 5, 2, 1);
    out.push(p.done("room_storage", "Storage room (barrels, chest)", "Rooms", true));
    let mut p = room(6, 5, t::WOOD_FLOOR, t::HOUSE_WALL);
    p.set(1, TR, 2, 1, t::WINDOW);
    out.push(p.done("room_small", "Small empty room", "Rooms", true));

    // --- Outdoor features
    let mut p = P::new(7, 5);
    p.fill(0, TR, 0, 0, 7, 5, t::SAND);
    p.fill(0, TR, 1, 1, 5, 3, t::WATER);
    out.push(p.done("pond", "Pond with sandy shore", "Outdoor", true));
    let mut p = P::new(5, 1);
    p.fill(0, TR, 0, 0, 5, 1, t::BRIDGE);
    out.push(p.done("bridge_h", "Wooden bridge (5)", "Outdoor", true));
    let mut p = P::new(5, 5);
    p.fill(0, TR, 0, 0, 5, 5, t::PATH);
    p.prop("well", 2, 2, 1);
    p.prop("barrel", 4, 1, 1);
    out.push(p.done("well_square", "Village well square", "Outdoor", true));
    let mut p = P::new(5, 5);
    for (x, y) in [(1, 1), (3, 2), (0, 3), (2, 4), (4, 4)] {
        p.set(1, TR, x, y, t::TRUNK);
        p.set(2, TR, x, y - 1, t::CANOPY);
    }
    out.push(p.done("tree_grove", "Grove of trees", "Outdoor", false));
    let mut p = P::new(4, 5);
    for (x, y) in [(0, 1), (2, 2), (1, 4), (3, 4)] {
        p.set(1, TR, x, y, t::PINE_TRUNK);
        p.set(2, TR, x, y - 1, t::PINE_TOP);
    }
    out.push(p.done("pine_grove", "Grove of pines", "Outdoor", false));
    let mut p = P::new(3, 2);
    p.set(1, TR, 0, 0, t::ROCK);
    p.set(1, TR, 1, 0, t::BUSH);
    p.set(1, TR, 2, 1, t::ROCK);
    p.set(1, TR, 0, 1, t::MUSHROOM);
    out.push(p.done("rocks_bushes", "Rocks and bushes", "Outdoor", false));
    out
}

const CITY: &str = "city";
const DG: &str = "dungeon";

/// City house: `roof`/`edge` shingle tiles, `wall` tiles, door in the bottom row.
fn town_house(w: i32, roof: u32, edge: u32, wall: u32) -> P {
    let mut p = P::new(w as u32, 6);
    p.fill(1, CITY, 0, 0, w, 2, roof);
    p.fill(1, CITY, 0, 2, w, 1, edge);
    p.fill(1, CITY, 0, 3, w, 3, wall);
    p.set(1, CITY, w / 2, 5, 12);
    p.walkable(w / 2, 5);
    p
}

/// Dungeon room: ceiling (void) around, a wall row at the top, flagstone floor, door below.
fn dungeon_room(w: i32, h: i32) -> P {
    let mut p = P::new(w as u32, h as u32);
    p.fill(0, DG, 1, 2, w - 2, h - 3, 0);
    p.fill(1, DG, 0, 0, w, 1, 16);
    p.fill(1, DG, 1, 1, w - 2, 1, 8);
    p.fill(1, DG, 0, 0, 1, h, 16);
    p.fill(1, DG, w - 1, 0, 1, h, 16);
    p.fill(1, DG, 0, h - 1, w, 1, 16);
    p.set(0, DG, w / 2, h - 1, 0);
    p.set(1, DG, w / 2, h - 1, 6);
    p.walkable(w / 2, h - 1);
    p
}

/// Library-only structures (city and dungeon tilesets, props of the starter library).
pub fn library_structures() -> Vec<StructureDef> {
    let mut out = Vec::new();

    // --- City buildings
    let mut p = town_house(5, 16, 17, 9);
    p.set(1, CITY, 1, 3, 14);
    p.set(1, CITY, 3, 3, 14);
    p.set(1, CITY, 1, 5, 15);
    p.set(1, CITY, 3, 4, 13);
    out.push(p.done("townhouse_blue", "Town house (blue roof)", "Buildings", false));
    let mut p = town_house(7, 18, 19, 8);
    p.set(1, CITY, 1, 3, 14);
    p.set(1, CITY, 5, 3, 14);
    p.fill(1, CITY, 1, 4, 2, 1, 11);
    p.fill(1, CITY, 4, 4, 2, 1, 11);
    out.push(p.done("brick_shop", "Brick shop (orange roof)", "Buildings", false));
    let mut p = P::new(4, 6);
    p.fill(1, CITY, 0, 0, 4, 6, 10);
    p.fill(1, CITY, 1, 1, 2, 1, 31);
    p.set(1, CITY, 1, 5, 12);
    p.walkable(1, 5);
    out.push(p.done("watchtower", "Stone watchtower", "Buildings", false));

    // --- City walls & fences
    let mut p = P::new(6, 2);
    p.fill(1, CITY, 0, 0, 6, 2, 10);
    out.push(p.done("city_wall_h", "City wall, horizontal (6)", "Walls & fences", false));
    let mut p = P::new(2, 6);
    p.fill(1, CITY, 0, 0, 2, 6, 10);
    out.push(p.done("city_wall_v", "City wall, vertical (6)", "Walls & fences", false));
    let mut p = P::new(5, 1);
    p.fill(1, CITY, 0, 0, 5, 1, 21);
    out.push(p.done("iron_fence", "Iron fence (5)", "Walls & fences", false));
    let mut p = P::new(5, 1);
    p.fill(1, CITY, 0, 0, 5, 1, 22);
    out.push(p.done("hedge", "Hedge (5)", "Walls & fences", false));

    // --- City outdoor
    let mut p = P::new(7, 7);
    p.fill(0, CITY, 0, 0, 7, 7, 0);
    p.prop("fountain", 3, 3, 2);
    p.prop("bench", 1, 5, 2);
    p.prop("bench", 5, 5, 2);
    p.prop("street_lamp", 0, 1, 1);
    p.prop("street_lamp", 6, 1, 1);
    out.push(p.done("plaza", "Plaza with fountain", "Outdoor", true));
    let mut p = P::new(7, 4);
    p.fill(0, CITY, 0, 0, 7, 4, 0);
    p.prop("market_stall", 1, 2, 3);
    p.prop("market_stall", 5, 2, 3);
    p.prop("crate", 3, 3, 1);
    p.prop("sack", 3, 2, 1);
    out.push(p.done("market", "Market stalls", "Outdoor", true));
    let mut p = P::new(5, 4);
    p.fill(0, CITY, 0, 0, 5, 1, 0);
    p.fill(0, CITY, 0, 1, 5, 2, 23);
    p.fill(0, CITY, 0, 3, 5, 1, 0);
    out.push(p.done("canal", "Canal section", "Outdoor", true));
    let mut p = P::new(5, 4);
    p.prop("tent", 1, 1, 2);
    p.prop("campfire", 3, 2, 1);
    p.prop("bench", 3, 3, 2);
    p.prop("sack", 0, 3, 1);
    out.push(p.done("campsite", "Campsite (tent, campfire)", "Outdoor", false));

    // --- Dungeon
    let mut p = dungeon_room(7, 6);
    p.set(1, DG, 2, 1, 10);
    p.set(1, DG, 4, 1, 10);
    p.set(0, DG, 1, 4, 20);
    out.push(p.done("dungeon_room", "Dungeon room (torches)", "Dungeon", true));
    let mut p = dungeon_room(7, 7);
    p.set(1, DG, 3, 1, 22);
    p.set(1, DG, 1, 2, 15);
    p.set(1, DG, 5, 2, 15);
    p.fill(0, DG, 3, 2, 1, 4, 3);
    p.set(2, DG, 1, 2, 18);
    out.push(p.done("dungeon_shrine", "Shrine room (altar, pillars)", "Dungeon", true));
    let mut p = P::new(6, 3);
    p.fill(1, DG, 0, 0, 6, 1, 8);
    p.fill(0, DG, 0, 1, 6, 1, 0);
    p.fill(1, DG, 0, 2, 6, 1, 9);
    out.push(p.done("corridor_h", "Corridor, horizontal (6)", "Dungeon", true));
    let mut p = P::new(3, 6);
    p.fill(1, DG, 0, 0, 1, 6, 16);
    p.fill(0, DG, 1, 0, 1, 6, 0);
    p.fill(1, DG, 2, 0, 1, 6, 16);
    out.push(p.done("corridor_v", "Corridor, vertical (6)", "Dungeon", true));
    let mut p = P::new(4, 4);
    p.fill(1, DG, 0, 0, 4, 1, 8);
    p.set(1, DG, 1, 0, 11);
    p.fill(0, DG, 0, 1, 4, 2, 0);
    p.set(0, DG, 1, 2, 17);
    p.set(0, DG, 2, 1, 21);
    p.fill(1, DG, 0, 3, 4, 1, 14);
    out.push(p.done("prison_cell", "Prison cell (bars)", "Dungeon", true));
    out
}
