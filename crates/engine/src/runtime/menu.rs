//! Pause menu (Esc): resume, save to a named slot, load / delete saves, save and quit,
//! quit to the title screen; and the title screen (main menu) itself.

use std::time::{SystemTime, UNIX_EPOCH};

use macroquad::prelude::*;

use super::{input, ui::panel, Arrival, Fade, Game, FADE_TIME};

const W: f32 = 520.0;
const ROW_H: f32 = 40.0;
const MAX_ROWS: usize = 9;
const MAX_NAME: usize = 32;

#[derive(Clone, Debug, PartialEq)]
enum Page {
    /// Title screen (main menu at game start).
    Title,
    Main,
    Save,
    Load,
    /// Typing the name of a new save.
    Name,
    ConfirmOverwrite(String),
    ConfirmDelete(String),
    /// Quit to the title screen without saving?
    ConfirmQuit,
    /// The window's close button was clicked: save before exiting? Holds the page to go back
    /// to on Cancel (None = back to the game).
    ConfirmClose(Option<Box<Page>>),
}

struct SaveInfo {
    slot: String,
    map: String,
    /// Seconds since the epoch when saved (file time for old saves).
    saved: u64,
    play_time: f64,
}

pub(super) struct Menu {
    open: bool,
    page: Page,
    sel: usize,
    scroll: usize,
    saves: Vec<SaveInfo>,
    name: String,
    message: Option<String>,
    pub quit: bool,
    /// Showing the title screen (instead of the pause menu).
    title: bool,
}

impl Default for Menu {
    fn default() -> Self {
        Menu { open: false, page: Page::Main, sel: 0, scroll: 0, saves: Vec::new(), name: String::new(), message: None, quit: false, title: false }
    }
}

/// What a row of the current page does.
#[derive(Clone, Debug, PartialEq)]
enum Row {
    NewGame,
    /// Load the newest save.
    Continue,
    Exit,
    Resume,
    SaveMenu,
    LoadMenu,
    SaveQuit,
    Quit,
    NewSave,
    Slot(String),
    Yes,
    No,
    /// Save, then exit the program.
    SaveExit,
    /// Exit the program without saving.
    ExitNoSave,
}

pub(super) fn now_secs() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs())
}

/// Slot names become file names: letters, digits, space, '-' and '_'.
fn sanitize(name: &str) -> String {
    let s: String = name.chars().filter(|c| c.is_alphanumeric() || matches!(c, ' ' | '-' | '_')).take(MAX_NAME).collect();
    s.trim().to_string()
}

fn display_name(slot: &str) -> String {
    if slot == "quick" { "Quicksave".to_string() } else { slot.to_string() }
}

fn ago(secs: u64) -> String {
    let d = now_secs().saturating_sub(secs);
    match d {
        0..=59 => "just now".into(),
        60..=3599 => format!("{} min ago", d / 60),
        3600..=86_399 => format!("{} h ago", d / 3600),
        _ => format!("{} days ago", d / 86_400),
    }
}

fn play_time(secs: f64) -> String {
    let s = secs as u64;
    if s >= 3600 { format!("{}:{:02}:{:02}", s / 3600, s / 60 % 60, s % 60) } else { format!("{}:{:02}", s / 60, s % 60) }
}

impl Game {
    /// Lists the save files (newest first).
    fn scan_saves(&mut self) {
        let dir = self.project.saves_dir();
        let mut out = Vec::new();
        for e in std::fs::read_dir(&dir).into_iter().flatten().flatten() {
            let p = e.path();
            if p.extension().is_none_or(|x| x != "ron") {
                continue;
            }
            let Some(slot) = p.file_stem().map(|s| s.to_string_lossy().into_owned()) else { continue };
            let file_time = e.metadata().and_then(|m| m.modified()).ok().and_then(|t| t.duration_since(UNIX_EPOCH).ok()).map_or(0, |d| d.as_secs());
            let world = std::fs::read_to_string(&p).ok().and_then(|s| crate::state::WorldState::from_ron(&s).ok());
            let (map, saved, play) = match &world {
                Some(w) => (w.current_map.clone(), if w.saved_at > 0 { w.saved_at } else { file_time }, w.play_time),
                None => ("(unreadable)".into(), file_time, 0.0),
            };
            out.push(SaveInfo { slot, map, saved, play_time: play });
        }
        out.sort_by_key(|s| std::cmp::Reverse(s.saved));
        self.menu.saves = out;
    }

