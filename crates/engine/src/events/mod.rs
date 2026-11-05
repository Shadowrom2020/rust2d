//! Event-driven interactions.
//!
//! Gameplay emits [`GameEvent`]s. Entities, zones, maps and items own [`Trigger`]s
//! that react to matching events: if all [`Condition`]s hold, the trigger's
//! [`Action`]s are queued on the [`runner::ActionRunner`], which executes them in
//! order (some, like dialogs, block the sequence until finished).

pub mod runner;

use serde::{Deserialize, Serialize};

use crate::entity::Facing;
use crate::state::WorldState;

/// Something that happened in the game.
#[derive(Clone, Debug, PartialEq)]
pub enum GameEvent {
    /// Player pressed the interact key facing an entity or standing in a zone.
    Interact,
    /// Player entered / left a zone.
    Enter,
    Leave,
    /// Player used an item (on the entity in front of them, or on nothing).
    UseItem { item: String },
    /// The map finished loading and the player was placed.
    MapEnter,
    /// User-defined event raised by an `Emit` action or `emit()` in scripts.
    Custom { name: String },
    /// An entity's hit points dropped to zero.
    Defeat,
    /// A prop's lock was opened.
    Unlock,
    /// An opened prop was locked again.
    Lock,
}

/// Which events a trigger reacts to.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum TriggerOn {
    Interact,
    Enter,
    Leave,
    /// `item` empty = any item.
    UseItem { item: String },
    MapEnter,
    Custom { name: String },
    /// The owning character was defeated in combat.
    Defeat,
    /// The owning prop's lock was opened with its key (or by a Set locked action).
    Unlock,
    /// The owning prop was locked again (by the player with its key, or by a Set locked action).
    Lock,
}

/// Where a trigger lives. The game only sends some events to each kind of owner (see
/// `runtime::Game::collect`), so only those make sense in its trigger list.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TriggerOwner {
    /// Triggers of a map (not of an object on it).
    Map,
    Zone,
    /// A placed character or a character definition.
    Character,
    /// A placed prop or a prop definition.
    Prop,
    /// An item pickup placed on a map.
    Pickup,
    /// An item definition's "on use" triggers.
    ItemUse,
}

impl TriggerOwner {
    /// Whether events of this kind ever reach a trigger of this owner.
    pub fn allows(self, on: &TriggerOn) -> bool {
        use TriggerOn as T;
        use TriggerOwner as O;
        match (self, on) {
            // emit() / "Emit event" reach the map, its zones and its entities.
            (O::Map | O::Zone | O::Character | O::Prop | O::Pickup, T::Custom { .. }) => true,
            (O::Map, T::MapEnter) => true,
            (O::Zone, T::Enter | T::Leave | T::Interact) => true,
            (O::Character, T::Interact | T::UseItem { .. } | T::Defeat) => true,
            (O::Prop, T::Interact | T::UseItem { .. } | T::Unlock | T::Lock) => true,
            (O::Pickup, T::Interact | T::UseItem { .. }) => true,
            (O::ItemUse, T::UseItem { .. }) => true,
            _ => false,
        }
    }

    /// The events offered for this owner (first = default for new triggers).
    pub fn templates(self) -> Vec<TriggerOn> {
        let mut v: Vec<TriggerOn> = TriggerOn::templates().into_iter().filter(|t| self.allows(t)).collect();
        // The most likely event first.
        let first = match self {
            TriggerOwner::Map => TriggerOn::MapEnter,
            TriggerOwner::Zone => TriggerOn::Enter,
            TriggerOwner::ItemUse => TriggerOn::UseItem { item: String::new() },
            _ => TriggerOn::Interact,
        };
        if let Some(k) = v.iter().position(|t| *t == first) {
            let t = v.remove(k);
            v.insert(0, t);
        }
        v
    }

