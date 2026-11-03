//! Quest journal (J): active quests first, then finished ones; the selected quest shows its
//! description and the objectives reached so far. Up/Down or click select, J / Esc close.

use macroquad::prelude::*;

use super::{input, ui::panel, Game};
use crate::quest::{QuestChange, QuestOp, QuestStatus};
use crate::render::wrap_text;

const W: f32 = 820.0;
const H: f32 = 500.0;
const LIST_W: f32 = 280.0;
const PAD: f32 = 16.0;
const ROW: f32 = 34.0;

#[derive(Default)]
pub(super) struct Journal {
    open: bool,
    sel: usize,
    /// Quest to select when the journal opens next (the last one that changed).
    focus: Option<String>,
}

impl Journal {
    pub fn is_open(&self) -> bool {
        self.open
    }
}

fn rect() -> Rect {
    let (sw, sh) = (screen_width(), screen_height());
    Rect::new(((sw - W) / 2.0).max(4.0).round(), ((sh - H) / 2.0).max(4.0).round(), W, H)
}

/// Header rows are `None`, quests `Some(id)`.
type Entry = (Option<String>, String);

impl Game {
    pub(super) fn journal_open(&self) -> bool {
        self.journal.is_open()
    }

    /// Runs a "Quest" action and tells the player what changed.
    pub(super) fn quest_changed(&mut self, quest: &str, op: QuestOp, stage: u32) {
        let Some(change) = self.world.apply_quest(quest, op, stage) else { return };
        let title = self.project.db.quest(quest).map_or(quest.to_string(), |d| d.title.clone());
        let msg = match change {
            QuestChange::Started => format!("New quest: {title}  (J: journal)"),
            QuestChange::Updated => format!("Quest updated: {title}"),
            QuestChange::Completed => format!("Quest completed: {title}"),
            QuestChange::Failed => format!("Quest failed: {title}"),
        };
        self.ui.toast(msg);
        self.audio.play(&self.project.config.sfx.pickup);
        self.journal.focus = Some(quest.to_string());
    }

    /// Journal rows: headers and quests (active newest first, then finished ones).
    fn journal_entries(&self) -> Vec<Entry> {
        let mut quests: Vec<(&String, &crate::quest::QuestProgress)> = self.world.quests.iter().collect();
        quests.sort_by_key(|a| std::cmp::Reverse(a.1.order));
        let title = |id: &str| self.project.db.quest(id).map_or(id.to_string(), |d| d.title.clone());
        let mut out = Vec::new();
        for (head, active) in [("Active", true), ("Finished", false)] {
            let list: Vec<_> = quests.iter().filter(|(_, p)| (p.status == QuestStatus::Active) == active).collect();
            if list.is_empty() {
                continue;
            }
            out.push((None, head.to_string()));
            for (id, _) in list {
                out.push((Some(id.to_string()), title(id)));
            }
        }
        out
    }

    fn quest_rows(entries: &[Entry]) -> Vec<usize> {
        entries.iter().enumerate().filter(|(_, e)| e.0.is_some()).map(|(i, _)| i).collect()
    }

    /// Handles J and the open journal. Returns (key consumed, mouse consumed).
    pub(super) fn update_journal(&mut self, allow: bool, key_consumed: bool) -> (bool, bool) {
        if !allow {
            return (false, false);
        }
        if !self.journal.open {
            if !key_consumed && input::pressed(KeyCode::J) {
                self.journal.open = true;
                let entries = self.journal_entries();
                let rows = Self::quest_rows(&entries);
                let focus = self.journal.focus.take();
                self.journal.sel = focus.and_then(|f| rows.iter().position(|&i| entries[i].0.as_deref() == Some(f.as_str()))).unwrap_or(0);
                return (true, false);
            }
            return (false, false);
        }
        let entries = self.journal_entries();
        let rows = Self::quest_rows(&entries);
        let r = rect();
        let m = input::mouse_pos();
        let close = Rect::new(r.x + r.w - 30.0, r.y + 8.0, 22.0, 22.0);
        if input::any_pressed(&[KeyCode::J, KeyCode::Escape]) || (input::mouse_pressed(MouseButton::Left) && close.contains(m)) {
            self.journal.open = false;
            return (true, true);
        }
        if input::any_pressed(&[KeyCode::Down, KeyCode::S]) {
            self.journal.sel = (self.journal.sel + 1).min(rows.len().saturating_sub(1));
        }
        if input::any_pressed(&[KeyCode::Up, KeyCode::W]) {
            self.journal.sel = self.journal.sel.saturating_sub(1);
        }
        if input::mouse_pressed(MouseButton::Left) {
            for (k, &i) in rows.iter().enumerate() {
                if Self::entry_rect(r, i).contains(m) {
                    self.journal.sel = k;
                }
            }
        }
        (true, true)
    }

    fn entry_rect(r: Rect, i: usize) -> Rect {
        Rect::new(r.x + PAD, r.y + 56.0 + i as f32 * ROW, LIST_W - PAD, ROW - 4.0)
    }

