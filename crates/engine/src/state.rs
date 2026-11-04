//! Persistent game state: flags, variables, inventory and per-map entity changes.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use crate::entity::Facing;
use crate::defs::EquipSlot;
use crate::inventory::{Inventory, ItemStack};

/// Items worn / held by the player (not part of the inventory grid).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Equipment {
    pub head: Option<ItemStack>,
    pub body: Option<ItemStack>,
    pub weapon: Option<ItemStack>,
    /// Left hand: shield or light.
    pub offhand: Option<ItemStack>,
}

impl Equipment {
    pub fn get(&self, slot: EquipSlot) -> Option<&ItemStack> {
        match slot {
            EquipSlot::Head => self.head.as_ref(),
            EquipSlot::Body => self.body.as_ref(),
            EquipSlot::Weapon => self.weapon.as_ref(),
            EquipSlot::Offhand => self.offhand.as_ref(),
        }
    }
    pub fn slot_mut(&mut self, slot: EquipSlot) -> &mut Option<ItemStack> {
        match slot {
            EquipSlot::Head => &mut self.head,
            EquipSlot::Body => &mut self.body,
            EquipSlot::Weapon => &mut self.weapon,
            EquipSlot::Offhand => &mut self.offhand,
        }
    }
    pub fn item(&self, slot: EquipSlot) -> Option<&str> {
        self.get(slot).map(|s| s.item.as_str())
    }
    pub fn count(&self, item: &str) -> u32 {
        EquipSlot::ALL.iter().filter_map(|s| self.get(*s)).filter(|s| s.item == item).map(|s| s.count).sum()
    }
}

/// Runtime changes to a placed entity, persisted across map changes and saves.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct EntityState {
    pub active: bool,
    pub frame: u32,
    pub x: f32,
    pub y: f32,
    pub facing: Facing,
    /// Remaining stack size of a partly picked-up item.
    #[serde(default)]
    pub count: Option<u32>,
    /// Its lock has been opened.
    #[serde(default)]
    pub unlocked: bool,
    /// Current state of a prop (None in saves from before named states: `frame` is used).
    #[serde(default)]
    pub state: Option<String>,
    /// Where a patrolling character is on its route.
    #[serde(default)]
    pub patrol: Option<PatrolState>,
}

/// Progress of a patrolling character (saved, so a loaded game continues the route).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PatrolState {
    /// Waypoint it is heading for.
    pub index: u32,
    /// Walking the route backwards (back-and-forth routes).
    #[serde(default)]
    pub back: bool,
    /// Stopped by an action / script.
    #[serde(default)]
    pub stopped: bool,
    /// A route given by an action / script instead of the placed character's own.
    #[serde(default)]
    pub route: Option<crate::entity::PatrolRoute>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct WorldState {
    pub flags: BTreeSet<String>,
    pub vars: BTreeMap<String, i64>,
    pub inventory: Inventory,
    /// map name -> entity uid -> state
    pub entities: BTreeMap<String, BTreeMap<u32, EntityState>>,
    /// Keys of `once` triggers that already fired.
    pub fired: BTreeSet<String>,
    pub current_map: String,
    pub player_pos: (f32, f32),
    pub player_facing: Facing,
    /// `None` until the game starts (then set to the player's max HP).
    #[serde(default)]
    pub player_hp: Option<i32>,
    /// Old saves only: item id of the equipped weapon (migrated into `equipment`).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub equipped: String,
    #[serde(default)]
    pub equipment: Equipment,
    /// Quick action bar: item ids per slot ("" = empty).
    #[serde(default)]
    pub quickbar: Vec<String>,
    /// Seconds since the Unix epoch when this was saved.
    #[serde(default)]
    pub saved_at: u64,
    /// Total play time in seconds.
    #[serde(default)]
    pub play_time: f64,
    /// Time of day in hours (0..24), advanced while playing.
    #[serde(default = "crate::project::default_start_hour")]
    pub hour: f32,
    /// map -> spawner zone uid -> characters defeated for good (`Respawn::Never`).
    #[serde(default)]
    pub spawner_kills: BTreeMap<String, BTreeMap<u32, u32>>,
    /// map -> entity uid -> contents of containers that have been opened.
    #[serde(default)]
    pub containers: BTreeMap<String, BTreeMap<u32, Inventory>>,
    /// map -> entity uid -> current stock of merchants the player traded with.
    #[serde(default)]
    pub shops: BTreeMap<String, BTreeMap<u32, Vec<crate::defs::ShopEntry>>>,
    /// quest id -> progress (started quests only).
    #[serde(default)]
    pub quests: BTreeMap<String, crate::quest::QuestProgress>,
    /// The player character's name, sex and look (sprite sheet; "" = the player
    /// character definition's), chosen in the character creator.
    #[serde(default)]
    pub player_name: String,
    #[serde(default)]
    pub player_sex: Sex,
    #[serde(default)]
    pub player_look: String,
    /// map -> items the player dropped there (they stay until picked up again).
    #[serde(default)]
    pub dropped: BTreeMap<String, Vec<DroppedItem>>,
    /// Counter for the uids of dropped items (see [`DROPPED_UID_BASE`]).
    #[serde(default)]
    pub next_drop: u32,
}

