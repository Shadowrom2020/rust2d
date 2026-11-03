//! The game runtime: loads a project and plays it.

mod combat;
mod equipment;
mod inventory_ui;
mod journal;
mod shop_ui;
mod locks;
mod menu;
pub mod input;
mod mouse;
mod spawner;
mod ui;

use std::collections::{HashSet, VecDeque};

use anyhow::{anyhow, Result};
use macroquad::prelude::*;

use crate::audio::Audio;
use crate::defs::{Behavior, Weapon};
use crate::entity::{EntityKind, Facing};
use crate::events::runner::{conditions_met, ActionCtx, ActionHost, ActionRunner, Step};
use crate::events::{Action, GameEvent, Trigger};
use crate::map::transition::edge_destination;
use crate::map::{Edge, EdgeLink, Map, ZoneKind};
use crate::project::Project;
use crate::render::{self, Assets};
use crate::script::ScriptHost;
use crate::state::{EntityState, WorldState};
use combat::{PlayerCombat, Projectile};
use mouse::Order;
use spawner::SpawnerRt;
use ui::Ui;

/// Callback collecting `(once-key, triggers, owner)` while gathering event handlers.
type TriggerSink<'a> = dyn FnMut(String, &[Trigger], Option<u32>) + 'a;

/// How many recently left maps are kept in memory.
const MAP_CACHE: usize = 4;
const FADE_TIME: f32 = 0.25;

#[derive(Clone, Debug)]
pub enum Arrival {
    Spawn(String),
    /// Next to the named zone (the far end of a map transition).
    Beside(String),
    Pos(Vec2),
    Edge { edge: Edge, pos: Vec2, link: EdgeLink },
}

/// Overrides where the game starts (used by the editor's play-test).
#[derive(Clone, Debug, Default)]
pub struct StartOptions {
    pub map: Option<String>,
    pub spawn: Option<String>,
    pub pos: Option<Vec2>,
    /// Show the title screen (main menu) first.
    pub title: bool,
}

/// A moving thing: the player or a runtime entity.
#[derive(Clone, Debug)]
struct Body {
    pos: Vec2,
    facing: Facing,
    hitbox: Vec2,
    speed: f32,
    moving: bool,
    anim: f32,
    /// Scripted movement target (ignores collision).
    move_target: Option<Vec2>,
}

impl Body {
    fn rect_at(&self, p: Vec2) -> Rect {
        Rect::new(p.x - self.hitbox.x / 2.0, p.y - self.hitbox.y, self.hitbox.x, self.hitbox.y)
    }
    fn rect(&self) -> Rect {
        self.rect_at(self.pos)
    }
}

struct RtEntity {
    uid: u32,
    kind: EntityKind,
    def: String,
    name: String,
    /// Character model override ("" = the definition's sheet).
    sheet: String,
    body: Body,
    home: Vec2,
    active: bool,
    /// Legacy frame offset added to the state's sprite (SetFrame action).
    frame: u32,
    /// Current state of a prop (index into its definition's states; 0 = default).
    state: usize,
    solid: bool,
    behavior: Behavior,
    wander_timer: f32,
    wander_dir: Vec2,
    // Combat (max_hp 0 = can't be damaged).
    hp: i32,
    max_hp: i32,
    hostile: bool,
    damage: i32,
    invuln: f32,
    flash: f32,
    knock: Vec2,
    /// Ranged attack for hostile shooters.
    weapon: Option<Weapon>,
    attack_cd: f32,
    /// Spawner zone uid for characters created by a spawner (not saved).
    spawned_by: Option<u32>,
    /// Stack size of item pickups.
    count: u32,
    /// Subtracted from damage taken.
    defense: i32,
    /// Its key lock has been opened.
    unlocked: bool,
}

struct Loaded {
    map: Map,
    solid: Vec<bool>,
    entities: Vec<RtEntity>,
}

enum Fade {
    None,
    Out { t: f32, map: String, arrival: Arrival },
    In { t: f32 },
}

/// Which owner's triggers an event is delivered to.
#[derive(Clone, Debug)]
enum Source {
    Entity(u32),
    Zone(u32),
    Map,
    Item(String),
    /// Custom events: map, all zones and all active entities.
    Broadcast,
}

pub struct Game {
    pub project: Project,
    pub assets: Assets,
    pub audio: Audio,
    combat: PlayerCombat,
    /// Seconds since the player was defeated.
    game_over: Option<f32>,
    /// Short-lived visual effects: (position, age).
    effects: Vec<(Vec2, f32)>,
    projectiles: Vec<Projectile>,
    /// Click-to-move / click-to-act order.
    order: Option<Order>,
    /// Where the last click landed (marker, age).
    click_marker: Option<(Vec2, f32)>,
    spawners: Vec<SpawnerRt>,
    next_spawn_uid: u32,
    mouse: mouse::MouseState,
    inv: inventory_ui::InvUi,
    shop: shop_ui::ShopUi,
    journal: journal::Journal,
    menu: menu::Menu,
    pub world: WorldState,
    scripts: ScriptHost,
    runner: ActionRunner,
    loaded: Loaded,
    cache: VecDeque<Map>,
    player: Body,
    player_sheet: Option<String>,
    player_frames: u32,
    zones_inside: HashSet<u32>,
    fade: Fade,
    ui: Ui,
    pub debug: bool,
    pending_events: Vec<(GameEvent, Source)>,
    /// Save slot the current game was last saved to / loaded from ("Save and quit" uses it).
    slot: Option<String>,
}

