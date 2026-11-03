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
    /// Characters: sprite sheet ("model") used instead of the definition's ("" = definition's).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub sheet: String,
    /// Overrides the character definition's merchant spec (stock, prices).
    #[serde(default)]
    pub shop: Option<crate::defs::ShopSpec>,
}

fn yes() -> bool {
    true
}
fn one() -> u32 {
    1
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
            sheet: String::new(),
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