/// Uids of dropped items start here, far above the uids of map entities and of spawned
/// characters (`0x4000_0000..`).
pub const DROPPED_UID_BASE: u32 = 0x6000_0000;

/// An item stack the player dropped on a map.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DroppedItem {
    pub uid: u32,
    pub item: String,
    pub count: u32,
    /// Feet position in pixels.
    pub x: f32,
    pub y: f32,
}

/// The player character's sex (for dialogs, conditions and scripts).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Sex {
    #[default]
    Male,
    Female,
}

impl Sex {
    pub const ALL: [Sex; 2] = [Sex::Male, Sex::Female];
    pub fn label(self) -> &'static str {
        match self {
            Sex::Male => "Male",
            Sex::Female => "Female",
        }
    }
    /// "male" / "female" (scripts).
    pub fn key(self) -> &'static str {
        match self {
            Sex::Male => "male",
            Sex::Female => "female",
        }
    }
}

impl WorldState {
    /// Fills in text placeholders (dialogs, choices, quest texts):
    /// `{player}` = the player's name, `{his|her}` = the first word for a male player, the
    /// second for a female one, `{name}` = the value of variable `name`. Anything else in
    /// braces is kept as it is.
    pub fn expand_text(&self, text: &str) -> String {
        let mut out = String::with_capacity(text.len());
        let mut rest = text;
        while let Some(i) = rest.find('{') {
            out.push_str(&rest[..i]);
            let after = &rest[i + 1..];
            let Some(j) = after.find('}') else {
                out.push_str(&rest[i..]);
                return out;
            };
            let inner = &after[..j];
            let ident = !inner.is_empty() && inner.chars().all(|c| c.is_alphanumeric() || c == '_');
            if inner == "player" {
                out.push_str(&self.player_name);
            } else if inner.contains('|') && !inner.contains('{') {
                let mut parts = inner.splitn(2, '|');
                let (m, f) = (parts.next().unwrap_or(""), parts.next().unwrap_or(""));
                out.push_str(if self.player_sex == Sex::Female { f } else { m });
            } else if ident {
                out.push_str(&self.var(inner).to_string());
            } else {
                out.push('{');
                rest = after;
                continue;
            }
            rest = &after[j + 1..];
        }
        out.push_str(rest);
        out
    }

    /// Puts dropped items on a map: onto a pile of the same item within `merge` pixels, else
    /// as a new pile. Returns the pile's uid and whether it is new.
    pub fn add_dropped(&mut self, map: &str, item: &str, count: u32, x: f32, y: f32, merge: f32) -> (u32, bool) {
        let piles = self.dropped.entry(map.to_string()).or_default();
        if let Some(p) = piles.iter_mut().find(|p| p.item == item && (p.x - x).hypot(p.y - y) <= merge) {
            p.count += count;
            return (p.uid, false);
        }
        let uid = DROPPED_UID_BASE + self.next_drop;
        self.next_drop += 1;
        piles.push(DroppedItem { uid, item: item.to_string(), count, x, y });
        (uid, true)
    }

    /// Saves from 0.0.5 numbered dropped piles from 0x4000_0000, the range of spawned
    /// characters: give such piles uids in the current range.
    pub fn renumber_old_drops(&mut self) {
        let mut next = self.next_drop;
        for p in self.dropped.values_mut().flatten() {
            if p.uid < DROPPED_UID_BASE {
                p.uid = DROPPED_UID_BASE + next;
                next += 1;
            }
        }
        self.next_drop = next;
    }

    /// Updates the size of a dropped pile (0 removes it).
    pub fn set_dropped_count(&mut self, map: &str, uid: u32, count: u32) {
        if let Some(piles) = self.dropped.get_mut(map) {
            if count == 0 {
                piles.retain(|p| p.uid != uid);
            } else if let Some(p) = piles.iter_mut().find(|p| p.uid == uid) {
                p.count = count;
            }
            if piles.is_empty() {
                self.dropped.remove(map);
            }
        }
    }

    pub fn flag(&self, name: &str) -> bool {
        self.flags.contains(name)
    }
    pub fn set_flag(&mut self, name: &str, value: bool) {
        if value {
            self.flags.insert(name.to_string());
        } else {
            self.flags.remove(name);
        }
    }
    pub fn var(&self, name: &str) -> i64 {
        self.vars.get(name).copied().unwrap_or(0)
    }
    pub fn set_var(&mut self, name: &str, v: i64) {
        self.vars.insert(name.to_string(), v);
    }
    /// Items in the inventory plus equipped ones.
    pub fn item_count(&self, item: &str) -> u32 {
        self.inventory.count(item) + self.equipment.count(item)
    }
    /// Adds items to the player's inventory; returns how many did not fit.
    pub fn give_item(&mut self, item: &str, count: u32) -> u32 {
        self.inventory.add(item, count)
    }
    /// Removes up to `count` items; returns how many were removed.
    /// Takes from the inventory first, then from equipped items.
    pub fn take_item(&mut self, item: &str, count: u32) -> u32 {
        let mut taken = self.inventory.remove(item, count);
        for slot in EquipSlot::ALL {
            if taken == count {
                break;
            }
            let s = self.equipment.slot_mut(slot);
            if let Some(st) = s.as_mut().filter(|st| st.item == item) {
                let n = (count - taken).min(st.count);
                st.count -= n;
                taken += n;
                if st.count == 0 {
                    *s = None;
                }
            }
        }
        taken
    }

