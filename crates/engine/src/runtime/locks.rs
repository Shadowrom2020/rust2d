//! Key locks on props (gates, doors, chests).

use macroquad::prelude::*;

use super::{Game, Source};
use crate::defs::{LockSpec, OnUnlock};
use crate::events::GameEvent;

/// Padlocks are shown on locked props closer than this (tiles).
const PADLOCK_DIST: f32 = 4.0;

impl Game {
    /// The lock of entity `i` if it is still locked.
    pub(super) fn active_lock(&self, i: usize) -> Option<LockSpec> {
        let e = &self.loaded.entities[i];
        if e.unlocked {
            return None;
        }
        let inst = self.loaded.map.entity(e.uid)?;
        self.project.db.lock_of(inst).cloned()
    }

    /// The lock of entity `i` if it is open and can be locked again.
    pub(super) fn relockable(&self, i: usize) -> Option<LockSpec> {
        let e = &self.loaded.entities[i];
        if !e.unlocked || !e.active {
            return None;
        }
        let inst = self.loaded.map.entity(e.uid)?;
        self.project.db.lock_of(inst).filter(|l| l.can_relock()).cloned()
    }

    /// Locks the opened entity `i` again if the player has its key (`using` = the item being
    /// used on it). Returns false if it can't be relocked by the player now (no key / not
    /// relockable), true if handled (locked, or blocked with a message).
    pub(super) fn try_relock(&mut self, i: usize, using: Option<&str>) -> bool {
        let Some(lock) = self.relockable(i) else { return false };
        if !lock.key.is_empty() {
            let has_key = using.is_none_or(|u| u == lock.key) && self.world.item_count(&lock.key) > 0;
            if !has_key {
                return false;
            }
        } else if using.is_some() {
            return false;
        }
        // Closing a door / gate on somebody is not possible.
        if matches!(lock.on_unlock, OnUnlock::Open { .. } | OnUnlock::ToState { .. }) && self.closed_solid(i) {
            let r = self.loaded.entities[i].body.rect();
            let uid = self.loaded.entities[i].uid;
            let blocked = self.player.rect().overlaps(&r)
                || self.loaded.entities.iter().any(|e| e.active && e.uid != uid && e.kind == crate::entity::EntityKind::Character && e.body.rect().overlaps(&r));
            if blocked {
                self.ui.toast("Something is in the way.".into());
                return true;
            }
        }
        let msg = if !lock.lock_text.is_empty() {
            lock.lock_text.clone()
        } else if lock.key.is_empty() {
            "Closed.".to_string()
        } else {
            format!("Locked with the {}.", self.item_name(&lock.key))
        };
        self.ui.toast(msg);
        let sfx = if lock.key.is_empty() { &self.project.config.sfx.portal } else { &self.project.config.sfx.unlock };
        self.audio.play(sfx);
        self.apply_lock(i);
        let uid = self.loaded.entities[i].uid;
        self.fire(GameEvent::Lock, Source::Entity(uid));
        true
    }

    /// Whether entity `i` is a lootable container.
    pub(super) fn is_container(&self, i: usize) -> bool {
        let uid = self.loaded.entities[i].uid;
        self.loaded.map.entity(uid).is_some_and(|e| self.project.db.container_of(e).is_some())
    }

    /// The state a placed prop starts in (its "closed" state for locks).
    fn initial_state(&self, i: usize) -> String {
        let uid = self.loaded.entities[i].uid;
        self.loaded.map.entity(uid).map(|e| e.state.clone()).unwrap_or_default()
    }

    /// Whether entity `i` blocks movement in its closed (initial) state.
    fn closed_solid(&self, i: usize) -> bool {
        let e = &self.loaded.entities[i];
        let start = self.initial_state(i);
        self.project.db.prop(&e.def).is_some_and(|d| d.state_index(&start).is_some_and(|s| d.state_look(s).1))
    }

    /// Marks entity `i` locked and undoes the lock's "open" effect (saved).
    pub(super) fn apply_lock(&mut self, i: usize) {
        let uid = self.loaded.entities[i].uid;
        let inst = self.loaded.map.entity(uid);
        let closed_frame = inst.map(|e| e.frame);
        let on = inst.and_then(|e| self.project.db.lock_of(e)).map(|l| l.on_unlock.clone());
        let solid = self.closed_solid(i);
        let start = self.initial_state(i);
        self.loaded.entities[i].unlocked = false;
        match on {
            Some(OnUnlock::Open { .. }) => {
                let e = &mut self.loaded.entities[i];
                e.frame = closed_frame.unwrap_or(0);
                e.solid = solid;
            }
            Some(OnUnlock::ToState { .. }) => {
                self.set_prop_state(i, &start);
            }
            _ => {}
        }
        self.persist(i);
    }

    pub(super) fn item_name(&self, id: &str) -> String {
        self.project.db.item(id).map_or(id.to_string(), |d| d.name.clone())
    }

