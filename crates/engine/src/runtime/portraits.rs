//! Dialog portraits: the speaker's head and shoulders at the left of the dialog box, drawn
//! from their layered model (as dressed now). Nothing to set up per character: the speaker is
//! found from the dialog's speaker text (`ProjectConfig::dialog_portraits` turns it off).

use macroquad::prelude::*;

use super::Game;
use crate::defs::Appearance;
use crate::entity::{EntityKind, Facing};
use crate::state::Equipment;

/// Whose portrait a dialog shows.
#[derive(Clone, Debug, PartialEq)]
pub(super) enum Speaker {
    Player,
    /// A character on the map (entity uid).
    Entity(u32),
    /// A character definition (a speaker who isn't on this map).
    Def(String),
}

/// Does a character's name fit a speaker's? Letter case is ignored, and a name fits when all
/// words of the shorter one appear in the longer one ("Elder" speaks as "Elder Rowan", "guard"
/// as "Town Guard").
fn names_match(a: &str, b: &str) -> bool {
    let words = |s: &str| s.split_whitespace().map(str::to_lowercase).collect::<Vec<_>>();
    let (a, b) = (words(a), words(b));
    if a.is_empty() || b.is_empty() {
        return false;
    }
    let (short, long) = if a.len() <= b.len() { (a, b) } else { (b, a) };
    short.iter().all(|w| long.contains(w))
}

/// The part of a 32 x 48 character frame a portrait shows: the head and the shoulders.
const BUST: Rect = Rect { x: 0.125, y: 0.0, w: 0.75, h: 0.56 };

impl Game {
    /// Finds who speaks: `raw` is the dialog's speaker as written ("{self}", "{player}", a
    /// name), `shown` the expanded text, `owner` the trigger's owner. In order: none for
    /// narration; the entity a placeholder names; the player; the trigger's owner if the name
    /// is its own; a character on this map with that name (entity or definition name, see
    /// `names_match`; the nearest one); a character definition with that name.
    pub(super) fn find_speaker(&self, raw: &str, shown: &str, owner: Option<u32>) -> Option<Speaker> {
        let name = shown.trim();
        if !self.project.config.dialog_portraits || name.is_empty() {
            return None;
        }
        let is_char = |uid: u32| self.loaded.entities.iter().any(|e| e.uid == uid && e.kind == EntityKind::Character);
        if raw.contains("{self}") {
            return owner.filter(|u| is_char(*u)).map(Speaker::Entity);
        }
        if let Some(i) = raw.find("{uid:") {
            let uid = raw[i + 5..].split('}').next().and_then(|n| n.parse::<u32>().ok());
            return uid.filter(|u| is_char(*u)).map(Speaker::Entity);
        }
        if raw.trim() == "{player}" || name.eq_ignore_ascii_case(self.world.player_name.trim()) {
            return Some(Speaker::Player);
        }
        let db = &self.project.db;
        let named = |e: &super::RtEntity| e.kind == EntityKind::Character && (names_match(&e.name, name) || db.character(&e.def).is_some_and(|d| names_match(&d.name, name)));
        if let Some(uid) = owner.filter(|u| self.loaded.entities.iter().any(|e| e.uid == *u && named(e))) {
            return Some(Speaker::Entity(uid));
        }
        let near = self
            .loaded
            .entities
            .iter()
            .filter(|e| e.active && named(e))
            .min_by(|a, b| a.body.pos.distance(self.player.pos).total_cmp(&b.body.pos.distance(self.player.pos)));
        if let Some(e) = near {
            return Some(Speaker::Entity(e.uid));
        }
        db.characters.iter().find(|d| names_match(&d.name, name) && !d.appearance.body.is_empty()).map(|d| Speaker::Def(d.id.clone()))
    }

    /// Shows the portrait of whoever `raw` / `shown` names in the dialog just opened.
    pub(super) fn set_dialog_portrait(&mut self, raw: &str, shown: &str, owner: Option<u32>) {
        self.portrait = self.find_speaker(raw, shown, owner);
        if self.portrait.is_some() {
            self.ui.with_portrait();
        }
    }

    /// The look and gear a portrait shows.
    fn speaker_look(&self, s: &Speaker) -> Option<(Appearance, Equipment)> {
        match s {
            Speaker::Player => Some((self.player_appearance()?, self.world.equipment.clone())),
            Speaker::Entity(uid) => {
                let e = self.loaded.entities.iter().find(|e| e.uid == *uid)?;
                (!e.appearance.body.is_empty()).then(|| (e.appearance.clone(), e.equipment.clone()))
            }
            Speaker::Def(id) => self.project.db.character(id).map(|d| (d.appearance.clone(), d.equipment.clone())),
        }
    }

    /// Head and shoulders of a character, scaled into `r` (bottom-aligned, centered).
    pub(super) fn draw_bust(&self, a: &Appearance, eq: &Equipment, r: Rect) {
        let (fw, fh) = self.character_frame_size(a);
        let part = if (fw, fh) == (32.0, 48.0) { BUST } else { Rect::new(0.0, 0.0, 1.0, 0.6) };
        let (pw, ph) = (fw * part.w, fh * part.h);
        let scale = (r.w / pw).min(r.h / ph);
        let (w, h) = (pw * scale, ph * scale);
        let dest = Rect::new(r.x + (r.w - w) / 2.0, r.bottom() - h, w, h);
        for l in self.character_layers(a, eq, Facing::Down, 0, false) {
            self.assets.draw_frame_part_in(&l.sheet, l.frame, part, dest, l.skin, l.hair);
        }
    }

    /// Draws the open dialog's portrait (after the dialog box).
    pub(super) fn draw_dialog_portrait(&self) {
        let (Some(r), Some(s)) = (self.ui.portrait_rect, &self.portrait) else { return };
        draw_rectangle(r.x, r.y, r.w, r.h, Color::new(0.0, 0.0, 0.0, 0.45));
        if let Some((a, eq)) = self.speaker_look(s) {
            // Head and shoulders, scaled to the square (weapons are left out: they'd poke in).
            let eq = Equipment { weapon: None, offhand: None, tool: None, ..eq };
            self.draw_bust(&a, &eq, Rect::new(r.x + 4.0, r.y + 4.0, r.w - 8.0, r.h - 4.0));
        }
        draw_rectangle_lines(r.x, r.y, r.w, r.h, 2.0, Color::new(0.85, 0.72, 0.38, 1.0));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn speaker_names() {
        assert!(names_match("Elder", "Elder Rowan"));
        assert!(names_match("guard", "Town Guard"));
        assert!(names_match("Tilda", "tilda"));
        assert!(!names_match("Tilda", "Tomas"));
        assert!(!names_match("", "Sign"));
        assert!(!names_match("Old Man", "Old Woman"));
    }
}
