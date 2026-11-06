//! Generates `example_project/`: procedural pixel art, definitions, maps and scripts.
//!
//! Run: `cargo run -p rustpg2d-engine --example gen_example [OUT_DIR]`

use std::path::{Path, PathBuf};

use rustpg2d_engine::defs::*;
use rustpg2d_engine::effects::{EffectKind, EffectSpec};
use rustpg2d_engine::achievements::AchievementDef;
use rustpg2d_engine::crafting::RecipeDef;
use rustpg2d_engine::entity::{EntityInstance, EntityKind, Facing, ScheduleStop};
use rustpg2d_engine::events::*;
use rustpg2d_engine::map::{EdgeLink, LightMode, Map, MapLighting, Respawn, Tile, Zone, ZoneKind};
use rustpg2d_engine::audio::wav_bytes;
use rustpg2d_engine::inventory::ItemStack;
use rustpg2d_engine::project::{GameSfx, Project, ProjectConfig};
use rustpg2d_engine::quest::{QuestCheck, QuestDef, QuestOp};
use rustpg2d_engine::skills::{SkillDef, SpellDef};

mod art;
mod music;
mod structures;
use art::*;

// ------------------------------------------------------------------ sounds

const RATE: u32 = 22050;

#[derive(Clone, Copy)]
enum Wave {
    Square,
    Triangle,
    Noise,
}

/// A note sweeping from `f0` to `f1` Hz with a short attack and exponential decay.
fn tone(f0: f32, f1: f32, dur: f32, wave: Wave, vol: f32, decay: f32) -> Vec<f32> {
    let n = (dur * RATE as f32) as usize;
    let mut phase = 0.0f32;
    let mut noise = Rng(5);
    let mut held = 0.0f32;
    (0..n)
        .map(|i| {
            let t = i as f32 / RATE as f32;
            let f = f0 + (f1 - f0) * (t / dur);
            phase = (phase + f / RATE as f32).fract();
            let v = match wave {
                Wave::Square => if phase < 0.5 { 1.0 } else { -1.0 },
                Wave::Triangle => 4.0 * (phase - 0.5).abs() - 1.0,
                Wave::Noise => {
                    // Sample-and-hold noise; pitch controls the hold rate.
                    if phase < f / RATE as f32 {
                        held = noise.range(2000) as f32 / 1000.0 - 1.0;
                    }
                    held
                }
            };
            let env = (t / 0.005).min(1.0) * (-t * decay).exp() * (1.0 - t / dur).clamp(0.0, 1.0).powf(0.3);
            v * env * vol
        })
        .collect()
}

fn concat(parts: &[Vec<f32>]) -> Vec<f32> {
    parts.concat()
}

fn mix(a: &mut [f32], b: &[f32], at: usize) {
    for (i, v) in b.iter().enumerate() {
        if let Some(s) = a.get_mut(at + i) {
            *s += v;
        }
    }
}

fn midi(n: i32) -> f32 {
    440.0 * 2f32.powf((n - 69) as f32 / 12.0)
}

type SoundFn = fn() -> Vec<f32>;

const SOUNDS: [(&str, SoundFn); 12] = [
    // A bright, bell-like arpeggio for magic (the forest shrine).
    ("chime", || {
        let mut out = vec![0.0; (1.6 * RATE as f32) as usize];
        for (k, n) in [76, 80, 83, 88, 92].iter().enumerate() {
            let f = midi(*n);
            mix(&mut out, &tone(f, f, 1.2, Wave::Triangle, 0.22, 2.2), k * (0.09 * RATE as f32) as usize);
            mix(&mut out, &tone(f * 2.0, f * 2.0, 0.6, Wave::Triangle, 0.06, 4.0), k * (0.09 * RATE as f32) as usize);
        }
        out
    }),
    ("unlock", || concat(&[tone(1800.0, 1800.0, 0.03, Wave::Square, 0.25, 40.0), tone(900.0, 700.0, 0.06, Wave::Square, 0.25, 25.0), tone(2400.0, 2400.0, 0.04, Wave::Square, 0.2, 40.0)])),
    ("creak", || concat(&[tone(90.0, 160.0, 0.18, Wave::Square, 0.18, 4.0), tone(160.0, 120.0, 0.12, Wave::Triangle, 0.4, 6.0)])),
    ("pickup", || concat(&[tone(880.0, 880.0, 0.06, Wave::Square, 0.3, 10.0), tone(1320.0, 1320.0, 0.12, Wave::Square, 0.3, 12.0)])),
    ("swing", || tone(2400.0, 600.0, 0.12, Wave::Noise, 0.35, 15.0)),
    ("hit", || concat(&[tone(220.0, 90.0, 0.08, Wave::Square, 0.45, 20.0), tone(3000.0, 800.0, 0.05, Wave::Noise, 0.25, 30.0)])),
    ("hurt", || tone(500.0, 120.0, 0.3, Wave::Square, 0.4, 6.0)),
    ("defeat", || concat(&[tone(400.0, 800.0, 0.08, Wave::Square, 0.3, 5.0), tone(4000.0, 300.0, 0.3, Wave::Noise, 0.35, 8.0)])),
    ("blip", || tone(1200.0, 1200.0, 0.03, Wave::Square, 0.15, 30.0)),
    // Fireball: a whoosh when it leaves the hand, a rumbling blast when it lands.
    ("fireball", || tone(500.0, 1600.0, 0.22, Wave::Noise, 0.35, 7.0)),
    ("explosion", || concat(&[tone(2000.0, 200.0, 0.12, Wave::Noise, 0.55, 6.0), tone(400.0, 40.0, 0.5, Wave::Noise, 0.6, 3.0)])),
    ("door", || concat(&[tone(150.0, 110.0, 0.12, Wave::Triangle, 0.6, 8.0), tone(200.0, 140.0, 0.15, Wave::Triangle, 0.5, 8.0)])),
];

// ------------------------------------------------------------------ maps

struct B {
    m: Map,
}

impl B {
    fn new(name: &str, w: u32, h: u32) -> Self {
        let mut m = Map::new(name, w, h);
        m.tileset_slot("terrain");
        B { m }
    }
    fn set(&mut self, layer: usize, x: i32, y: i32, tile: u32) {
        if self.m.in_bounds(x, y) {
            self.m.layers[layer].set(x as u32, y as u32, Tile::new(0, tile));
        }
    }
    fn fill(&mut self, layer: usize, x: i32, y: i32, w: i32, h: i32, tile: u32) {
        for yy in y..y + h {
            for xx in x..x + w {
                self.set(layer, xx, yy, tile);
            }
        }
    }
    fn get(&self, layer: usize, x: i32, y: i32) -> Option<u32> {
        if !self.m.in_bounds(x, y) {
            return None;
        }
        self.m.layers[layer].get(x as u32, y as u32).parts().map(|p| p.1)
    }
    fn tree(&mut self, x: i32, y: i32) {
        self.set(1, x, y, t::TRUNK);
        self.set(2, x, y - 1, t::CANOPY);
    }
    fn pine(&mut self, x: i32, y: i32) {
        self.set(1, x, y, t::PINE_TRUNK);
        self.set(2, x, y - 1, t::PINE_TOP);
    }
    fn free(&self, x: i32, y: i32) -> bool {
        self.get(1, x, y).is_none() && self.get(1, x, y + 1).is_none()
    }
    /// House with roof; returns the door tile.
    fn house(&mut self, x: i32, y: i32, w: i32) -> (i32, i32) {
        self.fill(1, x, y, w, 3, t::ROOF);
        self.fill(1, x, y + 2, w, 1, t::ROOF_EDGE);
        self.fill(1, x, y + 3, w, 2, t::HOUSE_WALL);
        self.set(1, x + 1, y + 3, t::WINDOW);
        self.set(1, x + w - 2, y + 3, t::WINDOW);
        let door = (x + w / 2, y + 4);
        self.set(1, door.0, door.1, t::DOOR);
        self.set(1, door.0, door.1 - 1, t::HOUSE_WALL);
        // The door tile is walkable so the player can step into the portal.
        self.m.set_collision(door.0 as u32, door.1 as u32, rustpg2d_engine::map::CollisionOverride::Walkable);
        door
    }
    fn zone(&mut self, name: &str, x: u32, y: u32, w: u32, h: u32, kind: ZoneKind) -> u32 {
        let uid = self.m.alloc_uid();
        self.m.zones.push(Zone { uid, name: name.into(), x, y, w, h, kind, triggers: vec![], loot: None });
        uid
    }
    fn spawn(&mut self, name: &str, x: u32, y: u32) {
        self.zone(name, x, y, 1, 1, ZoneKind::Spawn);
    }
    fn portal(&mut self, x: u32, y: u32, map: &str, spawn: &str) {
        self.zone(&format!("to_{map}"), x, y, 1, 1, ZoneKind::Portal { target_map: map.into(), target_spawn: spawn.into() });
    }
    /// Places a multi-tile prop whose bottom row is `ty`, horizontally centered on tile `tx`
    /// (odd widths) or on the left edge of tile `tx` (even widths); clears tiles under it.
    fn big(&mut self, def: &str, tx: i32, ty: i32, w: i32, h: i32) -> &mut EntityInstance {
        let x0 = tx - w / 2;
        for y in ty - h + 1..=ty {
            for x in x0..x0 + w {
                if self.m.in_bounds(x, y) {
                    self.m.layers[1].set(x as u32, y as u32, Tile::EMPTY);
                    self.m.layers[2].set(x as u32, y as u32, Tile::EMPTY);
                }
            }
        }
        // Trees above the footprint may have a crown hanging into it.
        if self.m.in_bounds(tx, ty - h) && self.get(1, tx, ty - h + 1).is_none() {
            self.m.layers[2].set(tx as u32, (ty - h) as u32, Tile::EMPTY);
        }
        let uid = self.m.alloc_uid();
        let x = if w.rem_euclid(2) == 0 { tx as f32 * TS as f32 } else { (tx as f32 + 0.5) * TS as f32 };
        let y = (ty as f32 + 1.0) * TS as f32 - 1.0;
        self.m.entities.push(EntityInstance::new(uid, EntityKind::Prop, def, x, y));
        self.m.entities.last_mut().unwrap()
    }

    fn entity(&mut self, kind: EntityKind, def: &str, tx: i32, ty: i32) -> &mut EntityInstance {
        let uid = self.m.alloc_uid();
        let (x, y) = ((tx as f32 + 0.5) * TS as f32, (ty as f32 + 1.0) * TS as f32 - 1.0);
        self.m.entities.push(EntityInstance::new(uid, kind, def, x, y));
        self.m.entities.last_mut().unwrap()
    }
}

/// An item that gives light (equipped or lying on the ground).
fn light(mut d: ItemDef, l: LightSpec) -> ItemDef {
    d.light = Some(l);
    d
}

/// A healing item the player can also use on a companion (they choose whom to heal).
fn for_allies(d: ItemDef) -> ItemDef {
    ItemDef { use_on: rustpg2d_engine::defs::UseTarget::Choose, ..d }
}

/// A scroll or book that teaches a spell when used.
fn teach(mut d: ItemDef, spell: &str) -> ItemDef {
    d.teaches = spell.into();
    d
}

/// The example's skills: combat, defense, lockpicking, haggling, arcana and a scripted one.
fn skills() -> Vec<SkillDef> {
    use rustpg2d_engine::skills::SkillEffect as E;
    let icon = |f: u32| SpriteRef::new("items", f);
    vec![
        SkillDef::new("swordsmanship", "Swordsmanship", "Training with blades and fists: every level adds 10% to your melee damage.", icon(it::SWORD), 5, E::MeleeDamage { percent: 10.0 }),
        SkillDef::new("archery", "Archery", "A steady hand with bow and crossbow: every level adds 10% to ranged weapon damage.", icon(it::BOW), 5, E::RangedDamage { percent: 10.0 }),
        SkillDef::new("toughness", "Toughness", "Take hits like a stone wall: every level adds 1 defense.", icon(it::CHAIN), 3, E::Defense { points: 1 }),
        SkillDef::new("lockpicking", "Lockpicking", "Opens locks without their key. Level 1 picks simple padlocks, level 2 old mine locks.", icon(it::LOCKPICK), 3, E::Lockpick),
        SkillDef::new("haggling", "Haggling", "Merchants give you better prices: 5% per level when buying and selling.", icon(it::COIN), 4, E::Haggle { percent: 5.0 }),
        SkillDef::new("arcana", "Arcana", "Understanding of magic. Each level adds 10 mana, and harder spells need it to be learned.", icon(it::STAR), 5, E::Mana { points: 10 }),
        SkillDef::new("spellcraft", "Spellcraft", "Focus and precision: every level adds 15% to the damage of Spark and Fire Burst.", icon(it::SPELL_SPARK), 4, E::SpellDamage { percent: 15.0 }),
        SkillDef::new("scavenging", "Scavenging", "An eye for what others leave behind: defeated slimes drop an extra coin per level (a scripted skill: see scavenging() in main.rhai).", icon(it::APPLE), 3, E::Script { function: "scavenging".into() }),
    ]
}

/// The example's achievements (Database › Achievements; unlocks are the player's, kept next to
/// the game's settings). Hidden Hoard has no conditions: the mine chest's Unlock trigger
/// unlocks it with the action.
fn achievements() -> Vec<AchievementDef> {
    let icon = |f: u32| SpriteRef::new("items", f);
    let defeated = |c: &str, n: i64| cond(Cond::Defeated { character: c.into(), op: CmpOp::Ge, count: n });
    vec![
        AchievementDef::new("first_blood", "First Blood", "Defeat your first slime.", icon(it::SWORD), vec![defeated("slime", 1)]),
        AchievementDef::new("pest_control", "Pest Control", "Rid Tilda's deliveries of the slimes.", icon(it::POTION), vec![quest_is("pest_control", QuestCheck::Completed, 0)]),
        AchievementDef::new("lights_in_the_dark", "Lights in the Dark", "Wake the old shrine in Whisperwood.", icon(it::SPELL_LIGHT), vec![cond(flag("shrine_lit"))]),
        AchievementDef::new("seasoned", "Seasoned", "Reach level 5.", icon(it::STAR), vec![cond(Cond::Level { op: CmpOp::Ge, value: 5 })]),
        AchievementDef::new(
            "naturalist",
            "Naturalist",
            "Face a slime, a goblin archer, an outlaw and the bandit chief.",
            icon(it::BOOK),
            vec![defeated("slime", 1), defeated("goblin", 1), defeated("outlaw", 1), defeated("bandit_chief", 1)],
        ),
        AchievementDef { hidden: true, ..AchievementDef::new("hidden_hoard", "Hidden Hoard", "Pick the lock of the old mine's treasure chest.", icon(it::LOCKPICK), vec![]) },
    ]
}

