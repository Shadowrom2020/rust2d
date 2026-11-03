//! Rhai scripting. Script functions read/modify the world state directly and
//! queue world actions (dialogs, teleports, movement...) which the game then
//! runs through the same [`ActionRunner`](crate::events::runner::ActionRunner)
//! as editor-authored triggers.

use std::cell::RefCell;
use std::path::Path;
use std::rc::Rc;

use rhai::{Array, Dynamic, Engine, Scope, AST};

use crate::entity::Facing;
use crate::events::{Action, VarOp};
use crate::state::WorldState;

#[derive(Default)]
struct Ctx {
    world: WorldState,
    actions: Vec<Action>,
}

pub struct ScriptHost {
    engine: Engine,
    ast: AST,
    ctx: Rc<RefCell<Ctx>>,
    pub errors: Vec<String>,
}

impl ScriptHost {
    pub fn new() -> Self {
        let ctx = Rc::new(RefCell::new(Ctx::default()));
        let mut engine = Engine::new();
        engine.set_max_operations(2_000_000);
        engine.set_max_call_levels(64);
        engine.on_print(|s| println!("[script] {s}"));
        register_api(&mut engine, &ctx);
        ScriptHost { engine, ast: AST::empty(), ctx, errors: Vec::new() }
    }

    /// Compiles every `*.rhai` file in `dir` into one module (functions are global).
    pub fn load_dir(&mut self, dir: &Path) {
        self.ast = AST::empty();
        self.errors.clear();
        let Ok(rd) = std::fs::read_dir(dir) else { return };
        let mut files: Vec<_> = rd
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|e| e == "rhai"))
            .collect();
        files.sort();
        for f in files {
            match std::fs::read_to_string(&f).map_err(|e| e.to_string()).and_then(|src| {
                self.engine.compile(&src).map_err(|e| e.to_string())
            }) {
                Ok(ast) => self.ast = self.ast.merge(&ast),
                Err(e) => self.report(format!("{}: {e}", f.display())),
            }
        }
    }

    pub fn load_source(&mut self, src: &str) -> Result<(), String> {
        let ast = self.engine.compile(src).map_err(|e| e.to_string())?;
        self.ast = self.ast.merge(&ast);
        Ok(())
    }

    /// Names of all script functions (sorted, deduplicated).
    pub fn function_names(&self) -> Vec<String> {
        let mut v: Vec<String> = self.ast.iter_functions().map(|f| f.name.to_string()).collect();
        v.sort();
        v.dedup();
        v
    }

    pub fn has_function(&self, name: &str) -> bool {
        self.ast.iter_functions().any(|f| f.name == name)
    }

    fn report(&mut self, msg: String) {
        eprintln!("[script] error: {msg}");
        self.errors.push(msg);
    }

    fn call_inner(&mut self, world: &mut WorldState, function: &str, owner: Option<u32>) -> Option<Dynamic> {
        {
            let mut c = self.ctx.borrow_mut();
            c.world = std::mem::take(world);
            c.actions.clear();
        }
        let mut scope = Scope::new();
        scope.push_constant("SELF", owner.map_or(-1, |u| u as i64));
        let res = self.engine.call_fn::<Dynamic>(&mut scope, &self.ast, function, ());
        *world = std::mem::take(&mut self.ctx.borrow_mut().world);
        match res {
            Ok(v) => Some(v),
            Err(e) => {
                self.report(format!("calling {function}(): {e}"));
                None
            }
        }
    }

    /// Runs a script function; returns the world actions it queued.
    pub fn call(&mut self, world: &mut WorldState, function: &str, owner: Option<u32>) -> Vec<Action> {
        self.call_inner(world, function, owner);
        std::mem::take(&mut self.ctx.borrow_mut().actions)
    }

    /// Runs a script function used as a condition (must return `bool`).
    pub fn condition(&mut self, world: &mut WorldState, function: &str, owner: Option<u32>) -> bool {
        let r = self.call_inner(world, function, owner);
        self.ctx.borrow_mut().actions.clear();
        r.and_then(|v| v.as_bool().ok()).unwrap_or(false)
    }
}

impl Default for ScriptHost {
    fn default() -> Self {
        Self::new()
    }
}

