//! Daily routines: a placed character with a schedule (`EntityInstance.schedule`) walks to the
//! stop of the current hour (A*, like patrols) and stays there, doing what its definition's
//! behaviour does (standing, wandering around the stop). A stop can be indoors: the character
//! goes out of sight there until the next stop. Entering a map puts scheduled characters
//! straight at their current stop (they went on with their day while the player was away).

use super::patrol::{Walk, Walker};
use super::Game;
use crate::entity::{current_stop, ScheduleStop};

/// Runtime schedule of a character.
#[derive(Clone, Debug, Default)]
pub(super) struct Schedule {
    pub stops: Vec<ScheduleStop>,
    /// The stop it is at or heading for.
    pub current: Option<usize>,
    /// At the current stop (or gave up reaching it).
    pub arrived: bool,
    /// Followed (false: stopped by the Schedule action / script; saved).
    pub on: bool,
    walker: Walker,
}

impl Schedule {
    pub fn new(stops: Vec<ScheduleStop>) -> Self {
        Schedule { stops, on: true, ..Default::default() }
    }
}

impl Game {
    /// Puts character `i` at its current stop right away (entering a map).
    pub(super) fn place_on_schedule(&mut self, i: usize) {
        let hour = self.world.hour;
        let Some(s) = self.loaded.entities[i].schedule.as_mut().filter(|s| s.on) else { return };
        let Some(k) = current_stop(&s.stops, hour) else { return };
        s.current = Some(k);
        s.walker = Walker::default();
        let tile = (s.stops[k].x, s.stops[k].y);
        self.loaded.entities[i].body.pos = self.tile_spot(tile);
        self.arrive(i);
    }

    /// Reached the current stop: face its way, settle there, go inside.
    fn arrive(&mut self, i: usize) {
        let e = &mut self.loaded.entities[i];
        let Some(s) = e.schedule.as_mut() else { return };
        s.arrived = true;
        let Some(stop) = s.current.and_then(|k| s.stops.get(k)).cloned() else { return };
        if let Some(f) = stop.facing {
            e.body.facing = f;
        }
        e.body.moving = false;
        e.home = e.body.pos;
        if stop.inside && e.active {
            e.active = false;
            e.away = true;
        }
    }

    /// Moves a character along its schedule. None: it has none (or it is stopped), other
    /// behaviours apply; Some(true): walking (or inside), nothing else to do; Some(false): at
    /// its stop, where its definition's behaviour applies (patrol routes don't).
    pub(super) fn schedule_step(&mut self, i: usize, dt: f32) -> Option<bool> {
        let hour = self.world.hour;
        let e = &mut self.loaded.entities[i];
        let s = e.schedule.as_mut().filter(|s| s.on)?;
        let now = current_stop(&s.stops, hour)?;
        if s.current != Some(now) {
            // A new stop: come out and set off.
            s.current = Some(now);
            s.arrived = false;
            s.walker = Walker::default();
            if e.away {
                e.away = false;
                e.active = true;
            }
        }
        if e.away {
            return Some(true);
        }
        if s.arrived {
            return Some(false);
        }
        let goal = (s.stops[now].x, s.stops[now].y);
        let mut walker = std::mem::take(&mut s.walker);
        let walked = self.walk_to_tile(i, goal, &mut walker, 0.0, dt);
        let e = &mut self.loaded.entities[i];
        if let Some(s) = &mut e.schedule {
            s.walker = walker;
        }
        match walked {
            Walk::Walking | Walk::GaveUp => {}
            Walk::Arrived => self.arrive(i),
            Walk::Unreachable => {
                eprintln!("[game] {} can't reach its schedule stop {goal:?}; staying here", e.def);
                self.arrive(i);
            }
        }
        Some(true)
    }

    /// The "Schedule" action: follow the daily routine again, or stop following it (the
    /// character stays where it is and does what its definition says).
    pub(super) fn set_schedule(&mut self, i: usize, on: bool) {
        let e = &mut self.loaded.entities[i];
        let Some(s) = &mut e.schedule else {
            eprintln!("[game] {} has no schedule", e.def);
            return;
        };
        s.on = on;
        s.current = None; // on: head for the current stop
        s.arrived = false;
        s.walker = Walker::default();
        if e.away {
            e.away = false;
            e.active = true;
        }
        e.body.moving = false;
        self.persist(i);
    }

    /// Characters with a schedule on this map (for automated play-throughs):
    /// "name:stop@x,y" plus "(walking)", "(inside)" or "(stopped)"; stop = "HH:MM".
    pub(super) fn schedule_status(&self) -> Vec<String> {
        self.loaded
            .entities
            .iter()
            .filter_map(|e| {
                let s = e.schedule.as_ref()?;
                let t = self.tile_of(e.body.pos);
                let name = if e.name.is_empty() { e.def.as_str() } else { e.name.as_str() };
                let stop = s.current.and_then(|k| s.stops.get(k)).map_or("-".to_string(), |st| st.time_text());
                let how = if !s.on {
                    "(stopped)"
                } else if e.away {
                    "(inside)"
                } else if !s.arrived {
                    "(walking)"
                } else {
                    ""
                };
                Some(format!("{name}:{stop}{how}@{},{}", t.0, t.1))
            })
            .collect()
    }
}