/// The example's crafting recipes, all brewed at the alchemy table ("alchemy" station).
fn recipes() -> Vec<RecipeDef> {
    let i = ItemStack::new;
    vec![
        RecipeDef::new("brew_potion", "potion", 1, vec![i("herb", 2)], "alchemy"),
        RecipeDef::new("brew_antidote", "antidote", 1, vec![i("herb", 1), i("mushroom", 1)], "alchemy"),
        RecipeDef::new("brew_mana", "mana_potion", 1, vec![i("mushroom", 2), i("apple", 1)], "alchemy"),
        // Harder brews need some knowledge of magic.
        RecipeDef { skill: "arcana".into(), level: 1, ..RecipeDef::new("brew_swiftness", "swiftness", 1, vec![i("herb", 2), i("mushroom", 2)], "alchemy") },
    ]
}

/// The example's spells, learned from scrolls and books.
fn spells() -> Vec<SpellDef> {
    use rustpg2d_engine::skills::SpellEffect as E;
    let icon = |f: u32| SpriteRef::new("items", f);
    vec![
        SpellDef::new("heal", "Heal", "Mends your wounds: restores 3 hearts.", icon(it::SPELL_HEAL), 10, 2.0, E::Heal { amount: 6 }),
        SpellDef::new("light", "Light", "A glow around you for a minute - handy in the old mine.", icon(it::SPELL_LIGHT), 6, 1.0, E::Light { radius: 5.0, seconds: 60.0 }),
        SpellDef::new("spark", "Spark", "Hurls a bolt of lightning in the direction you face.", icon(it::SPELL_SPARK), 5, 0.6, E::Bolt { damage: 3, projectile: icon(it::SPARK_SHOT), speed: 320.0 * R, range: 7.0 }).requires("arcana", 1),
        SpellDef::new("fire_burst", "Fire Burst", "Flames burst out around you, hitting everything nearby.", icon(it::SPELL_BLAST), 15, 4.0, E::Blast { damage: 4, radius: 2.0 }).requires("arcana", 2),
        // A scripted spell: cast_fireball() in main.rhai aims, throws and explodes it.
        SpellDef::new("fireball", "Fireball", "Click a target: a ball of fire flies there and explodes, burning everything within 2 tiles.", SpriteRef::new("fireball", 0), 12, 1.5, E::Script { function: "cast_fireball".into() }).requires("arcana", 3),
    ]
}

fn quest(id: &str, op: QuestOp, stage: u32) -> Action {
    Action::Quest { quest: id.into(), op, stage }
}
fn quest_is(id: &str, check: QuestCheck, stage: u32) -> Condition {
    cond(Cond::Quest { quest: id.into(), check, stage })
}

fn dialog(speaker: &str, text: &str) -> Action {
    Action::Dialog { speaker: speaker.into(), text: text.into() }
}
fn cond(c: Cond) -> Condition {
    Condition::new(c)
}
fn not(c: Cond) -> Condition {
    Condition { cond: c, negate: true }
}
fn flag(n: &str) -> Cond {
    Cond::Flag { name: n.into() }
}
fn trig(on: TriggerOn, conditions: Vec<Condition>, actions: Vec<Action>) -> Trigger {
    Trigger { on, conditions, actions, once: false }
}

/// Index of the terrain tileset's tile on `layer` at a cell, if any.
fn tile_at(m: &Map, layer: usize, x: i32, y: i32) -> Option<u32> {
    if !m.in_bounds(x, y) {
        return None;
    }
    let (slot, i) = m.layers.get(layer)?.get(x as u32, y as u32).parts()?;
    (m.tilesets.get(slot)? == "terrain").then_some(i)
}

/// Ground tile (layer 0) of the terrain tileset at a cell.
fn ground(m: &Map, x: i32, y: i32) -> Option<u32> {
    tile_at(m, 0, x, y)
}

/// Re-tiles a map's ground with the terrain tileset's auto-tiling terrains, as painting them
/// with the editor's Terrain tool would: `class` says which terrain a cell is (by its plain
/// tile and place); a cell counts as a terrain for its edges when it is of that terrain or of
/// one it joins, or when `extra` says so (doors, bridges, caves the paths lead to).
fn autotile_terrain(m: &mut Map, class: impl Fn(&Map, i32, i32) -> Option<&'static str>, extra: impl Fn(&Map, &str, i32, i32) -> bool) {
    let terrains = terrains();
    let slot = m.tileset_slot("terrain");
    rustpg2d_engine::autotile::retile(
        m,
        0,
        slot,
        &terrains,
        |m, x, y| class(m, x, y).map(terrain_index),
        |m, k, x, y| {
            let t = &terrains[k];
            class(m, x, y).is_some_and(|c| c == t.name || t.joins.iter().any(|j| j == c)) || extra(m, &t.name, x, y)
        },
    );
}

fn village() -> Map {
    let (w, h) = (48, 36);
    let mut b = B::new("village", w, h);
    let mut rng = Rng(42);
    b.fill(0, 0, 0, w as i32, h as i32, t::GRASS);
    for _ in 0..160 {
        let (x, y) = (rng.range(w) as i32, rng.range(h) as i32);
        b.set(0, x, y, if rng.chance(0.5) { t::FLOWERS } else { t::TALL_GRASS });
    }
    // Main road from the west edge (where travellers arrive) to the east edge (y = 18), and
    // the north-south street.
    b.fill(0, 0, 17, w as i32, 3, t::PATH);
    b.fill(0, 21, 8, 3, 12, t::PATH);
    // Pond with sand rim and bridge-less shore.
    b.fill(0, 5, 24, 11, 7, t::SAND);
    b.fill(0, 6, 25, 9, 5, t::WATER);
    // Houses.
    let door = b.house(17, 3, 7);
    b.fill(0, door.0, door.1 + 1, 1, 4, t::PATH);
    // Second house: Tilda's shop.
    let door2 = b.house(30, 6, 6);
    b.fill(0, door2.0, door2.1 + 1, 1, 6, t::PATH);
    // Fences around a garden.
    b.fill(1, 30, 24, 10, 1, t::FENCE);
    // Gap in the top fence for a double gate (placed after the random trees below).
    b.m.layers[1].set(34, 24, Tile::EMPTY);
    b.m.layers[1].set(35, 24, Tile::EMPTY);
    b.fill(1, 30, 31, 10, 1, t::FENCE);
    b.fill(1, 30, 24, 1, 8, t::FENCE);
    b.fill(1, 39, 24, 1, 5, t::FENCE);
    for y in 26..30 {
        for x in (32..38).step_by(2) {
            b.set(1, x, y, t::BUSH);
        }
    }
    // Tree border, leaving the road open to the west and the east.
    for x in 0..w as i32 {
        b.tree(x, 1);
        b.tree(x, h as i32 - 1);
    }
    for y in 2..h as i32 - 1 {
        if !(16..=20).contains(&y) {
            b.tree(0, y);
            b.tree(w as i32 - 1, y);
        }
    }
    for _ in 0..40 {
        let (x, y) = (rng.range(w - 4) as i32 + 2, rng.range(h - 6) as i32 + 3);
        let on_grass = matches!(b.get(0, x, y), Some(t::GRASS | t::FLOWERS | t::TALL_GRASS));
        if on_grass && b.free(x, y) && b.free(x, y - 1) && !(15..=21).contains(&y) {
            b.tree(x, y);
        }
    }
    b.set(1, 12, 10, t::ROCK);
    b.set(1, 27, 12, t::ROCK);
    // Multi-tile props: garden double gate (2x2), big oaks (3x4), a statue (1x2).
    b.big("double_gate", 35, 24, 2, 2);
    b.big("big_oak", 6, 9, 3, 4);
    b.big("big_oak", 41, 14, 3, 4);
    b.big("big_pine", 9, 31, 2, 4);
    b.big("statue", 15, 13, 1, 2);

    b.spawn("start", 22, 14);
    b.spawn("house_door", door.0 as u32, door.1 as u32 + 1);
    b.portal(door.0 as u32, door.1 as u32, "house", "entrance");

    // Gate blocking the road east, opened with the key.
    // East exit: the gap in the border trees (rows 16-20) is closed by a wall with a
    // vertical double gate across the three road tiles (rows 17-19).
    let gate = b.big("vgate", 46, 19, 1, 4);
    gate.name = "east_gate".into();
    // The lock (Gate Key, kept, gate opens, relockable) comes from the gate definition;
    // these triggers only react to it being opened / locked again.
    gate.triggers = vec![
        trig(
            TriggerOn::Unlock,
            vec![],
            vec![Action::SetFlag { name: "gate_open".into(), value: true }, dialog("", "The gate creaks open. The forest lies beyond.")],
        ),
        // Quest "The Strange Lights": the gate is open, now find the shrine.
        trig(TriggerOn::Unlock, vec![quest_is("strange_lights", QuestCheck::AtStage, 0)], vec![quest("strange_lights", QuestOp::SetStage, 1)]),
        trig(TriggerOn::Lock, vec![], vec![Action::SetFlag { name: "gate_open".into(), value: false }]),
    ];
    for y in [14, 15, 16, 20, 21, 22] {
        b.set(1, 46, y, t::STONE_WALL);
        b.m.layers[2].set(46, (y - 1) as u32, Tile::EMPTY); // no tree crown floating over the wall
    }
    let sign = b.entity(EntityKind::Prop, "sign", 19, 16);
    sign.triggers = vec![trig(TriggerOn::Interact, vec![], vec![dialog("Sign", "Emberbrook. East: Whisperwood. Please keep the gate locked!")])];
    b.entity(EntityKind::Prop, "well", 25, 13);
    let barrel = b.entity(EntityKind::Prop, "barrel", 16, 7);
    barrel.container = Some(ContainerSpec { size: 4, items: vec![ItemStack::new("apple", 3), ItemStack::new("arrows", 10), ItemStack::new("leather_cap", 1)], ..Default::default() });
    let barrel = b.entity(EntityKind::Prop, "barrel", 24, 7);
    barrel.container = Some(ContainerSpec { size: 4, items: vec![ItemStack::new("apple", 2), ItemStack::new("bronze_key", 1)], ..Default::default() });

    let elder = b.entity(EntityKind::Character, "elder", 26, 15);
    elder.name = "elder".into();
    elder.triggers = vec![
        trig(
            TriggerOn::Interact,
            vec![not(flag("quest_accepted"))],
            vec![
                dialog("Elder Rowan", "Ah, a traveller! Welcome to Emberbrook, {player}. Strange lights have been seen in Whisperwood, east of here."),
                Action::Choice {
                    speaker: "Elder Rowan".into(),
                    text: "Would you investigate the old shrine for us?".into(),
                    options: vec!["Of course!".into(), "Not right now.".into()],
                    var: "elder_answer".into(),
                },
                Action::If {
                    conditions: vec![cond(Cond::Var { name: "elder_answer".into(), op: CmpOp::Eq, value: 0 })],
                    then: vec![
                        Action::SetFlag { name: "quest_accepted".into(), value: true },
                        quest("strange_lights", QuestOp::Start, 0),
                        dialog("Elder Rowan", "Wonderful! The gate key is in the chest in my house, just north of here."),
                    ],
                    otherwise: vec![dialog("Elder Rowan", "I understand. Come back if you change your mind.")],
                },
            ],
        ),
        trig(
            TriggerOn::Interact,
            vec![cond(flag("quest_accepted")), not(flag("shrine_lit"))],
            vec![dialog("Elder Rowan", "The shrine lies deep in Whisperwood. Take the key from my chest and open the east gate.")],
        ),
        trig(
            TriggerOn::Interact,
            vec![cond(flag("shrine_lit"))],
            vec![
                dialog("Elder Rowan", "The lights have calmed! You have my thanks, brave {lad|lass}."),
                Action::If {
                    conditions: vec![not(flag("rewarded"))],
                    then: vec![
                        Action::SetFlag { name: "rewarded".into(), value: true },
                        quest("strange_lights", QuestOp::Complete, 0),
                        Action::GiveItem { item: "coin".into(), count: 25 },
                        dialog("", "Received 25 coins!"),
                        Action::GiveSkillPoints { amount: 2 },
                    ],
                    otherwise: vec![],
                },
            ],
        ),
    ];
    let v = b.entity(EntityKind::Character, "villager", 12, 19);
    v.name = "mira".into();
    v.triggers = vec![trig(TriggerOn::Interact, vec![], vec![Action::RunScript { function: "mira_talk".into() }])];
    let v2 = b.entity(EntityKind::Character, "villager", 34, 14);
    // Tomas has a daily schedule (map inspector: Daily schedule): the shop in the morning, the
    // duck pond at noon, the big pond in the afternoon, home (inside the elder's house, whose
    // door is at 20,7) at night. What he says depends on the time (Time of day condition).
    v2.name = "tomas".into();
    v2.schedule = vec![
        ScheduleStop::new(7.0, 34, 14),
        ScheduleStop { facing: Some(Facing::Down), ..ScheduleStop::new(12.0, 22, 23) },
        ScheduleStop { facing: Some(Facing::Down), ..ScheduleStop::new(16.0, 9, 23) },
        ScheduleStop { facing: Some(Facing::Up), inside: true, ..ScheduleStop::new(20.5, 20, 7) },
    ];
    let daytime = |from: f32, to: f32| cond(Cond::TimeBetween { from, to });
    v2.triggers = vec![
        trig(TriggerOn::Interact, vec![daytime(6.0, 18.0)], vec![dialog("Tomas", "Lovely weather today, isn't it, {sir|miss}?")]),
        trig(TriggerOn::Interact, vec![daytime(18.0, 6.0)], vec![dialog("Tomas", "Getting late, {sir|miss}. I'll be heading home soon - the elder keeps the door open for me.")]),
    ];
    b.entity(EntityKind::Item, "apple", 8, 12);
    b.entity(EntityKind::Item, "apple", 37, 21);


    // Tilda's shop: door portal + a sign in front (added last so older uids stay the same).
    b.spawn("shop_door", door2.0 as u32, door2.1 as u32 + 1);
    b.portal(door2.0 as u32, door2.1 as u32, "shop", "entrance");
    for (x, y) in [(door2.0 - 1, door2.1 + 1), (door2.0 - 1, door2.1 + 2)] {
        b.m.layers[1].set(x as u32, y as u32, Tile::EMPTY);
        b.m.layers[2].set(x as u32, y as u32, Tile::EMPTY);
    }
    let shop_sign = b.entity(EntityKind::Prop, "sign", door2.0 - 1, door2.1 + 1);
    shop_sign.triggers = vec![trig(TriggerOn::Interact, vec![], vec![dialog("Sign", "TILDA'S WARES - Potions, arrows, armor. Fair prices, we buy too!")])];

    // A town guard patrolling the main road (back and forth, pausing at each point).
    let guard = b.entity(EntityKind::Character, "guard", 8, 18);
    guard.name = "guard".into();
    // The guard carries a lantern: at night it lights the road as he walks.
    guard.light = Some(LightSpec::new(4.5, [255, 220, 170], false));
    guard.patrol = Some(rustpg2d_engine::entity::PatrolRoute {
        points: vec![(8, 18), (20, 18), (30, 18), (38, 18)],
        mode: rustpg2d_engine::entity::PatrolMode::PingPong,
        pause: 1.5,
        speed: 0.0,
    });
    guard.triggers = vec![trig(TriggerOn::Interact, vec![], vec![Action::RunScript { function: "guard_talk".into() }])];

    // A duck pond on the green with a footpath from the road (auto-tiled below).
    // (Trees there go: trunks on the detail layer, crowns one row higher on the overhead one.)
    for y in 23..30u32 {
        for x in 18..27u32 {
            b.m.layers[1].set(x, y + 1, Tile::EMPTY);
            b.m.layers[2].set(x, y, Tile::EMPTY);
        }
    }
    b.fill(0, 22, 20, 1, 5, t::PATH);
    for (y, row) in ["...###..", "..#####.", ".######.", "..###..."].iter().enumerate() {
        for (x, c) in row.bytes().enumerate() {
            if c == b'#' {
                b.set(0, 18 + x as i32, 25 + y as i32, t::WATER);
            }
        }
    }

    // The big pond is a fishing spot (the zone covers its water; fish from the shore with the
    // Fishing Rod from Tilda): mostly trout, now and then an old boot (chances are weights).
    let spot = b.zone("pond_fishing", 6, 25, 9, 5, ZoneKind::FishingSpot { rod: "fishing_rod".into() });
    if let Some(z) = b.m.zones.iter_mut().find(|z| z.uid == spot) {
        z.loot = Some(vec![LootEntry::new("trout", 75.0, 1, 1), LootEntry::new("old_boot", 25.0, 1, 1)]);
    }
    // Benches by the road to sit on (furniture).
    b.entity(EntityKind::Prop, "bench", 12, 16);
    b.entity(EntityKind::Prop, "bench", 31, 16);

    b.m.edges.east = Some(EdgeLink { target_map: "forest".into(), offset: 6, depth: 1 });
    b.m.triggers = vec![
        // A new game's intro: a cut-scene (the script's intro_cutscene()), then the welcome.
        Trigger { once: true, ..trig(TriggerOn::MapEnter, vec![], vec![Action::Cutscene { function: "intro_cutscene".into() }]) },
        Trigger {
        on: TriggerOn::MapEnter,
        conditions: vec![],
        actions: vec![dialog(
            "",
            "Welcome to the R(ust)PG2D demo! Move: arrows/WASD or left-click. Interact: E/Space or right-click. Attack: X or left-click an enemy. Switch weapon: Q. Character & inventory: C or I, skills: K, spells: L. Quest journal: J. Map: M. Save/Load: F5/F9. Debug: F3.",
        )],
        once: true,
    }];
    b.m.background = [20, 40, 20];
    b.m.music = "music_village".into();
    b.m.lighting = MapLighting { mode: LightMode::Outdoor, darkness: 0.7 };
    // Auto-tiling (Database › Tilesets › Terrains): soft edges for the roads, the sandy pond
    // and the duck pond. Paths run up to the doors.
    autotile_terrain(
        &mut b.m,
        |m, x, y| match ground(m, x, y)? {
            t::PATH => Some("Dirt path"),
            t::SAND => Some("Sand"),
            t::WATER if (18..26).contains(&x) && (25..29).contains(&y) => Some("Pond"),
            t::WATER => Some("Shore"),
            _ => None,
        },
        |m, terrain, x, y| terrain == "Dirt path" && tile_at(m, 1, x, y) == Some(t::DOOR),
    );
    b.m
}