impl Game {
    pub fn new(project: Project, start: StartOptions) -> Result<Game> {
        if project.config.tile_size != crate::project::TILE_SIZE {
            anyhow::bail!(
                "this project uses {}px tiles; open it once in the rust2d editor to convert it to {}px tiles",
                project.config.tile_size,
                crate::project::TILE_SIZE
            );
        }
        let assets = Assets::load(&project);
        let audio = Audio::load(&project);
        let combat = PlayerCombat::new(project.db.character(&project.config.player));
        let mut scripts = ScriptHost::new();
        scripts.load_dir(&project.scripts_dir());
        let ts = assets.tile_size;
        let (hitbox, speed, sheet, frames) = match project.db.character(&project.config.player) {
            Some(c) => (vec2(c.hitbox[0], c.hitbox[1]), c.speed, Some(c.sheet.clone()), c.walk_frames),
            None => (vec2(ts * 0.75, ts * 0.5), 70.0, None, 1),
        };
        let map_name = start.map.clone().unwrap_or_else(|| project.config.start_map.clone());
        let map = project.load_map(&map_name)?;
        let player = Body {
            pos: Vec2::ZERO,
            facing: Facing::Down,
            hitbox,
            speed,
            moving: false,
            anim: 0.0,
            move_target: None,
        };
        let mut game = Game {
            loaded: Loaded { map: Map::new("", 1, 1), solid: vec![], entities: vec![] },
            project,
            assets,
            audio,
            combat,
            game_over: None,
            effects: Vec::new(),
            projectiles: Vec::new(),
            order: None,
            click_marker: None,
            spawners: Vec::new(),
            next_spawn_uid: spawner::SPAWN_UID_BASE,
            mouse: Default::default(),
            inv: Default::default(),
            shop: Default::default(),
            journal: Default::default(),
            menu: Default::default(),
            world: WorldState::default(),
            scripts,
            runner: ActionRunner::default(),
            cache: VecDeque::new(),
            player,
            player_sheet: sheet,
            player_frames: frames.max(1),
            zones_inside: HashSet::new(),
            fade: Fade::In { t: FADE_TIME },
            ui: Ui::default(),
            debug: false,
            pending_events: Vec::new(),
            slot: None,
        };
        let arrival = match (start.pos, start.spawn) {
            (Some(p), _) => Arrival::Pos(p),
            (None, Some(s)) => Arrival::Spawn(s),
            (None, None) if start.map.is_some() => Arrival::Spawn(String::new()),
            _ => Arrival::Spawn(game.project.config.start_spawn.clone()),
        };
        game.world.player_hp = Some(game.combat.max_hp);
        game.prepare_world();
        if start.title {
            game.open_title();
        }
        game.enter_map(map, arrival);
        Ok(game)
    }

    fn ts(&self) -> f32 {
        self.assets.tile_size
    }

    /// One "art pixel" of the classic 16px tile grid: world-space sizes of effects, markers
    /// and knockback are given in these so they keep their proportions at any tile size.
    fn u(&self) -> f32 {
        self.ts() / 16.0
    }

    // ---------------------------------------------------------------- maps

    fn take_map(&mut self, name: &str) -> Result<Map> {
        if let Some(i) = self.cache.iter().position(|m| m.name == name) {
            return Ok(self.cache.remove(i).unwrap());
        }
        self.project.load_map(name)
    }

    /// Builds the runtime state of an entity from its definition.
    #[allow(clippy::too_many_arguments)]
    fn new_rt_entity(
        &self,
        kind: EntityKind,
        def: &str,
        uid: u32,
        name: &str,
        pos: Vec2,
        facing: Facing,
        active: bool,
        frame: u32,
    ) -> RtEntity {
        let ts = self.ts();
        let (hitbox, speed, solid, behavior) = match kind {
            EntityKind::Prop => match self.project.db.prop(def) {
                Some(d) => (vec2(d.hitbox[0], d.hitbox[1]), 0.0, d.solid, Behavior::Static),
                None => (vec2(ts, ts), 0.0, false, Behavior::Static),
            },
            EntityKind::Character => match self.project.db.character(def) {
                Some(d) => (vec2(d.hitbox[0], d.hitbox[1]), d.speed, true, d.behavior.clone()),
                None => (vec2(ts * 0.75, ts * 0.5), 50.0, true, Behavior::Static),
            },
            EntityKind::Item => (vec2(ts * 0.75, ts * 0.75), 0.0, false, Behavior::Static),
        };
        let (max_hp, hostile, damage, weapon, defense) = match (kind, self.project.db.character(def)) {
            (EntityKind::Character, Some(d)) => (d.max_hp.max(0), d.hostile, d.damage.max(0), d.weapon.clone(), d.defense),
            _ => (0, false, 0, None, 0),
        };
        RtEntity {
            uid,
            kind,
            def: def.to_string(),
            name: name.to_string(),
            sheet: String::new(),
            body: Body { pos, facing, hitbox, speed, moving: false, anim: 0.0, move_target: None },
            home: pos,
            active,
            frame,
            // Hostiles overlap the player (contact damage) instead of blocking.
            solid: solid && !hostile,
            behavior,
            wander_timer: rand::gen_range(0.5, 2.0),
            wander_dir: Vec2::ZERO,
            hp: max_hp,
            max_hp,
            hostile,
            damage,
            invuln: 0.0,
            flash: 0.0,
            knock: Vec2::ZERO,
            weapon,
            attack_cd: rand::gen_range(0.5, 1.5),
            spawned_by: None,
            count: 1,
            defense,
            unlocked: false,
            state: 0,
        }
    }

    /// Switches prop `i` to the named state (sprite + solidity) and saves it.
    fn set_prop_state(&mut self, i: usize, name: &str) -> bool {
        let e = &self.loaded.entities[i];
        let Some(d) = self.project.db.prop(&e.def) else { return false };
        let Some(idx) = d.state_index(name) else {
            eprintln!("[game] prop '{}' has no state '{name}'", e.def);
            return false;
        };
        let solid = d.state_look(idx).1;
        let e = &mut self.loaded.entities[i];
        e.state = idx;
        e.frame = 0;
        e.solid = solid;
        self.persist(i);
        true
    }

    fn build_entities(&self, map: &Map) -> Vec<RtEntity> {
        map.entities
            .iter()
            .map(|e| {
                let st = self.world.entity_state(&map.name, e.uid);
                let pos = st.map_or(vec2(e.x, e.y), |s| vec2(s.x, s.y));
                let count = st.and_then(|s| s.count).unwrap_or(e.count);
                let mut rt = self.new_rt_entity(
                    e.kind,
                    &e.def,
                    e.uid,
                    &e.name,
                    pos,
                    st.map_or(e.facing, |s| s.facing),
                    st.map_or(e.active, |s| s.active),
                    st.map_or(e.frame, |s| s.frame),
                );
                rt.count = count;
                rt.sheet = e.sheet.clone();
                rt.unlocked = st.is_some_and(|s| s.unlocked);
                if let Some(d) = (e.kind == EntityKind::Prop).then(|| self.project.db.prop(&e.def)).flatten() {
                    // Saved state; saves from before named states store a frame offset.
                    let name = match st {
                        Some(s) => match &s.state {
                            Some(n) => Some(n.clone()),
                            None => d.state_of_offset(s.frame),
                        },
                        None => Some(e.state.clone()),
                    };
                    if let Some(idx) = name.and_then(|n| d.state_index(&n)) {
                        rt.state = idx;
                        if st.is_none_or(|s| s.state.is_some() || d.state_of_offset(s.frame).is_some()) {
                            rt.frame = 0;
                        }
                        rt.solid = d.state_look(idx).1;
                    }
                }
                rt
            })
            .collect()
    }