    fn rows(&self) -> Vec<(Row, String, String)> {
        let save_rows = |m: &Menu| -> Vec<(Row, String, String)> {
            m.saves
                .iter()
                .map(|s| {
                    let detail = format!("{} · {} played · {}", s.map, play_time(s.play_time), ago(s.saved));
                    (Row::Slot(s.slot.clone()), display_name(&s.slot), detail)
                })
                .collect()
        };
        match &self.menu.page {
            Page::Title => {
                let latest = self.menu.saves.first().map_or("no saved game yet".to_string(), |s| {
                    format!("{} · {} · {}", display_name(&s.slot), s.map, ago(s.saved))
                });
                vec![
                    (Row::NewGame, "New game".into(), String::new()),
                    (Row::Continue, "Continue".into(), latest),
                    (Row::LoadMenu, "Load game".into(), String::new()),
                    (Row::Exit, "Exit".into(), String::new()),
                ]
            }
            Page::Main => vec![
                (Row::Resume, "Resume".into(), String::new()),
                (Row::SaveMenu, "Save game".into(), String::new()),
                (Row::LoadMenu, "Load game".into(), String::new()),
                (Row::SaveQuit, "Save and quit".into(), format!("Saves to \"{}\" and returns to the main menu", display_name(&self.save_quit_slot()))),
                (Row::Quit, "Quit".into(), "Back to the main menu without saving".into()),
            ],
            Page::Save => {
                let mut v = vec![(Row::NewSave, "+ New save...".into(), String::new())];
                v.extend(save_rows(&self.menu).into_iter().filter(|r| r.0 != Row::Slot("quick".into())));
                v
            }
            Page::Load => save_rows(&self.menu),
            Page::Name => vec![],
            Page::ConfirmQuit => vec![
                (Row::Yes, "Quit".into(), "Progress since the last save is lost".into()),
                (Row::No, "Cancel".into(), String::new()),
            ],
            Page::ConfirmClose(_) => vec![
                (
                    Row::SaveExit,
                    "Save and exit".into(),
                    if self.runner.busy() {
                        "Not possible during an event: Cancel, finish it, then close again".into()
                    } else {
                        format!("Saves to \"{}\", then closes the game", display_name(&self.save_quit_slot()))
                    },
                ),
                (Row::ExitNoSave, "Exit without saving".into(), "Progress since the last save is lost".into()),
                (Row::No, "Cancel".into(), "Back to the game".into()),
            ],
            Page::ConfirmOverwrite(_) | Page::ConfirmDelete(_) => {
                vec![(Row::Yes, "Yes".into(), String::new()), (Row::No, "No".into(), String::new())]
            }
        }
    }

    fn title(&self) -> String {
        match &self.menu.page {
            Page::Title => String::new(),
            Page::Main => "Paused".into(),
            Page::Save => "Save game".into(),
            Page::Load => "Load game".into(),
            Page::Name => "Name your save".into(),
            Page::ConfirmOverwrite(s) => format!("Overwrite \"{}\"?", display_name(s)),
            Page::ConfirmDelete(s) => format!("Delete \"{}\"?", display_name(s)),
            Page::ConfirmQuit => "Quit without saving?".into(),
            Page::ConfirmClose(_) => "Exit the game?".into(),
        }
    }

