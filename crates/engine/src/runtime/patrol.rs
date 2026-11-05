//! Patrolling characters: walk from waypoint to waypoint (A* on the tile grid around solid
//! tiles), pause at each, loop or go back and forth. Talking to them, dialogs and scripted
//! moves pause the patrol; its progress is saved with the character's state.

use macroquad::prelude::*;

use super::Game;
use crate::entity::{Facing, PatrolRoute};
use crate::path::{find_path, TilePos};
use crate::state::PatrolState;

/// A* budget per waypoint (routes are short; this keeps big maps cheap).
const MAX_NODES: usize = 6_000;
/// Seconds without progress before the path is planned again / the waypoint is skipped.
const REPLAN_AFTER: f32 = 1.0;
const SKIP_AFTER: f32 = 4.0;

/// Runtime patrol state of a character.
#[derive(Clone, Debug, Default)]
pub(super) struct Patrol {
    pub route: PatrolRoute,
    /// The route came from an action / script (saved with the character).
    pub custom: bool,
    pub on: bool,
    pub index: usize,
    pub back: bool,
    pub wait: f32,
    /// Remaining tiles to the current waypoint (next first).
    path: Vec<TilePos>,
    stuck: f32,
}

impl Patrol {
    pub fn new(route: PatrolRoute) -> Self {
        Patrol { route, on: true, ..Default::default() }
    }

    /// Applies saved progress.
    pub fn restore(&mut self, st: &PatrolState) {
        if let Some(r) = &st.route {
            self.route = r.clone();
            self.custom = true;
        }
        self.index = (st.index as usize).min(self.route.points.len().saturating_sub(1));
        self.back = st.back;
        self.on = !st.stopped;
    }

    pub fn state(&self) -> PatrolState {
        PatrolState { index: self.index as u32, back: self.back, stopped: !self.on, route: self.custom.then(|| self.route.clone()) }
    }

    /// Starts over with another route.
    pub fn set_route(&mut self, route: PatrolRoute, custom: bool) {
        *self = Patrol { route, custom, on: true, ..Default::default() };
    }

    fn advance(&mut self) {
        let (i, back) = self.route.next(self.index, self.back);
        self.index = i;
        self.back = back;
        self.path.clear();
        self.stuck = 0.0;
    }
}

impl Game {
    /// Feet position a character aims for on a tile.
    pub(super) fn tile_spot(&self, t: TilePos) -> Vec2 {
        let ts = self.ts();
        vec2((t.0 as f32 + 0.5) * ts, (t.1 as f32 + 1.0) * ts - 2.0)
    }

    pub(super) fn tile_of(&self, p: Vec2) -> TilePos {
        let ts = self.ts();
        ((p.x / ts).floor() as i32, ((p.y - 1.0) / ts).floor() as i32)
    }