    pub(super) fn draw_journal(&self) {
        if !self.journal.open {
            return;
        }
        let r = rect();
        let m = input::mouse_pos();
        draw_rectangle(0.0, 0.0, screen_width(), screen_height(), Color::new(0.0, 0.0, 0.0, 0.35));
        panel(r.x, r.y, r.w, r.h);
        draw_text("Journal", r.x + PAD, r.y + 34.0, 30.0, GOLD);
        let c = Rect::new(r.x + r.w - 30.0, r.y + 8.0, 22.0, 22.0);
        draw_rectangle_lines(c.x, c.y, c.w, c.h, 1.5, Color::new(0.85, 0.8, 0.6, 1.0));
        draw_line(c.x + 6.0, c.y + 6.0, c.x + c.w - 6.0, c.y + c.h - 6.0, 2.0, WHITE);
        draw_line(c.x + c.w - 6.0, c.y + 6.0, c.x + 6.0, c.y + c.h - 6.0, 2.0, WHITE);
        draw_text("Up/Down or click: select   J / Esc: close", r.x + PAD, r.y + r.h - 14.0, 16.0, GRAY);

        let entries = self.journal_entries();
        if entries.is_empty() {
            draw_text("No quests yet. Talk to people - someone may need your help.", r.x + PAD, r.y + 80.0, 22.0, LIGHTGRAY);
            return;
        }
        draw_line(r.x + LIST_W + 8.0, r.y + 50.0, r.x + LIST_W + 8.0, r.y + r.h - 40.0, 1.0, Color::new(0.85, 0.8, 0.6, 0.4));
        let rows = Self::quest_rows(&entries);
        let sel = rows.get(self.journal.sel).copied();
        for (i, (id, label)) in entries.iter().enumerate() {
            let er = Self::entry_rect(r, i);
            if er.y + er.h > r.y + r.h - 40.0 {
                break;
            }
            match id {
                None => {
                    draw_text(label, er.x, er.y + 22.0, 18.0, Color::new(0.85, 0.8, 0.6, 0.8));
                }
                Some(qid) => {
                    let on = sel == Some(i);
                    if on {
                        draw_rectangle(er.x, er.y, er.w, er.h, Color::new(0.85, 0.7, 0.3, 0.22));
                        draw_rectangle_lines(er.x, er.y, er.w, er.h, 2.0, GOLD);
                    } else if er.contains(m) {
                        draw_rectangle(er.x, er.y, er.w, er.h, Color::new(1.0, 1.0, 1.0, 0.08));
                    }
                    let status = self.world.quest(qid).map(|q| q.status);
                    let col = match status {
                        Some(QuestStatus::Completed) => Color::new(0.6, 0.85, 0.6, 1.0),
                        Some(QuestStatus::Failed) => Color::new(0.9, 0.5, 0.5, 1.0),
                        _ => WHITE,
                    };
                    let mut text: String = label.chars().take(26).collect();
                    if text.len() < label.len() {
                        text.push_str("...");
                    }
                    draw_text(&text, er.x + 8.0, er.y + 21.0, 20.0, col);
                }
            }
        }

        // Details of the selected quest.
        let Some(qid) = sel.and_then(|i| entries[i].0.clone()) else { return };
        let Some(p) = self.world.quest(&qid) else { return };
        let def = self.project.db.quest(&qid);
        let x = r.x + LIST_W + 26.0;
        let w = r.x + r.w - PAD - x;
        let mut y = r.y + 74.0;
        let title = def.map_or(qid.clone(), |d| d.title.clone());
        draw_text(&title, x, y, 28.0, GOLD);
        let tag = match p.status {
            QuestStatus::Active => None,
            QuestStatus::Completed => Some(("COMPLETED", Color::new(0.6, 0.85, 0.6, 1.0))),
            QuestStatus::Failed => Some(("FAILED", Color::new(0.9, 0.5, 0.5, 1.0))),
        };
        if let Some((t, c)) = tag {
            let tw = measure_text(t, None, 18, 1.0).width;
            draw_text(t, x + w - tw, y, 18.0, c);
        }
        y += 14.0;
        if let Some(d) = def {
            for line in wrap_text(&self.world.fill_vars(&d.description), 20, w) {
                y += 24.0;
                draw_text(&line, x, y, 20.0, LIGHTGRAY);
            }
        }
        y += 22.0;
        draw_text("Objectives", x, y + 14.0, 18.0, Color::new(0.85, 0.8, 0.6, 0.8));
        y += 20.0;
        let stages = def.map_or(&[][..], |d| d.stages.as_slice());
        let finished = p.status != QuestStatus::Active;
        // Reached stages: earlier ones are done, the current one is open (all done once completed).
        let shown = (p.stage as usize + 1).min(stages.len());
        for (k, s) in stages.iter().take(shown).enumerate() {
            let done = k < p.stage as usize || p.status == QuestStatus::Completed;
            let (mark, col) = if done {
                ("[x]", Color::new(0.6, 0.6, 0.6, 1.0))
            } else if finished {
                ("[-]", Color::new(0.9, 0.5, 0.5, 1.0))
            } else {
                ("[ ]", WHITE)
            };
            let lines = wrap_text(&self.world.fill_vars(s), 20, w - 34.0);
            for (n, line) in lines.iter().enumerate() {
                y += 24.0;
                if y > r.y + r.h - 44.0 {
                    return;
                }
                if n == 0 {
                    draw_text(mark, x, y, 20.0, col);
                }
                draw_text(line, x + 34.0, y, 20.0, col);
            }
            y += 4.0;
        }
    }
}
