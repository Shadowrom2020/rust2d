//! Character creator, shown after "New game" when the project enables it: the player types a
//! name, picks male / female and one of the project's looks for that sex.
//!
//! Keyboard: typing edits the name (Backspace deletes), Up/Down select a row, Left/Right
//! change sex / look, Enter starts, Esc goes back to the title screen. Everything can be
//! clicked as well.

use macroquad::prelude::*;

use super::{input, ui::panel, Game};
use crate::project::PlayerLook;
use crate::state::Sex;

const W: f32 = 720.0;
const H: f32 = 480.0;
const MAX_NAME: usize = 16;
/// Rows: name, sex, look, start, back.
const ROWS: usize = 5;

pub(super) struct Creator {
    name: String,
    sex: Sex,
    /// Selected look per sex (index into the looks of that sex).
    look: [usize; 2],
    row: usize,
    time: f32,
}

struct Layout {
    panel: Rect,
    name: Rect,
    sex: [Rect; 2],
    prev: Rect,
    next: Rect,
    preview: Rect,
    start: Rect,
    back: Rect,
}

fn layout() -> Layout {
    let (sw, sh) = (screen_width(), screen_height());
    let p = Rect::new(((sw - W) / 2.0).round(), ((sh - H) / 2.0).round(), W, H);
    let (lx, fx) = (p.x + 30.0, p.x + 170.0);
    Layout {
        panel: p,
        name: Rect::new(fx, p.y + 78.0, 250.0, 38.0),
        sex: [Rect::new(fx, p.y + 138.0, 118.0, 36.0), Rect::new(fx + 132.0, p.y + 138.0, 118.0, 36.0)],
        prev: Rect::new(fx, p.y + 270.0, 40.0, 40.0),
        next: Rect::new(fx + 210.0, p.y + 270.0, 40.0, 40.0),
        preview: Rect::new(fx + 55.0, p.y + 196.0, 140.0, 190.0),
        start: Rect::new(lx, p.y + H - 62.0, 220.0, 42.0),
        back: Rect::new(lx + 236.0, p.y + H - 62.0, 120.0, 42.0),
    }
}

fn button(r: Rect, label: &str, selected: bool, active: bool, hover: bool) {
    let fill = if active {
        Color::new(0.45, 0.35, 0.12, 1.0)
    } else if hover {
        Color::new(0.3, 0.26, 0.14, 1.0)
    } else {
        Color::new(0.12, 0.12, 0.2, 0.9)
    };
    draw_rectangle(r.x, r.y, r.w, r.h, fill);
    draw_rectangle_lines(r.x, r.y, r.w, r.h, if selected { 3.0 } else { 1.5 }, if selected { GOLD } else { Color::new(0.85, 0.8, 0.6, 0.8) });
    let tw = measure_text(label, None, 24, 1.0).width;
    draw_text(label, r.x + (r.w - tw) / 2.0, r.y + r.h / 2.0 + 8.0, 24.0, WHITE);
}

impl Game {
    /// Name used when the creator is skipped (play-tests, old saves).
    pub(super) fn default_player_name(&self) -> String {
        let c = &self.project.config;
        let name = if c.creation.default_name.trim().is_empty() {
            self.project.db.character(&c.player).map(|d| d.name.clone()).unwrap_or_default()
        } else {
            c.creation.default_name.clone()
        };
        if name.trim().is_empty() { "Hero".into() } else { name }
    }

    /// Sprite sheet the player is drawn with: the chosen look, else the player character's.
    pub(super) fn player_sheet_id(&self) -> Option<&str> {
        let look = self.world.player_look.as_str();
        if !look.is_empty() && self.assets.sheets.contains_key(look) {
            return Some(look);
        }
        self.player_sheet.as_deref()
    }

    fn looks_of(&self, sex: Sex) -> Vec<&PlayerLook> {
        self.project.config.creation.looks.iter().filter(|l| l.sex == sex && self.assets.sheets.contains_key(&l.sheet)).collect()
    }

    /// Opens the creator (from the title screen's "New game").
    pub(super) fn open_creator(&mut self) {
        while input::next_char().is_some() {} // drop keys typed before
        let sex = self.project.config.creation.sex.fixed().unwrap_or(Sex::Male);
        self.creator = Some(Creator { name: self.default_player_name(), sex, look: [0, 0], row: 0, time: 0.0 });
    }

