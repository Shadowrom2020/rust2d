//! Crafting recipes (`defs/recipes.ron`): ingredients from the player's items become a result
//! item at a crafting station (a name such as "alchemy" or "anvil"; the *Open crafting* action
//! / `open_crafting(station)` opens the window with that station's recipes).

use serde::{Deserialize, Serialize};

use crate::inventory::ItemStack;
use crate::state::WorldState;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RecipeDef {
    pub id: String,
    /// Item made.
    pub result: String,
    /// How many are made at once.
    #[serde(default = "one")]
    pub count: u32,
    /// Items used up.
    #[serde(default)]
    pub ingredients: Vec<ItemStack>,
    /// Where it is made ("" = the recipes of `open_crafting("")`, e.g. a crafting kit).
    #[serde(default)]
    pub station: String,
    /// Skill needed (id, "" = none) and its level.
    #[serde(default)]
    pub skill: String,
    #[serde(default)]
    pub level: u32,
    /// Tool kinds needed (`ItemDef::tool`, e.g. "hammer"), not used up: in the Tool slot, held
    /// or in the inventory.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tools: Vec<String>,
}

/// Item id -> tool kind (`Database::tool_kinds`), to tell which tools the player has.
pub type ToolKinds = std::collections::HashMap<String, String>;

/// Whether the player has a tool of `kind`: worn / held, or in the inventory.
pub fn has_tool(world: &WorldState, kinds: &ToolKinds, kind: &str) -> bool {
    let is = |id: &str| kinds.get(id).is_some_and(|k| k.eq_ignore_ascii_case(kind));
    world.equipment.stacks().any(|s| is(&s.item)) || world.inventory.item_ids().iter().any(|id| is(id))
}

fn one() -> u32 {
    1
}

/// Why a recipe can't be made right now.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Missing {
    /// Ingredients short: (item, have, need).
    Items(Vec<(String, u32, u32)>),
    /// Skill level too low: (skill, have, need).
    Skill(String, u32, u32),
    /// Tools the player doesn't have (tool kinds).
    Tools(Vec<String>),
}

impl RecipeDef {
    pub fn new(id: &str, result: &str, count: u32, ingredients: Vec<ItemStack>, station: &str) -> Self {
        RecipeDef { id: id.into(), result: result.into(), count, ingredients, station: station.into(), skill: String::new(), level: 0, tools: Vec::new() }
    }

    /// What is missing to make it once (None = it can be made).
    pub fn missing(&self, world: &WorldState, kinds: &ToolKinds) -> Option<Missing> {
        if !self.skill.is_empty() && world.skill_level(&self.skill) < self.level {
            return Some(Missing::Skill(self.skill.clone(), world.skill_level(&self.skill), self.level));
        }
        let tools: Vec<String> = self.tools.iter().filter(|t| !t.trim().is_empty() && !has_tool(world, kinds, t)).cloned().collect();
        if !tools.is_empty() {
            return Some(Missing::Tools(tools));
        }
        let short: Vec<(String, u32, u32)> = self
            .ingredients
            .iter()
            .filter(|s| !s.item.is_empty())
            .map(|s| (s.item.clone(), world.item_count(&s.item), s.count))
            .filter(|(_, have, need)| have < need)
            .collect();
        (!short.is_empty()).then_some(Missing::Items(short))
    }

    /// Makes it once: takes the ingredients, gives the result. Returns how many of the result
    /// didn't fit into the inventory (they are lost: callers check `fits` first), or what is
    /// missing.
    pub fn craft(&self, world: &mut WorldState, kinds: &ToolKinds) -> Result<u32, Missing> {
        if let Some(m) = self.missing(world, kinds) {
            return Err(m);
        }
        for s in self.ingredients.iter().filter(|s| !s.item.is_empty()) {
            world.take_item(&s.item, s.count);
        }
        Ok(world.give_item(&self.result, self.count.max(1)))
    }

    /// Whether the result would fit into the inventory once the ingredients are used up.
    pub fn fits(&self, world: &WorldState) -> bool {
        let mut w = WorldState { inventory: world.inventory.clone(), equipment: world.equipment.clone(), ..Default::default() };
        for s in self.ingredients.iter().filter(|s| !s.item.is_empty()) {
            w.take_item(&s.item, s.count);
        }
        w.give_item(&self.result, self.count.max(1)) == 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crafting_uses_ingredients() {
        let r = RecipeDef::new("potion", "potion", 2, vec![ItemStack::new("herb", 2), ItemStack::new("apple", 1)], "alchemy");
        let mut w = WorldState::default();
        w.give_item("herb", 3);
        assert_eq!(r.missing(&w, &ToolKinds::new()), Some(Missing::Items(vec![("apple".into(), 0, 1)])));
        assert!(r.craft(&mut w, &ToolKinds::new()).is_err());
        assert_eq!(w.item_count("herb"), 3, "nothing used when it fails");
        w.give_item("apple", 1);
        assert!(r.fits(&w));
        assert_eq!(r.craft(&mut w, &ToolKinds::new()), Ok(0));
        assert_eq!((w.item_count("herb"), w.item_count("apple"), w.item_count("potion")), (1, 0, 2));
        let skilled = RecipeDef { skill: "alchemy".into(), level: 2, ..r };
        assert_eq!(skilled.missing(&w, &ToolKinds::new()), Some(Missing::Skill("alchemy".into(), 0, 2)));
    }

    #[test]
    fn tools_are_needed_not_used() {
        let r = RecipeDef { tools: vec!["hammer".into()], ..RecipeDef::new("blade", "sword", 1, vec![ItemStack::new("ingot", 1)], "anvil") };
        let kinds: ToolKinds = [("smith_hammer".to_string(), "hammer".to_string())].into_iter().collect();
        let mut w = WorldState::default();
        w.give_item("ingot", 2);
        assert_eq!(r.missing(&w, &kinds), Some(Missing::Tools(vec!["hammer".into()])));
        w.give_item("smith_hammer", 1);
        assert_eq!(r.craft(&mut w, &kinds), Ok(0));
        assert_eq!((w.item_count("smith_hammer"), w.item_count("ingot")), (1, 1), "the tool stays");
        // Equipped counts too.
        w.take_item("smith_hammer", 1);
        *w.equipment.slot_mut(crate::defs::EquipSlot::Tool) = Some(ItemStack::new("smith_hammer", 1));
        assert!(has_tool(&w, &kinds, "Hammer"));
    }

    #[test]
    fn minimal_recipe_in_ron() {
        let r: RecipeDef = ron::from_str("(id: \"a\", result: \"b\")").unwrap();
        assert_eq!((r.count, r.ingredients.len(), r.station.as_str(), r.level), (1, 0, "", 0));
    }
}
