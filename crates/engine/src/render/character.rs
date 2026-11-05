//! Layered characters: which sheets make up a character (body, clothing, armor, hair, held
//! items) in drawing order. Used by the game and by the editor's previews.
//!
//! Layer order (back to front): held items on the far side → body → clothing → body armor →
//! hair, beard → head armor → held items on the near side. Every layer uses the character
//! layout (4 walk frames per row, rows = facings); the body sheet sets the frame size.

use crate::defs::{Appearance, CharacterDef, Database, EquipSlot};
use crate::entity::{EntityInstance, Facing};
use crate::state::Equipment;

/// Walk frames per row of the character layout.
pub const LAYOUT_FRAMES: u32 = 4;

/// The look of a placed character: its own, else its definition's.
pub fn appearance_of<'a>(def: &'a CharacterDef, inst: Option<&'a EntityInstance>) -> &'a Appearance {
    inst.and_then(|e| e.appearance.as_ref()).unwrap_or(&def.appearance)
}

/// The equipment of a placed character: its own, else its definition's.
pub fn equipment_of<'a>(def: &'a CharacterDef, inst: Option<&'a EntityInstance>) -> &'a Equipment {
    inst.and_then(|e| e.equipment.as_ref()).unwrap_or(&def.equipment)
}

/// One sheet frame to draw (recolored for bodies / hair).
#[derive(Clone, Debug, PartialEq)]
pub struct Layer {
    pub sheet: String,
    pub frame: u32,
    pub skin: Option<[u8; 3]>,
    pub hair: Option<[u8; 3]>,
}

/// Is the item in `slot` on the far side of the body (drawn behind it) when facing `f`? The
/// weapon is in the right hand, the left-hand slot in the left one.
pub fn held_behind(slot: EquipSlot, f: Facing) -> bool {
    match f {
        Facing::Up => true,
        Facing::Down => false,
        Facing::Left => slot == EquipSlot::Weapon,
        Facing::Right => slot == EquipSlot::Offhand,
    }
}

/// Overlay sheet an equipped item adds in `slot` ("" = none): outfit / armor overlays (the
/// female cut on a female body), the held sprite of weapons and left-hand items.
fn overlay_of(db: &Database, eq: &Equipment, slot: EquipSlot, female: bool) -> Option<String> {
    let d = db.item(eq.item(slot)?)?;
    let s = match slot {
        EquipSlot::Weapon | EquipSlot::Offhand => d.held.clone(),
        _ => d.armor.as_ref().filter(|a| a.slot == slot).map(|a| a.overlay_for(female).to_string())?,
    };
    (!s.is_empty()).then_some(s)
}

/// The layers of a character in drawing order. `col` = walk frame; `hide_weapon` while a
/// melee swing shows the weapon instead.
pub fn layers(db: &Database, a: &Appearance, eq: &Equipment, facing: Facing, col: u32, hide_weapon: bool) -> Vec<Layer> {
    let female = db.female_body(&a.body);
    let over = facing as u32 * LAYOUT_FRAMES + col % LAYOUT_FRAMES;
    let plain = |sheet: String| Layer { sheet, frame: over, skin: None, hair: None };
    let held = |behind: bool| {
        [EquipSlot::Weapon, EquipSlot::Offhand]
            .into_iter()
            .filter(move |s| held_behind(*s, facing) == behind && !(hide_weapon && *s == EquipSlot::Weapon))
            .filter_map(|s| overlay_of(db, eq, s, female))
            .map(plain)
    };
    let mut out: Vec<Layer> = held(true).collect();
    out.push(Layer { sheet: a.body.clone(), frame: over, skin: Some(a.skin), hair: None });
    out.extend(overlay_of(db, eq, EquipSlot::Clothing, female).map(plain));
    out.extend(overlay_of(db, eq, EquipSlot::Body, female).map(plain));
    for s in [&a.hair, &a.beard].into_iter().filter(|s| !s.is_empty()) {
        out.push(Layer { sheet: s.clone(), frame: over, skin: None, hair: Some(a.hair_color) });
    }
    out.extend(overlay_of(db, eq, EquipSlot::Head, female).map(plain));
    out.extend(held(false));
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::defs::{Armor, ItemDef, SheetDef, SheetKind};
    use crate::inventory::ItemStack;

    #[test]
    fn hands_by_facing() {
        // Facing down both hands are in front, facing up both behind; sideways the far hand.
        assert!(!held_behind(EquipSlot::Weapon, Facing::Down) && !held_behind(EquipSlot::Offhand, Facing::Down));
        assert!(held_behind(EquipSlot::Weapon, Facing::Up) && held_behind(EquipSlot::Offhand, Facing::Up));
        assert!(held_behind(EquipSlot::Weapon, Facing::Left) && !held_behind(EquipSlot::Offhand, Facing::Left));
        assert!(!held_behind(EquipSlot::Weapon, Facing::Right) && held_behind(EquipSlot::Offhand, Facing::Right));
    }

    #[test]
    fn female_bodies_wear_the_female_cut() {
        let mut db = Database::default();
        db.sheets.push(SheetDef { kind: SheetKind::Body, ..SheetDef::new("body_male", "m.png", 32, 48) });
        db.sheets.push(SheetDef { kind: SheetKind::Body, female: true, ..SheetDef::new("body_female", "f.png", 32, 48) });
        let mut tunic: ItemDef = ron::from_str(r#"(id: "tunic", name: "Tunic", icon: (sheet: "items", frame: 0))"#).unwrap();
        tunic.armor = Some(Armor { slot: EquipSlot::Clothing, defense: 0, speed: 0.0, overlay: "outfit_tunic".into(), overlay_f: "outfit_tunic_f".into() });
        db.items.push(tunic);
        let eq = Equipment { clothing: Some(ItemStack::new("tunic", 1)), ..Default::default() };
        let sheets = |body: &str| {
            let a = Appearance { body: body.into(), hair: "hair_long".into(), ..Default::default() };
            layers(&db, &a, &eq, Facing::Down, 1, false).into_iter().map(|l| l.sheet).collect::<Vec<_>>()
        };
        assert_eq!(sheets("body_male"), ["body_male", "outfit_tunic", "hair_long"]);
        assert_eq!(sheets("body_female"), ["body_female", "outfit_tunic_f", "hair_long"]);
        // Old projects: the pre-drawn sheet becomes the body.
        let mut old = Database::default();
        old.sheets.push(SheetDef::new("hero", "hero.png", 32, 48));
        old.characters.push(ron::from_str(r#"(id: "hero", name: "Hero", sheet: "hero")"#).unwrap());
        old.upgrade_characters();
        assert_eq!(old.characters[0].appearance.body, "hero");
        assert!(old.characters[0].sheet.is_empty());
        assert_eq!(old.sheets[0].kind, SheetKind::Body);
    }
}