    fn enter_map(&mut self, map: Map, arrival: Arrival) {
        let ts = self.ts();
        let solid = self.assets.compute_solid(&map);
        let entities = self.build_entities(&map);
        let old = std::mem::replace(&mut self.loaded, Loaded { map, solid, entities });
        self.restore_unlocked();
        if !old.map.name.is_empty() && old.map.name != self.loaded.map.name {
            self.cache.push_front(old.map);
            self.cache.truncate(MAP_CACHE);
        }
        let m = &self.loaded.map;
        let center = vec2(m.width as f32 * ts / 2.0, m.height as f32 * ts / 2.0);
        let pos = match &arrival {
            Arrival::Pos(p) => *p,
            Arrival::Spawn(name) => {
                let spawn = m.spawn_point(name).or_else(|| {
                    m.zones.iter().find(|z| matches!(z.kind, ZoneKind::Spawn))
                });
                if spawn.is_none() {
                    eprintln!("[game] map '{}' has no spawn '{name}', using center", m.name);
                }
                spawn.map_or(center, |z| z.arrival_point(ts))
            }
            Arrival::Edge { edge, pos, link } => {
                edge_destination(*edge, *pos, link, m.width, m.height, &m.edges, ts)
            }
            Arrival::Beside(name) => match m.zones.iter().find(|z| z.name == *name) {
                Some(z) => z.beside_point(m.width, m.height, ts),
                None => {
                    eprintln!("[game] map '{}' has no transition zone '{name}', using center", m.name);
                    center
                }
            },
        };
        self.player.pos = pos;
        self.player.move_target = None;
        self.world.current_map = self.loaded.map.name.clone();
        // Zones we arrive in don't fire Enter (avoids bouncing back through portals).
        self.zones_inside = self.zones_at(self.player.pos);
        self.effects.clear();
        self.projectiles.clear();
        self.order = None;
        self.setup_spawners();
        let music = self.loaded.map.music.clone();
        if !music.is_empty() {
            self.audio.play_music(&music);
        }
        self.pending_events.push((GameEvent::MapEnter, Source::Map));
    }

    /// Starts a fade-out; the map switches when the screen is black.
    pub fn transition(&mut self, map: &str, arrival: Arrival) {
        if matches!(self.fade, Fade::Out { .. }) {
            return;
        }
        self.fade = Fade::Out { t: 0.0, map: map.to_string(), arrival };
    }

    fn finish_transition(&mut self, map_name: String, arrival: Arrival) {
        if map_name.is_empty() || map_name == self.loaded.map.name {
            // Same map: just reposition.
            let ts = self.ts();
            let m = &self.loaded.map;
            self.player.pos = match &arrival {
                Arrival::Pos(p) => *p,
                Arrival::Spawn(s) => m.spawn_point(s).map_or(self.player.pos, |z| z.arrival_point(ts)),
                Arrival::Beside(n) => m.zones.iter().find(|z| z.name == *n).map_or(self.player.pos, |z| z.beside_point(m.width, m.height, ts)),
                Arrival::Edge { .. } => self.player.pos,
            };
            self.zones_inside = self.zones_at(self.player.pos);
            return;
        }
        match self.take_map(&map_name) {
            Ok(map) => self.enter_map(map, arrival),
            Err(e) => {
                eprintln!("[game] cannot load map '{map_name}': {e:#}");
                self.ui.toast(format!("Missing map: {map_name}"));
            }
        }
    }

    // ---------------------------------------------------------------- collision

    fn tile_solid(&self, tx: i32, ty: i32) -> bool {
        let m = &self.loaded.map;
        if !m.in_bounds(tx, ty) {
            return true;
        }
        self.loaded.solid[(ty as u32 * m.width + tx as u32) as usize]
    }

    fn rect_blocked(&self, r: Rect, ignore: Option<u32>, check_player: bool) -> bool {
        let ts = self.ts();
        let x0 = (r.x / ts).floor() as i32;
        let y0 = (r.y / ts).floor() as i32;
        let x1 = ((r.x + r.w - 0.001) / ts).floor() as i32;
        let y1 = ((r.y + r.h - 0.001) / ts).floor() as i32;
        for ty in y0..=y1 {
            for tx in x0..=x1 {
                if self.tile_solid(tx, ty) {
                    return true;
                }
            }
        }
        if check_player && self.player.rect().overlaps(&r) {
            return true;
        }
        self.loaded
            .entities
            .iter()
            .any(|e| e.active && e.solid && Some(e.uid) != ignore && e.body.rect().overlaps(&r))
    }

    /// Moves along each axis separately so actors slide along walls.
    fn try_move(&self, body: &Body, delta: Vec2, ignore: Option<u32>, check_player: bool) -> Vec2 {
        let mut p = body.pos;
        let nx = p + vec2(delta.x, 0.0);
        if delta.x != 0.0 && !self.rect_blocked(body.rect_at(nx), ignore, check_player) {
            p = nx;
        }
        let ny = p + vec2(0.0, delta.y);
        if delta.y != 0.0 && !self.rect_blocked(body.rect_at(ny), ignore, check_player) {
            p = ny;
        }
        p
    }

    fn zones_at(&self, p: Vec2) -> HashSet<u32> {
        let ts = self.ts();
        self.loaded
            .map
            .zones
            .iter()
            .filter(|z| z.pixel_rect(ts).contains(p))
            .map(|z| z.uid)
            .collect()
    }

    // ---------------------------------------------------------------- events

    fn entity_index(&self, target: &str, owner: Option<u32>) -> Option<usize> {
        let uid = match target {
            "" | "self" => owner?,
            name => return self.loaded.entities.iter().position(|e| e.name == name),
        };
        self.loaded.entities.iter().position(|e| e.uid == uid)
    }

    /// Collects actions of all triggers reacting to `ev` from `source`.
    fn collect(&mut self, ev: &GameEvent, source: &Source) -> Vec<(Action, ActionCtx)> {
        let map = &self.loaded.map;
        let mut candidates: Vec<(String, Trigger, Option<u32>)> = Vec::new();
        let mut add = |key: String, triggers: &[Trigger], owner: Option<u32>| {
            for (i, t) in triggers.iter().enumerate() {
                if t.on.matches(ev) {
                    candidates.push((format!("{key}/{i}"), t.clone(), owner));
                }
            }
        };
        let db = &self.project.db;
        let rt = &self.loaded.entities;
        let add_entity = |uid: u32, add: &mut TriggerSink| {
            if let Some(e) = rt.iter().find(|e| e.uid == uid) {
                add(format!("{}/def:{}", map.name, e.uid), db.def_triggers(e.kind, &e.def), Some(uid));
            }
            if let Some(e) = map.entities.iter().find(|e| e.uid == uid) {
                add(format!("{}/e:{}", map.name, e.uid), &e.triggers, Some(uid));
            }
        };
        match source {
            Source::Entity(uid) => add_entity(*uid, &mut add),
            Source::Zone(uid) => {
                if let Some(z) = map.zones.iter().find(|z| z.uid == *uid) {
                    add(format!("{}/z:{}", map.name, z.uid), &z.triggers, None);
                }
            }
            Source::Map => add(format!("{}/map", map.name), &map.triggers, None),
            Source::Item(id) => {
                if let Some(d) = db.item(id) {
                    add(format!("item:{id}"), &d.on_use, None);
                }
            }
            Source::Broadcast => {
                add(format!("{}/map", map.name), &map.triggers, None);
                for z in &map.zones {
                    add(format!("{}/z:{}", map.name, z.uid), &z.triggers, None);
                }
                let active: Vec<u32> =
                    self.loaded.entities.iter().filter(|e| e.active).map(|e| e.uid).collect();
                for uid in active {
                    add_entity(uid, &mut add);
                }
            }
        }

        let mut out = Vec::new();
        for (key, trigger, owner) in candidates {
            if trigger.once && self.world.fired.contains(&key) {
                continue;
            }
            let ctx = ActionCtx { owner };
            if !conditions_met(self, &trigger.conditions, &ctx) {
                continue;
            }
            if trigger.once {
                self.world.fired.insert(key);
            }
            out.extend(trigger.actions.into_iter().map(|a| (a, ctx.clone())));
        }
        out
    }

