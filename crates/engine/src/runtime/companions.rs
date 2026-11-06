//! Companions: placed characters that joined the player (`Action::Companion`, `follow(name)`).
//! They follow the player to the tile behind them (A*, like patrols), come along to other maps
//! (`WorldState::companions`), fight hostile characters near the player with their weapon, and
//! are hunted and hurt by enemies like the player. Talking to one runs its own triggers. A
//! companion knocked out, or dismissed, goes back to where it is placed.

use macroquad::prelude::*;

use super::patrol::{Walk, Walker};
use super::{Game, RtEntity};
use crate::entity::{EntityKind, Facing};
use crate::events::Trigger;
use crate::path::TilePos;
use crate::state::Companion;

/// Runtime uids of companions (`COMPANION_UID_BASE + index`), apart from placed entities and
/// dropped items.
pub(super) const COMPANION_UID_BASE: u32 = 0x5000_0000;
/// Farther than this (tiles) from the player: it catches up at once (after a teleport).
const CATCH_UP: f32 = 14.0;
/// It stops following closer than this (tiles).
const NEAR: f32 = 1.6;
/// Hostile characters this close (tiles) to the companion and the player are fought.
const ENGAGE: f32 = 5.0;
/// Seconds per HP regained while no enemy hunts.
const REGEN: f32 = 4.0;

/// What makes a runtime entity a companion.
#[derive(Clone, Debug)]
pub(super) struct Link {
    /// Where it is placed (save keys, going home).
    pub map: String,
    pub uid: u32,
    /// Its placed instance's triggers (talking to it).
    pub triggers: Vec<Trigger>,
    walker: Walker,
    goal: Option<TilePos>,
    regen: f32,
}

impl Game {
    /// The placed entity `uid` of `map` (from the current map, the cache or the file).
    fn placed_instance(&self, map: &str, uid: u32) -> Option<crate::entity::EntityInstance> {
        let find = |m: &crate::map::Map| m.entities.iter().find(|e| e.uid == uid).cloned();
        if self.loaded.map.name == map {
            return find(&self.loaded.map);
        }
        if let Some(m) = self.cache.iter().find(|m| m.name == map) {
            return find(m);
        }
        self.project.load_map(map).ok().as_ref().and_then(find)
    }

    /// Runtime entities of the companions (entering a map; placed around the player later).
    pub(super) fn build_companions(&self) -> Vec<RtEntity> {
        let mut out = Vec::new();
        for (k, c) in self.world.companions.iter().enumerate() {
            let Some(inst) = self.placed_instance(&c.map, c.uid) else {
                eprintln!("[game] companion {}#{} is no longer placed; it is left out", c.map, c.uid);
                continue;
            };
            let mut rt = self.build_placed(&c.map, &inst);
            make_companion(&mut rt, &c.map, inst.triggers.clone(), k);
            if let Some(hp) = c.hp {
                rt.hp = hp.clamp(1, rt.max_hp.max(1));
            }
            out.push(rt);
        }
        out
    }

    /// Puts the companions on free spots behind the player (entering a map, catching up).
    pub(super) fn place_companions(&mut self) {
        for i in 0..self.loaded.entities.len() {
            if self.loaded.entities[i].companion.is_some() {
                self.place_companion(i);
            }
        }
    }

    fn place_companion(&mut self, i: usize) {
        let k = self.companion_rank(i);
        let goal = self.companion_goal(k);
        let spot = self.free_spot(self.tile_spot(goal));
        let facing = self.player.facing;
        let e = &mut self.loaded.entities[i];
        e.body.pos = spot;
        e.body.facing = facing;
        e.body.moving = false;
        e.home = spot;
        if let Some(l) = &mut e.companion {
            l.walker = Walker::default();
            l.goal = None;
        }
    }

    /// Position of companion `i` in the line behind the player (0 = first).
    fn companion_rank(&self, i: usize) -> usize {
        self.loaded.entities[..i].iter().filter(|e| e.companion.is_some()).count()
    }

