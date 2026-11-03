//! Renaming ids: updates every reference in definitions, project config and maps.
//!
//! Strings inside Rhai scripts are not rewritten.

use crate::defs::Database;
use crate::entity::EntityKind;
use crate::events::{Action, Cond, Condition, Trigger, TriggerOn};
use crate::map::{Map, ZoneKind};
use crate::project::ProjectConfig;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, serde::Serialize, serde::Deserialize)]
pub enum RefKind {
    Tileset,
    Sheet,
    Item,
    Prop,
    Character,
    Sound,
    Map,
}

struct Renamer<'a> {
    kind: RefKind,
    old: &'a str,
    new: &'a str,
    count: usize,
    /// Collecting mode: records every non-empty reference instead of renaming.
    found: Option<Vec<(RefKind, String)>>,
}

impl Renamer<'_> {
    fn lock(&mut self, l: &mut Option<crate::defs::LockSpec>) {
        if let Some(l) = l {
            self.fix(RefKind::Item, &mut l.key);
        }
    }

    fn container(&mut self, c: &mut Option<crate::defs::ContainerSpec>) {
        if let Some(c) = c {
            for s in &mut c.items {
                self.fix(RefKind::Item, &mut s.item);
            }
            self.conditions(&mut c.conditions);
        }
    }

    fn weapon(&mut self, w: &mut Option<crate::defs::Weapon>) {
        if let Some(r) = w.as_mut().and_then(|w| w.ranged.as_mut()) {
            self.fix(RefKind::Sheet, &mut r.projectile.sheet);
            self.fix(RefKind::Item, &mut r.ammo);
        }
    }

    fn fix(&mut self, kind: RefKind, s: &mut String) {
        if let Some(found) = &mut self.found {
            if !s.is_empty() {
                found.push((kind, s.clone()));
            }
            return;
        }
        if kind == self.kind && s == self.old {
            *s = self.new.to_string();
            self.count += 1;
        }
    }

    fn triggers(&mut self, ts: &mut [Trigger]) {
        for t in ts {
            if let TriggerOn::UseItem { item } = &mut t.on {
                self.fix(RefKind::Item, item);
            }
            self.conditions(&mut t.conditions);
            self.actions(&mut t.actions);
        }
    }

    fn conditions(&mut self, cs: &mut [Condition]) {
        for c in cs {
            if let Cond::HasItem { item, .. } = &mut c.cond {
                self.fix(RefKind::Item, item);
            }
        }
    }

    fn actions(&mut self, acts: &mut [Action]) {
        for a in acts {
            match a {
                Action::GiveItem { item, .. } | Action::TakeItem { item, .. } => self.fix(RefKind::Item, item),
                Action::Teleport { map, .. } => self.fix(RefKind::Map, map),
                Action::PlaySound { sound } | Action::PlayMusic { sound } => self.fix(RefKind::Sound, sound),
                Action::If { conditions, then, otherwise } => {
                    self.conditions(conditions);
                    self.actions(then);
                    self.actions(otherwise);
                }
                _ => {}
            }
        }
    }
}

/// Updates references inside definitions and the project config. Returns the number changed.
/// (The definition's own `id` must be renamed by the caller.)
pub fn rename_in_db(db: &mut Database, config: &mut ProjectConfig, kind: RefKind, old: &str, new: &str) -> usize {
    let mut r = Renamer { kind, old, new, count: 0, found: None };
    for d in &mut db.items {
        r.fix(RefKind::Sheet, &mut d.icon.sheet);
        if let Some(a) = &mut d.armor {
            r.fix(RefKind::Sheet, &mut a.overlay);
        }
        r.triggers(&mut d.on_use);
        r.weapon(&mut d.weapon);
    }
    for d in &mut db.props {
        r.fix(RefKind::Sheet, &mut d.sprite.sheet);
        for st in &mut d.states {
            r.fix(RefKind::Sheet, &mut st.sprite.sheet);
        }
        r.triggers(&mut d.triggers);
        r.container(&mut d.container);
        r.lock(&mut d.lock);
    }
    for d in &mut db.characters {
        r.fix(RefKind::Sheet, &mut d.sheet);
        r.triggers(&mut d.triggers);
        r.weapon(&mut d.weapon);
    }
    r.fix(RefKind::Character, &mut config.player);
    r.fix(RefKind::Map, &mut config.start_map);
    for (_, s) in config.sfx.fields_mut() {
        r.fix(RefKind::Sound, s);
    }
    r.count
}

/// Updates references inside a map. Returns the number changed.
pub fn rename_in_map(map: &mut Map, kind: RefKind, old: &str, new: &str) -> usize {
    let mut r = Renamer { kind, old, new, count: 0, found: None };
    for t in &mut map.tilesets {
        r.fix(RefKind::Tileset, t);
    }
    r.fix(RefKind::Sound, &mut map.music);
    r.triggers(&mut map.triggers);
    for e in &mut map.entities {
        let k = match e.kind {
            EntityKind::Prop => RefKind::Prop,
            EntityKind::Character => RefKind::Character,
            EntityKind::Item => RefKind::Item,
        };
        r.fix(k, &mut e.def);
        r.triggers(&mut e.triggers);
        r.container(&mut e.container);
        r.lock(&mut e.lock);
    }
    for z in &mut map.zones {
        match &mut z.kind {
            ZoneKind::Portal { target_map, .. } => r.fix(RefKind::Map, target_map),
            ZoneKind::Spawner { character, .. } => r.fix(RefKind::Character, character),
            _ => {}
        }
        r.triggers(&mut z.triggers);
    }
    for l in [&mut map.edges.north, &mut map.edges.south, &mut map.edges.east, &mut map.edges.west].into_iter().flatten() {
        r.fix(RefKind::Map, &mut l.target_map);
    }
    if kind == RefKind::Map && map.name == old {
        map.name = new.to_string();
        r.count += 1;
    }
    r.count
}