    /// Moves a patrolling character. Returns false if it doesn't patrol (other behaviours apply).
    pub(super) fn patrol_step(&mut self, i: usize, dt: f32) -> bool {
        let Some(p) = self.loaded.entities[i].patrol.as_ref() else { return false };
        if p.route.points.is_empty() {
            return false;
        }
        if !p.on {
            self.loaded.entities[i].body.moving = false;
            return true; // stopped: stands still
        }
        if p.wait > 0.0 {
            let e = &mut self.loaded.entities[i];
            e.body.moving = false;
            if let Some(p) = &mut e.patrol {
                p.wait -= dt;
            }
            return true;
        }
        let goal = p.route.points[p.index.min(p.route.points.len() - 1)];
        let pos = self.loaded.entities[i].body.pos;
        let here = self.tile_of(pos);
        // Plan the way to the waypoint when needed.
        if p.path.is_empty() {
            let blocked = |x: i32, y: i32| !self.loaded.map.in_bounds(x, y) || self.tile_solid(x, y);
            let planned = find_path(here, goal, |t| t == goal, blocked, MAX_NODES);
            let at_goal = pos.distance(self.tile_spot(goal)) < 2.0;
            let e = &mut self.loaded.entities[i];
            let p = e.patrol.as_mut().unwrap();
            match planned {
                Some(path) if path.is_empty() && at_goal => {
                    // Arrived: pause, then head for the next waypoint.
                    p.wait = p.route.pause.max(0.0);
                    p.advance();
                    e.body.moving = false;
                    return true;
                }
                Some(path) if path.is_empty() => p.path = vec![goal],
                Some(path) => p.path = path,
                None => {
                    eprintln!("[game] {} can't reach patrol point {goal:?}; skipping it", e.def);
                    p.wait = 1.0;
                    p.advance();
                    return true;
                }
            }
        }
        let e = &self.loaded.entities[i];
        let p = e.patrol.as_ref().unwrap();
        let target = self.tile_spot(p.path[0]);
        let speed = if p.route.speed > 0.0 { p.route.speed } else { e.body.speed.max(30.0) };
        let to = target - pos;
        let step = to.clamp_length_max(speed * dt);
        let (body, uid) = (e.body.clone(), e.uid);
        let new_pos = if step.length() > 0.0 { self.try_move(&body, step, Some(uid), true) } else { pos };
        let e = &mut self.loaded.entities[i];
        e.body.facing = Facing::from_vec(step.x, step.y).unwrap_or(e.body.facing);
        e.body.pos = new_pos;
        e.body.moving = new_pos != pos;
        e.body.anim += dt;
        e.home = new_pos;
        let p = e.patrol.as_mut().unwrap();
        if new_pos.distance(target) < 1.0 {
            p.path.remove(0);
            p.stuck = 0.0;
        } else if new_pos.distance(pos) < 0.01 {
            // Blocked (by the player, another character, a corner): wait, re-plan, give up.
            p.stuck += dt;
            if p.stuck > SKIP_AFTER {
                p.advance();
            } else if p.stuck > REPLAN_AFTER && p.path.len() > 1 {
                p.path.clear();
            }
        } else {
            p.stuck = 0.0;
        }
        true
    }

    /// Saves the progress of every patrolling character on the current map (before leaving
    /// it and before saving the game).
    pub(super) fn persist_patrols(&mut self) {
        if self.loaded.map.name.is_empty() {
            return;
        }
        for i in 0..self.loaded.entities.len() {
            if self.loaded.entities[i].patrol.is_some() && self.loaded.entities[i].spawned_by.is_none() {
                self.persist(i);
            }
        }
    }

    /// The "Patrol" action: start (the character's own route or `points`) or stop.
    pub(super) fn set_patrol(&mut self, i: usize, active: bool, points: &[(i32, i32)]) {
        let own = self.loaded.map.entity(self.loaded.entities[i].uid).and_then(|e| e.patrol.clone());
        let e = &mut self.loaded.entities[i];
        if !active {
            if let Some(p) = &mut e.patrol {
                p.on = false;
                p.path.clear();
            }
            e.body.moving = false;
        } else if !points.is_empty() {
            // A route from the action / script; mode, pause and speed as the own route.
            let base = own.or_else(|| e.patrol.as_ref().map(|p| p.route.clone())).unwrap_or_default();
            let route = PatrolRoute { points: points.to_vec(), ..base };
            match &mut e.patrol {
                Some(p) => p.set_route(route, true),
                None => e.patrol = Some(Patrol { custom: true, ..Patrol::new(route) }),
            }
        } else {
            match (&mut e.patrol, own) {
                (Some(p), _) => p.on = true,
                (None, Some(r)) => e.patrol = Some(Patrol::new(r)),
                (None, None) => eprintln!("[game] {} has no patrol route", e.def),
            }
        }
        self.persist(i);
    }

    /// Patrolling characters on this map (for automated play-throughs): "name:index@x,y".
    pub(super) fn patrol_status(&self) -> Vec<String> {
        self.loaded
            .entities
            .iter()
            .filter_map(|e| {
                let p = e.patrol.as_ref()?;
                let t = self.tile_of(e.body.pos);
                let name = if e.name.is_empty() { e.def.as_str() } else { e.name.as_str() };
                Some(format!("{name}:{}{}@{},{}", p.index, if p.on { "" } else { "(stopped)" }, t.0, t.1))
            })
            .collect()
    }
}