fn shop() -> Map {
    let (w, h) = (12, 10);
    let mut b = B::new("shop", w, h);
    b.fill(0, 0, 0, w as i32, h as i32, t::VOID);
    b.fill(0, 1, 2, 10, 7, t::WOOD_FLOOR);
    b.fill(1, 1, 0, 10, 2, t::HOUSE_WALL);
    b.set(1, 2, 1, t::WINDOW);
    b.set(1, 9, 1, t::WINDOW);
    b.fill(1, 0, 0, 1, h as i32, t::VOID);
    b.fill(1, 11, 0, 1, h as i32, t::VOID);
    b.fill(1, 0, 9, w as i32, 1, t::VOID);
    // Shelves along the back wall, Tilda behind the counter, a rug for customers.
    b.fill(1, 3, 2, 6, 1, t::SHELF);
    // The counter spans the room: customers stay in front and talk to Tilda across it.
    b.fill(1, 1, 4, 10, 1, t::TABLE);
    b.fill(0, 4, 6, 5, 2, t::RUG);
    b.set(0, 6, 9, t::WOOD_FLOOR);
    b.set(1, 6, 9, t::DOOR);
    b.m.set_collision(6, 9, rustpg2d_engine::map::CollisionOverride::Walkable);
    b.spawn("entrance", 6, 8);
    b.portal(6, 9, "village", "shop_door");
    // Tilda behind her counter, with her own stock and prices.
    let merchant = b.entity(EntityKind::Character, "merchant_f", 6, 3);
    merchant.name = "tilda".into();
    merchant.facing = rustpg2d_engine::entity::Facing::Down;
    merchant.shop = Some(ShopSpec {
        title: "Tilda's Wares".into(),
        currency: "coin".into(),
        items: vec![
            ShopEntry { unlimited: true, ..ShopEntry::new("apple", 0, 4) },
            ShopEntry { unlimited: true, ..ShopEntry::new("herb", 0, 3) },
            ShopEntry::new("fishing_rod", 2, 20),
            ShopEntry::new("potion", 5, 0),
            ShopEntry { unlimited: true, ..ShopEntry::new("arrows", 0, 2) },
            ShopEntry::new("bolts", 20, 0),
            ShopEntry::new("leather_cap", 1, 0),
            ShopEntry::new("spear", 1, 0),
            ShopEntry::new("iron_helmet", 1, 0),
            ShopEntry::new("chainmail", 1, 0),
            ShopEntry::new("lantern", 1, 0),
            ShopEntry { unlimited: true, ..ShopEntry::new("torch", 0, 0) },
            ShopEntry::new("wooden_shield", 1, 0),
            ShopEntry::new("scroll_heal", 2, 0),
            ShopEntry::new("bandage", 6, 0),
            ShopEntry::new("antidote", 5, 0),
            ShopEntry::new("elixir_strength", 2, 0),
            ShopEntry::new("swiftness", 2, 0),
            ShopEntry::new("scroll_light", 1, 0),
            ShopEntry::new("tunic_green", 1, 0),
            ShopEntry::new("tunic_red", 1, 0),
            ShopEntry::new("tome_fireball", 1, 0),
            ShopEntry { unlimited: true, ..ShopEntry::new("mana_potion", 0, 0) },
        ],
        sell_rate: 50,
        open_on_talk: true,
    });
    // Talked to across the counter: a greeting the first time, her quest, then (after these
    // triggers) her shop opens because the merchant spec has "talking opens the shop".
    merchant.triggers = vec![
        trig(
            TriggerOn::Interact,
            vec![not(flag("met_tilda"))],
            vec![
                Action::SetFlag { name: "met_tilda".into(), value: true },
                dialog("Tilda", "Welcome, welcome, {good sir|my lady}! Potions, arrows, armor - and I pay fair coin for whatever you find out there."),
            ],
        ),
        // Quest "Pest Control": offered until accepted (not any more once the shrine has driven
        // the monsters out of Whisperwood), rewarded once the slimes are gone.
        trig(
            TriggerOn::Interact,
            vec![quest_is("pest_control", QuestCheck::NotStarted, 0), not(flag("shrine_lit"))],
            vec![
                Action::Choice {
                    speaker: "Tilda".into(),
                    text: "Slimes from Whisperwood keep eating my deliveries! Could you get rid of five of them? 30 coins and a potion are yours.".into(),
                    options: vec!["I'll do it.".into(), "Not right now.".into()],
                    var: "tilda_answer".into(),
                },
                Action::If {
                    conditions: vec![cond(Cond::Var { name: "tilda_answer".into(), op: CmpOp::Eq, value: 0 })],
                    then: vec![quest("pest_control", QuestOp::Start, 0), dialog("Tilda", "Wonderful! Come back when it's done. Now, care to look at my wares?")],
                    otherwise: vec![dialog("Tilda", "The offer stands. Now, care to look at my wares?")],
                },
            ],
        ),
        trig(
            TriggerOn::Interact,
            vec![quest_is("pest_control", QuestCheck::AtStage, 1)],
            vec![
                dialog("Tilda", "The slimes are gone? My deliveries are safe again - thank you, {player}! Here, as promised."),
                Action::GiveItem { item: "coin".into(), count: 30 },
                Action::GiveItem { item: "potion".into(), count: 1 },
                Action::GiveSkillPoints { amount: 1 },
                quest("pest_control", QuestOp::Complete, 0),
            ],
        ),
        // The shrine was lit before her quest was taken: no slimes left to hunt - she thanks the hero.
        trig(
            TriggerOn::Interact,
            vec![quest_is("pest_control", QuestCheck::NotStarted, 0), cond(flag("shrine_lit")), not(flag("tilda_thanked"))],
            vec![
                Action::SetFlag { name: "tilda_thanked".into(), value: true },
                dialog("Tilda", "So you're the one who woke the old shrine! Not a single slime has touched my deliveries since. You're a hero, {player} - the whole town owes you."),
                dialog("Tilda", "Now, care to look at my wares?"),
            ],
        ),
    ];
    // Stock in the corners (barrels are small containers).
    let barrel = b.entity(EntityKind::Prop, "barrel", 1, 6);
    barrel.container = Some(ContainerSpec { size: 4, items: vec![ItemStack::new("apple", 1)], ..Default::default() });
    b.entity(EntityKind::Prop, "barrel", 10, 6);
    b.entity(EntityKind::Prop, "barrel", 10, 8);
    // An alchemy table for the customers (crafting: Database › Recipes, station "alchemy").
    b.entity(EntityKind::Prop, "alchemy_table", 1, 8);
    // A chair to rest on.
    b.entity(EntityKind::Prop, "chair", 2, 6);
    b.m.background = [13, 13, 20];
    b.m.music = "music_village".into();
    autotile_terrain(&mut b.m, |m, x, y| (ground(m, x, y)? == t::RUG).then_some("Rug"), |_, _, _, _| false);
    b.m
}

fn house() -> Map {
    let (w, h) = (12, 10);
    let mut b = B::new("house", w, h);
    b.fill(0, 0, 0, w as i32, h as i32, t::VOID);
    b.fill(0, 1, 2, 10, 7, t::WOOD_FLOOR);
    b.fill(1, 1, 0, 10, 2, t::HOUSE_WALL);
    b.set(1, 3, 1, t::WINDOW);
    b.set(1, 8, 1, t::WINDOW);
    b.fill(1, 0, 0, 1, h as i32, t::VOID);
    b.fill(1, 11, 0, 1, h as i32, t::VOID);
    b.fill(1, 0, 9, w as i32, 1, t::VOID);
    b.fill(0, 4, 4, 4, 3, t::RUG);
    // (The bed is a prop now: furniture the player sleeps in, placed at the end.)
    b.fill(1, 6, 2, 2, 1, t::SHELF);
    b.set(1, 9, 5, t::TABLE);
    b.set(0, 6, 9, t::WOOD_FLOOR);
    b.set(1, 6, 9, t::DOOR);
    b.m.set_collision(6, 9, rustpg2d_engine::map::CollisionOverride::Walkable);
    b.spawn("entrance", 6, 8);
    b.portal(6, 9, "village", "house_door");
    // The elder's chest: a container that only opens once the quest was accepted.
    let chest = b.entity(EntityKind::Prop, "chest", 9, 3);
    chest.container = Some(ContainerSpec {
        size: 8,
        items: vec![ItemStack::new("key", 1), ItemStack::new("sword", 1), ItemStack::new("coin", 10), ItemStack::new("potion", 2), ItemStack::new("letter", 1), ItemStack::new("spellbook_spark", 1)],
        conditions: vec![cond(flag("quest_accepted"))],
        locked_text: "It's the elder's chest. Better not open it without asking.".into(),
        open_state: "open".into(),
        ..Default::default()
    });
    b.entity(EntityKind::Item, "potion", 3, 6);
    b.entity(EntityKind::Item, "bow", 1, 7);
    let arrows = b.entity(EntityKind::Item, "arrows", 2, 8);
    arrows.count = 20;
    // The elder's bed: sleep until morning (Database › Props › Bed › Furniture).
    b.entity(EntityKind::Prop, "bed", 2, 3);
    // The elder's table with a chair on each side (chairs face every way).
    b.set(1, 8, 5, t::TABLE);
    b.entity(EntityKind::Prop, "chair", 8, 4);
    b.entity(EntityKind::Prop, "chair_right", 7, 5);
    b.entity(EntityKind::Prop, "chair_left", 10, 5);
    b.entity(EntityKind::Prop, "chair_up", 9, 6);
    // A sideways bed for a guest by the door.
    b.entity(EntityKind::Prop, "bed_side", 4, 8);
    b.m.background = [13, 13, 20];
    b.m.music = "music_village".into();
    autotile_terrain(&mut b.m, |m, x, y| (ground(m, x, y)? == t::RUG).then_some("Rug"), |_, _, _, _| false);
    b.m
}

