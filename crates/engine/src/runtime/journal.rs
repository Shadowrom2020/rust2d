//! The journal (J) with two tabs. *Quests*: active quests first, then finished ones; the
//! selected quest shows its description and the objectives reached so far. *Bestiary*: every
//! kind of character the player has defeated, with a portrait, the description and its stats.
//! Up/Down or click select, Tab / Left / Right or a click on a tab switch, J / Esc close.

use macroquad::prelude::*;

use super::{input, ui::panel, Game};
use crate::quest::{QuestChange, QuestOp, QuestStatus};
use crate::render::wrap_text;

const W: f32 = 820.0;
const H: f32 = 500.0;
const LIST_W: f32 = 280.0;
const PAD: f32 = 16.0;
const ROW: f32 = 34.0;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) enum JournalTab {
    #[default]
    Quests,
    Bestiary,
}

impl JournalTab {
    const ALL: [JournalTab; 2] = [JournalTab::Quests, JournalTab::Bestiary];
    fn label(self) -> &'static str {
        match self {
            JournalTab::Quests => "Quests",
            JournalTab::Bestiary => "Bestiary",
        }
    }
}

#[derive(Default)]
pub(super) struct Journal {
    open: bool,
    tab: JournalTab,
    sel: usize,
    /// Selected bestiary entry.
    best_sel: usize,
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

/// The tabs at the top of the journal.
fn tab_rects(r: Rect) -> [(JournalTab, Rect); 2] {
    let w = 120.0;
    JournalTab::ALL.map(|t| {
        let k = JournalTab::ALL.iter().position(|x| *x == t).unwrap() as f32;
        (t, Rect::new(r.x + 170.0 + k * (w + 8.0), r.y + 12.0, w, 30.0))
    })
}

impl Game {
    pub(super) fn journal_open(&self) -> bool {
        self.journal.is_open()
    }

    /// Screen position of a journal tab ("quests" / "bestiary") for automated play.
    pub(super) fn journal_tab_pos(&self, name: &str) -> Option<Vec2> {
        let t = JournalTab::ALL.into_iter().find(|t| t.label().eq_ignore_ascii_case(name))?;
        tab_rects(rect()).into_iter().find(|(x, _)| *x == t).map(|(_, r)| r.center())
    }

    /// Bestiary entries: (definition id, name, defeated), by name.
    fn bestiary_entries(&self) -> Vec<(String, String, u32)> {
        let mut v: Vec<(String, String, u32)> = self
            .world
            .bestiary
            .iter()
            .filter(|(_, n)| **n > 0)
            .map(|(id, n)| (id.clone(), self.project.db.character(id).map_or(id.clone(), |d| d.name.clone()), *n))
            .collect();
        v.sort_by_key(|a| a.1.to_lowercase());
        v
    }

