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
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