fn forest() -> Map {
    let (w, h) = (64, 48);
    let mut b = B::new("forest", w, h);
    let mut rng = Rng(1234);
    b.fill(0, 0, 0, w as i32, h as i32, t::FOREST_FLOOR);
    for _ in 0..300 {
        let (x, y) = (rng.range(w) as i32, rng.range(h) as i32);
        b.set(0, x, y, if rng.chance(0.6) { t::GRASS } else { t::TALL_GRASS });
    }
    // Winding path from the west edge (y = 24) to the shrine clearing.
    let mut y = 23;
    for x in 0..44 {
        b.fill(0, x, y, 1, 3, t::PATH);
        if x % 7 == 3 {
            y += if rng.chance(0.5) { 1 } else { -1 };
            y = y.clamp(18, 28);
            b.fill(0, x, y, 1, 3, t::PATH);
        }
    }
    // Clearing with the shrine on a stone platform (centered on the 3-tile shrine at 46-48).
    b.fill(0, 40, 16, 14, 14, t::GRASS);
    b.fill(0, 43, 19, 9, 7, t::STONE_FLOOR);
    // Stream; where it crosses the path it becomes a bridge.
    for yy in 0..h as i32 {
        let x = 28 + ((yy as f32 / 6.0).sin() * 2.0) as i32;
        for xx in x..x + 2 {
            let tile = if b.get(0, xx, yy) == Some(t::PATH) { t::BRIDGE } else { t::WATER };
            b.set(0, xx, yy, tile);
        }
    }
    // Cave entrance to the wilds in the north-east.
    b.fill(1, 52, 3, 9, 4, t::CLIFF);
    b.set(1, 56, 6, t::CAVE);
    b.m.set_collision(56, 6, rustpg2d_engine::map::CollisionOverride::Walkable);
    b.fill(0, 56, 7, 1, 10, t::PATH);
    b.portal(56, 6, "wilds", "start");
    b.spawn("cave", 56, 7);

    // Dense trees, keeping paths, water and clearing free.
    for yy in 1..h as i32 {
        for xx in 0..w as i32 {
            let border = xx == 0 || yy == 1 || xx == w as i32 - 1 || yy == h as i32 - 1;
            let ground = b.get(0, xx, yy);
            let open = matches!(ground, Some(t::PATH | t::WATER | t::BRIDGE | t::STONE_FLOOR))
                || (40..54).contains(&xx) && (16..30).contains(&yy)
                || (xx <= 1 && (22..=27).contains(&yy));
            let density = if border { 1.0 } else { 0.22 };
            if !open && b.free(xx, yy) && rng.chance(density) && b.get(1, xx, yy - 1).is_none() {
                if rng.chance(0.4) { b.pine(xx, yy) } else { b.tree(xx, yy) }
            } else if !open && rng.chance(0.03) && b.get(1, xx, yy).is_none() {
                b.set(1, xx, yy, t::MUSHROOM);
            }
        }
    }

    b.spawn("west", 2, 24);
    // Two statues guard the shrine, one tile in from the platform's edges.
    b.big("statue", 44, 21, 1, 2);
    b.big("statue", 50, 21, 1, 2);
    b.big("big_oak", 39, 15, 3, 4);
    b.big("big_pine", 54, 28, 2, 4);
    b.big("big_oak", 24, 12, 3, 4);
    let shrine = b.big("shrine", 47, 21, 3, 2);
    shrine.name = "shrine".into();
    shrine.triggers = vec![trig(TriggerOn::Interact, vec![], vec![Action::RunScript { function: "shrine_touch".into() }])];
    let clearing = b.zone("clearing", 40, 16, 14, 14, ZoneKind::Trigger);
    if let Some(z) = b.m.zones.iter_mut().find(|z| z.uid == clearing) {
        z.triggers = vec![Trigger {
            on: TriggerOn::Enter,
            conditions: vec![],
            actions: vec![dialog("", "The air hums with a strange energy. An old shrine stands in the clearing.")],
            once: true,
        }];
    }
    let chest = b.entity(EntityKind::Prop, "chest", 42, 18);
    chest.container = Some(ContainerSpec {
        size: 6,
        items: vec![ItemStack::new("crossbow", 1), ItemStack::new("bolts", 12), ItemStack::new("leather_armor", 1), ItemStack::new("potion", 1), ItemStack::new("coin", 7)],
        open_frame: 1,
        ..Default::default()
    });
    // A locked chest: needs the Bronze Key from the village.
    chest.lock = Some(LockSpec {
        key: "bronze_key".into(),
        locked_text: "A sturdy padlock. You need a Bronze Key - or a steady hand with a lockpick.".into(),
        pick_difficulty: 1,
        ..Default::default()
    });
    // Spawners (invisible in game): slimes come back whenever you re-enter the forest,
    // goblin archers return 20 s after being defeated.
    let den = b.zone("slime_den", 8, 28, 14, 10, ZoneKind::Spawner { character: "slime".into(), count: 4, respawn: Respawn::OnEnter });
    // Slimes from the den have their own loot table: the slime's coins and apples plus a potion.
    b.m.zones.iter_mut().find(|z| z.uid == den).unwrap().loot = Some(vec![
        LootEntry::new("coin", 50.0, 1, 3),
        LootEntry::new("apple", 10.0, 1, 1),
        LootEntry::new("potion", 10.0, 1, 1),
    ]);
    let camp = b.zone("goblin_camp", 32, 30, 8, 8, ZoneKind::Spawner { character: "goblin".into(), count: 2, respawn: Respawn::After { seconds: 20.0 } });
    // The goblin camp's archers carry more than a lone goblin: arrows, coins, bolts, now and
    // then a leather cap, and rarely a crossbow (the spawner's table replaces the goblin's own).
    b.m.zones.iter_mut().find(|z| z.uid == camp).unwrap().loot = Some(vec![
        LootEntry::new("arrows", 80.0, 2, 5),
        LootEntry::new("coin", 40.0, 2, 5),
        LootEntry::new("bolts", 25.0, 2, 4),
        LootEntry::new("leather_cap", 10.0, 1, 1),
        LootEntry::new("crossbow", 5.0, 1, 1),
    ]);
    // Placed slimes stay dead once defeated.
    for (x, y) in [(36, 30), (15, 12), (50, 40)] {
        // Move off trees / mushrooms if needed.
        let (x, y) = (0..6).flat_map(|d| [(x + d, y), (x - d, y), (x, y + d), (x, y - d)]).find(|&(x, y)| b.get(1, x, y).is_none()).unwrap();
        b.entity(EntityKind::Character, "slime", x, y);
    }
    b.entity(EntityKind::Item, "potion", 20, 20);
    let coins = b.entity(EntityKind::Item, "coin", 46, 27);
    coins.count = 5;
    // Custom event raised by the shrine script.
    b.m.triggers = vec![trig(
        TriggerOn::Custom { name: "shrine_lit".into() },
        vec![],
        vec![Action::Wait { seconds: 0.4 }, dialog("", "A warm light spreads through the forest. The slime den and the goblin camp fall silent.")],
    )];
    b.m.edges.west = Some(EdgeLink { target_map: "village".into(), offset: -6, depth: 1 });
    b.m.background = [10, 25, 10];
    b.m.music = "music_forest".into();
    b.m.lighting = MapLighting { mode: LightMode::Outdoor, darkness: 0.8 };
    // An eerie fog over Whisperwood: it lifts when the shrine is lit (shrine_awakening).
    b.m.weather = rustpg2d_engine::map::Weather::new(rustpg2d_engine::map::WeatherKind::Fog, 0.45);
    // Explored regions: the map screen reveals it as the player walks around.
    b.m.fog = true;
    // The old mine: a second cave mouth in the cliff, west of the one to the wilds.
    for yy in 7..=9 {
        for xx in 52..=56 {
            b.m.layers[1].set(xx as u32, yy as u32, Tile::EMPTY);
            b.m.layers[2].set(xx as u32, (yy - 1) as u32, Tile::EMPTY);
        }
    }
    b.set(1, 53, 6, t::CAVE);
    b.m.set_collision(53, 6, rustpg2d_engine::map::CollisionOverride::Walkable);
    b.fill(0, 53, 7, 4, 1, t::PATH);
    b.portal(53, 6, "mine", "entrance");
    b.spawn("mine", 53, 7);
    // A torch lies at the mine entrance (it glows on the ground at night).
    let torch = b.entity(EntityKind::Item, "torch", 54, 8);
    torch.count = 2;
    // The bandit camp (south-east): the chief, and two outlaws who only show up when he calls
    // for help at half his HP. Spots are moved off trees like the slimes'.
    let free = |b: &B, x: i32, y: i32| (0..6).flat_map(|d| [(x + d, y), (x - d, y), (x, y + d), (x, y - d)]).find(|&(x, y)| b.free(x, y)).unwrap();
    let (cx, cy) = free(&b, 58, 38);
    let chief = b.entity(EntityKind::Character, "bandit_chief", cx, cy);
    chief.name = "bandit_chief".into();
    chief.triggers = vec![Trigger {
        on: TriggerOn::HpBelow { percent: 50 },
        conditions: vec![],
        actions: vec![
            dialog("{self}", "Argh! Lads, to me! Cut this one down!"),
            Action::SetActive { target: "outlaw_1".into(), active: true },
            Action::SetActive { target: "outlaw_2".into(), active: true },
        ],
        once: true,
    }];
    for (k, (x, y)) in [(55, 41), (61, 35)].into_iter().enumerate() {
        let (x, y) = free(&b, x, y);
        let o = b.entity(EntityKind::Character, "outlaw", x, y);
        o.name = format!("outlaw_{}", k + 1);
        o.active = false;
    }
    // Auto-tiling: the forest floor gives way softly to the clearing and the grass patches;
    // paths, the stream and the shrine's flagstones get their edges. The path is a forest path
    // in the woods and a dirt path on the clearing's grass; it runs over the bridge and up to
    // the caves.
    let clearing = |x: i32, y: i32| (40..54).contains(&x) && (16..30).contains(&y);
    // The path fades out on the clearing's grass instead of stopping at its edge.
    for y in 16..30 {
        if b.get(0, 39, y) == Some(t::PATH) {
            b.fill(0, 40, y, 2, 1, t::PATH);
        }
    }
    // Herbs and mushrooms along the path (alchemy ingredients).
    for (k, x) in [5, 11, 17, 24, 31, 37].into_iter().enumerate() {
        if let Some(y) = (16..32).find(|&y| b.get(0, x, y) == Some(t::PATH) && b.free(x, y)) {
            b.entity(EntityKind::Item, if k % 2 == 0 { "herb" } else { "mushroom" }, x, y);
        }
    }
    autotile_terrain(
        &mut b.m,
        |m, x, y| match ground(m, x, y)? {
            t::FOREST_FLOOR => Some("Forest floor"),
            t::PATH if clearing(x, y) => Some("Dirt path"),
            t::PATH => Some("Forest path"),
            t::WATER => Some("Stream"),
            t::STONE_FLOOR => Some("Flagstones"),
            _ => None,
        },
        |m, terrain, x, y| match terrain {
            "Stream" | "Forest path" | "Dirt path" | "Forest floor" => ground(m, x, y) == Some(t::BRIDGE) || (terrain.ends_with("path") && tile_at(m, 1, x, y) == Some(t::CAVE)),
            _ => false,
        },
    );
    b.m
}

/// A 1024x1024 procedurally generated map showing the engine's maximum size.
fn wilds() -> Map {
    let n = 1024u32;
    let mut b = B::new("wilds", n, n);
    // Value noise.
    let grid = 32usize;
    let cells = n as usize / grid + 2;
    let mut rng = Rng(99);
    let lattice: Vec<f32> = (0..cells * cells).map(|_| rng.range(1000) as f32 / 1000.0).collect();
    let lat = |x: usize, y: usize| lattice[y * cells + x];
    let smooth = |t: f32| t * t * (3.0 - 2.0 * t);
    let noise = |x: u32, y: u32| {
        let (gx, gy) = (x as usize / grid, y as usize / grid);
        let (fx, fy) = (smooth((x as usize % grid) as f32 / grid as f32), smooth((y as usize % grid) as f32 / grid as f32));
        let a = lat(gx, gy) * (1.0 - fx) + lat(gx + 1, gy) * fx;
        let c = lat(gx, gy + 1) * (1.0 - fx) + lat(gx + 1, gy + 1) * fx;
        a * (1.0 - fy) + c * fy
    };
    for y in 0..n {
        for x in 0..n {
            let mut v = noise(x, y) * 0.75 + noise((x * 3) % n, (y * 3) % n) * 0.25;
            // Keep the arrival area dry.
            let d = ((x as f32 - 512.0).powi(2) + (y as f32 - 512.0).powi(2)).sqrt();
            if d < 12.0 {
                v = 0.5;
            }
            let tile = match v {
                v if v < 0.3 => t::WATER,
                v if v < 0.36 => t::SAND,
                v if v < 0.62 => if rng.chance(0.05) { t::FLOWERS } else { t::GRASS },
                v if v < 0.78 => t::FOREST_FLOOR,
                _ => t::SNOW,
            };
            b.set(0, x as i32, y as i32, tile);
            if (tile == t::FOREST_FLOOR && rng.chance(0.25)) || (tile == t::GRASS && rng.chance(0.02)) {
                if y > 1 && b.free(x as i32, y as i32) && b.get(1, x as i32, y as i32 - 1).is_none() && d > 4.0 {
                    if v > 0.7 { b.pine(x as i32, y as i32) } else { b.tree(x as i32, y as i32) }
                }
            } else if tile == t::SNOW && rng.chance(0.03) && d > 4.0 {
                b.set(1, x as i32, y as i32, t::ROCK);
            }
        }
    }
    // Border walls of cliffs.
    for i in 0..n as i32 {
        for (x, y) in [(i, 0), (i, n as i32 - 1), (0, i), (n as i32 - 1, i)] {
            b.set(1, x, y, t::CLIFF);
        }
    }
    b.fill(1, 510, 506, 5, 3, t::CLIFF);
    b.set(1, 512, 508, t::CAVE);
    b.m.set_collision(512, 508, rustpg2d_engine::map::CollisionOverride::Walkable);
    b.portal(512, 508, "forest", "cave");
    b.spawn("start", 512, 510);
    // Scatter coins across the land.
    for _ in 0..200 {
        let (x, y) = (rng.range(n - 4) as i32 + 2, rng.range(n - 4) as i32 + 2);
        if matches!(b.get(0, x, y), Some(t::GRASS | t::SAND)) && b.get(1, x, y).is_none() {
            b.entity(EntityKind::Item, "coin", x, y);
        }
    }
    let sign = b.entity(EntityKind::Prop, "sign", 514, 510);
    sign.triggers = vec![trig(TriggerOn::Interact, vec![], vec![dialog("Sign", "The Wilds: 1024 x 1024 tiles of procedurally generated land. Coins are scattered everywhere!")])];
    // Enemy spawners scattered over the wilds (not too close to the arrival point).
    let mut placed = 0;
    while placed < 40 {
        let (x, y) = (rng.range(n - 40) + 20, rng.range(n - 40) + 20);
        let far = ((x as i32 - 512).pow(2) + (y as i32 - 510).pow(2)) > 30 * 30;
        if !far {
            continue;
        }
        let (character, count, respawn) = match placed % 4 {
            0 => ("goblin", 2, Respawn::After { seconds: 45.0 }),
            1 => ("slime", 5, Respawn::Never),
            2 => ("goblin_f", 2, Respawn::After { seconds: 45.0 }),
            _ => ("slime", 4, Respawn::After { seconds: 30.0 }),
        };
        b.zone(&format!("spawner_{placed}"), x, y, 12, 12, ZoneKind::Spawner { character: character.into(), count, respawn });
        placed += 1;
    }
    // A camp right next to the arrival point, to try ranged combat.
    b.zone("practice", 520, 500, 10, 8, ZoneKind::Spawner { character: "slime".into(), count: 3, respawn: Respawn::After { seconds: 8.0 } });
    let bolts = b.entity(EntityKind::Item, "bolts", 510, 511);
    bolts.count = 10;
    b.big("boulder", 506, 515, 2, 2);
    b.big("boulder", 521, 513, 2, 2);
    let mut trees = 0;
    while trees < 300 {
        let (x, y) = (rng.range(n - 20) as i32 + 10, rng.range(n - 20) as i32 + 10);
        if ((x - 512).pow(2) + (y - 510).pow(2)) < 15 * 15 {
            continue;
        }
        match b.get(0, x, y) {
            Some(t::GRASS) => {
                b.big("big_oak", x, y, 3, 4);
            }
            Some(t::FOREST_FLOOR) | Some(t::SNOW) => {
                b.big("big_pine", x, y, 2, 4);
            }
            _ => continue,
        }
        trees += 1;
    }
    // The armory: heavier gear for the wilds.
    let chest = b.entity(EntityKind::Prop, "chest", 508, 511);
    chest.container = Some(ContainerSpec {
        size: 8,
        items: vec![
            ItemStack::new("spear", 1),
            ItemStack::new("axe", 1),
            ItemStack::new("iron_helmet", 1),
            ItemStack::new("chainmail", 1),
            ItemStack::new("potion", 3),
        ],
        open_frame: 1,
        ..Default::default()
    });
    b.m.background = [30, 60, 120];
    b.m.music = "music_forest".into();
    b.m.lighting = MapLighting { mode: LightMode::Outdoor, darkness: 0.75 };
    b.m.weather = rustpg2d_engine::map::Weather::new(rustpg2d_engine::map::WeatherKind::Rain, 0.5);
    // Explored regions: the map screen reveals it as the player walks around.
    b.m.fog = true;
    // Auto-tiling: lakes with sandy shores, soft edges between grass, forest and snow.
    autotile_terrain(
        &mut b.m,
        |m, x, y| match ground(m, x, y)? {
            t::WATER => Some("Shore"),
            t::SAND => Some("Sand"),
            t::FOREST_FLOOR => Some("Forest floor"),
            t::SNOW => Some("Snowfield"),
            _ => None,
        },
        |_, _, _, _| false,
    );
    b.m
}