    /// For `status()`: definition id x defeated.
    pub(super) fn bestiary_status(&self) -> Vec<String> {
        self.world.bestiary.iter().map(|(id, n)| format!("{id}x{n}")).collect()
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
        if change == QuestChange::Completed {
            let xp = self.project.db.quest(quest).map_or(0, |d| d.xp);
            self.give_xp(xp as i64);
        }
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
                self.journal.tab = JournalTab::Quests;
                let entries = self.journal_entries();
                let rows = Self::quest_rows(&entries);
                let focus = self.journal.focus.take();
                self.journal.sel = focus.and_then(|f| rows.iter().position(|&i| entries[i].0.as_deref() == Some(f.as_str()))).unwrap_or(0);
                return (true, false);
            }
            return (false, false);
        }
        let r = rect();
        let m = input::mouse_pos();
        let close = Rect::new(r.x + r.w - 30.0, r.y + 8.0, 22.0, 22.0);
        if input::any_pressed(&[KeyCode::J, KeyCode::Escape]) || (input::mouse_pressed(MouseButton::Left) && close.contains(m)) {
            self.journal.open = false;
            return (true, true);
        }
        // Switching tabs.
        if input::any_pressed(&[KeyCode::Tab, KeyCode::Left, KeyCode::Right, KeyCode::A, KeyCode::D]) {
            self.journal.tab = match self.journal.tab {
                JournalTab::Quests => JournalTab::Bestiary,
                JournalTab::Bestiary => JournalTab::Quests,
            };
        }
        if input::mouse_pressed(MouseButton::Left) {
            if let Some((t, _)) = tab_rects(r).into_iter().find(|(_, tr)| tr.contains(m)) {
                self.journal.tab = t;
                return (true, true);
            }
        }
        if self.journal.tab == JournalTab::Bestiary {
            let n = self.bestiary_entries().len();
            if input::any_pressed(&[KeyCode::Down, KeyCode::S]) {
                self.journal.best_sel = (self.journal.best_sel + 1).min(n.saturating_sub(1));
            }
            if input::any_pressed(&[KeyCode::Up, KeyCode::W]) {
                self.journal.best_sel = self.journal.best_sel.saturating_sub(1);
            }
            if input::mouse_pressed(MouseButton::Left) {
                if let Some(k) = (0..n).find(|&k| Self::entry_rect(r, k).contains(m)) {
                    self.journal.best_sel = k;
                }
            }
            return (true, true);
        }
        let entries = self.journal_entries();
        let rows = Self::quest_rows(&entries);
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
        draw_text("Up/Down or click: select   Tab: quests / bestiary   J / Esc: close", r.x + PAD, r.y + r.h - 14.0, 16.0, GRAY);
        for (t, tr) in tab_rects(r) {
            let on = self.journal.tab == t;
            let fill = if on { Color::new(0.85, 0.7, 0.3, 0.3) } else if tr.contains(m) { Color::new(1.0, 1.0, 1.0, 0.08) } else { Color::new(0.0, 0.0, 0.0, 0.25) };
            draw_rectangle(tr.x, tr.y, tr.w, tr.h, fill);
            draw_rectangle_lines(tr.x, tr.y, tr.w, tr.h, if on { 2.0 } else { 1.0 }, if on { GOLD } else { Color::new(0.85, 0.8, 0.6, 0.5) });
            let tw = measure_text(t.label(), None, 20, 1.0).width;
            draw_text(t.label(), tr.x + (tr.w - tw) / 2.0, tr.y + 21.0, 20.0, if on { WHITE } else { LIGHTGRAY });
        }
        if self.journal.tab == JournalTab::Bestiary {
            self.draw_bestiary(r);
            return;
        }

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
            for line in wrap_text(&self.world.expand_text(&d.description), 20, w) {
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
            let lines = wrap_text(&self.world.expand_text(s), 20, w - 34.0);
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

    /// The bestiary tab: the defeated kinds of characters, and the selected one's page.
    fn draw_bestiary(&self, r: Rect) {
        let m = input::mouse_pos();
        let entries = self.bestiary_entries();
        if entries.is_empty() {
            draw_text("No creatures yet. Every kind you defeat is described here.", r.x + PAD, r.y + 80.0, 22.0, LIGHTGRAY);
            return;
        }
        draw_line(r.x + LIST_W + 8.0, r.y + 50.0, r.x + LIST_W + 8.0, r.y + r.h - 40.0, 1.0, Color::new(0.85, 0.8, 0.6, 0.4));
        let sel = self.journal.best_sel.min(entries.len() - 1);
        for (k, (_, name, n)) in entries.iter().enumerate() {
            let er = Self::entry_rect(r, k);
            if er.y + er.h > r.y + r.h - 40.0 {
                break;
            }
            if k == sel {
                draw_rectangle(er.x, er.y, er.w, er.h, Color::new(0.85, 0.7, 0.3, 0.22));
                draw_rectangle_lines(er.x, er.y, er.w, er.h, 2.0, GOLD);
            } else if er.contains(m) {
                draw_rectangle(er.x, er.y, er.w, er.h, Color::new(1.0, 1.0, 1.0, 0.08));
            }
            let label: String = name.chars().take(20).collect();
            draw_text(&label, er.x + 8.0, er.y + 21.0, 20.0, WHITE);
            let count = format!("x{n}");
            let cw = measure_text(&count, None, 18, 1.0).width;
            draw_text(&count, er.x + er.w - cw - 8.0, er.y + 21.0, 18.0, LIGHTGRAY);
        }

        let (id, name, n) = &entries[sel];
        let Some(d) = self.project.db.character(id) else { return };
        let db = &self.project.db;
        let x = r.x + LIST_W + 26.0;
        let w = r.x + r.w - PAD - x;
        let top = r.y + 58.0;
        // Portrait: the definition's look, as dressed.
        let plate = Rect::new(x, top, 110.0, 140.0);
        draw_rectangle(plate.x, plate.y, plate.w, plate.h, Color::new(0.0, 0.0, 0.0, 0.4));
        draw_rectangle_lines(plate.x, plate.y, plate.w, plate.h, 1.5, Color::new(0.55, 0.45, 0.22, 1.0));
        let (fw, fh) = self.character_frame_size(&d.appearance);
        let scale = ((plate.w - 10.0) / fw).min((plate.h - 10.0) / fh).floor().max(1.0);
        let (pw, ph) = (fw * scale, fh * scale);
        self.draw_character_in(&d.appearance, &d.equipment, Rect::new(plate.x + (plate.w - pw) / 2.0, plate.bottom() - 5.0 - ph, pw, ph));

        // Name and numbers beside the portrait.
        let sx = plate.right() + 18.0;
        draw_text(name, sx, top + 26.0, 28.0, GOLD);
        let armor: i32 = [crate::defs::EquipSlot::Clothing, crate::defs::EquipSlot::Head, crate::defs::EquipSlot::Body, crate::defs::EquipSlot::Offhand]
            .into_iter()
            .filter_map(|s| db.item(d.equipment.item(s)?)?.armor.as_ref().map(|a| a.defense))
            .sum();
        let weapon = d.equipment.item(crate::defs::EquipSlot::Weapon).and_then(|i| db.item(i)).map(|i| (i.name.clone(), i.weapon.clone()));
        let mut lines = vec![format!("Defeated    {n}"), format!("Health      {}", d.max_hp), format!("Damage      {}", d.damage)];
        if d.defense + armor != 0 {
            lines.push(format!("Defense     {}", d.defense + armor));
        }
        if self.progression_on() && d.xp > 0 {
            lines.push(format!("Experience  {} XP", d.xp));
        }
        match weapon {
            Some((wn, Some(wd))) => lines.push(format!("Weapon      {wn} ({})", wd.damage)),
            _ => {
                if let Some(wd) = d.weapon.as_ref().filter(|w| w.ranged.is_some()) {
                    lines.push(format!("Shoots      {} damage", wd.damage));
                }
            }
        }
        if let Some(fx) = &d.touch_effect {
            lines.push(format!("Touch       {}", fx.kind.label()));
        }
        for (k, l) in lines.iter().enumerate() {
            draw_text(l, sx, top + 54.0 + k as f32 * 21.0, 18.0, if k == 0 { WHITE } else { LIGHTGRAY });
        }

        // Description and loot under the portrait.
        let mut y = top.max(top + 54.0 + lines.len() as f32 * 21.0 - 21.0).max(plate.bottom()) + 10.0;
        if !d.description.is_empty() {
            for line in wrap_text(&self.world.expand_text(&d.description), 20, w) {
                y += 24.0;
                if y > r.y + r.h - 50.0 {
                    return;
                }
                draw_text(&line, x, y, 20.0, LIGHTGRAY);
            }
            y += 8.0;
        }
        if !d.loot.is_empty() {
            y += 24.0;
            draw_text("Drops", x, y, 18.0, Color::new(0.85, 0.8, 0.6, 0.8));
            let drops: Vec<String> = d
                .loot
                .iter()
                .map(|l| {
                    let item = db.item(&l.item).map_or(l.item.clone(), |i| i.name.clone());
                    let n = if l.max > 1 { format!(" {}-{}", l.min.max(1), l.max) } else { String::new() };
                    format!("{item}{n} ({}%)", l.chance.round())
                })
                .collect();
            for line in wrap_text(&drops.join(", "), 18, w) {
                y += 22.0;
                if y > r.y + r.h - 46.0 {
                    return;
                }
                draw_text(&line, x, y, 18.0, LIGHTGRAY);
            }
        }
    }
}
