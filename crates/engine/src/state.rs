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
}

impl Equipment {
    pub fn get(&self, slot: EquipSlot) -> Option<&ItemStack> {
        match slot {
            EquipSlot::Head => self.head.as_ref(),
            EquipSlot::Body => self.body.as_ref(),
            EquipSlot::Weapon => self.weapon.as_ref(),
        }
    }
    pub fn slot_mut(&mut self, slot: EquipSlot) -> &mut Option<ItemStack> {
        match slot {
            EquipSlot::Head => &mut self.head,
            EquipSlot::Body => &mut self.body,
            EquipSlot::Weapon => &mut self.weapon,
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
    /// map -> spawner zone uid -> characters defeated for good (`Respawn::Never`).
    #[serde(default)]
    pub spawner_kills: BTreeMap<String, BTreeMap<u32, u32>>,
    /// map -> entity uid -> contents of containers that have been opened.
    #[serde(default)]
    pub containers: BTreeMap<String, BTreeMap<u32, Inventory>>,
}

impl WorldState {
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
            EntityState { active: false, frame: 1, x: 1.0, y: 2.0, facing: Facing::Up, count: None, unlocked: true, state: Some("open".into()) },
        );
        let back = WorldState::from_ron(&w.to_ron().unwrap()).unwrap();
        assert!(back.flag("a"));
        assert_eq!(back.var("x"), -4);
        assert_eq!(back.entity_state("village", 7).unwrap().frame, 1);
    }
}