    fn fire(&mut self, ev: GameEvent, source: Source) -> bool {
        let actions = self.collect(&ev, &source);
        let any = !actions.is_empty();
        for (a, ctx) in actions {
            self.runner.push(&[a], &ctx);
        }
        any
    }

    fn persist(&mut self, idx: usize) {
        let e = &self.loaded.entities[idx];
        if e.spawned_by.is_some() {
            return; // spawner characters are not saved individually
        }
        let state = (e.kind == EntityKind::Prop).then(|| self.project.db.prop(&e.def)).flatten().map(|d| d.state_name(e.state).to_string());
        let st = EntityState {
            active: e.active,
            frame: e.frame,
            x: e.body.pos.x,
            y: e.body.pos.y,
            facing: e.body.facing,
            count: (e.kind == EntityKind::Item).then_some(e.count),
            unlocked: e.unlocked,
            state,
        };
        let map = self.loaded.map.name.clone();
        let uid = e.uid;
        self.world.set_entity_state(&map, uid, st);
    }

    // ---------------------------------------------------------------- update

    /// Runs one frame: update and draw.
    pub fn frame(&mut self) {
        let dt = get_frame_time().min(0.05);
        self.update(dt);
        self.draw();
    }

    pub fn update(&mut self, dt: f32) {
        // Esc opens the pause menu (unless a window or dialog uses Esc); it pauses everything.
        let can_open = !self.inventory_open() && !self.ui.dialog_open() && self.game_over.is_none() && matches!(self.fade, Fade::None);
        if self.on_title() {
            // The start fade-in still plays behind the title screen.
            if let Fade::In { t } = &mut self.fade {
                *t -= dt;
                if *t <= 0.0 {
                    self.fade = Fade::None;
                }
            }
        }
        if self.update_menu(can_open) {
            return;
        }
        self.world.play_time += dt as f64;
        if input::pressed(KeyCode::F3) {
            self.debug = !self.debug;
        }
        if input::pressed(KeyCode::F5) {
            match self.save_game("quick") {
                Ok(()) => self.ui.toast("Game saved".into()),
                Err(e) => self.ui.toast(format!("Save failed: {e}")),
            }
        }
        if input::pressed(KeyCode::F9) {
            if let Err(e) = self.load_game("quick") {
                self.ui.toast(format!("Load failed: {e}"));
            }
        }

        // Fades / map transitions.
        match &mut self.fade {
            Fade::None => {}
            Fade::Out { t, .. } => {
                *t += dt;
                if *t >= FADE_TIME {
                    if let Fade::Out { map, arrival, .. } = std::mem::replace(&mut self.fade, Fade::In { t: FADE_TIME }) {
                        self.finish_transition(map, arrival);
                    }
                }
            }
            Fade::In { t } => {
                *t -= dt;
                if *t <= 0.0 {
                    self.fade = Fade::None;
                }
            }
        }

        for (ev, src) in std::mem::take(&mut self.pending_events) {
            self.fire(ev, src);
        }

        if self.update_game_over(dt) {
            return;
        }

        // UI gets input first; a key that closes a dialog must not also interact.
        let allow_inventory = !self.runner.busy() && matches!(self.fade, Fade::None);
        let input = self.ui.update(dt, &mut self.world);
        let allow_windows = allow_inventory && !self.ui.dialog_open() && self.game_over.is_none();
        let (shop_key, shop_mouse) = self.update_shop_ui(allow_windows);
        let (j_key, j_mouse) = self.update_journal(allow_windows && !self.shop_open() && !self.inv.is_open(), input.consumed || shop_key);
        let others = self.shop_open() || self.journal_open();
        let (inv_key, inv_mouse) = self.update_inventory_ui(allow_windows && !others, input.consumed || shop_key || j_key);
        let (inv_key, inv_mouse) = (inv_key || shop_key || j_key, inv_mouse || shop_mouse || j_mouse);
        let consumed = input.consumed || inv_key;
        if !input.mouse_consumed && !inv_mouse && allow_inventory && !self.modal() {
            self.handle_mouse();
        }

        let mut runner = std::mem::take(&mut self.runner);
        runner.update(self, dt);
        self.runner = runner;

        let frozen = self.runner.busy() || !matches!(self.fade, Fade::None) || self.modal();
        self.update_player(dt, frozen, consumed);
        self.update_entities(dt, frozen);
        self.update_combat(dt, frozen, consumed);
        if !frozen {
            self.update_projectiles(dt);
            self.update_spawners(dt);
        }
        if let Some(m) = &mut self.click_marker {
            m.1 += dt;
            if m.1 > 0.6 {
                self.click_marker = None;
            }
        }
    }