    /// What to use instead of an event that never reaches this owner.
    pub fn replacement(self, on: &TriggerOn) -> TriggerOn {
        match (self, on) {
            (TriggerOwner::Map, _) => TriggerOn::MapEnter,
            (TriggerOwner::Zone, TriggerOn::MapEnter) => TriggerOn::Enter,
            _ => self.templates().remove(0),
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            TriggerOwner::Map => "map triggers",
            TriggerOwner::Zone => "zone triggers",
            TriggerOwner::Character => "character triggers",
            TriggerOwner::Prop => "prop triggers",
            TriggerOwner::Pickup => "item pickup triggers",
            TriggerOwner::ItemUse => "item use triggers",
        }
    }
}

impl TriggerOn {
    pub fn matches(&self, ev: &GameEvent) -> bool {
        match (self, ev) {
            (TriggerOn::Interact, GameEvent::Interact)
            | (TriggerOn::Enter, GameEvent::Enter)
            | (TriggerOn::Leave, GameEvent::Leave)
            | (TriggerOn::MapEnter, GameEvent::MapEnter)
            | (TriggerOn::Defeat, GameEvent::Defeat)
            | (TriggerOn::Unlock, GameEvent::Unlock)
            | (TriggerOn::Lock, GameEvent::Lock) => true,
            (TriggerOn::UseItem { item: want }, GameEvent::UseItem { item }) => {
                want.is_empty() || want == item
            }
            (TriggerOn::Custom { name: want }, GameEvent::Custom { name }) => want == name,
            _ => false,
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            TriggerOn::Interact => "Interact",
            TriggerOn::Enter => "Enter",
            TriggerOn::Leave => "Leave",
            TriggerOn::UseItem { .. } => "Use item",
            TriggerOn::MapEnter => "Map enter",
            TriggerOn::Custom { .. } => "Custom event",
            TriggerOn::Defeat => "Defeated",
            TriggerOn::Unlock => "Unlocked",
            TriggerOn::Lock => "Locked again",
        }
    }