    /// Checks the lock of entity `i` against the player's items (or the item being used).
    /// Unlocks (consuming the key) if possible. Returns true if the entity is now unlocked.
    pub(super) fn try_unlock(&mut self, i: usize, using: Option<&str>) -> bool {
        let Some(lock) = self.active_lock(i) else { return true };
        // Lockpicking: a skill of at least the lock's difficulty opens it without the key.
        let picked = using.is_none() && lock.pick_difficulty > 0 && self.lockpick_level() >= lock.pick_difficulty;
        if picked {
            self.ui.toast(if lock.key.is_empty() { "You pick the lock.".into() } else { format!("You pick the lock instead of using the {}.", self.item_name(&lock.key)) });
            self.audio.play(&self.project.config.sfx.unlock);
            self.apply_unlock(i, &lock.on_unlock);
            let uid = self.loaded.entities[i].uid;
            self.fire(GameEvent::Unlock, Source::Entity(uid));
            return true;
        }
        // Lock without a key that can only be picked.
        if lock.key.is_empty() && lock.pick_difficulty > 0 {
            if using.is_none() {
                let msg = if lock.locked_text.is_empty() { format!("It's locked. A lockpicking skill of {} could open it.", lock.pick_difficulty) } else { lock.locked_text.clone() };
                self.ui.toast(msg);
            }
            return false;
        }
        // No key item: a door / gate that simply opens when used.
        if lock.key.is_empty() {
            if using.is_some() {
                return false;
            }
            if !lock.unlock_text.is_empty() {
                self.ui.toast(lock.unlock_text.clone());
            }
            self.audio.play(&self.project.config.sfx.portal);
            self.apply_unlock(i, &lock.on_unlock);
            let uid = self.loaded.entities[i].uid;
            self.fire(GameEvent::Unlock, Source::Entity(uid));
            return true;
        }
        let key_name = self.item_name(&lock.key);
        let has_key = match using {
            Some(item) => item == lock.key && self.world.item_count(item) > 0,
            None => self.world.item_count(&lock.key) > 0,
        };
        if !has_key {
            let hint = if lock.pick_difficulty > 0 && self.skills_on() { format!(" (or lockpicking {})", lock.pick_difficulty) } else { String::new() };
            let msg = if lock.locked_text.is_empty() { format!("It's locked. You need the {key_name}{hint}.") } else { lock.locked_text.clone() };
            self.ui.toast(msg);
            return false;
        }
        if lock.consume_key {
            self.world.take_item(&lock.key, 1);
        }
        let msg = if lock.unlock_text.is_empty() { format!("Unlocked with the {key_name}.") } else { lock.unlock_text.clone() };
        self.ui.toast(msg);
        self.audio.play(&self.project.config.sfx.unlock);
        self.apply_unlock(i, &lock.on_unlock);
        let uid = self.loaded.entities[i].uid;
        self.fire(GameEvent::Unlock, Source::Entity(uid));
        true
    }

    /// Marks entity `i` unlocked and applies the lock's effect (saved).
    pub(super) fn apply_unlock(&mut self, i: usize, on: &OnUnlock) {
        let e = &mut self.loaded.entities[i];
        e.unlocked = true;
        match on {
            OnUnlock::Nothing => {}
            OnUnlock::Disappear => e.active = false,
            OnUnlock::Open { frame } => {
                e.frame = *frame;
                e.solid = false;
            }
            OnUnlock::ToState { state } => {
                let state = state.clone();
                self.set_prop_state(i, &state);
            }
        }
        self.persist(i);
    }

    /// Restores runtime effects of an unlocked lock when a map is loaded.
    pub(super) fn restore_unlocked(&mut self) {
        for i in 0..self.loaded.entities.len() {
            if !self.loaded.entities[i].unlocked {
                continue;
            }
            let uid = self.loaded.entities[i].uid;
            let open = self
                .loaded
                .map
                .entity(uid)
                .and_then(|inst| self.project.db.lock_of(inst))
                .is_some_and(|l| matches!(l.on_unlock, OnUnlock::Open { .. }));
            if open {
                self.loaded.entities[i].solid = false;
            }
        }
    }

    /// Small padlocks over nearby locked props (world space).
    pub(super) fn draw_padlocks(&self) {
        let ts = self.ts();
        for (i, e) in self.loaded.entities.iter().enumerate() {
            // Doors without a key get no padlock.
            if !e.active || e.unlocked || e.body.pos.distance(self.player.pos) > PADLOCK_DIST * ts || self.active_lock(i).is_none_or(|l| l.key.is_empty()) {
                continue;
            }
            let r = self.entity_click_rect(e);
            let u = self.u();
            let (x, y) = ((r.x + r.w - 4.0 * u).round(), (r.y - 3.0 * u).round());
            let gold = Color::new(0.95, 0.78, 0.25, 1.0);
            let dark = Color::new(0.25, 0.18, 0.05, 1.0);
            draw_rectangle_lines(x + u, y - 3.0 * u, 4.0 * u, 4.0 * u, u, dark);
            draw_rectangle(x, y, 6.0 * u, 5.0 * u, gold);
            draw_rectangle(x + 2.5 * u, y + 1.5 * u, u, 2.0 * u, dark);
        }
    }
}