    /// The tile companion number `k` heads for: behind the player (in a line), else beside.
    fn companion_goal(&self, k: usize) -> TilePos {
        let here = self.tile_of(self.player.pos);
        let (dx, dy) = self.player.facing.dir();
        let back = (-dx as i32, -dy as i32);
        let side = (back.1, back.0);
        let n = k as i32 + 1;
        let tries = [
            (here.0 + back.0 * n, here.1 + back.1 * n),
            (here.0 + back.0 * n + side.0, here.1 + back.1 * n + side.1),
            (here.0 + back.0 * n - side.0, here.1 + back.1 * n - side.1),
            (here.0 + side.0 * n, here.1 + side.1 * n),
            (here.0 - side.0 * n, here.1 - side.1 * n),
        ];
        tries.into_iter().find(|t| !self.tile_solid(t.0, t.1)).unwrap_or(here)
    }

    /// Moves / fights for companion `i`. Returns false if `i` is no companion.
    pub(super) fn companion_step(&mut self, i: usize, dt: f32) -> bool {
        if self.loaded.entities[i].companion.is_none() {
            return false;
        }
        let ts = self.ts();
        let pos = self.loaded.entities[i].body.pos;
        let dist = pos.distance(self.player.pos);
        if dist > CATCH_UP * ts {
            self.place_companion(i);
            return true;
        }
        self.companion_regen(i, dt);
        if self.companion_fight(i, dt) {
            return true;
        }
        // Close enough (but not on top of the player, who walks through companions): wait.
        if dist < NEAR * ts && dist > 0.7 * ts {
            let (facing, still) = (self.player.facing, !self.player.moving);
            let e = &mut self.loaded.entities[i];
            e.body.moving = false;
            if still {
                e.body.facing = facing;
            }
            if let Some(l) = &mut e.companion {
                l.walker = Walker::default();
                l.goal = None;
            }
            return true;
        }
        let goal = self.companion_goal(self.companion_rank(i));
        let speed = self.player.speed * if dist > 3.0 * ts { 1.5 } else { 1.0 };
        let Some(l) = self.loaded.entities[i].companion.as_mut() else { return true };
        if l.goal != Some(goal) {
            l.goal = Some(goal);
            l.walker = Walker::default();
        }
        let mut walker = std::mem::take(&mut l.walker);
        let walked = self.walk_to_tile(i, goal, &mut walker, speed, dt);
        if let Some(l) = self.loaded.entities[i].companion.as_mut() {
            l.walker = walker;
            if walked != Walk::Walking {
                l.goal = None; // there, or no way: plan again next frame
            }
        }
        true
    }

    /// Regains 1 HP every few seconds while no enemy hunts.
    fn companion_regen(&mut self, i: usize, dt: f32) {
        let calm = !self.loaded.entities.iter().any(|e| e.active && e.hostile && e.ai.aggro > 0.0);
        let e = &mut self.loaded.entities[i];
        if e.max_hp <= 0 || e.hp >= e.max_hp {
            return;
        }
        let Some(l) = &mut e.companion else { return };
        l.regen = if calm { l.regen + dt } else { 0.0 };
        if l.regen >= REGEN {
            l.regen = 0.0;
            e.hp += 1;
        }
    }

    /// Writes companion `i`'s HP into the world state (saved).
    pub(super) fn sync_companion_hp(&mut self, i: usize) {
        let e = &self.loaded.entities[i];
        let Some(l) = &e.companion else { return };
        let hp = (e.hp < e.max_hp).then_some(e.hp);
        if let Some(c) = self.world.companions.iter_mut().find(|c| c.map == l.map && c.uid == l.uid) {
            c.hp = hp;
        }
    }