    fn current_look(&self, c: &Creator) -> Option<&PlayerLook> {
        let looks = self.looks_of(c.sex);
        looks.get(c.look[c.sex as usize] % looks.len().max(1)).copied()
    }

    fn cycle_look(&mut self, step: i32) {
        let Some(c) = &self.creator else { return };
        let n = self.looks_of(c.sex).len().max(1) as i32;
        let c = self.creator.as_mut().unwrap();
        let k = c.sex as usize;
        c.look[k] = (c.look[k] as i32 + step).rem_euclid(n) as usize;
    }

    fn start_created(&mut self) {
        let Some(c) = self.creator.take() else { return };
        let look = self.current_look(&c).map(|l| l.sheet.clone()).unwrap_or_default();
        let name = c.name.trim().to_string();
        self.new_game();
        self.world.player_name = if name.is_empty() { self.default_player_name() } else { name };
        self.world.player_sex = c.sex;
        self.world.player_look = look;
        self.close_title();
    }

    /// Handles the open creator. Returns false when it isn't open.
    pub(super) fn update_creator(&mut self, dt: f32) -> bool {
        let Some(c) = &mut self.creator else { return false };
        c.time += dt;
        while let Some(ch) = input::next_char() {
            if !ch.is_control() && c.name.chars().count() < MAX_NAME {
                c.name.push(ch);
            }
        }
        if input::pressed(KeyCode::Backspace) {
            c.name.pop();
        }
        if input::pressed(KeyCode::Escape) {
            self.creator = None;
            return true;
        }
        // A fixed sex can't be changed: its row is skipped.
        let fixed = self.project.config.creation.sex.fixed().is_some();
        let step = |r: usize, d: usize| {
            let n = (r + d) % ROWS;
            if fixed && n == 1 { (n + d) % ROWS } else { n }
        };
        if input::pressed(KeyCode::Down) {
            c.row = step(c.row, 1);
        }
        if input::pressed(KeyCode::Up) {
            c.row = step(c.row, ROWS - 1);
        }
        let (left, right) = (input::pressed(KeyCode::Left), input::pressed(KeyCode::Right));
        match c.row {
            1 if (left || right) && !fixed => c.sex = if c.sex == Sex::Male { Sex::Female } else { Sex::Male },
            2 if left => self.cycle_look(-1),
            2 if right => self.cycle_look(1),
            3 | 4 if left || right => c.row = if c.row == 3 { 4 } else { 3 },
            _ => {}
        }
        let Some(c) = &mut self.creator else { return true };
        if input::pressed(KeyCode::Enter) {
            if c.row == 4 {
                self.creator = None;
            } else {
                self.start_created();
            }
            return true;
        }
        // Mouse.
        if input::mouse_pressed(MouseButton::Left) {
            let l = layout();
            let m = input::mouse_pos();
            if l.name.contains(m) {
                c.row = 0;
            } else if let Some(k) = l.sex.iter().position(|r| r.contains(m)).filter(|_| !fixed) {
                c.row = 1;
                c.sex = Sex::ALL[k];
            } else if l.prev.contains(m) {
                c.row = 2;
                self.cycle_look(-1);
            } else if l.next.contains(m) || l.preview.contains(m) {
                c.row = 2;
                self.cycle_look(1);
            } else if l.start.contains(m) {
                self.start_created();
            } else if l.back.contains(m) {
                self.creator = None;
            }
        }
        true
    }

    /// Creator state for automated play-throughs.
    pub(super) fn creator_status(&self) -> Option<String> {
        let c = self.creator.as_ref()?;
        Some(format!("{}|{:?}|{}", c.name, c.sex, self.current_look(c).map_or("-", |l| l.sheet.as_str())))
    }