    /// Panel rect and the screen rects of the visible rows.
    fn menu_layout(&self, n: usize) -> (Rect, Vec<Rect>) {
        let (sw, sh) = (screen_width(), screen_height());
        let rows = n.clamp(1, MAX_ROWS) as f32;
        // The title page has no heading (the game name is drawn above the panel).
        let head = if self.menu.page == Page::Title { 20.0 } else { 64.0 };
        let h = head + 6.0 + rows * ROW_H + 50.0 + if self.menu.page == Page::Name { 60.0 } else { 0.0 };
        let p = Rect::new(((sw - W) / 2.0).round(), ((sh - h) / 2.0).round() + if self.menu.title { 40.0 } else { 0.0 }, W, h);
        let top = p.y + head + if self.menu.page == Page::Name { 60.0 } else { 0.0 };
        let rects = (0..n.min(MAX_ROWS)).map(|k| Rect::new(p.x + 16.0, top + k as f32 * ROW_H, W - 32.0, ROW_H - 4.0)).collect();
        (p, rects)
    }

    fn go(&mut self, page: Page) {
        if matches!(page, Page::Main | Page::Save | Page::Load | Page::Title) {
            self.scan_saves();
        }
        self.menu.page = page;
        self.menu.sel = 0;
        self.menu.scroll = 0;
    }

    fn back(&mut self) {
        match self.menu.page.clone() {
            Page::Title => {}
            Page::Main => self.menu.open = false,
            Page::Load if self.menu.title => {
                self.go(Page::Title);
                self.menu.sel = 2; // back on "Load game"
            }
            Page::Save | Page::Load => {
                let row = if self.menu.page == Page::Save { 1 } else { 2 };
                self.go(Page::Main);
                self.menu.sel = row; // back on the row we came from
            }
            Page::Name | Page::ConfirmOverwrite(_) => self.go(Page::Save),
            Page::ConfirmDelete(_) => self.go(Page::Load),
            Page::ConfirmQuit => {
                self.go(Page::Main);
                self.menu.sel = 4; // back on "Quit"
            }
            Page::ConfirmClose(back) => match back {
                Some(p) => self.go(*p),
                None => self.menu.open = false,
            },
        }
    }

    /// Slot "Save and quit" writes to: the one this game was last saved to / loaded from,
    /// otherwise a new "Save N".
    fn save_quit_slot(&self) -> String {
        if let Some(s) = &self.slot {
            return s.clone();
        }
        (1..).map(|n| format!("Save {n}")).find(|name| !self.menu.saves.iter().any(|s| &s.slot == name)).unwrap()
    }

    /// Leaves the running game for the title screen, with a fresh game behind it.
    fn quit_to_title(&mut self) {
        self.reset_world();
        let (map, spawn) = (self.project.config.start_map.clone(), self.project.config.start_spawn.clone());
        self.finish_transition(map, Arrival::Spawn(spawn));
        self.fade = Fade::In { t: FADE_TIME };
        self.open_title();
    }

    fn do_save(&mut self, slot: &str) {
        match self.save_game(slot) {
            Ok(()) => {
                self.menu.open = false;
                self.ui.toast(format!("Saved \"{}\"", display_name(slot)));
            }
            Err(e) => self.menu.message = Some(format!("Save failed: {e}")),
        }
    }