    pub fn templates() -> Vec<TriggerOn> {
        vec![
            TriggerOn::Interact,
            TriggerOn::Enter,
            TriggerOn::Leave,
            TriggerOn::UseItem { item: String::new() },
            TriggerOn::MapEnter,
            TriggerOn::Custom { name: String::new() },
            TriggerOn::Defeat,
            TriggerOn::Unlock,
            TriggerOn::Lock,
        ]
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Trigger {
    pub on: TriggerOn,
    #[serde(default)]
    pub conditions: Vec<Condition>,
    #[serde(default)]
    pub actions: Vec<Action>,
    /// Fire at most once per save game.
    #[serde(default)]
    pub once: bool,
}

impl Trigger {
    pub fn new(on: TriggerOn) -> Self {
        Trigger { on, conditions: Vec::new(), actions: Vec::new(), once: false }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum CmpOp {
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
}

impl CmpOp {
    pub const ALL: [CmpOp; 6] = [CmpOp::Eq, CmpOp::Ne, CmpOp::Lt, CmpOp::Le, CmpOp::Gt, CmpOp::Ge];
    pub fn apply(self, a: i64, b: i64) -> bool {
        match self {
            CmpOp::Eq => a == b,
            CmpOp::Ne => a != b,
            CmpOp::Lt => a < b,
            CmpOp::Le => a <= b,
            CmpOp::Gt => a > b,
            CmpOp::Ge => a >= b,
        }
    }
    pub fn symbol(self) -> &'static str {
        match self {
            CmpOp::Eq => "==",
            CmpOp::Ne => "!=",
            CmpOp::Lt => "<",
            CmpOp::Le => "<=",
            CmpOp::Gt => ">",
            CmpOp::Ge => ">=",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Cond {
    Flag { name: String },
    Var { name: String, op: CmpOp, value: i64 },
    HasItem { item: String, count: u32 },
    /// Calls a Rhai function returning `bool`.
    Script { function: String },
    PlayerHp { op: CmpOp, value: i64 },
    /// A prop is in the given state.
    PropState { target: TargetName, state: String },
    /// A quest's status / stage (`stage` is used by the stage checks).
    Quest { quest: String, check: crate::quest::QuestCheck, stage: u32 },
    /// The player character is male / female.
    PlayerSex { sex: crate::state::Sex },
    /// The time of day is between `from` and `to` hours (wraps around midnight, e.g. 20 to 6).
    TimeBetween { from: f32, to: f32 },
    /// A skill's level compared with a value.
    Skill { skill: String, op: CmpOp, level: i64 },
    /// The player has learned the spell.
    KnowsSpell { spell: String },
}

/// Whether `hour` lies in `from..to` (hours 0..24; `from > to` wraps around midnight).
pub fn hour_between(hour: f32, from: f32, to: f32) -> bool {
    let h = hour.rem_euclid(24.0);
    if from <= to {
        h >= from && h < to
    } else {
        h >= from || h < to
    }
}

impl Cond {
    pub fn label(&self) -> &'static str {
        match self {
            Cond::Flag { .. } => "Flag set",
            Cond::Var { .. } => "Variable",
            Cond::HasItem { .. } => "Has item",
            Cond::Script { .. } => "Script",
            Cond::PlayerHp { .. } => "Player HP",
            Cond::PropState { .. } => "Prop state",
            Cond::Quest { .. } => "Quest",
            Cond::PlayerSex { .. } => "Player sex",
            Cond::TimeBetween { .. } => "Time of day",
            Cond::Skill { .. } => "Skill level",
            Cond::KnowsSpell { .. } => "Knows spell",
        }
    }
    pub fn templates() -> Vec<Cond> {
        vec![
            Cond::Flag { name: String::new() },
            Cond::Var { name: String::new(), op: CmpOp::Eq, value: 0 },
            Cond::HasItem { item: String::new(), count: 1 },
            Cond::Script { function: String::new() },
            Cond::PlayerHp { op: CmpOp::Lt, value: 3 },
            Cond::PropState { target: "self".into(), state: String::new() },
            Cond::Quest { quest: String::new(), check: crate::quest::QuestCheck::Active, stage: 0 },
            Cond::PlayerSex { sex: crate::state::Sex::Female },
            Cond::TimeBetween { from: 20.0, to: 6.0 },
            Cond::Skill { skill: String::new(), op: CmpOp::Ge, level: 1 },
            Cond::KnowsSpell { spell: String::new() },
        ]
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Condition {
    pub cond: Cond,
    #[serde(default)]
    pub negate: bool,
}

impl Condition {
    pub fn new(cond: Cond) -> Self {
        Condition { cond, negate: false }
    }

    pub fn eval(&self, world: &WorldState, script: &mut dyn FnMut(&str) -> bool) -> bool {
        let v = match &self.cond {
            Cond::Flag { name } => world.flag(name),
            Cond::Var { name, op, value } => op.apply(world.var(name), *value),
            Cond::HasItem { item, count } => world.item_count(item) >= *count,
            Cond::Script { function } => script(function),
            Cond::PlayerHp { op, value } => op.apply(world.player_hp.unwrap_or(0) as i64, *value),
            // Needs the running game (see `conditions_met`).
            Cond::PropState { .. } => false,
            Cond::Quest { quest, check, stage } => world.check_quest(quest, *check, *stage),
            Cond::PlayerSex { sex } => world.player_sex == *sex,
            Cond::TimeBetween { from, to } => hour_between(world.hour, *from, *to),
            Cond::Skill { skill, op, level } => op.apply(world.skill_level(skill) as i64, *level),
            Cond::KnowsSpell { spell } => world.spells.contains(spell),
        };
        v != self.negate
    }
}

pub fn all_met(
    conds: &[Condition],
    world: &WorldState,
    script: &mut dyn FnMut(&str) -> bool,
) -> bool {
    conds.iter().all(|c| c.eval(world, script))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum VarOp {
    Set,
    Add,
}

/// Entity addressed by an action: `"self"` (trigger owner), `"player"`, or an entity name.
pub type TargetName = String;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Action {
    /// Show a text box and wait for the player to confirm.
    Dialog { speaker: String, text: String },
    /// Ask a question; the chosen option index (0-based) is stored in variable `var`.
    Choice { speaker: String, text: String, options: Vec<String>, var: String },
    GiveItem { item: String, count: u32 },
    TakeItem { item: String, count: u32 },
    SetFlag { name: String, value: bool },
    SetVar { name: String, op: VarOp, value: i64 },
    /// Move the player to a spawn point (on another map or the current one).
    Teleport { map: String, spawn: String },
    /// Show/hide an entity (hidden entities don't collide or interact). Persisted.
    SetActive { target: TargetName, active: bool },
    /// Change a prop's frame offset (e.g. open a chest). Persisted.
    SetFrame { target: TargetName, frame: u32 },
    /// Walk an entity by a tile offset; blocks until it arrives.
    Move { target: TargetName, dx: i32, dy: i32 },
    Face { target: TargetName, facing: Facing },
    Wait { seconds: f32 },
    /// Raise a custom event; matching triggers run before the rest of this sequence.
    Emit { name: String },
    /// Call a Rhai function from the project's scripts.
    RunScript { function: String },
    If { conditions: Vec<Condition>, then: Vec<Action>, otherwise: Vec<Action> },
    PlaySound { sound: String },
    /// Switch the looping music (empty = stop).
    PlayMusic { sound: String },
    /// Restore (or with a negative amount, remove) player hit points.
    Heal { amount: i32 },
    /// Locks or unlocks a prop without a key (unlocking applies its "on unlock" effect).
    SetLocked { target: TargetName, locked: bool },
    /// Switches a prop to one of its states (sprite and whether it blocks movement).
    SetState { target: TargetName, state: String },
    /// Opens the trade window of a merchant character.
    OpenShop { target: TargetName },
    /// Starts / advances / completes / fails a quest (`stage` is used by `SetStage`).
    Quest { quest: String, op: crate::quest::QuestOp, stage: u32 },
    /// Starts (the character's own route, or `points` in tile coordinates) or stops a
    /// character's patrol.
    Patrol {
        target: TargetName,
        active: bool,
        #[serde(default)]
        points: Vec<(i32, i32)>,
    },
    /// Sets the time of day (hours, 0..24).
    SetTime { hour: f32 },
    /// Skill points the player can spend (negative takes them away).
    GiveSkillPoints { amount: i32 },
    /// Sets a skill to a level directly (no points spent).
    SetSkill { skill: String, level: u32 },
    /// Teaches a spell.
    LearnSpell { spell: String },
    /// Mana (negative = drain).
    AddMana { amount: i32 },
    /// Damages every character within `radius` tiles of the player (spell effects).
    DamageAround { damage: i32, radius: f32 },
    /// A light around the player for a while (radius in tiles).
    PlayerLight {
        radius: f32,
        seconds: f32,
        #[serde(default = "spell_light_color")]
        color: [u8; 3],
        #[serde(default)]
        flicker: bool,
    },
    /// Starts magic targeting: the player's next click (an enemy or a spot) calls the Rhai
    /// function `function(x, y)` with the target in tile coordinates (X: straight ahead,
    /// right-click / Esc cancel). Started by a spell's script, the spell's mana and cooldown are
    /// only spent once a target is chosen.
    Aim { function: String },
    /// An animated sprite flies from the player to tile point (`x`, `y`) at `speed` tiles per
    /// second, then calls `function(x, y)` (if set).
    Fly { sprite: FxSprite, x: f32, y: f32, speed: f32, function: String },
    /// Plays an animated sprite once, centered on tile point (`x`, `y`) (explosions, sparkles).
    Effect { sprite: FxSprite, x: f32, y: f32 },
    /// Damages every character within `radius` tiles of tile point (`x`, `y`) (scaled by the
    /// player's spell damage skills).
    DamageArea { x: f32, y: f32, radius: f32, damage: i32 },
    /// A light at tile point (`x`, `y`) for `seconds` (radius in tiles; fades out at the end).
    LightAt { x: f32, y: f32, radius: f32, seconds: f32, color: [u8; 3], flicker: bool },
    /// Heals the player and friendly characters within `radius` tiles of tile point (`x`, `y`).
    HealArea { x: f32, y: f32, radius: f32, amount: i32 },
    /// Puts an item into its slot (clothing, armor, weapon, left hand) on a character; what it
    /// wore there goes to its carried items. The player takes it from the inventory (or gets it).
    Equip { target: TargetName, item: String },
    /// Takes off what a character wears / holds in a slot (into its carried items).
    Unequip { target: TargetName, slot: crate::defs::EquipSlot },
    /// Adds items to what a character carries (the player: the inventory).
    GiveItemTo { target: TargetName, item: String, count: u32 },
    /// Takes items from what a character carries.
    TakeItemFrom { target: TargetName, item: String, count: u32 },
    /// Hides or shows the game's HUD (status frame, clock, quick bar, buttons), e.g. for a
    /// cut-scene; dialogs and windows the player opens still show. The player toggles it with H.
    SetUiVisible { visible: bool },
}

/// Default color of spell lights (a cool white-blue).
pub fn spell_light_color() -> [u8; 3] {
    [190, 210, 255]
}

/// An animated sprite for the `Fly` / `Effect` actions: frames `first..=last` of a sprite sheet
/// (both 0 = every frame of the sheet).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct FxSprite {
    pub sheet: String,
    pub first: u32,
    pub last: u32,
    /// Frames per second.
    pub fps: f32,
    /// Size factor (1 = the sheet's frame size).
    pub scale: f32,
    /// A warm light around it (radius in tiles, 0 = none); explosions' lights fade out.
    pub light: f32,
}

impl Default for FxSprite {
    fn default() -> Self {
        FxSprite { sheet: String::new(), first: 0, last: 0, fps: 12.0, scale: 1.0, light: 0.0 }
    }
}

impl Action {
    pub fn label(&self) -> &'static str {
        match self {
            Action::Dialog { .. } => "Dialog",
            Action::Choice { .. } => "Choice",
            Action::GiveItem { .. } => "Give item",
            Action::TakeItem { .. } => "Take item",
            Action::SetFlag { .. } => "Set flag",
            Action::SetVar { .. } => "Set variable",
            Action::Teleport { .. } => "Teleport",
            Action::SetActive { .. } => "Set active",
            Action::SetFrame { .. } => "Set frame",
            Action::Move { .. } => "Move",
            Action::Face { .. } => "Face",
            Action::Wait { .. } => "Wait",
            Action::Emit { .. } => "Emit event",
            Action::RunScript { .. } => "Run script",
            Action::If { .. } => "If",
            Action::PlaySound { .. } => "Play sound",
            Action::PlayMusic { .. } => "Play music",
            Action::Heal { .. } => "Heal player",
            Action::SetLocked { .. } => "Set locked",
            Action::SetState { .. } => "Set prop state",
            Action::OpenShop { .. } => "Open shop",
            Action::Quest { .. } => "Quest",
            Action::Patrol { .. } => "Patrol",
            Action::SetTime { .. } => "Set time of day",
            Action::GiveSkillPoints { .. } => "Give skill points",
            Action::SetSkill { .. } => "Set skill level",
            Action::LearnSpell { .. } => "Learn spell",
            Action::AddMana { .. } => "Add mana",
            Action::DamageAround { .. } => "Damage around player",
            Action::PlayerLight { .. } => "Light around player",
            Action::Aim { .. } => "Aim (then run script)",
            Action::Fly { .. } => "Fly sprite to point",
            Action::Effect { .. } => "Play effect",
            Action::DamageArea { .. } => "Damage area",
            Action::LightAt { .. } => "Light at point",
            Action::HealArea { .. } => "Heal area",
            Action::Equip { .. } => "Equip item",
            Action::Unequip { .. } => "Unequip slot",
            Action::GiveItemTo { .. } => "Give item to character",
            Action::TakeItemFrom { .. } => "Take item from character",
            Action::SetUiVisible { .. } => "Show / hide UI",
        }
    }

    pub fn templates() -> Vec<Action> {
        let s = String::new;
        vec![
            Action::Dialog { speaker: s(), text: s() },
            Action::Choice { speaker: s(), text: s(), options: vec!["Yes".into(), "No".into()], var: "choice".into() },
            Action::GiveItem { item: s(), count: 1 },
            Action::TakeItem { item: s(), count: 1 },
            Action::SetFlag { name: s(), value: true },
            Action::SetVar { name: s(), op: VarOp::Set, value: 0 },
            Action::Teleport { map: s(), spawn: s() },
            Action::SetActive { target: "self".into(), active: false },
            Action::SetFrame { target: "self".into(), frame: 1 },
            Action::Move { target: "self".into(), dx: 0, dy: 1 },
            Action::Face { target: "self".into(), facing: Facing::Down },
            Action::Wait { seconds: 0.5 },
            Action::Emit { name: s() },
            Action::RunScript { function: s() },
            Action::If { conditions: Vec::new(), then: Vec::new(), otherwise: Vec::new() },
            Action::PlaySound { sound: s() },
            Action::PlayMusic { sound: s() },
            Action::Heal { amount: 1 },
            Action::SetLocked { target: "self".into(), locked: false },
            Action::SetState { target: "self".into(), state: String::new() },
            Action::OpenShop { target: "self".into() },
            Action::Quest { quest: s(), op: crate::quest::QuestOp::Start, stage: 0 },
            Action::Patrol { target: "self".into(), active: true, points: Vec::new() },
            Action::SetTime { hour: 8.0 },
            Action::GiveSkillPoints { amount: 1 },
            Action::SetSkill { skill: String::new(), level: 1 },
            Action::LearnSpell { spell: String::new() },
            Action::AddMana { amount: 10 },
            Action::DamageAround { damage: 2, radius: 2.0 },
            Action::PlayerLight { radius: 5.0, seconds: 30.0, color: spell_light_color(), flicker: false },
            Action::Aim { function: s() },
            Action::Fly { sprite: FxSprite::default(), x: 0.0, y: 0.0, speed: 8.0, function: s() },
            Action::Effect { sprite: FxSprite::default(), x: 0.0, y: 0.0 },
            Action::DamageArea { x: 0.0, y: 0.0, radius: 2.0, damage: 2 },
            Action::LightAt { x: 0.0, y: 0.0, radius: 3.0, seconds: 10.0, color: [255, 190, 120], flicker: true },
            Action::HealArea { x: 0.0, y: 0.0, radius: 2.0, amount: 2 },
            Action::Equip { target: "self".into(), item: s() },
            Action::Unequip { target: "self".into(), slot: crate::defs::EquipSlot::Weapon },
            Action::GiveItemTo { target: "self".into(), item: s(), count: 1 },
            Action::TakeItemFrom { target: "self".into(), item: s(), count: 1 },
            Action::SetUiVisible { visible: false },
        ]
    }
}

/// Triggers in a project whose event never reaches them: "where: On <event>".
pub fn useless_triggers(project: &crate::project::Project) -> Vec<String> {
    use crate::entity::EntityKind;
    let mut out = Vec::new();
    let mut check = |owner: TriggerOwner, ts: &[Trigger], place: String| {
        for t in ts.iter().filter(|t| !owner.allows(&t.on)) {
            out.push(format!("{place}: On {}", t.on.label()));
        }
    };
    let db = &project.db;
    for d in &db.items {
        check(TriggerOwner::ItemUse, &d.on_use, format!("item {}", d.id));
    }
    for d in &db.props {
        check(TriggerOwner::Prop, &d.triggers, format!("prop {}", d.id));
    }
    for d in &db.characters {
        check(TriggerOwner::Character, &d.triggers, format!("character {}", d.id));
    }
    for name in project.list_maps() {
        let Ok(map) = project.load_map(&name) else { continue };
        check(TriggerOwner::Map, &map.triggers, format!("map {name}"));
        for z in &map.zones {
            check(TriggerOwner::Zone, &z.triggers, format!("map {name}, zone {}", z.name));
        }
        for e in &map.entities {
            let owner = match e.kind {
                EntityKind::Character => TriggerOwner::Character,
                EntityKind::Prop => TriggerOwner::Prop,
                EntityKind::Item => TriggerOwner::Pickup,
            };
            check(owner, &e.triggers, format!("map {name}, {}", e.label()));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    #[test]
    fn time_of_day_condition_wraps_midnight() {
        use crate::state::WorldState;
        assert!(hour_between(22.0, 20.0, 6.0) && hour_between(3.0, 20.0, 6.0) && !hour_between(12.0, 20.0, 6.0));
        assert!(hour_between(9.0, 8.0, 18.0) && !hour_between(18.0, 8.0, 18.0));
        let mut w = WorldState { hour: 23.5, ..Default::default() };
        let night = Condition::new(Cond::TimeBetween { from: 20.0, to: 6.0 });
        assert!(night.eval(&w, &mut |_| false));
        w.hour = 10.0;
        assert!(!night.eval(&w, &mut |_| false));
    }

    use super::*;

    #[test]
    fn trigger_owners() {
        assert!(TriggerOwner::Map.allows(&TriggerOn::MapEnter) && !TriggerOwner::Map.allows(&TriggerOn::Enter));
        assert!(TriggerOwner::Zone.allows(&TriggerOn::Enter) && !TriggerOwner::Zone.allows(&TriggerOn::MapEnter));
        assert!(!TriggerOwner::Zone.allows(&TriggerOn::UseItem { item: String::new() }));
        assert!(TriggerOwner::Prop.allows(&TriggerOn::Unlock) && !TriggerOwner::Character.allows(&TriggerOn::Unlock));
        assert!(TriggerOwner::Character.allows(&TriggerOn::Defeat) && !TriggerOwner::Prop.allows(&TriggerOn::Defeat));
        assert!(!TriggerOwner::ItemUse.allows(&TriggerOn::Custom { name: "x".into() }));
        assert_eq!(TriggerOwner::Map.templates().iter().map(|t| t.label()).collect::<Vec<_>>(), vec!["Map enter", "Custom event"]);
        assert_eq!(TriggerOwner::Map.replacement(&TriggerOn::Enter), TriggerOn::MapEnter);
        assert_eq!(TriggerOwner::Zone.replacement(&TriggerOn::MapEnter), TriggerOn::Enter);
    }

    #[test]
    fn example_project_has_no_useless_triggers() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../example_project");
        let project = crate::project::Project::load(&root).unwrap();
        assert_eq!(useless_triggers(&project), Vec::<String>::new());
    }

    #[test]
    fn trigger_matching() {
        let any = TriggerOn::UseItem { item: String::new() };
        let key = TriggerOn::UseItem { item: "key".into() };
        let ev = GameEvent::UseItem { item: "key".into() };
        assert!(any.matches(&ev) && key.matches(&ev));
        assert!(!key.matches(&GameEvent::UseItem { item: "potion".into() }));
        assert!(!TriggerOn::Interact.matches(&GameEvent::Enter));
        let c = TriggerOn::Custom { name: "boom".into() };
        assert!(c.matches(&GameEvent::Custom { name: "boom".into() }));
    }

    #[test]
    fn conditions() {
        let mut w = WorldState::default();
        w.set_flag("door_open", true);
        w.set_var("gold", 5);
        w.give_item("key", 2);
        let mut no_script = |_: &str| panic!("not called");
        let met = |c: &[Condition], w: &WorldState, s: &mut dyn FnMut(&str) -> bool| all_met(c, w, s);
        assert!(met(&[Condition::new(Cond::Flag { name: "door_open".into() })], &w, &mut no_script));
        assert!(met(
            &[Condition { cond: Cond::Flag { name: "nope".into() }, negate: true }],
            &w,
            &mut no_script
        ));
        assert!(met(
            &[Condition::new(Cond::Var { name: "gold".into(), op: CmpOp::Ge, value: 5 })],
            &w,
            &mut no_script
        ));
        assert!(!met(
            &[Condition::new(Cond::HasItem { item: "key".into(), count: 3 })],
            &w,
            &mut no_script
        ));
        let mut yes = |f: &str| f == "ok";
        assert!(met(&[Condition::new(Cond::Script { function: "ok".into() })], &w, &mut yes));
    }

    #[test]
    fn triggers_serialize_to_ron() {
        let t = Trigger {
            on: TriggerOn::Interact,
            conditions: vec![Condition::new(Cond::HasItem { item: "key".into(), count: 1 })],
            actions: vec![Action::Dialog { speaker: "Bob".into(), text: "Hi".into() }],
            once: true,
        };
        let s = ron::to_string(&t).unwrap();
        let back: Trigger = ron::from_str(&s).unwrap();
        assert_eq!(back, t);
    }
}