    fn update_player(&mut self, dt: f32, frozen: bool, key_consumed: bool) {
        let ts = self.ts();
        if let Some(target) = self.player.move_target {
            let (pos, done) = step_towards(self.player.pos, target, self.player.speed * dt);
            self.player.facing = Facing::from_vec(target.x - pos.x, target.y - pos.y).unwrap_or(self.player.facing);
            self.player.pos = pos;
            self.player.moving = !done;
            self.player.anim += dt;
            if done {
                self.player.move_target = None;
            }
            return;
        }
        if frozen {
            self.player.moving = false;
            return;
        }
        if self.apply_player_knockback(dt) {
            return;
        }

        let mut dir = Vec2::ZERO;
        if input::down(KeyCode::Left) || input::down(KeyCode::A) {
            dir.x -= 1.0;
        }
        if input::down(KeyCode::Right) || input::down(KeyCode::D) {
            dir.x += 1.0;
        }
        if input::down(KeyCode::Up) || input::down(KeyCode::W) {
            dir.y -= 1.0;
        }
        if input::down(KeyCode::Down) || input::down(KeyCode::S) {
            dir.y += 1.0;
        }
        let mut run = if input::down(KeyCode::LeftShift) { 1.8 } else { 1.0 };
        if dir != Vec2::ZERO {
            // Keyboard movement cancels click-to-move.
            self.order = None;
        } else if self.order.is_some() {
            dir = self.steer_order(dt);
            run = 1.0;
            if self.fade_active() {
                return; // the order triggered a transition
            }
        }
        if dir != Vec2::ZERO {
            dir = dir.normalize();
            if let Some(f) = Facing::from_vec(dir.x, dir.y) {
                self.player.facing = f;
            }
            let delta = dir * self.player.speed * self.speed_factor() * run * dt;
            self.player.pos = self.try_move(&self.player, delta, None, false);
            self.player.moving = true;
            self.player.anim += dt * run;
        } else {
            self.player.moving = false;
        }

        // Map edges.
        let m = &self.loaded.map;
        if let Some(edge) = m.edges.crossing(self.player.pos, dir, m.width, m.height, ts) {
            let link = m.edges.get(edge).unwrap().clone();
            let target = link.target_map.clone();
            self.transition(&target, Arrival::Edge { edge, pos: self.player.pos, link });
            return;
        }

        // Zones.
        let now = self.zones_at(self.player.pos);
        let entered: Vec<u32> = now.difference(&self.zones_inside).copied().collect();
        let left: Vec<u32> = self.zones_inside.difference(&now).copied().collect();
        self.zones_inside = now;
        for uid in left {
            self.fire(GameEvent::Leave, Source::Zone(uid));
        }
        for uid in entered {
            self.fire(GameEvent::Enter, Source::Zone(uid));
            let portal = self.loaded.map.zones.iter().find(|z| z.uid == uid).and_then(|z| match &z.kind {
                ZoneKind::Portal { target_map, target_spawn } => Some((target_map.clone(), Arrival::Spawn(target_spawn.clone()))),
                ZoneKind::Transition { target_map, target_zone } if !target_map.is_empty() => Some((target_map.clone(), Arrival::Beside(target_zone.clone()))),
                _ => None,
            });
            if let Some((map, arrival)) = portal {
                self.audio.play(&self.project.config.sfx.portal);
                self.transition(&map, arrival);
                return;
            }
        }

        if !key_consumed && input::any_pressed(&[KeyCode::E, KeyCode::Space, KeyCode::Enter]) {
            self.interact();
        }
    }

    /// The entity in front of the player, if any.
    /// A character two tiles away in a straight line with a solid tile in between (e.g. a
    /// merchant behind a counter) can be talked to across it.
    fn across_counter(&self, i: usize) -> bool {
        let e = &self.loaded.entities[i];
        if e.kind != EntityKind::Character || !e.active || e.hostile {
            return false;
        }
        let ts = self.ts();
        let tile = |p: Vec2| ((p.x / ts).floor() as i32, ((p.y - 1.0) / ts).floor() as i32);
        let ((px, py), (ex, ey)) = (tile(self.player.pos), tile(e.body.pos));
        let (dx, dy) = (ex - px, ey - py);
        let straight = (dx.abs() == 2 && dy == 0) || (dy.abs() == 2 && dx == 0);
        straight && self.tile_solid(px + dx / 2, py + dy / 2)
    }

    fn facing_entity(&self) -> Option<usize> {
        self.facing_entity_near().or_else(|| {
            // Nothing right in front: someone across a counter in the facing direction?
            let ts = self.ts();
            let (fx, fy) = self.player.facing.dir();
            (0..self.loaded.entities.len()).find(|&i| {
                let d = self.loaded.entities[i].body.pos - self.player.pos;
                self.across_counter(i) && (d.x * fx + d.y * fy) > ts
            })
        })
    }

    fn facing_entity_near(&self) -> Option<usize> {
        let ts = self.ts();
        let (dx, dy) = self.player.facing.dir();
        let center = self.player.pos + vec2(dx, dy) * ts * 0.6 - vec2(0.0, self.player.hitbox.y / 2.0);
        let probe = Rect::new(center.x - ts * 0.4, center.y - ts * 0.4, ts * 0.8, ts * 0.8);
        self.loaded
            .entities
            .iter()
            .enumerate()
            .filter(|(_, e)| e.active && e.body.rect().overlaps(&probe))
            .min_by(|(_, a), (_, b)| {
                let da = a.body.pos.distance_squared(center);
                let db = b.body.pos.distance_squared(center);
                da.total_cmp(&db)
            })
            .map(|(i, _)| i)
    }

    fn fade_active(&self) -> bool {
        !matches!(self.fade, Fade::None)
    }

    fn interact(&mut self) {
        if let Some(i) = self.facing_entity() {
            self.interact_entity(i);
            return;
        }
        for uid in self.zones_inside.clone() {
            self.fire(GameEvent::Interact, Source::Zone(uid));
        }
    }

    fn interact_entity(&mut self, i: usize) {
        {
            let uid = self.loaded.entities[i].uid;
            match self.loaded.entities[i].kind {
                EntityKind::Item => {
                    let (def, count) = (self.loaded.entities[i].def.clone(), self.loaded.entities[i].count.max(1));
                    let left = self.world.give_item(&def, count);
                    if left == count {
                        self.ui.toast("Inventory full".into());
                        return;
                    }
                    // What didn't fit stays on the ground.
                    self.loaded.entities[i].count = left;
                    self.loaded.entities[i].active = left > 0;
                    self.persist(i);
                    self.item_given(&def, count - left, 0);
                    if left > 0 {
                        self.ui.toast("Inventory full".into());
                    }
                }
                EntityKind::Character => {
                    let p = self.player.pos;
                    let e = &mut self.loaded.entities[i].body;
                    e.facing = Facing::from_vec(p.x - e.pos.x, p.y - e.pos.y).unwrap_or(e.facing);
                    e.moving = false;
                }
                EntityKind::Prop => {
                    // Open gates / doors (not containers) can be locked again with their key.
                    if self.loaded.entities[i].unlocked && !self.is_container(i) && self.try_relock(i, None) {
                        return;
                    }
                    // Locked props need their key first (it is used up when unlocking).
                    if !self.try_unlock(i, None) {
                        return;
                    }
                    if !self.loaded.entities[i].active {
                        return; // e.g. a gate that disappeared on unlocking
                    }
                }
            }
            self.fire(GameEvent::Interact, Source::Entity(uid));
            match self.loaded.entities[i].kind {
                EntityKind::Prop => self.try_open_container(i),
                EntityKind::Character => {
                    self.try_open_shop(i, true);
                }
                EntityKind::Item => {}
            }
        }
    }

    /// Gameplay is paused while a dialog or an inventory window is open.
    fn modal(&self) -> bool {
        self.ui.dialog_open() || self.inventory_open()
    }