fn register_api(engine: &mut Engine, ctx: &Rc<RefCell<Ctx>>) {
    macro_rules! with {
        ($ctx:ident, |$c:ident $(, $arg:ident : $ty:ty)*| $body:expr) => {{
            let ctx = $ctx.clone();
            move |$($arg: $ty),*| {
                #[allow(unused_mut)]
                let mut $c = ctx.borrow_mut();
                $body
            }
        }};
    }

    // World state (immediate).
    engine.register_fn("flag", with!(ctx, |c, n: &str| c.world.flag(n)));
    engine.register_fn("set_flag", with!(ctx, |c, n: &str, v: bool| c.world.set_flag(n, v)));
    engine.register_fn("get_var", with!(ctx, |c, n: &str| c.world.var(n)));
    engine.register_fn("set_var", with!(ctx, |c, n: &str, v: i64| c.world.set_var(n, v)));
    engine.register_fn("add_var", with!(ctx, |c, n: &str, v: i64| {
        let cur = c.world.var(n);
        c.world.set_var(n, cur + v)
    }));
    engine.register_fn("item_count", with!(ctx, |c, n: &str| c.world.item_count(n) as i64));
    engine.register_fn("has_item", with!(ctx, |c, n: &str| c.world.item_count(n) > 0));
    // Returns how many did not fit into the inventory.
    engine.register_fn("give_item", with!(ctx, |c, n: &str, k: i64| c.world.give_item(n, k.max(0) as u32) as i64));
    engine.register_fn("take_item", with!(ctx, |c, n: &str, k: i64| c.world.take_item(n, k.max(0) as u32) as i64));
    engine.register_fn("current_map", with!(ctx, |c| c.world.current_map.clone()));
    engine.register_fn("player_hp", with!(ctx, |c| c.world.player_hp.unwrap_or(0) as i64));
    // random(n): integer in 0..n
    engine.register_fn("random", |n: i64| if n <= 0 { 0 } else { macroquad::rand::gen_range(0, n) });

    // Queued world actions.
    engine.register_fn("say", with!(ctx, |c, text: &str| c.actions.push(Action::Dialog {
        speaker: String::new(),
        text: text.into()
    })));
    engine.register_fn("say", with!(ctx, |c, speaker: &str, text: &str| c.actions.push(Action::Dialog {
        speaker: speaker.into(),
        text: text.into()
    })));
    engine.register_fn("choice", with!(ctx, |c, text: &str, options: Array, var: &str| {
        let options = options.into_iter().map(|o| o.to_string()).collect();
        c.actions.push(Action::Choice { speaker: String::new(), text: text.into(), options, var: var.into() })
    }));
    engine.register_fn("teleport", with!(ctx, |c, map: &str, spawn: &str| c.actions.push(Action::Teleport {
        map: map.into(),
        spawn: spawn.into()
    })));
    engine.register_fn("set_active", with!(ctx, |c, t: &str, a: bool| c.actions.push(Action::SetActive {
        target: t.into(),
        active: a
    })));
    engine.register_fn("set_frame", with!(ctx, |c, t: &str, f: i64| c.actions.push(Action::SetFrame {
        target: t.into(),
        frame: f.max(0) as u32
    })));
    engine.register_fn("quest_stage", with!(ctx, |c, q: &str| c.world.quest(q).map_or(-1, |p| p.stage as i64)));
    engine.register_fn("quest_active", with!(ctx, |c, q: &str| c.world.check_quest(q, crate::quest::QuestCheck::Active, 0)));
    engine.register_fn("quest_done", with!(ctx, |c, q: &str| c.world.check_quest(q, crate::quest::QuestCheck::Completed, 0)));
    {
        use crate::quest::QuestOp;
        let op = |o: QuestOp| move |q: &str| Action::Quest { quest: q.into(), op: o, stage: 0 };
        let (st, adv, done, fail) = (op(QuestOp::Start), op(QuestOp::Advance), op(QuestOp::Complete), op(QuestOp::Fail));
        engine.register_fn("start_quest", with!(ctx, |c, q: &str| c.actions.push(st(q))));
        engine.register_fn("advance_quest", with!(ctx, |c, q: &str| c.actions.push(adv(q))));
        engine.register_fn("complete_quest", with!(ctx, |c, q: &str| c.actions.push(done(q))));
        engine.register_fn("fail_quest", with!(ctx, |c, q: &str| c.actions.push(fail(q))));
    }
    engine.register_fn("set_quest_stage", with!(ctx, |c, q: &str, n: i64| c.actions.push(Action::Quest {
        quest: q.into(),
        op: crate::quest::QuestOp::SetStage,
        stage: n.max(0) as u32
    })));
    engine.register_fn("open_shop", with!(ctx, |c, t: &str| c.actions.push(Action::OpenShop { target: t.into() })));
    engine.register_fn("set_state", with!(ctx, |c, t: &str, s: &str| c.actions.push(Action::SetState { target: t.into(), state: s.into() })));
    engine.register_fn("move_by", with!(ctx, |c, t: &str, dx: i64, dy: i64| c.actions.push(Action::Move {
        target: t.into(),
        dx: dx as i32,
        dy: dy as i32
    })));
    engine.register_fn("face", with!(ctx, |c, t: &str, dir: &str| {
        let facing = match dir {
            "up" => Facing::Up,
            "left" => Facing::Left,
            "right" => Facing::Right,
            _ => Facing::Down,
        };
        c.actions.push(Action::Face { target: t.into(), facing })
    }));
    engine.register_fn("wait", with!(ctx, |c, s: f64| c.actions.push(Action::Wait { seconds: s as f32 })));
    engine.register_fn("emit", with!(ctx, |c, n: &str| c.actions.push(Action::Emit { name: n.into() })));
    engine.register_fn("heal", with!(ctx, |c, n: i64| c.actions.push(Action::Heal { amount: n as i32 })));
    engine.register_fn("set_locked", with!(ctx, |c, t: &str, locked: bool| c.actions.push(Action::SetLocked {
        target: t.into(),
        locked
    })));
    engine.register_fn("play_sound", with!(ctx, |c, id: &str| c.actions.push(Action::PlaySound { sound: id.into() })));
    engine.register_fn("play_music", with!(ctx, |c, id: &str| c.actions.push(Action::PlayMusic { sound: id.into() })));
    // Continue in another function after the queued actions (e.g. after a choice).
    engine.register_fn("then_call", with!(ctx, |c, f: &str| c.actions.push(Action::RunScript {
        function: f.into()
    })));
    // Queued variant of set_var, for ordering relative to dialogs.
    engine.register_fn("queue_set_var", with!(ctx, |c, n: &str, v: i64| c.actions.push(Action::SetVar {
        name: n.into(),
        op: VarOp::Set,
        value: v
    })));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn api_mutates_world_and_queues_actions() {
        let mut host = ScriptHost::new();
        host.load_source(
            r#"
            fn talk() {
                if !flag("met") {
                    set_flag("met", true);
                    give_item("coin", 2);
                    say("Bob", "Hello!");
                    choice("Buy?", ["Yes", "No"], "buy");
                    then_call("after");
                } else {
                    say("Again?");
                }
            }
            fn rich() { add_var("n", 2); get_var("n") == 2 && item_count("coin") >= 2 && random(5) < 5 }
            fn fight() { if player_hp() < 3 { heal(2); play_sound("ding"); set_locked("door", false); } }
            "#,
        )
        .unwrap();
        let mut world = WorldState::default();
        let actions = host.call(&mut world, "talk", Some(3));
        assert!(world.flag("met"));
        assert_eq!(world.item_count("coin"), 2);
        assert_eq!(actions.len(), 3);
        assert!(matches!(&actions[1], Action::Choice { options, .. } if options.len() == 2));
        assert!(host.condition(&mut world, "rich", None));
        let actions = host.call(&mut world, "talk", None);
        assert_eq!(actions, vec![Action::Dialog { speaker: String::new(), text: "Again?".into() }]);
        world.player_hp = Some(1);
        let actions = host.call(&mut world, "fight", None);
        assert_eq!(
            actions,
            vec![
                Action::Heal { amount: 2 },
                Action::PlaySound { sound: "ding".into() },
                Action::SetLocked { target: "door".into(), locked: false }
            ]
        );
    }

    #[test]
    fn errors_are_reported_not_fatal() {
        let mut host = ScriptHost::new();
        let mut world = WorldState::default();
        world.set_flag("keep", true);
        assert!(host.call(&mut world, "missing", None).is_empty());
        assert_eq!(host.errors.len(), 1);
        assert!(world.flag("keep"), "world must be restored after a failed call");
        host.load_source("fn spin() { loop {} }").unwrap();
        host.call(&mut world, "spin", None);
        assert_eq!(host.errors.len(), 2);
    }
}