    /// Fights the nearest hostile character close to it and to the player: walks up to it and
    /// hits it in reach (melee), or shoots it in range and in sight. Returns true while fighting.
    fn companion_fight(&mut self, i: usize, dt: f32) -> bool {
        let ts = self.ts();
        let e = &self.loaded.entities[i];
        let Some(w) = e.weapon.clone() else { return false };
        if w.damage <= 0 || crate::effects::stunned(&e.effects) {
            return false;
        }
        let me = e.body.pos;
        let eye = me - vec2(0.0, e.body.hitbox.y / 2.0);
        let target = (0..self.loaded.entities.len())
            .filter(|&t| {
                let o = &self.loaded.entities[t];
                o.active && o.hostile && o.max_hp > 0 && o.body.pos.distance(me) <= ENGAGE * ts && o.body.pos.distance(self.player.pos) <= ENGAGE * 1.5 * ts
            })
            .filter(|&t| self.line_clear(eye, self.loaded.entities[t].body.pos - vec2(0.0, self.loaded.entities[t].body.hitbox.y / 2.0)))
            .min_by(|&a, &b| self.loaded.entities[a].body.pos.distance(me).total_cmp(&self.loaded.entities[b].body.pos.distance(me)));
        let Some(t) = target else { return false };
        let o = &self.loaded.entities[t];
        let to = o.body.pos - me;
        let dir = to.try_normalize().unwrap_or(vec2(0.0, 1.0));
        let radius = o.body.hitbox.x.max(o.body.hitbox.y) / 2.0;
        let e = &mut self.loaded.entities[i];
        e.attack_cd -= dt;
        e.body.facing = Facing::from_vec(dir.x, dir.y).unwrap_or(e.body.facing);
        let damage = crate::effects::scale_damage(&e.effects, w.damage);
        if let Some(r) = &w.ranged {
            if to.length() <= r.range * ts {
                e.body.moving = false;
                if e.attack_cd <= 0.0 {
                    e.attack_cd = w.cooldown.max(0.3) + 0.2;
                    let (r, uid, fx) = (r.clone(), e.uid, w.effect);
                    self.audio.play(&self.project.config.sfx.attack);
                    self.shoot(eye, (o_mid(&self.loaded.entities[t]) - eye).normalize_or_zero(), &r, damage, true, Some(uid), fx);
                }
                return true;
            }
        } else if to.length() - radius <= w.reach.max(0.5) * ts {
            e.body.moving = false;
            if e.attack_cd <= 0.0 {
                e.attack_cd = w.cooldown.max(0.3) + 0.15;
                let fx = w.effect;
                self.audio.play(&self.project.config.sfx.attack);
                if let Some(fx) = &fx {
                    crate::effects::apply(&mut self.loaded.entities[t].effects, fx);
                }
                self.damage_entity(t, damage, me, w.knockback);
            }
            return true;
        }
        // Not in reach yet: close in.
        self.step_towards_dir(i, dir, dt, None);
        true
    }

    /// A placed character on this map joins (`Action::Companion`).
    pub(super) fn recruit(&mut self, i: usize) {
        let e = &self.loaded.entities[i];
        if e.companion.is_some() {
            return;
        }
        if e.kind != EntityKind::Character || e.spawned_by.is_some() || e.uid >= crate::state::DROPPED_UID_BASE || e.hostile {
            eprintln!("[game] Companion: '{}' can't join (only placed, friendly characters can)", e.def);
            return;
        }
        let map = self.loaded.map.name.clone();
        let Some(inst) = self.loaded.map.entity(e.uid).cloned() else { return };
        // Its runtime uid: one past the companions here (uids stay unique on this map).
        let k = self.loaded.entities.iter().filter(|e| (COMPANION_UID_BASE..crate::state::DROPPED_UID_BASE).contains(&e.uid)).map(|e| e.uid - COMPANION_UID_BASE + 1).max().unwrap_or(0) as usize;
        self.world.companions.push(Companion { map: map.clone(), uid: e.uid, name: e.name.clone(), hp: (e.hp < e.max_hp).then_some(e.hp) });
        make_companion(&mut self.loaded.entities[i], &map, inst.triggers, k);
        let name = self.display_name(i);
        self.ui.toast(format!("{name} joins you."));
    }