// ------------------------------------------------------------------ definitions

fn database() -> Database {
    let sprite = |sheet: &str, frame: u32| SpriteRef::new(sheet, frame);
    let prop = |id: &str, name: &str, frame: u32, solid: bool, hitbox: [f32; 2]| PropDef {
        id: id.into(),
        name: name.into(),
        sprite: sprite("props", frame),
        solid,
        hitbox: [hitbox[0] * R, hitbox[1] * R],
        triggers: vec![],
        container: None,
        lock: None,
        default_state: default_state_name(),
        states: vec![],
        light: None,
        use_spot: None,
    };
    let state = |name: &str, sprite: SpriteRef, solid: bool| PropState { name: name.into(), sprite, solid, light: None };
    // `body` = the body sheet of monsters; people get their layered look in `layered_characters`.
    let ch = |id: &str, name: &str, body: &str, speed: f32, behavior: Behavior| CharacterDef {
        id: id.into(),
        name: name.into(),
        sheet: String::new(),
        speed: speed * R,
        hitbox: [10.0 * R, 6.0 * R],
        behavior,
        max_hp: 0,
        damage: 0,
        hostile: false,
        weapon: None,
        defense: 0,
        xp: 0,
        touch_effect: None,
        description: String::new(),
        triggers: vec![],
        shop: None,
        loot: vec![],
        light: None,
        appearance: Appearance { body: body.into(), ..Default::default() },
        equipment: Default::default(),
        inventory: Vec::new(),
    };
    let item = |id: &str, name: &str, desc: &str, frame: u32, consumable: bool, on_use: Vec<Trigger>| ItemDef {
        id: id.into(),
        name: name.into(),
        description: desc.into(),
        icon: sprite("items", frame),
        consumable,
        on_use,
        weapon: None,
        armor: None,
        max_stack: 99,
        value: 0,
        light: None,
        teaches: String::new(),
        held: String::new(),
        tool: String::new(),
        use_on: Default::default(),
    };
    let stack = |mut i: ItemDef, n: u32| {
        i.max_stack = n;
        i
    };
    let weapon = |mut i: ItemDef, w: Weapon| {
        i.weapon = Some(w);
        i
    };
    let armor = |mut i: ItemDef, a: Armor| {
        i.armor = Some(a);
        i
    };
    let use_trig = |actions: Vec<Action>| vec![trig(TriggerOn::UseItem { item: String::new() }, vec![], actions)];
    let mut slime = ch("slime", "Slime", "slime", 32.0, Behavior::chase(4.0));
    slime.hitbox = [12.0 * R, 6.0 * R];
    slime.max_hp = 3;
    slime.damage = 2;
    slime.hostile = true;
    slime.xp = 5;
    slime.description = "A wobbling lump of green jelly from the Whisperwood dens. Slow and harmless-looking, but its touch is poisonous - keep an antidote handy.".into();
    // A slime's touch poisons: half a heart over six seconds (antidotes cure it).
    slime.touch_effect = Some(EffectSpec::new(EffectKind::Poison, 0.5, 6.0));
    slime.triggers = vec![trig(TriggerOn::Defeat, vec![], vec![Action::RunScript { function: "slime_defeated".into() }])];
    slime.loot = vec![LootEntry::new("coin", 50.0, 1, 3), LootEntry::new("apple", 10.0, 1, 1)];
    // Goblin archers give up 10 tiles from home, run away once wounded (2 HP: at 1) and keep 4
    // tiles between them and the player while shooting.
    let mut goblin = ch("goblin", "Goblin Archer", "", 38.0, Behavior::Chase { sight: 7.0, leash: 10.0, flee_below: 0.6, keep_distance: 4.0 });
    goblin.max_hp = 2;
    goblin.damage = 1;
    goblin.hostile = true;
    goblin.xp = 12;
    goblin.description = "Goblin archers guard a camp south of the forest path. They shoot from a distance, back off when you close in and run once wounded - but never far from their camp.".into();
    goblin.weapon = Some(Weapon::ranged(2, 1.6, Ranged { projectile: sprite("items", it::ARROW_SHOT), speed: 150.0 * R, range: 6.0, ammo: String::new() }));
    // Goblin archers drop arrows (and sometimes coins) from their loot table.
    goblin.loot = vec![LootEntry::new("arrows", 70.0, 2, 4), LootEntry::new("coin", 25.0, 1, 2)];
    // Goblin scouts (female) roam the wilds with the archers.
    let goblin_f = CharacterDef {
        id: "goblin_f".into(),
        name: "Goblin Scout".into(),
        speed: 42.0 * R,
        description: "Quicker than the archers, goblin scouts roam the wilds in small bands, bows ready.".into(),
        ..goblin.clone()
    };
    // The bandits' camp in the south-east of Whisperwood: the chief calls two outlaws when hurt
    // (an 'HP below 50 %' trigger on the placed chief).
    let mut chief = ch("bandit_chief", "Bandit Chief", "", 40.0, Behavior::Chase { sight: 6.0, leash: 9.0, flee_below: 0.0, keep_distance: 0.0 });
    chief.max_hp = 10;
    chief.damage = 2;
    chief.hostile = true;
    chief.xp = 60;
    chief.description = "The leader of the bandits in south-east Whisperwood: tough, armed with an axe, and quick to call his outlaws for help when a fight turns against him.".into();
    chief.loot = vec![LootEntry::new("coin", 100.0, 10, 20), LootEntry::new("potion", 100.0, 1, 1), LootEntry::new("elixir_strength", 50.0, 1, 1)];
    let mut outlaw = ch("outlaw", "Outlaw", "", 46.0, Behavior::Chase { sight: 7.0, leash: 12.0, flee_below: 0.3, keep_distance: 0.0 });
    outlaw.max_hp = 4;
    outlaw.damage = 2;
    outlaw.hostile = true;
    outlaw.xp = 20;
    outlaw.description = "The Bandit Chief's men. They hide in the woods until he calls, fight with swords and flee when the fight goes badly.".into();
    outlaw.loot = vec![LootEntry::new("coin", 60.0, 2, 6)];
    let mut hero = ch("hero", "Hero", "", 70.0, Behavior::Static);
    hero.max_hp = 12; // 6 hearts
    hero.damage = 1;
    let mut db = Database {
        structures: structures::example_structures(),
        tilesets: vec![TilesetDef {
            id: "terrain".into(),
            image: "tilesets/terrain.png".into(),
            solid: terrain_solid(),
            animations: terrain_anims(),
            // Lit windows glow a little at night.
            lights: vec![TileLight { tile: t::WINDOW, light: LightSpec { intensity: 0.7, ..LightSpec::new(1.8, [255, 205, 130], false) } }],
            terrains: terrains(),
        },
        art::extra::dungeon_tileset().0],
        quests: vec![
            QuestDef {
                id: "strange_lights".into(),
                title: "The Strange Lights".into(),
                description: "Strange lights have been seen in Whisperwood, east of Emberbrook. Elder Rowan asked me to investigate the old shrine there.".into(),
                stages: vec![
                    "Take the gate key from the chest in Elder Rowan's house, north of the well.".into(),
                    "Open the east gate and find the old shrine deep in Whisperwood.".into(),
                    "Return to Elder Rowan.".into(),
                ],
                xp: 80,
            },
            QuestDef {
                id: "pest_control".into(),
                title: "Pest Control".into(),
                description: "Slimes from Whisperwood keep eating Tilda's deliveries. She promised 30 coins and a potion if I get rid of five of them.".into(),
                stages: vec!["Defeat slimes in Whisperwood ({pest_slimes}/5).".into(), "Return to Tilda's shop for the reward.".into()],
                xp: 40,
            },
        ],
        sounds: SOUNDS
            .iter()
            .map(|(id, _)| SoundDef { id: id.to_string(), file: format!("sounds/{id}.wav"), volume: 0.6 })
            .chain(music::TRACKS.iter().map(|(id, _)| SoundDef { id: id.to_string(), file: format!("sounds/{id}.ogg"), volume: 0.5 }))
            .collect(),
        sheets: vec![
            SheetDef { kind: SheetKind::Body, ..SheetDef::new("slime", "sprites/slime.png", 32, 32) },
            SheetDef { kind: SheetKind::Armor, ..SheetDef::new("ov_cap", "sprites/ov_cap.png", 32, 48) },
            SheetDef { kind: SheetKind::Armor, ..SheetDef::new("ov_helmet", "sprites/ov_helmet.png", 32, 48) },
            SheetDef { kind: SheetKind::Armor, ..SheetDef::new("ov_leather", "sprites/ov_leather.png", 32, 48) },
            SheetDef { kind: SheetKind::Armor, ..SheetDef::new("ov_chain", "sprites/ov_chain.png", 32, 48) },
            SheetDef::new("props", "sprites/props.png", 32, 32),
            SheetDef::new("items", "sprites/items.png", 32, 32),
            // Spell effects (the Fireball script).
            SheetDef::new("fireball", "sprites/fireball.png", 32, 32),
            SheetDef::new("explosion", "sprites/explosion.png", 64, 64),
            // The forest shrine's awakening (cut-scene).
            SheetDef::new("magic_burst", "sprites/magic_burst.png", 64, 64),
        ],
        items: vec![
            stack(item("key", "Gate Key", "An old iron key. Opens the east gate.", it::KEY, false, vec![]), 1),
            stack(item("bronze_key", "Bronze Key", "Opens a chest somewhere in Whisperwood.", it::BRONZE_KEY, false, vec![]), 5),
            stack(for_allies(item(
                "potion",
                "Red Potion",
                "Restores 3 hearts.",
                it::POTION,
                true,
                use_trig(vec![Action::Heal { amount: 6 }, Action::SetVar { name: "potions_drunk".into(), op: VarOp::Add, value: 1 }]),
            )), 10),
            item("coin", "Coin", "Shiny gold coins.", it::COIN, false, use_trig(vec![Action::RunScript { function: "count_coins".into() }])),
            stack(item("apple", "Apple", "Crunchy and sweet. Restores 1 heart.", it::APPLE, true, use_trig(vec![Action::Heal { amount: 2 }])), 20),
            weapon(
                stack(item("bow", "Hunting Bow", "Fast shots, uses arrows. Use it from the inventory to equip (or press Q).", it::BOW, false, vec![]), 1),
                Weapon::ranged(1, 0.45, Ranged { projectile: sprite("items", it::ARROW_SHOT), speed: 240.0 * R, range: 8.0, ammo: "arrows".into() }),
            ),
            weapon(
                stack(item("crossbow", "Crossbow", "Slow but powerful, uses bolts.", it::CROSSBOW, false, vec![]), 1),
                Weapon::ranged(3, 1.1, Ranged { projectile: sprite("items", it::BOLT_SHOT), speed: 320.0 * R, range: 10.0, ammo: "bolts".into() }),
            ),
            // Healing items set to "Player or companion": with a companion along the player chooses.
            stack(for_allies(item("bandage", "Bandage", "Linen for wounds: restores 1 heart. Use it on yourself or a companion.", it::BANDAGE, true, use_trig(vec![Action::Heal { amount: 2 }]))), 10),
            stack(item("arrows", "Arrows", "Ammunition for the bow.", it::ARROWS, false, vec![]), 50),
            stack(item("bolts", "Bolts", "Ammunition for the crossbow.", it::BOLTS, false, vec![]), 30),
            // Melee weapons: balanced sword, long narrow spear, slow heavy axe.
            stack(weapon(item("sword", "Short Sword", "A reliable blade.", it::SWORD, false, vec![]), Weapon { reach: 1.1, arc: 65.0, ..Weapon::melee(2, 0.35) }), 1),
            stack(weapon(item("spear", "Spear", "Hits from two tiles away, but only straight ahead.", it::SPEAR, false, vec![]), Weapon { reach: 1.9, arc: 22.0, knockback: 0.7, ..Weapon::melee(2, 0.55) }), 1),
            stack(weapon(item("axe", "Battle Axe", "Slow, wide swings that send enemies flying.", it::AXE, false, vec![]), Weapon { reach: 1.0, arc: 85.0, knockback: 1.8, ..Weapon::melee(4, 0.85) }), 1),
            // Armor: helmets and body armor.
            stack(armor(item("leather_cap", "Leather Cap", "Light head protection.", it::CAP, false, vec![]), Armor { slot: EquipSlot::Head, defense: 1, speed: 0.0, overlay: "ov_cap".into(), overlay_f: String::new() }), 1),
            stack(armor(item("iron_helmet", "Iron Helmet", "Sturdy, a little heavy.", it::HELMET, false, vec![]), Armor { slot: EquipSlot::Head, defense: 2, speed: -0.05, overlay: "ov_helmet".into(), overlay_f: String::new() }), 1),
            stack(armor(item("leather_armor", "Leather Armor", "Light body armor.", it::LEATHER, false, vec![]), Armor { slot: EquipSlot::Body, defense: 1, speed: 0.0, overlay: "ov_leather".into(), overlay_f: String::new() }), 1),
            stack(armor(item("chainmail", "Chainmail", "Strong protection, but you move slower.", it::CHAIN, false, vec![]), Armor { slot: EquipSlot::Body, defense: 3, speed: -0.15, overlay: "ov_chain".into(), overlay_f: String::new() }), 1),
            // Left hand: lights and a shield (not together with a two-handed weapon).
            light(stack(armor(item("lantern", "Lantern", "A steady light for dark places. Hold it in your left hand.", it::LANTERN, false, vec![]), Armor { slot: EquipSlot::Offhand, defense: 0, speed: 0.0, overlay: String::new(), overlay_f: String::new() }), 1), LightSpec::new(5.5, [255, 220, 170], false)),
            light(stack(armor(item("torch", "Torch", "A flickering torch for the left hand. Cheap, but it lights less than a lantern.", it::TORCH, false, vec![]), Armor { slot: EquipSlot::Offhand, defense: 0, speed: 0.0, overlay: String::new(), overlay_f: String::new() }), 5), LightSpec::torch(4.0)),
            // Magic: scrolls and books teach spells (scrolls are used up), mana potions.
            teach(stack(item("scroll_heal", "Scroll of Healing", "Reading it teaches the Heal spell. The scroll crumbles afterwards.", it::SCROLL, true, vec![]), 5), "heal"),
            teach(stack(item("scroll_light", "Scroll of Light", "Teaches the Light spell: a glow around you in dark places.", it::SCROLL, true, vec![]), 5), "light"),
            teach(stack(item("spellbook_spark", "Book of Sparks", "A worn spell book. Teaches Spark - but it takes some arcane knowledge to understand.", it::BOOK, false, vec![]), 1), "spark"),
            teach(stack(item("tome_fireball", "Tome of the Fireball", "Teaches Fireball: aim it at a spot and it explodes there. Needs Arcana 3.", it::BOOK, false, vec![]), 1), "fireball"),
            teach(stack(item("tome_fire", "Tome of Fire", "Fire Burst: flames around you. Needs a trained arcanist.", it::BOOK, false, vec![]), 1), "fire_burst"),
            stack(item("mana_potion", "Blue Potion", "Restores 20 mana.", it::MANA_POTION, true, use_trig(vec![Action::AddMana { amount: 20 }])), 10),
            // Status effect potions.
            stack(item("antidote", "Antidote", "Cures poison.", it::ANTIDOTE, true, use_trig(vec![Action::CureEffect { target: "player".into(), kind: Some(EffectKind::Poison) }])), 10),
            stack(
                item("elixir_strength", "Elixir of Strength", "Your blows hit 30% harder for a minute.", it::ELIXIR, true, use_trig(vec![Action::ApplyEffect { target: "player".into(), effect: EffectSpec::new(EffectKind::Strength, 30.0, 60.0) }])),
                10,
            ),
            stack(
                item("swiftness", "Swiftness Potion", "You move 30% faster for half a minute.", it::SWIFTNESS, true, use_trig(vec![Action::ApplyEffect { target: "player".into(), effect: EffectSpec::new(EffectKind::Haste, 30.0, 30.0) }])),
                10,
            ),
            // Fishing (village pond: a fishing spot zone). The rod is needed to fish there.
            // A tool (Tool slot, tool kind "fishing rod"): in the hand while fishing.
            stack(ItemDef { tool: "fishing rod".into(), ..item("fishing_rod", "Fishing Rod", "A tool: stand at the pond's edge and press E to cast.", it::ROD, false, vec![]) }, 1),
            stack(item("trout", "Trout", "A fresh trout. Eating it restores a heart.", it::TROUT, true, use_trig(vec![Action::Heal { amount: 2 }])), 10),
            stack(item("old_boot", "Old Boot", "Soggy. Nobody wants it - except perhaps a merchant who buys anything.", it::BOOT, false, vec![]), 5),
            // Crafting ingredients (Database › Recipes: brewed at the alchemy table in Tilda's shop).
            stack(item("herb", "Healing Herb", "A fragrant herb. Brewed into potions at an alchemy table.", it::HERB, false, vec![]), 20),
            stack(item("mushroom", "Red Cap", "A red forest mushroom. Useful in brews, not for dinner.", it::MUSHROOM, false, vec![]), 20),
            stack(armor(item("wooden_shield", "Wooden Shield", "Blocks some damage. Held in the left hand.", it::SHIELD, false, vec![]), Armor { slot: EquipSlot::Offhand, defense: 1, speed: 0.0, overlay: String::new(), overlay_f: String::new() }), 1),
            stack(item("letter", "Old Letter", "A letter addressed to the elder.", it::LETTER, false, use_trig(vec![dialog("Letter", "\"Rowan, the shrine's flame must never go out. - M.\"")])), 1),
        ],
        props: vec![
            PropDef {
                // Every chest is an (empty) container by default; instances set their contents.
                // Opening it the first time switches it to its "open" state.
                container: Some(ContainerSpec { size: 8, open_state: "open".into(), ..Default::default() }),
                default_state: "closed".into(),
                states: vec![state("open", sprite("props", p::CHEST + 1), true)],
                ..prop("chest", "Chest", p::CHEST, true, [14.0, 8.0])
            },
            prop("sign", "Sign", p::SIGN, true, [12.0, 6.0]),
            PropDef {
                // Swings open (its open image is a separate cell of the sheet) and can be
                // locked again with the key.
                lock: Some(LockSpec {
                    key: "key".into(),
                    consume_key: false,
                    on_unlock: OnUnlock::ToState { state: "open".into() },
                    locked_text: "The gate is locked. Maybe someone in the village has the key.".into(),
                    relock: true,
                    ..Default::default()
                }),
                // Closed, open, or destroyed (e.g. by a "Set prop state" action / set_state()).
                default_state: "closed".into(),
                states: vec![state("open", sprite("props", p::GATE_OPEN), false), state("destroyed", sprite("props", p::GATE_BROKEN), false)],
                ..prop("gate", "Iron Gate", p::GATE, true, [16.0, 16.0])
            },
            PropDef { container: Some(ContainerSpec { size: 4, ..Default::default() }), ..prop("barrel", "Barrel", p::BARREL, true, [10.0, 6.0]) },
            // The forest shrine: 3x2 cells, a monolith between two lamp pillars on a dais.
            PropDef {
                default_state: "dormant".into(),
                states: vec![state("glowing", SpriteRef::block("props", p::GRAND_SHRINE + 3, 3, 2), true)],
                sprite: SpriteRef::block("props", p::GRAND_SHRINE, 3, 2),
                ..prop("shrine", "Shrine", p::GRAND_SHRINE, true, [40.0, 7.0])
            },
            prop("well", "Well", p::WELL, true, [14.0, 8.0]),
            // Furniture (Database › Props › Furniture): sit on chairs and benches, sleep in beds.
            PropDef {
                use_spot: Some(UseSpot { pose: Pose::Sit, offset: [0.0, 2.0], facing: Facing::Down, ..Default::default() }),
                ..prop("chair", "Chair", p::CHAIR, true, [10.0, 4.0])
            },
            // The same chair facing the other ways (one prop per direction): facing up, its
            // backrest is drawn over the sitter (the cover).
            PropDef {
                use_spot: Some(UseSpot { pose: Pose::Sit, offset: [0.0, -8.0], facing: Facing::Up, cover: sprite("props", p::CHAIR_UP_BACK), ..Default::default() }),
                ..prop("chair_up", "Chair (facing up)", p::CHAIR_UP, true, [10.0, 4.0])
            },
            PropDef {
                use_spot: Some(UseSpot { pose: Pose::Sit, offset: [0.0, 2.0], facing: Facing::Left, ..Default::default() }),
                ..prop("chair_left", "Chair (facing left)", p::CHAIR_LEFT, true, [10.0, 4.0])
            },
            PropDef {
                use_spot: Some(UseSpot { pose: Pose::Sit, offset: [0.0, 2.0], facing: Facing::Right, ..Default::default() }),
                ..prop("chair_right", "Chair (facing right)", p::CHAIR_RIGHT, true, [10.0, 4.0])
            },
            PropDef {
                sprite: SpriteRef::block("props", p::BENCH, 2, 1),
                use_spot: Some(UseSpot { pose: Pose::Sit, offset: [0.0, 2.0], facing: Facing::Down, ..Default::default() }),
                ..prop("bench", "Bench", p::BENCH, true, [28.0, 4.0])
            },
            PropDef {
                sprite: SpriteRef::block("props", p::BED, 1, 2),
                // The head on the pillow; the quilt (cover) is drawn over the sleeper.
                use_spot: Some(UseSpot { pose: Pose::Lie, offset: [0.0, -17.0], facing: Facing::Up, sleep_until: 7.0, cover: SpriteRef::block("props", p::BLANKET, 1, 2), ..Default::default() }),
                ..prop("bed", "Bed", p::BED, true, [14.0, 28.0])
            },
            // A sideways bed: the head at the left (the sleeper is drawn turned).
            PropDef {
                sprite: SpriteRef::block("props", p::BED_SIDE, 2, 1),
                use_spot: Some(UseSpot { pose: Pose::Lie, offset: [19.0, -16.0], facing: Facing::Left, sleep_until: 7.0, cover: SpriteRef::block("props", p::BLANKET_SIDE, 2, 1), ..Default::default() }),
                ..prop("bed_side", "Bed (sideways)", p::BED_SIDE, true, [30.0, 12.0])
            },
            // A crafting station: its definition's Interact trigger opens the "alchemy" recipes.
            PropDef {
                triggers: vec![trig(TriggerOn::Interact, vec![], vec![Action::OpenCrafting { station: "alchemy".into(), title: "Alchemy Table".into() }])],
                ..prop("alchemy_table", "Alchemy Table", p::ALCHEMY, true, [26.0, 10.0])
            },
            // Multi-tile props.
            PropDef {
                sprite: SpriteRef::block("props", p::DOUBLE_GATE, 2, 2),
                // No key: it simply swings open and stops blocking.
                // Can be closed again ("relock" without a key).
                default_state: "closed".into(),
                states: vec![state("open", SpriteRef::block("props", p::DOUBLE_GATE + 2, 2, 2), false)],
                lock: Some(LockSpec {
                    key: String::new(),
                    on_unlock: OnUnlock::ToState { state: "open".into() },
                    unlock_text: "The gate swings open.".into(),
                    relock: true,
                    lock_text: "The gate swings shut.".into(),
                    ..Default::default()
                }),
                ..prop("double_gate", "Double Gate", 0, true, [32.0, 10.0])
            },
            PropDef { sprite: SpriteRef::block("props", p::BIG_OAK, 3, 4), ..prop("big_oak", "Big Oak", 0, true, [14.0, 8.0]) },
            PropDef { sprite: SpriteRef::block("props", p::BIG_PINE, 2, 4), ..prop("big_pine", "Big Pine", 0, true, [10.0, 8.0]) },
            PropDef { sprite: SpriteRef::block("props", p::BOULDER, 2, 2), ..prop("boulder", "Boulder", 0, true, [28.0, 12.0]) },
            PropDef { sprite: SpriteRef::block("props", p::STATUE, 1, 2), ..prop("statue", "Statue", 0, true, [12.0, 8.0]) },
            PropDef {
                // Stands in a north-south wall; its hitbox covers three tiles of road.
                sprite: SpriteRef::block("props", p::VGATE, 1, 4),
                // The key is kept, so the gate can be locked again behind you.
                default_state: "closed".into(),
                states: vec![state("open", SpriteRef::block("props", p::VGATE + 1, 1, 4), false)],
                lock: Some(LockSpec {
                    key: "key".into(),
                    consume_key: false,
                    on_unlock: OnUnlock::ToState { state: "open".into() },
                    locked_text: "The gate is locked. Maybe someone in the village has the key.".into(),
                    relock: true,
                    ..Default::default()
                }),
                ..prop("vgate", "Vertical Double Gate", 0, true, [12.0, 48.0])
            },
        ],
        characters: vec![
            hero,
            ch("elder", "Elder", "", 40.0, Behavior::Static),
            ch("villager", "Villager", "", 35.0, Behavior::Wander { radius: 3.0 }),
            // The guard can be hurt (he can come along as a companion: guard_talk()).
            CharacterDef { max_hp: 8, ..ch("guard", "Town Guard", "", 40.0, Behavior::Static) },
            slime,
            goblin,
            goblin_f,
            chief,
            outlaw,
            // Every merchant sells these by default; the one in the village has their own stock.
            CharacterDef {
                shop: Some(ShopSpec {
                    items: vec![ShopEntry { unlimited: true, ..ShopEntry::new("apple", 0, 0) }, ShopEntry::new("potion", 3, 0)],
                    ..Default::default()
                }),
                ..ch("merchant_f", "Merchant (female)", "", 35.0, Behavior::Static)
            },
        ],
        skills: skills(),
        spells: spells(),
        recipes: recipes(),
        achievements: achievements(),
    };
    layered_characters(&mut db);
    // Trade values (merchants use them unless their stock sets a price).
    for (id, v) in [
        ("potion", 15),
        ("bandage", 4),
        ("apple", 3),
        ("bow", 40),
        ("crossbow", 70),
        ("arrows", 1),
        ("bolts", 2),
        ("sword", 30),
        ("spear", 35),
        ("axe", 60),
        ("leather_cap", 20),
        ("iron_helmet", 45),
        ("leather_armor", 35),
        ("chainmail", 90),
        ("bronze_key", 5),
        ("lantern", 25),
        ("torch", 4),
        ("wooden_shield", 18),
        ("scroll_heal", 20),
        ("scroll_light", 12),
        ("spellbook_spark", 60),
        ("tome_fire", 120),
        ("tome_fireball", 150),
        ("tunic_green", 25),
        ("tunic_red", 25),
        ("mana_potion", 12),
        ("antidote", 8),
        ("elixir_strength", 25),
        ("swiftness", 20),
    ] {
        db.items.iter_mut().find(|i| i.id == id).expect(id).value = v;
    }
    // Spear, bow and crossbow need both hands (nothing in the left hand then).
    for id in ["spear", "bow", "crossbow"] {
        db.items.iter_mut().find(|i| i.id == id).and_then(|i| i.weapon.as_mut()).expect(id).two_handed = true;
    }
    // The shrine glows once it is lit.
    if let Some(st) = db.props.iter_mut().find(|p| p.id == "shrine").and_then(|p| p.states.iter_mut().find(|s| s.name == "glowing")) {
        st.light = Some(LightSpec { intensity: 0.9, ..LightSpec::new(6.5, [150, 210, 255], true) });
    }
    db
}