    /// Applies project rules (inventory size, stack limits) to the world state, e.g. after loading.
    fn prepare_world(&mut self) {
        self.migrate_equipment();
        let limits = self.project.db.stack_limits();
        self.world.inventory.resize(self.project.config.inventory_size.max(1) as usize);
        self.world.inventory.set_limits(limits.clone());
        for inv in self.world.containers.values_mut().flat_map(|m| m.values_mut()) {
            inv.set_limits(limits.clone());
        }
    }

    fn use_item(&mut self, item: &str) {
        if self.project.db.item(item).is_some_and(|d| d.equip_slot().is_some()) {
            self.equip_item(item);
            return;
        }
        // Using the right key on a locked prop opens it.
        if let Some(i) = self.facing_entity() {
            if self.active_lock(i).is_some_and(|l| l.key == item) {
                if self.try_unlock(i, Some(item)) && self.loaded.entities[i].active {
                    self.try_open_container(i);
                }
                return;
            }
            // ... and using it on an opened one locks it again (also containers).
            if self.relockable(i).is_some_and(|l| l.key == item) && self.try_relock(i, Some(item)) {
                return;
            }
        }
        let ev = GameEvent::UseItem { item: item.to_string() };
        let mut fired = false;
        if let Some(i) = self.facing_entity() {
            fired = self.fire(ev.clone(), Source::Entity(self.loaded.entities[i].uid));
        }
        if !fired {
            fired = self.fire(ev, Source::Item(item.to_string()));
        }
        if !fired {
            self.ui.toast("Nothing happens.".into());
        } else if self.project.db.item(item).is_some_and(|d| d.consumable) {
            self.world.take_item(item, 1);
        }
    }

    fn update_entities(&mut self, dt: f32, frozen: bool) {
        let ts = self.ts();
        for i in 0..self.loaded.entities.len() {
            let e = &self.loaded.entities[i];
            if !e.active || e.kind != EntityKind::Character {
                continue;
            }
            if let Some(target) = e.body.move_target {
                let (pos, done) = step_towards(e.body.pos, target, e.body.speed.max(30.0) * dt);
                let e = &mut self.loaded.entities[i];
                e.body.facing = Facing::from_vec(target.x - e.body.pos.x, target.y - e.body.pos.y)
                    .unwrap_or(e.body.facing);
                e.body.pos = pos;
                e.body.moving = !done;
                e.body.anim += dt;
                if done {
                    e.body.move_target = None;
                    e.home = pos;
                    self.persist(i);
                }
                continue;
            }
            if frozen {
                self.loaded.entities[i].body.moving = false;
                continue;
            }
            if self.entity_knockback(i, dt) || self.chase(i, dt) {
                continue;
            }
            let e = &self.loaded.entities[i];
            let radius = match e.behavior {
                Behavior::Wander { radius } => radius,
                Behavior::Chase { .. } => 3.0,
                Behavior::Static => continue,
            };
            let e = &mut self.loaded.entities[i];
            e.wander_timer -= dt;
            if e.wander_timer <= 0.0 {
                if e.wander_dir == Vec2::ZERO && rand::gen_range(0, 3) > 0 {
                    let dirs = [vec2(1., 0.), vec2(-1., 0.), vec2(0., 1.), vec2(0., -1.)];
                    e.wander_dir = dirs[rand::gen_range(0, 4)];
                    e.wander_timer = rand::gen_range(0.4, 1.4);
                } else {
                    e.wander_dir = Vec2::ZERO;
                    e.wander_timer = rand::gen_range(1.0, 3.0);
                }
            }
            if e.wander_dir == Vec2::ZERO {
                e.body.moving = false;
                continue;
            }
            let delta = e.wander_dir * e.body.speed * dt;
            let (home, body, uid) = (e.home, e.body.clone(), e.uid);
            let next = body.pos + delta;
            let within = next.distance(home) <= radius * ts || next.distance(home) < body.pos.distance(home);
            let new_pos = if within { self.try_move(&body, delta, Some(uid), true) } else { body.pos };
            let e = &mut self.loaded.entities[i];
            if new_pos == e.body.pos {
                e.wander_dir = Vec2::ZERO; // bumped into something
            }
            e.body.facing = Facing::from_vec(delta.x, delta.y).unwrap_or(e.body.facing);
            e.body.pos = new_pos;
            e.body.moving = new_pos != body.pos;
            e.body.anim += dt;
        }
    }

    /// The player chose Quit in the pause menu.
    pub fn wants_quit(&self) -> bool {
        self.menu.quit
    }

    /// One-line summary of the game state (for logs and automated tests).
    pub fn status(&self) -> String {
        let p = self.player.pos / self.ts();
        format!(
            "quests={:?} shop={:?} map={} tile=({},{}) hp={:?} vars={:?} quick={:?} equip={:?} enemies={} busy={} dialog={} inv={:?} flags={:?} music={:?}",
            self.world.quests.iter().map(|(k, q)| format!("{k}:{}:{:?}", q.stage, q.status)).collect::<Vec<_>>(),
            self.shop_status(),
            self.loaded.map.name,
            p.x.floor(),
            p.y.floor(),
            self.world.player_hp,
            self.world.vars,
            self.world.quickbar,
            [crate::defs::EquipSlot::Head, crate::defs::EquipSlot::Body, crate::defs::EquipSlot::Weapon]
                .map(|s| self.world.equipment.item(s).unwrap_or("-").to_string()),
            self.enemies_alive(),
            self.runner.busy(),
            self.ui.dialog_open(),
            self.world.inventory.summary(),
            self.world.flags,
            self.audio.current_music()
        )
    }

    // ---------------------------------------------------------------- save / load

    fn sync_world(&mut self) {
        self.world.current_map = self.loaded.map.name.clone();
        self.world.player_pos = (self.player.pos.x, self.player.pos.y);
        self.world.player_facing = self.player.facing;
    }

    pub fn save_game(&mut self, slot: &str) -> Result<()> {
        if self.runner.busy() {
            return Err(anyhow!("can't save during an event"));
        }
        self.sync_world();
        self.world.saved_at = menu::now_secs();
        let dir = self.project.saves_dir();
        std::fs::create_dir_all(&dir)?;
        std::fs::write(dir.join(format!("{slot}.ron")), self.world.to_ron()?)?;
        self.slot = Some(slot.to_string());
        Ok(())
    }

    pub fn load_game(&mut self, slot: &str) -> Result<()> {
        let s = std::fs::read_to_string(self.project.saves_dir().join(format!("{slot}.ron")))?;
        let world = WorldState::from_ron(&s)?;
        self.slot = Some(slot.to_string());
        let map = world.current_map.clone();
        let pos = vec2(world.player_pos.0, world.player_pos.1);
        self.player.facing = world.player_facing;
        self.world = world;
        self.prepare_world();
        self.runner.clear();
        self.ui = Ui::default();
        self.inv = Default::default();
        self.game_over = None;
        self.combat.reset();
        if self.world.player_hp.is_none_or(|hp| hp <= 0) {
            self.world.player_hp = Some(self.combat.max_hp);
        }
        // Rebuild the map from scratch so entity states from the save apply.
        self.cache.clear();
        self.loaded.map.name.clear();
        self.transition(&map, Arrival::Pos(pos));
        self.ui.toast("Game loaded".into());
        Ok(())
    }