    pub(super) fn draw_creator(&self) {
        let Some(c) = &self.creator else { return };
        let l = layout();
        let m = input::mouse_pos();
        let p = l.panel;
        draw_rectangle(0.0, 0.0, screen_width(), screen_height(), Color::new(0.0, 0.0, 0.0, 0.6));
        panel(p.x, p.y, p.w, p.h);
        let title = "Create your character";
        let tw = measure_text(title, None, 34, 1.0).width;
        draw_text(title, p.x + (p.w - tw) / 2.0, p.y + 46.0, 34.0, GOLD);
        let label = |t: &str, y: f32, row: usize| draw_text(t, p.x + 30.0, y, 24.0, if c.row == row { GOLD } else { LIGHTGRAY });

        // Name.
        label("Name", l.name.y + 27.0, 0);
        let r = l.name;
        draw_rectangle(r.x, r.y, r.w, r.h, Color::new(0.0, 0.0, 0.0, 0.5));
        draw_rectangle_lines(r.x, r.y, r.w, r.h, if c.row == 0 { 2.5 } else { 1.5 }, if c.row == 0 { GOLD } else { Color::new(0.85, 0.8, 0.6, 0.8) });
        let caret = if c.row == 0 && (get_time() * 2.0) as i64 % 2 == 0 { "_" } else { "" };
        draw_text(format!("{}{caret}", c.name), r.x + 10.0, r.y + 27.0, 26.0, WHITE);

        // Sex (or the one the game prescribes).
        label("Sex", l.sex[0].y + 26.0, 1);
        if self.project.config.creation.sex.fixed().is_some() {
            draw_text(c.sex.label(), l.sex[0].x + 6.0, l.sex[0].y + 26.0, 24.0, WHITE);
        } else {
            for (k, s) in Sex::ALL.iter().enumerate() {
                button(l.sex[k], s.label(), c.row == 1 && c.sex == *s, c.sex == *s, l.sex[k].contains(m));
            }
        }

        // Look: preview with arrows.
        label("Appearance", l.preview.y + 24.0, 2);
        let looks = self.looks_of(c.sex);
        let look = self.current_look(c);
        let frames = self.player_frames.max(1);
        let sheet = look.map(|l| l.sheet.as_str()).or(self.player_sheet.as_deref());
        let pr = l.preview;
        draw_rectangle(pr.x, pr.y, pr.w, pr.h, Color::new(0.0, 0.0, 0.0, 0.35));
        if c.row == 2 {
            draw_rectangle_lines(pr.x, pr.y, pr.w, pr.h, 2.0, GOLD);
        }
        if let Some(sheet) = sheet {
            // Walks in place, turning every 1.5 s.
            let facing = (c.time / 1.5) as u32 % 4;
            let col = (c.time * 8.0) as u32 % frames;
            let order = [0u32, 1, 3, 2]; // down, left, up, right
            self.assets.draw_frame_in(sheet, order[facing as usize] * frames + col, Rect::new(pr.x + 10.0, pr.y + 8.0, pr.w - 20.0, pr.h - 16.0), WHITE);
            // All four directions, small, to the right.
            for (k, row) in [0u32, 1, 2, 3].iter().enumerate() {
                let r = Rect::new(l.next.x + 64.0 + (k % 2) as f32 * 76.0, pr.y + (k / 2) as f32 * 96.0, 68.0, 90.0);
                draw_rectangle(r.x, r.y, r.w, r.h, Color::new(0.0, 0.0, 0.0, 0.25));
                self.assets.draw_frame_in(sheet, row * frames, r, WHITE);
            }
        }
        for (r, t) in [(l.prev, "<"), (l.next, ">")] {
            button(r, t, false, false, r.contains(m) && looks.len() > 1);
        }
        let caption = match look {
            Some(lk) => {
                let name = if lk.label.is_empty() { lk.sheet.clone() } else { lk.label.clone() };
                let k = looks.iter().position(|x| std::ptr::eq(*x, lk)).unwrap_or(0);
                format!("{name}  ({}/{})", k + 1, looks.len())
            }
            None => "(the player character's look)".into(),
        };
        let tw = measure_text(&caption, None, 20, 1.0).width;
        draw_text(&caption, pr.x + (pr.w - tw) / 2.0, pr.y + pr.h + 22.0, 20.0, LIGHTGRAY);

        button(l.start, "Start adventure", c.row == 3, false, l.start.contains(m));
        button(l.back, "Back", c.row == 4, false, l.back.contains(m));
        draw_text("Type a name  -  Up/Down: select  -  Left/Right: change  -  Enter: start  -  Esc: back", p.x + 30.0, p.y + H - 6.0, 16.0, GRAY);
    }
}