/// The old mine (off the forest): pitch dark except for its wall torches; a treasure room
/// without torches needs a light (a lantern or torch in the left hand).
fn mine() -> Map {
    let (w, h) = (28u32, 20u32);
    let mut b = B::new("mine", w, h);
    let dg = b.m.tileset_slot("dungeon");
    let set = |b: &mut B, layer: usize, x: i32, y: i32, tile: u32| {
        if b.m.in_bounds(x, y) {
            b.m.layers[layer].set(x as u32, y as u32, Tile::new(dg, tile));
        }
    };
    // Solid rock (ceiling) everywhere, then carved rooms and corridors.
    for y in 0..h as i32 {
        for x in 0..w as i32 {
            set(&mut b, 1, x, y, 16);
        }
    }
    let carve = |b: &mut B, x0: i32, y0: i32, cw: i32, ch: i32| {
        for y in y0..y0 + ch {
            for x in x0..x0 + cw {
                set(b, 0, x, y, 0);
                b.m.layers[1].set(x as u32, y as u32, Tile::EMPTY);
            }
        }
        // Wall faces above the carved area.
        for x in x0..x0 + cw {
            if b.m.layers[1].get(x as u32, (y0 - 1) as u32) == Tile::new(dg, 16) {
                set(b, 1, x, y0 - 1, 8);
            }
        }
    };
    carve(&mut b, 3, 3, 20, 5); // the big cavern
    carve(&mut b, 14, 8, 2, 5); // corridor down to the entrance hall
    carve(&mut b, 11, 13, 7, 6); // entrance hall
    carve(&mut b, 4, 8, 2, 3); // corridor to the treasure room
    carve(&mut b, 2, 11, 6, 5); // treasure room (no torches)
    // Entrance: a door in the bottom wall leads back to the forest.
    set(&mut b, 0, 14, 19, 0);
    set(&mut b, 1, 14, 19, 6);
    b.m.set_collision(14, 19, rustpg2d_engine::map::CollisionOverride::Walkable);
    b.portal(14, 19, "forest", "mine");
    b.spawn("entrance", 14, 18);
    // Wall torches (tile lights) in the entrance hall and the cavern.
    for (x, y) in [(11, 12), (17, 12), (6, 2), (13, 2), (20, 2)] {
        set(&mut b, 1, x, y, 10);
    }
    // Rubble, puddles and bones.
    for (x, y, tile) in [(5, 4, 20), (9, 6, 21), (17, 5, 17), (21, 6, 20), (12, 15, 20), (3, 14, 17), (6, 12, 21)] {
        set(&mut b, 0, x, y, tile);
    }
    // Slimes live in the cavern.
    b.zone("mine_slimes", 7, 3, 12, 5, ZoneKind::Spawner { character: "slime".into(), count: 3, respawn: Respawn::OnEnter });
    // The treasure: a chest with a shield, coins and a potion.
    let chest = b.entity(EntityKind::Prop, "chest", 4, 12);
    chest.container = Some(ContainerSpec {
        size: 8,
        items: vec![ItemStack::new("wooden_shield", 1), ItemStack::new("coin", 15), ItemStack::new("potion", 1), ItemStack::new("tome_fire", 1), ItemStack::new("mana_potion", 2)],
        open_state: "open".into(),
        ..Default::default()
    });
    // No key exists for it: only lockpicking 2 opens it.
    chest.lock = Some(LockSpec { pick_difficulty: 2, locked_text: "An old lock with no keyhole worth the name. A skilled lockpick might manage.".into(), ..Default::default() });
    // Opening it unlocks the hidden achievement "Hidden Hoard" (Database › Achievements).
    chest.triggers = vec![trig(TriggerOn::Unlock, vec![], vec![Action::UnlockAchievement { id: "hidden_hoard".into() }])];
    b.m.background = [0, 0, 0];
    b.m.lighting = MapLighting { mode: LightMode::Dark, darkness: 1.0 };
    // Explored regions: the map screen reveals it as the player walks around.
    b.m.fog = true;
    // Auto-tiling (the dungeon tileset's terrains): patches of earth in the cavern and the
    // entrance hall, and a chasm at the cavern's east end.
    let (dterrains, earth_tile, chasm_tile) = {
        let ts = art::extra::dungeon_tileset().0.terrains;
        let (e, c) = (ts[0].tile_for(15).unwrap(), ts[1].tile_for(15).unwrap());
        (ts, e, c)
    };
    for (x, y) in [(6, 5), (7, 5), (7, 6), (8, 5), (6, 6), (12, 4), (13, 4), (13, 5), (12, 16), (13, 16), (14, 16), (13, 17), (14, 17), (15, 17), (13, 15)] {
        set(&mut b, 0, x, y, earth_tile);
    }
    for (x, y) in [(19, 4), (19, 5), (20, 5), (20, 6), (19, 6), (21, 5)] {
        set(&mut b, 0, x, y, chasm_tile);
    }
    let of = |m: &Map, x: i32, y: i32| m.layers[0].get(x as u32, y as u32).parts().filter(|p| p.0 == dg as usize).map(|p| p.1);
    rustpg2d_engine::autotile::retile(
        &mut b.m,
        0,
        dg,
        &dterrains,
        |m, x, y| of(m, x, y).and_then(|i| rustpg2d_engine::autotile::terrain_of(&dterrains, i)),
        |m, k, x, y| of(m, x, y).is_some_and(|i| rustpg2d_engine::autotile::counts_as(&dterrains, k, i)),
    );
    b.m
}