    // ---------------------------------------------------------------- drawing

    /// The game camera: follows the player, clamped to the map.
    fn camera(&self) -> Camera2D {
        let ts = self.ts();
        let m = &self.loaded.map;
        let scale = self.project.config.scale.max(1.0);
        let (vw, vh) = (screen_width() / scale, screen_height() / scale);
        let (mw, mh) = (m.width as f32 * ts, m.height as f32 * ts);
        let clamp_axis = |p: f32, view: f32, size: f32| {
            if size <= view { size / 2.0 } else { p.clamp(view / 2.0, size - view / 2.0) }
        };
        let mut focus = vec2(self.player.pos.x, self.player.pos.y - 8.0);
        if self.on_title() {
            // The title screen slowly drifts over the start map.
            let t = get_time() as f32;
            focus += vec2((t * 0.07).sin() * 12.0 * ts, (t * 0.05).cos() * 6.0 * ts);
        }
        let target = vec2(clamp_axis(focus.x, vw, mw), clamp_axis(focus.y, vh, mh));
        render::pixel_camera(target, scale)
    }

    /// Screen position of the center of a tile (used by automated play-throughs).
    pub fn tile_to_screen(&self, tx: f32, ty: f32) -> Vec2 {
        let ts = self.ts();
        self.camera().world_to_screen(vec2((tx + 0.5) * ts, (ty + 0.5) * ts))
    }

    pub fn draw(&mut self) {
        let ts = self.ts();
        let m = &self.loaded.map;
        let bg = m.background;
        clear_background(Color::from_rgba(bg[0], bg[1], bg[2], 255));

        let cam = self.camera();
        set_camera(&cam);
        let view = render::camera_view(&cam);
        let sets = self.assets.map_tilesets(m);

        for layer in m.layers.iter().filter(|l| l.visible && !l.above_entities) {
            render::draw_layer(m, layer, &sets, ts, view, WHITE);
        }

        // Y-sorted entities + player.
        let mut order: Vec<(f32, Option<usize>)> = self
            .loaded
            .entities
            .iter()
            .enumerate()
            .filter(|(_, e)| e.active)
            .map(|(i, e)| (e.body.pos.y, Some(i)))
            .collect();
        order.push((self.player.pos.y, None));
        order.sort_by(|a, b| a.0.total_cmp(&b.0));
        let t = get_time() as f32;
        for (_, idx) in order {
            match idx {
                None if self.on_title() => {}
                None => {
                    // Blink while invulnerable after being hit.
                    if self.combat.invuln > 0.0 && (self.combat.invuln * 12.0) as i32 % 2 == 0 {
                        continue;
                    }
                    let col = if self.player.moving { (self.player.anim * 8.0) as u32 % self.player_frames } else { 0 };
                    let frame = self.player.facing as u32 * self.player_frames + col;
                    let drawn = self
                        .player_sheet
                        .as_ref()
                        .is_some_and(|s| self.assets.draw_frame(s, frame, self.player.pos, WHITE));
                    self.draw_armor_overlays(frame, self.player.pos, WHITE);
                    if !drawn {
                        let r = self.player.rect();
                        draw_rectangle(r.x, r.y - ts * 0.5, r.w, r.h + ts * 0.5, YELLOW);
                    }
                }
                Some(i) => self.draw_entity(&self.loaded.entities[i], t),
            }
        }

        self.draw_combat_world();
        if !self.on_title() {
            self.draw_mouse_world();
            self.draw_padlocks();
        }

        for layer in m.layers.iter().filter(|l| l.visible && l.above_entities) {
            render::draw_layer(m, layer, &sets, ts, view, WHITE);
        }

        if self.debug {
            self.draw_debug(view);
        }

        set_default_camera();
        let alpha = match &self.fade {
            Fade::None => 0.0,
            Fade::Out { t, .. } => (t / FADE_TIME).min(1.0),
            Fade::In { t } => (t / FADE_TIME).max(0.0),
        };
        if alpha > 0.0 {
            draw_rectangle(0.0, 0.0, screen_width(), screen_height(), Color::new(0., 0., 0., alpha));
        }
        if !self.on_title() {
            self.draw_hud();
            if self.shop_open() {
                self.draw_shop_ui();
            } else if self.journal_open() {
                self.draw_journal();
            } else {
                self.draw_inventory_ui();
            }
        }
        self.ui.draw_toasts();
        self.ui.draw_dialog();
        self.draw_game_over();
        self.draw_menu();
        if self.debug {
            let p = self.player.pos / ts;
            draw_text(
                format!("{}  tile {:.0},{:.0}  fps {}", self.loaded.map.name, p.x.floor(), p.y.floor(), get_fps()),
                8.0,
                screen_height() - 8.0,
                20.0,
                WHITE,
            );
        }
    }

    fn draw_entity(&self, e: &RtEntity, t: f32) {
        let ts = self.ts();
        let db = &self.project.db;
        let tint = if e.flash > 0.0 { Color::new(1.0, 0.35, 0.35, 1.0) } else { WHITE };
        let drawn = match e.kind {
            EntityKind::Prop => db.prop(&e.def).is_some_and(|d| self.assets.draw_sprite(d.state_look(e.state).0, e.frame, e.body.pos, WHITE)),
            EntityKind::Character => db.character(&e.def).is_some_and(|d| {
                let frames = d.walk_frames.max(1);
                let col = if e.body.moving { (e.body.anim * 8.0) as u32 % frames } else { 0 };
                let sheet = if e.sheet.is_empty() { &d.sheet } else { &e.sheet };
                self.assets.draw_frame(sheet, e.body.facing as u32 * frames + col, e.body.pos, tint)
            }),
            EntityKind::Item => db.item(&e.def).is_some_and(|d| {
                let u = self.u();
                let bob = ((t * 3.0 + e.uid as f32).sin() * 1.5 * u).round();
                self.assets.draw_sprite(&d.icon, 0, e.body.pos - vec2(0.0, bob + 2.0 * u), WHITE)
            }),
        };
        if !drawn {
            let r = e.body.rect();
            draw_rectangle(r.x, r.y - ts * 0.5, r.w, r.h + ts * 0.5, MAGENTA);
        }
    }