    fn activate_row(&mut self, row: Row) {
        self.menu.message = None;
        match (self.menu.page.clone(), row) {
            (_, Row::NewGame) => {
                if self.project.config.creation.enabled {
                    self.open_creator();
                } else {
                    self.new_game();
                    self.close_title();
                }
            }
            (_, Row::Continue) => match self.menu.saves.first().map(|s| s.slot.clone()) {
                Some(slot) => match self.load_game(&slot) {
                    Ok(()) => self.close_title(),
                    Err(e) => self.menu.message = Some(format!("Load failed: {e}")),
                },
                None => self.menu.message = Some("There is no saved game yet.".into()),
            },
            (_, Row::Exit) => self.menu.quit = true,
            (_, Row::Resume) => self.menu.open = false,
            (_, Row::SaveMenu) => {
                if self.runner.busy() {
                    self.menu.message = Some("You can't save in the middle of an event.".into());
                } else {
                    self.go(Page::Save);
                }
            }
            (_, Row::LoadMenu) => self.go(Page::Load),
            (_, Row::SaveQuit) => {
                let slot = self.save_quit_slot();
                match self.save_game(&slot) {
                    Ok(()) => {
                        self.quit_to_title();
                        self.ui.toast(format!("Saved \"{}\"", display_name(&slot)));
                    }
                    Err(_) if self.runner.busy() => self.menu.message = Some("You can't save in the middle of an event.".into()),
                    Err(e) => self.menu.message = Some(format!("Save failed: {e}")),
                }
            }
            (_, Row::Quit) => self.go(Page::ConfirmQuit),
            (_, Row::SaveExit) => {
                let slot = self.save_quit_slot();
                match self.save_game(&slot) {
                    Ok(()) => self.menu.quit = true,
                    Err(_) if self.runner.busy() => {
                        self.menu.message = Some("You can't save in the middle of an event. Cancel, finish it, then exit.".into())
                    }
                    Err(e) => self.menu.message = Some(format!("Save failed: {e}")),
                }
            }
            (_, Row::ExitNoSave) => self.menu.quit = true,
            (Page::ConfirmQuit, Row::Yes) => self.quit_to_title(),
            (_, Row::NewSave) => {
                let n = (1..).find(|n| !self.menu.saves.iter().any(|s| s.slot == format!("Save {n}"))).unwrap();
                self.menu.name = format!("Save {n}");
                self.menu.page = Page::Name;
                while get_char_pressed().is_some() {} // drop keys typed before
            }
            (Page::Save, Row::Slot(s)) => self.go(Page::ConfirmOverwrite(s)),
            (Page::Load, Row::Slot(s)) => match self.load_game(&s) {
                Ok(()) => self.close_title(),
                Err(e) => self.menu.message = Some(format!("Load failed: {e}")),
            },
            (Page::ConfirmOverwrite(s), Row::Yes) => self.do_save(&s),
            (Page::ConfirmDelete(s), Row::Yes) => {
                let _ = std::fs::remove_file(self.project.saves_dir().join(format!("{s}.ron")));
                self.go(Page::Load);
            }
            (_, Row::No) => self.back(),
            _ => {}
        }
    }

    /// The window's close button was clicked: asks whether to save first while a game is
    /// running; on the title / game over screen the game just exits.
    pub fn request_close(&mut self) {
        if (self.menu.open && self.menu.title) || self.game_over.is_some() {
            self.menu.quit = true;
            return;
        }
        if self.menu.open && matches!(self.menu.page, Page::ConfirmClose(_)) {
            return; // already asking
        }
        let back = self.menu.open.then(|| Box::new(self.menu.page.clone()));
        self.menu.open = true;
        self.menu.message = None;
        self.scan_saves();
        self.go(Page::ConfirmClose(back));
    }

    /// Shows the title screen (main menu). The selection starts on Continue if there are saves.
    pub(super) fn open_title(&mut self) {
        self.menu = Menu { open: true, title: true, ..Default::default() };
        self.go(Page::Title);
        if !self.menu.saves.is_empty() {
            self.menu.sel = 1;
        }
    }

    pub(super) fn close_title(&mut self) {
        self.menu.open = false;
        self.menu.title = false;
        // The start map's "map enter" from startup must not fire after loading another game;
        // a new game queues its own when the map is (re)entered.
        self.pending_events.clear();
    }

    pub(super) fn on_title(&self) -> bool {
        self.menu.title
    }