    /// Companion `i` leaves: it stays here if this is its map, else it goes back there.
    /// `knocked_out`: it was defeated (it goes home with full HP).
    pub(super) fn dismiss(&mut self, i: usize, knocked_out: bool) {
        let Some(link) = self.loaded.entities[i].companion.take() else { return };
        self.world.companions.retain(|c| !(c.map == link.map && c.uid == link.uid));
        let name = self.display_name(i);
        let (pos, facing) = (self.loaded.entities[i].body.pos, self.loaded.entities[i].body.facing);
        if link.map == self.loaded.map.name && !knocked_out {
            // Back to its own self on its own map, where it is now.
            if let Some(inst) = self.loaded.map.entity(link.uid).cloned() {
                let mut rt = self.build_placed(&link.map, &inst);
                rt.body.pos = pos;
                rt.body.facing = facing;
                rt.home = pos;
                rt.active = true;
                self.loaded.entities[i] = rt;
                self.persist(i);
                // A schedule takes it where the hour wants it.
                if let Some(s) = &mut self.loaded.entities[i].schedule {
                    s.current = None;
                }
            }
            self.ui.toast(format!("{name} stays here."));
            return;
        }
        // Gone home: where it is placed (its saved position is forgotten).
        if let Some(inst) = self.placed_instance(&link.map, link.uid) {
            if let Some(st) = self.world.entities.get_mut(&link.map).and_then(|m| m.get_mut(&link.uid)) {
                (st.x, st.y, st.active) = (inst.x, inst.y, true);
            }
            if link.map == self.loaded.map.name {
                let mut rt = self.build_placed(&link.map, &inst);
                rt.active = true;
                self.loaded.entities[i] = rt;
                self.place_on_schedule(i);
                self.toast_gone(&name, knocked_out);
                return;
            }
        }
        // Gone from this map (the entity stays, inactive, until the map is entered again:
        // indices of the frame's loops stay valid).
        let e = &mut self.loaded.entities[i];
        e.active = false;
        e.body.moving = false;
        self.toast_gone(&name, knocked_out);
    }

    fn toast_gone(&mut self, name: &str, knocked_out: bool) {
        self.ui.toast(if knocked_out { format!("{name} is knocked out and limps home.") } else { format!("{name} heads home.") });
    }

    /// A companion's name for messages.
    fn display_name(&self, i: usize) -> String {
        let e = &self.loaded.entities[i];
        self.project.db.character(&e.def).map(|d| d.name.clone()).filter(|n| !n.is_empty()).unwrap_or_else(|| e.name.clone())
    }

    /// Living companions that can be hurt (enemies go for them too): index and body middle.
    pub(super) fn companion_targets(&self) -> Vec<(usize, Vec2)> {
        (0..self.loaded.entities.len()).filter(|&i| is_hurtable_companion(&self.loaded.entities[i])).map(|i| (i, o_mid(&self.loaded.entities[i]))).collect()
    }

    /// Hostile characters touching companions hurt them (like the player).
    pub(super) fn companion_contact_damage(&mut self) {
        for c in 0..self.loaded.entities.len() {
            let e = &self.loaded.entities[c];
            if !is_hurtable_companion(e) || e.invuln > 0.0 {
                continue;
            }
            let me = e.body.rect();
            let hit = self
                .loaded
                .entities
                .iter()
                .position(|o| o.active && o.hostile && o.damage > 0 && !o.ai.harmless() && !crate::effects::stunned(&o.effects) && o.body.rect().overlaps(&me));
            if let Some(h) = hit {
                let o = &self.loaded.entities[h];
                let (dmg, from) = (crate::effects::scale_damage(&o.effects, o.damage), o.body.pos);
                self.damage_entity(c, dmg, from, 1.0);
            }
        }
    }

    /// For `status()`: "name:hp/max@x,y" per companion on this map.
    pub(super) fn companions_status(&self) -> Vec<String> {
        self.loaded
            .entities
            .iter()
            .filter(|e| e.companion.is_some())
            .map(|e| {
                let t = self.tile_of(e.body.pos);
                let name = if e.name.is_empty() { e.def.as_str() } else { e.name.as_str() };
                format!("{name}:{}/{}@{},{}", e.hp, e.max_hp, t.0, t.1)
            })
            .collect()
    }
}

/// Turns a placed character's runtime entity into companion number `k`.
fn make_companion(rt: &mut RtEntity, map: &str, triggers: Vec<Trigger>, k: usize) {
    let home_uid = rt.uid;
    rt.uid = COMPANION_UID_BASE + k as u32;
    rt.active = true;
    rt.away = false;
    rt.patrol = None;
    rt.schedule = None;
    rt.body.move_target = None;
    // The player walks through companions (they never block a corridor).
    rt.solid = false;
    rt.companion = Some(Box::new(Link { map: map.to_string(), uid: home_uid, triggers, walker: Walker::default(), goal: None, regen: 0.0 }));
}

fn is_hurtable_companion(e: &RtEntity) -> bool {
    e.active && e.companion.is_some() && e.max_hp > 0
}

/// The middle of a character's body (what is aimed at).
fn o_mid(e: &RtEntity) -> Vec2 {
    e.body.pos - vec2(0.0, e.body.hitbox.y / 2.0)
}