    fn draw_debug(&self, view: Rect) {
        let ts = self.ts();
        let m = &self.loaded.map;
        let x0 = (view.x / ts).floor().max(0.0) as u32;
        let y0 = (view.y / ts).floor().max(0.0) as u32;
        let x1 = ((view.x + view.w) / ts).ceil().min(m.width as f32) as u32;
        let y1 = ((view.y + view.h) / ts).ceil().min(m.height as f32) as u32;
        for y in y0..y1 {
            for x in x0..x1 {
                if self.loaded.solid[(y * m.width + x) as usize] {
                    draw_rectangle(x as f32 * ts, y as f32 * ts, ts, ts, Color::new(1., 0., 0., 0.25));
                }
            }
        }
        for z in &m.zones {
            let r = z.pixel_rect(ts);
            draw_rectangle_lines(r.x, r.y, r.w, r.h, 1.0, SKYBLUE);
        }
        for e in Edge::ALL {
            if let Some(r) = m.edges.strip_rect(e, m.width, m.height, ts) {
                draw_rectangle(r.x, r.y, r.w, r.h, Color::new(0., 1., 0., 0.15));
            }
        }
        for e in self.loaded.entities.iter().filter(|e| e.active) {
            let r = e.body.rect();
            draw_rectangle_lines(r.x, r.y, r.w, r.h, 1.0, ORANGE);
        }
        let r = self.player.rect();
        draw_rectangle_lines(r.x, r.y, r.w, r.h, 1.0, GREEN);
    }
}

fn step_towards(pos: Vec2, target: Vec2, step: f32) -> (Vec2, bool) {
    let d = target - pos;
    if d.length() <= step {
        (target, true)
    } else {
        (pos + d.normalize() * step, false)
    }
}

impl ActionHost for Game {
    fn world(&mut self) -> &mut WorldState {
        &mut self.world
    }

    fn execute(&mut self, action: &Action, ctx: &ActionCtx) -> Step {
        let ts = self.ts();
        match action {
            Action::Dialog { speaker, text } => {
                self.audio.play(&self.project.config.sfx.dialog);
                self.ui.show_dialog(speaker, text);
                Step::Block
            }
            Action::Choice { speaker, text, options, var } => {
                self.audio.play(&self.project.config.sfx.dialog);
                self.ui.show_choice(speaker, text, options, var);
                Step::Block
            }
            Action::Teleport { map, spawn } => {
                self.audio.play(&self.project.config.sfx.portal);
                self.transition(map, Arrival::Spawn(spawn.clone()));
                Step::Block
            }
            Action::PlaySound { sound } => {
                self.audio.play(sound);
                Step::Continue
            }
            Action::PlayMusic { sound } => {
                self.audio.play_music(sound);
                Step::Continue
            }
            Action::Heal { amount } => {
                self.heal(*amount);
                Step::Continue
            }
            Action::SetLocked { target, locked } => {
                let Some(i) = self.entity_index(target, ctx.owner) else { return Step::Continue };
                if *locked {
                    if !self.loaded.entities[i].unlocked {
                        return Step::Continue;
                    }
                    self.apply_lock(i);
                    let uid = self.loaded.entities[i].uid;
                    return Step::Insert(self.collect(&GameEvent::Lock, &Source::Entity(uid)));
                }
                let Some(lock) = self.active_lock(i) else { return Step::Continue };
                self.apply_unlock(i, &lock.on_unlock);
                let uid = self.loaded.entities[i].uid;
                // Unlock triggers run before the rest of this sequence.
                Step::Insert(self.collect(&GameEvent::Unlock, &Source::Entity(uid)))
            }
            Action::SetActive { target, active } => {
                if let Some(i) = self.entity_index(target, ctx.owner) {
                    self.loaded.entities[i].active = *active;
                    self.persist(i);
                }
                Step::Continue
            }
            Action::SetState { target, state } => {
                if let Some(i) = self.entity_index(target, ctx.owner) {
                    self.set_prop_state(i, state);
                }
                Step::Continue
            }
            Action::Quest { quest, op, stage } => {
                self.quest_changed(quest, *op, *stage);
                Step::Continue
            }
            Action::OpenShop { target } => {
                match self.entity_index(target, ctx.owner) {
                    Some(i) if self.try_open_shop(i, false) => {}
                    _ => eprintln!("Open shop: '{target}' is not a merchant on this map"),
                }
                Step::Continue
            }
            Action::SetFrame { target, frame } => {
                if let Some(i) = self.entity_index(target, ctx.owner) {
                    self.loaded.entities[i].frame = *frame;
                    self.persist(i);
                }
                Step::Continue
            }
            Action::Move { target, dx, dy } => {
                let delta = vec2(*dx as f32, *dy as f32) * ts;
                if target == "player" {
                    self.player.move_target = Some(self.player.pos + delta);
                } else if let Some(i) = self.entity_index(target, ctx.owner) {
                    let b = &mut self.loaded.entities[i].body;
                    b.move_target = Some(b.pos + delta);
                }
                Step::Block
            }
            Action::Face { target, facing } => {
                if target == "player" {
                    self.player.facing = *facing;
                } else if let Some(i) = self.entity_index(target, ctx.owner) {
                    self.loaded.entities[i].body.facing = *facing;
                }
                Step::Continue
            }
            Action::Emit { name } => {
                Step::Insert(self.collect(&GameEvent::Custom { name: name.clone() }, &Source::Broadcast))
            }
            Action::RunScript { function } => {
                let actions = self.scripts.call(&mut self.world, function, ctx.owner);
                Step::Insert(actions.into_iter().map(|a| (a, ctx.clone())).collect())
            }
            // Handled by the runner itself.
            Action::GiveItem { .. }
            | Action::TakeItem { .. }
            | Action::SetFlag { .. }
            | Action::SetVar { .. }
            | Action::Wait { .. }
            | Action::If { .. } => Step::Continue,
        }
    }

    fn blocked(&mut self) -> bool {
        self.ui.dialog_open()
            || !matches!(self.fade, Fade::None)
            || self.player.move_target.is_some()
            || self.loaded.entities.iter().any(|e| e.body.move_target.is_some())
    }

    fn script_condition(&mut self, function: &str, ctx: &ActionCtx) -> bool {
        self.scripts.condition(&mut self.world, function, ctx.owner)
    }

    fn prop_state(&mut self, target: &str, ctx: &ActionCtx) -> Option<String> {
        let i = self.entity_index(target, ctx.owner)?;
        let e = &self.loaded.entities[i];
        self.project.db.prop(&e.def).map(|d| d.state_name(e.state).to_string())
    }

    fn item_given(&mut self, item: &str, count: u32, lost: u32) {
        let name = self.project.db.item(item).map_or(item.to_string(), |d| d.name.clone());
        if count > 0 {
            self.audio.play(&self.project.config.sfx.pickup);
            self.ui.toast(if count > 1 { format!("Got {name} x{count}") } else { format!("Got {name}") });
        }
        if lost > 0 {
            self.ui.toast(format!("Inventory full: {name} x{lost} lost"));
        }
    }
}