    pub fn entity_state(&self, map: &str, uid: u32) -> Option<&EntityState> {
        self.entities.get(map)?.get(&uid)
    }
    pub fn set_entity_state(&mut self, map: &str, uid: u32, st: EntityState) {
        self.entities.entry(map.to_string()).or_default().insert(uid, st);
    }

    pub fn to_ron(&self) -> anyhow::Result<String> {
        Ok(ron::ser::to_string_pretty(self, ron::ser::PrettyConfig::default())?)
    }
    pub fn from_ron(s: &str) -> anyhow::Result<Self> {
        Ok(ron::from_str(s)?)
    }
}

#[cfg(test)]
mod tests {

    #[test]
    fn dropped_piles() {
        let mut w = WorldState::default();
        let (a, new_a) = w.add_dropped("village", "apple", 2, 100.0, 100.0, 16.0);
        // Same item close by joins the pile; another item or far away makes a new one.
        assert_eq!(w.add_dropped("village", "apple", 3, 105.0, 100.0, 16.0), (a, false));
        let (b, new_b) = w.add_dropped("village", "sword", 1, 100.0, 100.0, 16.0);
        assert!(new_a && new_b && a != b && a >= DROPPED_UID_BASE);
        assert_eq!(w.dropped["village"][0].count, 5);
        w.set_dropped_count("village", a, 1);
        assert_eq!(w.dropped["village"][0].count, 1);
        w.set_dropped_count("village", a, 0);
        w.set_dropped_count("village", b, 0);
        assert!(!w.dropped.contains_key("village"));
        // Piles of old saves move out of the spawned characters' uid range.
        let mut old = WorldState::default();
        old.dropped.insert("m".into(), vec![DroppedItem { uid: 0x4000_0000, item: "coin".into(), count: 1, x: 0.0, y: 0.0 }]);
        old.renumber_old_drops();
        assert_eq!((old.dropped["m"][0].uid, old.next_drop), (DROPPED_UID_BASE, 1));
        // Piles survive a save / load round trip.
        w.add_dropped("forest", "coin", 7, 1.0, 2.0, 16.0);
        let back: WorldState = ron::from_str(&ron::to_string(&w).unwrap()).unwrap();
        assert_eq!(back.dropped["forest"][0].count, 7);
        assert_eq!(back.next_drop, w.next_drop);
    }

    use super::*;

    #[test]
    fn equipment_counts_and_takes() {
        let mut w = WorldState::default();
        w.give_item("sword", 1);
        w.equipment.weapon = Some(ItemStack::new("sword", 1));
        assert_eq!(w.item_count("sword"), 2);
        assert_eq!(w.take_item("sword", 2), 2);
        assert!(w.equipment.weapon.is_none() && w.inventory.is_empty());
        // Old saves without `equipment` still load; `equipped` is kept for migration.
        let old: WorldState = ron::from_str(r#"(flags: [], vars: {}, inventory: {"bow": 1}, entities: {}, fired: [], current_map: "m", player_pos: (0.0, 0.0), player_facing: Down, equipped: "bow")"#).unwrap();
        assert_eq!((old.equipped.as_str(), old.equipment.weapon.is_none()), ("bow", true));
        assert!(old.quickbar.is_empty());
        let w = WorldState { quickbar: vec!["potion".into(), String::new()], ..Default::default() };
        let back = WorldState::from_ron(&w.to_ron().unwrap()).unwrap();
        assert_eq!(back.quickbar, w.quickbar);
    }

    #[test]
    fn inventory_and_save_roundtrip() {
        let mut w = WorldState::default();
        w.give_item("coin", 3);
        assert_eq!(w.take_item("coin", 5), 3);
        assert_eq!(w.item_count("coin"), 0);
        assert!(w.inventory.is_empty());
        w.set_flag("a", true);
        w.set_var("x", -4);
        w.set_entity_state(
            "village",
            7,
            EntityState { active: false, frame: 1, x: 1.0, y: 2.0, facing: Facing::Up, count: None, unlocked: true, state: Some("open".into()), patrol: None },
        );
        let back = WorldState::from_ron(&w.to_ron().unwrap()).unwrap();
        assert!(back.flag("a"));
        assert_eq!(back.var("x"), -4);
        assert_eq!(back.entity_state("village", 7).unwrap().frame, 1);
    }
}