fn collector() -> Renamer<'static> {
    Renamer { kind: RefKind::Map, old: "", new: "", count: 0, found: Some(Vec::new()) }
}

/// Everything an item definition refers to (icon / overlay / projectile sheets, ammo, items
/// and sounds used by its triggers, ...).
pub fn refs_of_item(d: &crate::defs::ItemDef) -> Vec<(RefKind, String)> {
    let mut d = d.clone();
    let mut r = collector();
    r.fix(RefKind::Sheet, &mut d.icon.sheet);
    if let Some(a) = &mut d.armor {
        r.fix(RefKind::Sheet, &mut a.overlay);
    }
    r.triggers(&mut d.on_use);
    r.weapon(&mut d.weapon);
    r.found.unwrap_or_default()
}

pub fn refs_of_prop(d: &crate::defs::PropDef) -> Vec<(RefKind, String)> {
    let mut d = d.clone();
    let mut r = collector();
    r.fix(RefKind::Sheet, &mut d.sprite.sheet);
    for st in &mut d.states {
        r.fix(RefKind::Sheet, &mut st.sprite.sheet);
    }
    r.triggers(&mut d.triggers);
    r.container(&mut d.container);
    r.lock(&mut d.lock);
    r.found.unwrap_or_default()
}

pub fn refs_of_character(d: &crate::defs::CharacterDef) -> Vec<(RefKind, String)> {
    let mut d = d.clone();
    let mut r = collector();
    r.fix(RefKind::Sheet, &mut d.sheet);
    r.triggers(&mut d.triggers);
    r.weapon(&mut d.weapon);
    r.found.unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::defs::{ItemDef, SpriteRef};
    use crate::entity::EntityInstance;
    use crate::map::{EdgeLink, Zone};

    #[test]
    fn renames_items_everywhere() {
        let mut m = Map::new("village", 4, 4);
        let mut e = EntityInstance::new(1, EntityKind::Item, "key", 0.0, 0.0);
        e.triggers.push(Trigger {
            on: TriggerOn::UseItem { item: "key".into() },
            conditions: vec![Condition::new(Cond::HasItem { item: "key".into(), count: 1 })],
            actions: vec![Action::If {
                conditions: vec![],
                then: vec![Action::TakeItem { item: "key".into(), count: 1 }],
                otherwise: vec![Action::GiveItem { item: "coin".into(), count: 1 }],
            }],
            once: false,
        });
        m.entities.push(e);
        let mut chest = EntityInstance::new(3, EntityKind::Prop, "chest", 0.0, 0.0);
        chest.container = Some(crate::defs::ContainerSpec {
            items: vec![crate::inventory::ItemStack::new("key", 1), crate::inventory::ItemStack::new("coin", 5)],
            ..Default::default()
        });
        m.entities.push(chest);
        // A prop with the same id must not be touched.
        m.entities.push(EntityInstance::new(2, EntityKind::Prop, "key", 0.0, 0.0));
        assert_eq!(rename_in_map(&mut m, RefKind::Item, "key", "gate_key"), 5);
        let items = &m.entities[1].container.as_ref().unwrap().items;
        assert_eq!((items[0].item.as_str(), items[1].item.as_str()), ("gate_key", "coin"));
        assert_eq!(m.entities[0].def, "gate_key");
        assert_eq!(m.entities[2].def, "key");
        let t = &m.entities[0].triggers[0];
        assert_eq!(t.on, TriggerOn::UseItem { item: "gate_key".into() });
        let Action::If { then, otherwise, .. } = &t.actions[0] else { panic!() };
        assert_eq!(then[0], Action::TakeItem { item: "gate_key".into(), count: 1 });
        assert_eq!(otherwise[0], Action::GiveItem { item: "coin".into(), count: 1 });
    }

    #[test]
    fn renames_maps_sheets_and_config() {
        let mut m = Map::new("forest", 4, 4);
        m.edges.west = Some(EdgeLink::new("village"));
        m.zones.push(Zone {
            uid: 1,
            name: "p".into(),
            x: 0,
            y: 0,
            w: 1,
            h: 1,
            kind: ZoneKind::Portal { target_map: "village".into(), target_spawn: "s".into() },
            triggers: vec![],
        });
        assert_eq!(rename_in_map(&mut m, RefKind::Map, "village", "town"), 2);
        assert_eq!(m.edges.west.as_ref().unwrap().target_map, "town");

        let mut db = Database::default();
        db.items.push(ItemDef {
            id: "coin".into(),
            name: "Coin".into(),
            description: String::new(),
            icon: SpriteRef::new("items", 0),
            consumable: false,
            on_use: vec![],
            weapon: None,
            armor: None,
            max_stack: 99,
        });
        let mut cfg = ProjectConfig {
            name: "p".into(),
            tile_size: 32,
            start_map: "village".into(),
            start_spawn: "s".into(),
            scale: 3.0,
            player: "hero".into(),
            sfx: Default::default(),
            inventory_size: 24,
        };
        cfg.sfx.pickup = "blip".into();
        assert_eq!(rename_in_db(&mut db, &mut cfg, RefKind::Sheet, "items", "icons"), 1);
        assert_eq!(db.items[0].icon.sheet, "icons");
        assert_eq!(rename_in_db(&mut db, &mut cfg, RefKind::Map, "village", "town"), 1);
        assert_eq!(cfg.start_map, "town");
        assert_eq!(rename_in_db(&mut db, &mut cfg, RefKind::Sound, "blip", "ding"), 1);
        assert_eq!(cfg.sfx.pickup, "ding");
    }
}
