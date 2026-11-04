//! Entities placed on a map: props, characters and item pickups.

use serde::{Deserialize, Serialize};

use crate::events::Trigger;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum EntityKind {
    Prop,
    Character,
    Item,
}

impl EntityKind {
    pub const ALL: [EntityKind; 3] = [EntityKind::Prop, EntityKind::Character, EntityKind::Item];
    pub fn label(self) -> &'static str {
        match self {
            EntityKind::Prop => "Prop",
            EntityKind::Character => "Character",
            EntityKind::Item => "Item",
        }
    }
}

/// Facing direction. The discriminant is the row in character sprite sheets.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Facing {
    #[default]
    Down = 0,
    Left = 1,
    Right = 2,
    Up = 3,
}

impl Facing {
    pub const ALL: [Facing; 4] = [Facing::Down, Facing::Left, Facing::Right, Facing::Up];
    pub fn from_vec(dx: f32, dy: f32) -> Option<Facing> {
        if dx == 0.0 && dy == 0.0 {
            None
        } else if dx.abs() > dy.abs() {
            Some(if dx < 0.0 { Facing::Left } else { Facing::Right })
        } else {
            Some(if dy < 0.0 { Facing::Up } else { Facing::Down })
        }
    }
    pub fn dir(self) -> (f32, f32) {
        match self {
            Facing::Down => (0.0, 1.0),
            Facing::Left => (-1.0, 0.0),
            Facing::Right => (1.0, 0.0),
            Facing::Up => (0.0, -1.0),
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            Facing::Down => "Down",
            Facing::Left => "Left",
            Facing::Right => "Right",
            Facing::Up => "Up",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct EntityInstance {
    pub uid: u32,
    pub kind: EntityKind,
    /// Id of the PropDef / CharacterDef / ItemDef.
    pub def: String,
    /// Optional name, used to address the entity from actions and scripts.
    #[serde(default)]
    pub name: String,
    /// Feet position in pixels (bottom-center of the sprite).
    pub x: f32,
    pub y: f32,
    #[serde(default)]
    pub facing: Facing,
    /// Inactive entities are hidden and not interactable (e.g. spawned later by an event).
    #[serde(default = "yes")]
    pub active: bool,
    /// Legacy prop frame offset (converted to `state` when the map is loaded).
    #[serde(default)]
    pub frame: u32,
    /// State a prop starts in ("" = its definition's default state).
    #[serde(default)]
    pub state: String,
    /// Stack size for item pickups.
    #[serde(default = "one")]
    pub count: u32,
    /// Instance triggers, run in addition to the definition's triggers.
    #[serde(default)]
    pub triggers: Vec<Trigger>,
    /// Overrides the prop definition's container (size, contents, lock).
    #[serde(default)]
    pub container: Option<crate::defs::ContainerSpec>,
    /// Overrides the prop definition's key lock.
    #[serde(default)]
    pub lock: Option<crate::defs::LockSpec>,
    /// Characters: loot table used instead of the definition's (None = definition's).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub loot: Option<Vec<crate::defs::LootEntry>>,
    /// Characters: a route the NPC walks along (instead of its definition's behaviour).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub patrol: Option<PatrolRoute>,
    /// Characters: sprite sheet ("model") used instead of the definition's ("" = definition's).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub sheet: String,
    /// Overrides the character definition's merchant spec (stock, prices).
    #[serde(default)]
    pub shop: Option<crate::defs::ShopSpec>,
    /// Light of this placed entity, instead of its definition's (props: of its current state).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub light: Option<crate::defs::LightSpec>,
}

fn yes() -> bool {
    true
}
fn one() -> u32 {
    1
}

/// How a patrol continues after its last waypoint.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum PatrolMode {
    /// Back to the first waypoint and around again.
    #[default]
    Loop,
    /// Turn around and walk the waypoints in reverse.
    PingPong,
}

impl PatrolMode {
    pub fn label(self) -> &'static str {
        match self {
            PatrolMode::Loop => "Loop",
            PatrolMode::PingPong => "Back and forth",
        }
    }
}

/// A patrol route: tile waypoints a character walks between (finding its way around
/// obstacles), pausing at each one.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PatrolRoute {
    /// Waypoints in tile coordinates.
    pub points: Vec<(i32, i32)>,
    #[serde(default)]
    pub mode: PatrolMode,
    /// Seconds to wait at each waypoint.
    #[serde(default = "default_pause")]
    pub pause: f32,
    /// Walking speed in pixels per second (0 = the character's speed).
    #[serde(default)]
    pub speed: f32,
}

fn default_pause() -> f32 {
    1.0
}

impl Default for PatrolRoute {
    fn default() -> Self {
        PatrolRoute { points: Vec::new(), mode: PatrolMode::Loop, pause: default_pause(), speed: 0.0 }
    }
}

impl PatrolRoute {
    /// The waypoint after `idx` (and the new direction for back-and-forth routes).
    pub fn next(&self, idx: usize, back: bool) -> (usize, bool) {
        let n = self.points.len();
        if n < 2 {
            return (0, false);
        }
        match self.mode {
            PatrolMode::Loop => ((idx + 1) % n, false),
            PatrolMode::PingPong if back && idx == 0 => (1, false),
            PatrolMode::PingPong if back => (idx - 1, true),
            PatrolMode::PingPong if idx + 1 >= n => (n - 2, true),
            PatrolMode::PingPong => (idx + 1, false),
        }
    }
}

impl EntityInstance {
    pub fn new(uid: u32, kind: EntityKind, def: &str, x: f32, y: f32) -> Self {
        EntityInstance {
            uid,
            kind,
            def: def.to_string(),
            name: String::new(),
            x,
            y,
            facing: Facing::Down,
            active: true,
            frame: 0,
            state: String::new(),
            count: 1,
            triggers: Vec::new(),
            container: None,
            lock: None,
            shop: None,
            light: None,
            sheet: String::new(),
            patrol: None,
            loot: None,
        }
    }

    /// Display label for editor lists.
    pub fn label(&self) -> String {
        if self.name.is_empty() {
            format!("#{} {}", self.uid, self.def)
        } else {
            format!("#{} {} ({})", self.uid, self.name, self.def)
        }
    }
}