const SCRIPT: &str = r#"// Rhai scripts for the example project.
// Functions here can be called from triggers with the "Run script" action
// or used as conditions ("Script" condition, must return a bool).
//
// World state:  flag(n) set_flag(n, b) get_var(n) set_var(n, v) add_var(n, v)
//               item_count(id) has_item(id) give_item(id, n) take_item(id, n)
// Queued:       say(text) say(speaker, text) choice(text, [options], var)
//               teleport(map, spawn) set_active(target, b) set_frame(target, f)
//               move_by(target, dx, dy) face(target, "up"|"down"|"left"|"right")
//               wait(seconds) emit(event) then_call(fn)
//               heal(n) play_sound(id) play_music(id) set_locked(target, b)
//               open_shop(target) set_state(target, state)
//               patrol(target) patrol(target, [[x, y], ...]) stop_patrol(target)
//               follow(target) stop_following(target) is_companion(name) companion_count()
//               unlock_achievement(id) achievement_unlocked(id) open_crafting(station[, title]) can_craft(recipe)
//               stop_schedule(target) follow_schedule(target)
// Cut-scenes:   cutscene(fn) walk_to(target, x, y) approach(target, other) place(target, x, y)
//               hide_ui() show_ui() dim(amount, seconds) disable_spawners(map) enable_spawners(map)
// Quests:       start_quest(id) advance_quest(id) set_quest_stage(id, n) complete_quest(id)
//               fail_quest(id) (queued); quest_stage(id) (-1 = not started) quest_active(id)
//               quest_done(id)
// Player:       player_name() player_sex() ("male"/"female") is_female() is_male()
//               text("...") fills in placeholders: {player} {sir|madam} {variable}
// Skills/magic: skill(id) skill_points() knows_spell(id) mana(); queued: set_skill(id, level)
//               give_skill_points(n) learn_spell(id) add_mana(n) damage_around(dmg, radius)
//               player_light(radius, seconds, #{ color: [r, g, b], flicker })
//               heal_area(x, y, radius, n) light_at(x, y, radius, seconds, #{ ... })
// Spell effects (queued; x, y in tiles): aim(fn) calls fn(x, y) with the player's target;
//               fly(sheet, x, y, fn, #{ speed, frames: [a, b], fps, scale, light }) calls
//               fn(x, y) on arrival; effect(sheet, x, y, #{ ... }); damage_area(x, y, r, dmg)
// Other:        player_hp() player_max_hp() player_x() player_y() player_facing()
//               random(n) current_map()
// Weather:      set_weather(kind, [intensity]) set_map_weather(map, kind, intensity) (queued;
//               "rain", "snow", "fog", "none", "default"); weather() weather_intensity()
// Targets: "self" (trigger owner), "player", or an entity name.

fn mira_talk() {
    add_var("mira_talks", 1);
    let n = get_var("mira_talks");
    if n == 1 {
        say("Mira", "Hi! I'm Mira. " + player_name() + ", right? Have you met Elder Rowan by the well?");
    } else if flag("shrine_lit") {
        // Scripts can check the player's sex too (or use "{hero|heroine}" in the text).
        if is_female() {
            say("Mira", "You lit the shrine? When I grow up I want to be a heroine like you!");
        } else {
            say("Mira", "You lit the shrine? When I grow up I want to be a hero like you!");
        }
    } else if n < 4 {
        say("Mira", "Talked to me " + n + " times now. You must be bored!");
    } else {
        say("Mira", "Want to see me walk?");
        choice("Watch Mira walk?", ["Sure", "No thanks"], "mira_walk");
        then_call("mira_walk");
    }
}

fn mira_walk() {
    if get_var("mira_walk") == 0 {
        move_by("mira", 2, 0);
        move_by("mira", 0, 1);
        move_by("mira", -2, 0);
        move_by("mira", 0, -1);
        face("mira", "down");
        say("Mira", "Ta-da!");
    }
}

fn shrine_touch() {
    if flag("shrine_lit") {
        say("The shrine glows softly. The forest is calm.");
        return;
    }
    if !flag("quest_accepted") {
        say("An old stone shrine. It seems dormant.");
        return;
    }
    cutscene("shrine_awakening");
}