    /// Handles Esc and the open menu. Returns true while the game is paused by it.
    pub(super) fn update_menu(&mut self, can_open: bool) -> bool {
        if !self.menu.open {
            if can_open && input::pressed(KeyCode::Escape) {
                self.menu = Menu { open: true, ..Default::default() };
                self.go(Page::Main);
                return true;
            }
            return false;
        }
        let rows = self.rows();
        let (panel_rect, rects) = self.menu_layout(rows.len());
        let m = input::mouse_pos();

        if self.menu.page == Page::Name {
            while let Some(c) = input::next_char() {
                if !c.is_control() && self.menu.name.chars().count() < MAX_NAME {
                    self.menu.name.push(c);
                }
            }
            if input::pressed(KeyCode::Backspace) {
                self.menu.name.pop();
            }
            if input::pressed(KeyCode::Escape) {
                self.back();
            } else if input::pressed(KeyCode::Enter) || (input::mouse_pressed(MouseButton::Left) && self.name_ok_rect().contains(m)) {
                let slot = sanitize(&self.menu.name);
                if slot.is_empty() {
                    self.menu.message = Some("Please enter a name.".into());
                } else if self.menu.saves.iter().any(|s| s.slot == slot) {
                    self.go(Page::ConfirmOverwrite(slot));
                } else {
                    self.do_save(&slot);
                }
            }
            return true;
        }

        let n = rows.len();
        if input::pressed(KeyCode::Escape) {
            self.back();
            return true;
        }
        if n > 0 {
            if input::pressed(KeyCode::Up) || input::pressed(KeyCode::W) {
                self.menu.sel = (self.menu.sel + n - 1) % n;
            }
            if input::pressed(KeyCode::Down) || input::pressed(KeyCode::S) {
                self.menu.sel = (self.menu.sel + 1) % n;
            }
            // Keep the selection visible.
            if self.menu.sel < self.menu.scroll {
                self.menu.scroll = self.menu.sel;
            } else if self.menu.sel >= self.menu.scroll + MAX_ROWS {
                self.menu.scroll = self.menu.sel + 1 - MAX_ROWS;
            }
            let wheel = mouse_wheel().1;
            if wheel != 0.0 && panel_rect.contains(m) && n > MAX_ROWS {
                self.menu.scroll = if wheel < 0.0 { (self.menu.scroll + 1).min(n - MAX_ROWS) } else { self.menu.scroll.saturating_sub(1) };
            }
            let hovered = rects.iter().position(|r| r.contains(m)).map(|k| k + self.menu.scroll).filter(|k| *k < n);
            if let Some(k) = hovered {
                if mouse_delta_position() != Vec2::ZERO {
                    self.menu.sel = k;
                }
            }
            if input::any_pressed(&[KeyCode::Enter, KeyCode::Space, KeyCode::E]) {
                self.activate_row(rows[self.menu.sel.min(n - 1)].0.clone());
            } else if input::mouse_pressed(MouseButton::Left) {
                match hovered {
                    Some(k) => {
                        self.menu.sel = k;
                        self.activate_row(rows[k].0.clone());
                    }
                    None if !panel_rect.contains(m) => self.back(),
                    None => {}
                }
            }
            // Delete a save from the load list.
            if self.menu.page == Page::Load && input::pressed(KeyCode::Delete) {
                if let Row::Slot(s) = &rows[self.menu.sel.min(n - 1)].0 {
                    self.go(Page::ConfirmDelete(s.clone()));
                }
            }
        } else if input::mouse_pressed(MouseButton::Left) && !panel_rect.contains(m) {
            self.back();
        }
        true
    }

    fn name_ok_rect(&self) -> Rect {
        let (p, _) = self.menu_layout(0);
        Rect::new(p.x + W - 116.0, p.y + 70.0, 100.0, 36.0)
    }