// The shrine awakens (a cut-scene, the first time it is touched on the elder's quest): the
// clearing darkens, the statues' eyes and the shrine light up, a burst of magic rolls out -
// every monster left in the forest is destroyed and its spawners go quiet. Tilda's slime
// hunt can't go on then: accepted, it is done (she rewards it); not accepted, she won't offer it.
fn shrine_awakening() {
    face("player", "up");
    say("You lay your hand on the cold stone. It begins to hum under your fingers...");
    dim(0.8, 1.5);
    play_sound("chime");
    wait(1.6);
    let blue = #{ color: [120, 200, 255], flicker: true };
    light_at(47.5, 20.5, 1.5, 8, blue);         // a glow in the crystal
    wait(0.8);
    light_at(44.5, 20.2, 2.5, 7, blue);         // the statues' eyes
    wait(0.4);
    light_at(50.5, 20.2, 2.5, 6.6, blue);
    wait(0.6);
    set_state("shrine", "glowing");
    play_sound("chime");
    effect("magic_burst", 47.5, 20.5, #{ scale: 1.5, fps: 14, light: 5, color: [140, 210, 255] });
    wait(0.5);
    effect("magic_burst", 47.5, 20.5, #{ scale: 3.5, fps: 9, light: 9, color: [170, 230, 255] });
    light_at(47.5, 21, 9, 3.5, #{ color: [180, 230, 255] });
    wait(1.0);
    say("Ancient runes flare up! A wave of light rolls out from the shrine, deep into Whisperwood.");
    defeat_enemies();                           // every monster left in the forest
    disable_spawners("forest");                 // and no new ones
    if quest_active("pest_control") && quest_stage("pest_control") == 0 {
        set_quest_stage("pest_control", 1);     // the slimes are gone: back to Tilda
    }
    set_flag("shrine_lit", true);
    set_quest_stage("strange_lights", 2);
    dim(0, 2);
    set_weather("none");                        // the eerie fog lifts (kept in the save game)
    wait(1.5);
    emit("shrine_lit");
}

// Called by the slime's "Defeated" trigger (what slimes drop is set in their loot table).
// A scripted skill effect (Database > Skills > Scavenging, effect "Script"): the engine calls
// scavenging(level) whenever the level changes, and once when a game starts or loads.
fn scavenging(level) {
    set_var("scavenging", level);
}

fn slime_defeated() {
    add_var("slimes_defeated", 1);
    // Scavenging (a scripted skill) adds a coin per level to every slime.
    if get_var("scavenging") > 0 {
        give_item("coin", get_var("scavenging"));
    }
    // Tilda's quest counts slimes once it was accepted.
    if quest_active("pest_control") && quest_stage("pest_control") == 0 {
        add_var("pest_slimes", 1);
        if get_var("pest_slimes") >= 5 {
            set_quest_stage("pest_control", 1);
        }
    }
    if get_var("slimes_defeated") == 5 {
        say("You have defeated 5 slimes! You feel stronger... (not really)");
    }
}

// The town guard patrols the road (see his patrol route in the map inspector). Once the
// shrine is lit he stops: stop_patrol(target) / patrol(target) / patrol(target, [[x, y], ...]).
// The intro of a new game, a cut-scene (village Map enter trigger, once): the player walks
// into town from the west, the guard comes over and sends them to the elder.
fn intro_cutscene() {
    place("player", 0, 18);
    face("player", "right");
    wait(0.8);
    walk_to("player", 5, 18);
    wait(0.4);
    approach("guard", "player");
    say("Guard", "Halt, traveller! ... {player}? Thank the stars, you came!");
    say("{player}", "I came as fast as I could. What is going on?");
    say("Guard", "Something is not right in Whisperwood, east of town. Strange lights flicker between the trees at night, and the slimes grow bolder every day.");
    say("Guard", "Please, talk to Elder Rowan at once - he is waiting by the well in the middle of town.");
    wait(0.3);
}

// Once the player took on the elder's quest, the guard offers to come along: follow(target)
// makes a character a companion (it follows from map to map and fights by the player's side),
// stop_following(target) sends it home; is_companion(name) tells.
fn guard_talk() {
    if is_companion("guard") {
        choice("Shall I go back to my post, {player}?", ["Yes, thank you", "No, stay with me"], "guard_leave");
        then_call("guard_answer");
    } else if flag("shrine_lit") {
        say("Guard", "The lights are gone - I can finally take a rest. Thank you, {player}!");
        stop_patrol("guard");
    } else if flag("quest_accepted") {
        say("Guard", "Whisperwood, is it? You shouldn't go alone - the slimes grow bolder every day.");
        choice("Take the guard along?", ["Come with me", "I'll manage"], "guard_join");
        then_call("guard_answer");
    } else {
        say("Guard", "Halt! Oh, it's you, {player}. Careful in Whisperwood, {sir|ma'am} - strange lights out there.");
    }
}

fn guard_answer() {
    if is_companion("guard") {
        if get_var("guard_leave") == 0 {
            say("Guard", "Back to the road, then. Good luck, {player}!");
            stop_following("guard");
        }
    } else if get_var("guard_join") == 0 {
        say("Guard", "Lead the way, {player}!");
        follow("guard");
    }
}

fn count_coins() {
    say("You have " + item_count("coin") + " coins.");
}

// Fireball, a scripted spell (Database > Spells > Fireball, effect "Script"). Casting it calls
// cast_fireball(); aim() lets the player click a target and calls fireball_at(x, y) with it (in
// tiles). The spell's mana is only spent once a target is chosen.
fn cast_fireball() {
    aim("fireball_at");
}

fn fireball_at(x, y) {
    play_sound("fireball");
    // An animated sprite (all frames of the "fireball" sheet) flies there and glows on the way.
    fly("fireball", x, y, "fireball_hit", #{ speed: 9, fps: 14, light: 2.5 });
}

fn fireball_hit(x, y) {
    play_sound("explosion");
    effect("explosion", x, y, #{ fps: 14, scale: 2.4, light: 5, shake: 0.3 });
    damage_area(x, y, 2, 5);
    // Embers glow on for a few seconds.
    light_at(x, y, 2, 4, #{ color: [255, 120, 50], flicker: true });
}
"#;

fn rgb(v: u32) -> [u8; 3] {
    [(v >> 16) as u8, (v >> 8) as u8, v as u8]
}

/// The example's outfits (clothing items): id, name, look they come from, description.
fn outfits() -> Vec<(&'static str, &'static str, Look, &'static str)> {
    let l = |shirt: u32, pants: u32| Look { shirt, pants, ..Default::default() };
    vec![
        ("tunic_blue", "Blue Tunic", l(0x2e6fdb, 0x3b3b4f), "A simple traveller's tunic."),
        ("tunic_green", "Green Tunic", l(0x2e8b57, 0x4a3a2a), "A sturdy green tunic."),
        ("tunic_red", "Red Tunic", l(0xb03a2e, 0x2f2f3f), "A bright red tunic."),
        ("elder_garb", "Elder's Garb", l(0x7b4fa0, 0x4a3a5a), "A dignified purple coat."),
        ("villager_clothes", "Villager Clothes", l(0x2e9d5b, 0x6b4a2a), "Plain work clothes."),
        ("merchant_dress", "Merchant Dress", Look { robe: Some(0x6a3a7a), apron: Some(0xe8dcc0), ..l(0x7a3a8a, 0x4a3a2a) }, "A long dress with an apron."),
        ("guard_uniform", "Guard Uniform", l(0x2b4c8c, 0x3a3a44), "The blue uniform of the town guard."),
        ("goblin_rags", "Goblin Rags", l(0x5d4037, 0x3e2723), "Smelly, but they cover the essentials."),
    ]
}

/// Characters as layered models (body + hair in their colors, wearing outfit items), the
/// clothing items and sheets they use, held sprites of weapons, shields and lights.
fn layered_characters(db: &mut Database) {
    use SheetKind as K;
    let sheet = |id: &str, kind: K, female: bool| SheetDef { kind, female, ..SheetDef::new(id, &format!("sprites/{id}.png"), 32, 48) };
    // The shared bodies are people: playable (the slime's body is not).
    for (id, _, kind, female) in shared_layers() {
        db.sheets.push(SheetDef { playable: kind == K::Body, ..sheet(&id, kind, female) });
    }
    for (k, (id, name, _, desc)) in outfits().into_iter().enumerate() {
        db.sheets.push(sheet(&format!("outfit_{id}"), K::Outfit, false));
        db.sheets.push(sheet(&format!("outfit_{id}_f"), K::Outfit, false));
        db.items.push(clothing_item(id, name, desc, "outfit_icons", k as u32, id, 0));
    }
    db.sheets.push(SheetDef::new("outfit_icons", "sprites/outfit_icons.png", 32, 32));
    // What weapons, shields and lights look like in a hand.
    for (item, held) in [("sword", "held_sword"), ("spear", "held_spear"), ("axe", "held_axe"), ("bow", "held_bow"), ("crossbow", "held_crossbow"), ("wooden_shield", "held_shield"), ("lantern", "held_lantern"), ("torch", "held_torch"), ("fishing_rod", "held_rod")] {
        db.items.iter_mut().find(|d| d.id == item).expect(item).held = held.into();
    }
    // The goblins' own bow: what they shoot with, dropped when they are defeated.
    let goblin_weapon = db.character("goblin").and_then(|d| d.weapon.clone()).expect("goblin weapon");
    let mut crude = db.item("bow").cloned().expect("bow");
    crude.id = "crude_bow".into();
    crude.name = "Crude Bow".into();
    crude.description = "A goblin's bow: weak and wobbly, but it shoots arrows.".into();
    crude.weapon = Some(Weapon { ranged: goblin_weapon.ranged.map(|r| Ranged { ammo: "arrows".into(), ..r }), ..goblin_weapon });
    crude.value = 2;
    db.items.push(crude);
    let look = |body: &str, skin: u32, hair: &str, hair_color: u32, beard: bool| Appearance {
        body: body.into(),
        skin: rgb(skin),
        hair: hair.into(),
        hair_color: rgb(hair_color),
        beard: if beard { "beard_full".into() } else { String::new() },
    };
    let worn = |slots: &[(EquipSlot, &str)]| {
        let mut e = rustpg2d_engine::state::Equipment::default();
        for (slot, item) in slots {
            *e.slot_mut(*slot) = Some(ItemStack::new(item, 1));
        }
        e
    };
    for d in &mut db.characters {
        let (app, eq): (Appearance, Vec<(EquipSlot, &str)>) = match d.id.as_str() {
            "hero" => (look("body_male", 0xf2c49b, "hair_short", 0x5d3a1a, false), vec![(EquipSlot::Clothing, "tunic_blue")]),
            "elder" => (look("body_male", 0xe8b88a, "hair_short", 0xdedede, true), vec![(EquipSlot::Clothing, "elder_garb")]),
            "villager" => (look("body_male", 0xc68a5e, "hair_short", 0xc0392b, false), vec![(EquipSlot::Clothing, "villager_clothes")]),
            "guard" => (look("body_male", 0xe8b88a, "hair_short", 0x3a2a1e, false), vec![(EquipSlot::Clothing, "guard_uniform"), (EquipSlot::Head, "iron_helmet"), (EquipSlot::Weapon, "spear")]),
            "merchant_f" => (look("body_female", 0xe8b88a, "hair_long", 0x3a2418, false), vec![(EquipSlot::Clothing, "merchant_dress")]),
            "goblin" => (look("body_goblin", 0x7cb342, "hair_short", 0x33691e, false), vec![(EquipSlot::Clothing, "goblin_rags"), (EquipSlot::Weapon, "crude_bow")]),
            "goblin_f" => (look("body_goblin_female", 0x8bc34a, "hair_ponytail", 0x2a4a14, false), vec![(EquipSlot::Clothing, "goblin_rags"), (EquipSlot::Weapon, "crude_bow")]),
            "bandit_chief" => (
                look("body_male", 0xd09068, "hair_long", 0x1e1612, true),
                // Light armor (defense 2) and an axe; all of it drops when he is defeated.
                vec![(EquipSlot::Clothing, "tunic_red"), (EquipSlot::Body, "leather_armor"), (EquipSlot::Head, "leather_cap"), (EquipSlot::Weapon, "axe")],
            ),
            "outlaw" => (look("body_male", 0xe8b88a, "hair_short", 0x7a4a22, false), vec![(EquipSlot::Clothing, "tunic_green"), (EquipSlot::Head, "leather_cap"), (EquipSlot::Weapon, "sword")]),
            _ => continue,
        };
        d.appearance = app;
        d.equipment = worn(&eq);
    }
    // The villager carries a few apples (dropped if anyone ever defeats him).
    if let Some(v) = db.characters.iter_mut().find(|d| d.id == "villager") {
        v.inventory = vec![ItemStack::new("apple", 2)];
    }
}

fn main() {
    let out = PathBuf::from(std::env::args().nth(1).unwrap_or_else(|| "example_project".into()));
    std::fs::create_dir_all(&out).unwrap();
    terrain(&out);
    // Layered characters: bodies in underwear, hair, beards, held items, and the outfits (in
    // both cuts).
    for (id, canvas, _, _) in shared_layers() {
        canvas.save(&out.join(format!("sprites/{id}.png")));
    }
    let outfit_looks: Vec<Look> = outfits().into_iter().map(|o| o.2).collect();
    for (id, _, look, _) in outfits() {
        for (sheet, canvas) in outfit_cuts(id, &look) {
            canvas.save(&out.join(format!("sprites/{sheet}.png")));
        }
    }
    outfit_icons(&outfit_looks).save(&out.join("sprites/outfit_icons.png"));
    overlay(&out, "ov_cap", Gear::Cap);
    overlay(&out, "ov_helmet", Gear::Helmet);
    overlay(&out, "ov_leather", Gear::Leather);
    overlay(&out, "ov_chain", Gear::Chain);
    slime(&out);
    props(&out);
    items(&out);
    fireball(&out);
    explosion(&out);
    magic_burst(&out);
    let (_, png) = art::extra::dungeon_tileset();
    std::fs::write(out.join("tilesets/dungeon.png"), png).unwrap();
    std::fs::create_dir_all(out.join("sounds")).unwrap();
    for (id, make) in SOUNDS {
        std::fs::write(out.join(format!("sounds/{id}.wav")), wav_bytes(&make(), RATE)).unwrap();
    }
    // The music: composed and rendered in music.rs, Ogg Vorbis.
    for (id, title) in music::TRACKS {
        let (l, r) = music::render(id);
        let bytes = music::ogg(id, &l, &r);
        println!("{title}: {:.1} s, {} KiB", l.len() as f32 / music::SR as f32, bytes.len() / 1024);
        std::fs::write(out.join(format!("sounds/{id}.ogg")), bytes).unwrap();
    }

    let project = Project {
        root: out.clone(),
        config: ProjectConfig {
            name: "R(ust)PG2D example".into(),
            tile_size: TS,
            start_map: "village".into(),
            start_spawn: "start".into(),
            scale: 1.5,
            player: "hero".into(),
            sfx: GameSfx {
                pickup: "pickup".into(),
                attack: "swing".into(),
                hit: "hit".into(),
                hurt: "hurt".into(),
                defeat: "defeat".into(),
                dialog: "blip".into(),
                portal: "door".into(),
                open_container: "creak".into(),
                unlock: "unlock".into(),
            },
            inventory_size: 24,
            creation: rustpg2d_engine::project::CharacterCreation {
                enabled: true,
                sex: rustpg2d_engine::project::SexChoice::Both,
                default_name: "Alex".into(),
                // The player picks skin, hair style, hair color and (men) a beard.
                body_male: "body_male".into(),
                body_female: "body_female".into(),
                skin_colors: [0xf2c49b, 0xe8b88a, 0xd09068, 0x8d5a3b, 0x5a3a24, 0x7cb342].map(rgb).to_vec(),
                hair_styles: vec!["hair_short".into(), "hair_long".into(), "hair_ponytail".into(), String::new()],
                hair_colors: [0x5d3a1a, 0xe0c060, 0x1e1612, 0x7a4a22, 0xd35400, 0xdedede].map(rgb).to_vec(),
                beards: vec![String::new(), "beard_full".into(), "beard_goatee".into()],
            },
        day_minutes: 24.0, start_hour: 8.0,
        skills: rustpg2d_engine::skills::SkillSettings { enabled: true, magic: true, start_points: 2, max_mana: 20, mana_regen: 0.5 },
        // Experience and levels: a few levels over the example's quests and fights.
        progression: rustpg2d_engine::progression::Progression { enabled: true, base_xp: 60, growth: 1.4, ..Default::default() },
        // Dialog boxes show the speaker's portrait (found from the speaker's name).
        dialog_portraits: true,
        },
        db: database(),
    };
    project.save_config().unwrap();
    project.save_defs().unwrap();
    for map in [village(), house(), shop(), forest(), wilds(), mine()] {
        project.save_map(&map).unwrap();
        println!("map {:<8} {}x{}", map.name, map.width, map.height);
    }
    std::fs::create_dir_all(project.scripts_dir()).unwrap();
    std::fs::write(project.scripts_dir().join("main.rhai"), SCRIPT).unwrap();
    println!("example project written to {}", out.display());
    write_starter_pack(&project);
}

/// The editor's starter asset library: every asset of the example project. Triggers that run
/// Rhai functions are dropped (they belong to the example's story); skills and spells made of
/// scripts keep the functions they need.
fn write_starter_pack(project: &Project) {
    use rustpg2d_engine::library::{all_assets, Bundle};
    let mut b = Bundle::collect(&project.db, &project.root, TS, "R(ust)PG2D starter assets", &all_assets(&project.db)).unwrap();
    // Library-only extras: more tilesets, props and characters.
    let extra = art::extra::library_extras();
    // (The example project's own version wins when an id exists in both, e.g. "merchant_f".)
    let sheets: Vec<String> = b.db.sheets.iter().map(|d| d.id.clone()).collect();
    let chars: Vec<String> = b.db.characters.iter().map(|d| d.id.clone()).collect();
    let tilesets: Vec<String> = b.db.tilesets.iter().map(|d| d.id.clone()).collect();
    b.db.tilesets.extend(extra.db.tilesets.into_iter().filter(|d| !tilesets.contains(&d.id)));
    b.db.sheets.extend(extra.db.sheets.into_iter().filter(|d| !sheets.contains(&d.id)));
    b.db.props.extend(extra.db.props);
    let items: Vec<String> = b.db.items.iter().map(|d| d.id.clone()).collect();
    b.db.items.extend(extra.db.items.into_iter().filter(|d| !items.contains(&d.id)));
    b.db.characters.extend(extra.db.characters.into_iter().filter(|d| !chars.contains(&d.id)));
    b.db.structures.extend(structures::library_structures());
    for (path, bytes) in extra.files {
        b.files.entry(path).or_insert(bytes);
    }
    let scripted = |t: &Trigger| t.actions.iter().any(|a| matches!(a, Action::RunScript { .. }));
    for d in &mut b.db.items {
        d.on_use.retain(|t| !scripted(t));
    }
    for d in &mut b.db.props {
        d.triggers.retain(|t| !scripted(t));
    }
    for d in &mut b.db.characters {
        d.triggers.retain(|t| !scripted(t));
    }
    // Script functions the remaining assets need travel along (Fireball's cast_fireball() and
    // what it calls, Scavenging's scavenging()); everything else in main.rhai stays behind.
    let fns = rustpg2d_engine::library::read_scripts(&project.root);
    let roots: Vec<String> = all_assets(&b.db).iter().flat_map(|a| rustpg2d_engine::library::script_roots(&b.db, a)).collect();
    let needed = rustpg2d_engine::script_refs::needed_functions(&fns, &roots);
    b.scripts = fns.into_iter().filter(|f| needed.contains(&f.name)).map(|f| (f.name, f.text)).collect();
    println!("starter scripts: {:?}", b.scripts.keys().collect::<Vec<_>>());
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../editor/assets/starter.r2dpack");
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    let bytes = b.to_pack().unwrap();
    std::fs::write(&path, &bytes).unwrap();
    println!("starter asset pack: {} ({} KiB)", path.display(), bytes.len() / 1024);
}