    pub(super) fn draw_menu(&self) {
        if !self.menu.open {
            return;
        }
        let (sw, sh) = (screen_width(), screen_height());
        draw_rectangle(0.0, 0.0, sw, sh, Color::new(0.0, 0.0, 0.0, 0.55));
        let rows = self.rows();
        let (p, rects) = self.menu_layout(rows.len());
        if self.menu.title {
            // Big game title above the menu.
            let name = &self.project.config.name;
            let size = 64u16;
            let tw = measure_text(name, None, size, 1.0).width;
            let y = (p.y - 60.0).max(80.0);
            draw_text(name, (sw - tw) / 2.0 + 3.0, y + 3.0, size as f32, Color::new(0.0, 0.0, 0.0, 0.7));
            draw_text(name, (sw - tw) / 2.0, y, size as f32, GOLD);
        }
        panel(p.x, p.y, p.w, p.h);
        let title = self.title();
        let tw = measure_text(&title, None, 34, 1.0).width;
        draw_text(&title, p.x + (p.w - tw) / 2.0, p.y + 42.0, 34.0, GOLD);
        let m = input::mouse_pos();

        if self.menu.page == Page::Name {
            let field = Rect::new(p.x + 16.0, p.y + 70.0, W - 146.0, 36.0);
            draw_rectangle(field.x, field.y, field.w, field.h, Color::new(0.0, 0.0, 0.0, 0.5));
            draw_rectangle_lines(field.x, field.y, field.w, field.h, 2.0, GOLD);
            let caret = if (get_time() * 2.0) as i64 % 2 == 0 { "_" } else { "" };
            draw_text(format!("{}{caret}", self.menu.name), field.x + 10.0, field.y + 26.0, 26.0, WHITE);
            let ok = self.name_ok_rect();
            draw_rectangle(ok.x, ok.y, ok.w, ok.h, Color::new(0.3, 0.25, 0.1, if ok.contains(m) { 1.0 } else { 0.8 }));
            draw_rectangle_lines(ok.x, ok.y, ok.w, ok.h, 1.5, GOLD);
            draw_text("Save", ok.x + 28.0, ok.y + 25.0, 24.0, WHITE);
        }

        if rows.is_empty() && self.menu.page != Page::Name {
            draw_text("No saved games yet.", p.x + 24.0, p.y + 92.0, 22.0, GRAY);
        }
        for (k, r) in rects.iter().enumerate() {
            let idx = k + self.menu.scroll;
            let Some((_, label, detail)) = rows.get(idx) else { break };
            let selected = idx == self.menu.sel;
            if selected {
                draw_rectangle(r.x, r.y, r.w, r.h, Color::new(1.0, 1.0, 1.0, 0.12));
                draw_rectangle_lines(r.x, r.y, r.w, r.h, 1.5, GOLD);
            }
            let disabled = self.menu.page == Page::Title && idx == 1 && self.menu.saves.is_empty();
            let c = if disabled { GRAY } else if selected { GOLD } else { WHITE };
            if detail.is_empty() {
                draw_text(label, r.x + 14.0, r.y + 26.0, 26.0, c);
            } else {
                draw_text(label, r.x + 14.0, r.y + 17.0, 22.0, c);
                draw_text(detail, r.x + 14.0, r.y + 33.0, 16.0, LIGHTGRAY);
            }
        }
        if rows.len() > MAX_ROWS {
            draw_text(format!("{}-{} of {}", self.menu.scroll + 1, (self.menu.scroll + MAX_ROWS).min(rows.len()), rows.len()), p.x + p.w - 110.0, p.y + 42.0, 16.0, GRAY);
        }
        let hint = match self.menu.page {
            Page::Title => "Up/Down + Enter or click",
            Page::Main => "Up/Down + Enter or click · Esc: resume",
            Page::Save => "Pick a save to overwrite, or make a new one · Esc: back",
            Page::Load => "Enter / click: load · Del: delete · Esc: back",
            Page::Name => "Type a name · Enter: save · Esc: back",
            _ => "Enter / click · Esc: back",
        };
        if let Some(msg) = &self.menu.message {
            draw_text(msg, p.x + 16.0, p.y + p.h - 34.0, 18.0, Color::new(1.0, 0.5, 0.5, 1.0));
        }
        draw_text(hint, p.x + 16.0, p.y + p.h - 12.0, 16.0, GRAY);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn save_names_and_labels() {
        assert_eq!(sanitize("  My/../save: 1 "), "Mysave 1");
        assert_eq!(sanitize(&"x".repeat(50)).len(), MAX_NAME);
        assert_eq!(display_name("quick"), "Quicksave");
        assert_eq!(play_time(3725.0), "1:02:05");
        assert_eq!(play_time(65.0), "1:05");
        assert_eq!(ago(now_secs()), "just now");
        assert_eq!(ago(now_secs() - 7200), "2 h ago");
    }
}
