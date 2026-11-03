//! Definition editors: items, props, characters, tilesets, sprite sheets, sounds, project and scripts.

use egui_macroquad::egui::{self, pos2, vec2, Color32, Sense, Stroke, StrokeKind};
use rust2d_engine::audio::Audio;
use rust2d_engine::defs::*;
use rust2d_engine::events::TriggerOn;
use rust2d_engine::refactor::RefKind;
use rust2d_engine::render::Assets;

use crate::app::Editor;
use crate::files::Purpose;
use crate::widgets::{self, Refs, Textures};

#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub enum Tab {
    #[default]
    Items,
    Props,
    Characters,
    Tilesets,
    Sheets,
    Sounds,
    Project,
    Scripts,
}

/// Inline "rename id" editor state; at most one rename is open at a time.
#[derive(Default)]
pub struct RenameUi {
    pub buf: Option<String>,
    /// (kind, old, new) requested by the user, executed after the frame's UI.
    pub request: Option<(RefKind, String, String)>,
}

#[derive(Default)]
pub struct DbState {
    pub tab: Tab,
    pub sel: usize,
    pub script_file: Option<std::path::PathBuf>,
    pub script_text: String,
    pub script_dirty: bool,
    pub new_script: String,
    pub rename: RenameUi,
    /// Tileset editor: animation that clicked tiles are appended to (None = toggle solidity).
    pub anim_target: Option<usize>,
    /// Prop editor: selected state (its sprite is edited below the list) and an open rename.
    pub state_sel: usize,
    pub state_rename: Option<(usize, String)>,
}

/// Everything the definition forms need besides the definition itself.
struct Cx<'a> {
    tex: &'a Textures,
    assets: &'a mut Assets,
    refs: &'a Refs,
    audio: &'a Audio,
    rename: &'a mut RenameUi,
    anim_target: &'a mut Option<usize>,
    /// Read-only copy of the definitions (for item icons / stack sizes).
    db: &'a Database,
    browse: &'a mut Option<Purpose>,
    /// Request to open an image in the pixel editor: (path, frame size, character layout).
    /// (image, frame size, character layout, frame to select).
    paint: &'a mut Option<(String, (u32, u32), bool, u32)>,
    state_sel: &'a mut usize,
    state_rename: &'a mut Option<(usize, String)>,
    /// A renamed prop state: (prop id, old name, new name), applied to the maps afterwards.
    state_renamed: &'a mut Option<(String, String, String)>,
}

/// An id not yet used in `ids`, based on `prefix`.
fn unique(prefix: &str, ids: &[String]) -> String {
    (1..).map(|n| format!("{prefix}_{n}")).find(|c| !ids.contains(c)).unwrap()
}

/// List with add / duplicate / delete. Returns true if the list changed.
fn list_ui<T: Clone>(
    ui: &mut egui::Ui,
    items: &mut Vec<T>,
    sel: &mut usize,
    label: impl Fn(&T) -> String,
    make: impl Fn(&[String]) -> T,
    id_of: impl Fn(&mut T) -> &mut String,
) -> bool {
    let mut changed = false;
    let mut ids: Vec<String> = items.iter_mut().map(|t| id_of(t).clone()).collect();
    ui.horizontal(|ui| {
        if ui.button("+ New").clicked() {
            items.push(make(&ids));
            *sel = items.len() - 1;
            changed = true;
        }
        if ui.add_enabled(*sel < items.len(), egui::Button::new("Duplicate")).clicked() {
            let mut c = items[*sel].clone();
            let id = id_of(&mut c);
            *id = unique(id, &ids);
            ids.push(id.clone());
            items.push(c);
            *sel = items.len() - 1;
            changed = true;
        }
        if ui.add_enabled(*sel < items.len(), egui::Button::new("Delete")).on_hover_text("References in maps are not removed").clicked() {
            items.remove(*sel);
            *sel = sel.saturating_sub(1);
            changed = true;
        }
    });
    ui.separator();
    egui::ScrollArea::vertical().id_salt("db_list").show(ui, |ui| {
        for (i, it) in items.iter().enumerate() {
            if ui.selectable_label(*sel == i, label(it)).clicked() {
                *sel = i;
            }
        }
    });
    changed
}

pub fn ui(ed: &mut Editor, ui: &mut egui::Ui) {
    ui.horizontal(|ui| {
        let tabs = [
            (Tab::Items, "Items"),
            (Tab::Props, "Props"),
            (Tab::Characters, "Characters"),
            (Tab::Tilesets, "Tilesets"),
            (Tab::Sheets, "Sprite sheets"),
            (Tab::Sounds, "Sounds"),
            (Tab::Project, "Project"),
            (Tab::Scripts, "Scripts"),
        ];
        for (t, l) in tabs {
            if ui.selectable_label(ed.db.tab == t, l).clicked() && ed.db.tab != t {
                ed.db.tab = t;
                ed.db.sel = 0;
                ed.db.rename = RenameUi::default();
                ed.db.anim_target = None;
            }
        }
        ui.separator();
        if ui.button("💾 Save definitions").clicked() {
            ed.save_defs();
        }
        if ui.button("⟳ Reload assets").clicked() {
            ed.reload_assets();
        }
        ui.separator();
        // The selected asset of this tab, for the library / package buttons.
        let i = ed.db.sel;
        let db = &ed.project.db;
        let current: Option<rust2d_engine::library::AssetRef> = match ed.db.tab {
            Tab::Items => db.items.get(i).map(|d| (RefKind::Item, d.id.clone())),
            Tab::Props => db.props.get(i).map(|d| (RefKind::Prop, d.id.clone())),
            Tab::Characters => db.characters.get(i).map(|d| (RefKind::Character, d.id.clone())),
            Tab::Tilesets => db.tilesets.get(i).map(|d| (RefKind::Tileset, d.id.clone())),
            Tab::Sheets => db.sheets.get(i).map(|d| (RefKind::Sheet, d.id.clone())),
            Tab::Sounds => db.sounds.get(i).map(|d| (RefKind::Sound, d.id.clone())),
            _ => None,
        };
        if let Some(a) = current {
            if ui.button("📂 Add to library").on_hover_text(format!("Copies '{}' and everything it needs into the shared asset library", a.1)).clicked() {
                let r = ed.project_to_library(std::slice::from_ref(&a));
                ed.show_result(r);
            }
            if ui.button("📦 Export…").on_hover_text(format!("Saves '{}' and everything it needs as a .r2dpack file", a.1)).clicked() {
                ed.open_file_dialog(Purpose::ExportPackage { from_library: false, assets: vec![a] });
            }
        }
        if ui.button("✨ New asset…").clicked() {
            ed.wizard = Some(crate::wizard::Wizard::new(&ed.project));
        }
    });
    ui.separator();

    let before = (ed.project.db.clone(), ed.project.config.clone());
    let mut paint: Option<(String, (u32, u32), bool, u32)> = None;
    let mut state_renamed: Option<(String, String, String)> = None;
    let mut list_changed = false;
    let tab = ed.db.tab;
    let sel_before = ed.db.sel;
    if matches!(tab, Tab::Project | Tab::Scripts) {
        egui::CentralPanel::default().show_inside(ui, |ui| match tab {
            Tab::Project => project_ui(ed, ui),
            _ => scripts_ui(ed, ui),
        });
    } else {
        let db = &mut ed.project.db;
        let sel = &mut ed.db.sel;
        egui::SidePanel::left("db_left").resizable(true).default_width(220.0).show_inside(ui, |ui| {
            list_changed = match tab {
                Tab::Items => list_ui(
                    ui,
                    &mut db.items,
                    sel,
                    |d| format!("{}  ({})", d.name, d.id),
                    |ids| ItemDef {
                        id: unique("item", ids),
                        name: "New item".into(),
                        description: String::new(),
                        icon: SpriteRef::default(),
                        consumable: false,
                        on_use: vec![],
                        weapon: None,
                        armor: None,
                        max_stack: 99,
                    },
                    |d| &mut d.id,
                ),
                Tab::Props => list_ui(
                    ui,
                    &mut db.props,
                    sel,
                    |d| format!("{}  ({})", d.name, d.id),
                    |ids| PropDef {
                        id: unique("prop", ids),
                        name: "New prop".into(),
                        sprite: SpriteRef::default(),
                        solid: true,
                        hitbox: [28.0, 16.0],
                        triggers: vec![],
                        container: None,
                        lock: None,
                        default_state: default_state_name(),
                        states: vec![],
                    },
                    |d| &mut d.id,
                ),
                Tab::Characters => list_ui(
                    ui,
                    &mut db.characters,
                    sel,
                    |d| format!("{}  ({})", d.name, d.id),
                    |ids| CharacterDef {
                        id: unique("character", ids),
                        name: "New character".into(),
                        sheet: String::new(),
                        walk_frames: 4,
                        speed: 100.0,
                        hitbox: [20.0, 12.0],
                        behavior: Behavior::Static,
                        max_hp: 0,
                        damage: 0,
                        hostile: false,
                        weapon: None,
                        defense: 0,
                        triggers: vec![],
                    },
                    |d| &mut d.id,
                ),
                Tab::Tilesets => list_ui(
                    ui,
                    &mut db.tilesets,
                    sel,
                    |d| d.id.clone(),
                    |ids| TilesetDef { id: unique("tileset", ids), image: "tilesets/new.png".into(), solid: vec![], animations: vec![] },
                    |d| &mut d.id,
                ),
                Tab::Sheets => list_ui(
                    ui,
                    &mut db.sheets,
                    sel,
                    |d| d.id.clone(),
                    |ids| SheetDef { id: unique("sheet", ids), image: "sprites/new.png".into(), frame_w: 32, frame_h: 32 },
                    |d| &mut d.id,
                ),
                Tab::Sounds => list_ui(
                    ui,
                    &mut db.sounds,
                    sel,
                    |d| d.id.clone(),
                    |ids| SoundDef { id: unique("sound", ids), file: "sounds/new.wav".into(), volume: 1.0 },
                    |d| &mut d.id,
                ),
                _ => false,
            };
        });
        if ed.db.sel != sel_before {
            ed.db.rename = RenameUi::default();
            ed.db.anim_target = None;
        }
        let db_snapshot = ed.project.db.clone();
        egui::CentralPanel::default().show_inside(ui, |ui| {
            egui::ScrollArea::vertical().id_salt("db_form").show(ui, |ui| {
                let mut cx = Cx {
                    tex: &ed.tex,
                    assets: &mut ed.assets,
                    refs: &ed.refs,
                    audio: &ed.audio,
                    rename: &mut ed.db.rename,
                    anim_target: &mut ed.db.anim_target,
                    browse: &mut ed.browse_request,
                    db: &db_snapshot,
                    paint: &mut paint,
                    state_sel: &mut ed.db.state_sel,
                    state_rename: &mut ed.db.state_rename,
                    state_renamed: &mut state_renamed,
                };
                let i = ed.db.sel;
                let db = &mut ed.project.db;
                match tab {
                    Tab::Items => {
                        if let Some(d) = db.items.get_mut(i) {
                            item_form(ui, d, &mut cx);
                        }
                    }
                    Tab::Props => {
                        if let Some(d) = db.props.get_mut(i) {
                            prop_form(ui, d, &mut cx);
                        }
                    }
                    Tab::Characters => {
                        if let Some(d) = db.characters.get_mut(i) {
                            character_form(ui, d, &mut cx);
                        }
                    }
                    Tab::Tilesets => {
                        if let Some(d) = db.tilesets.get_mut(i) {
                            tileset_form(ui, d, &mut cx);
                        }
                    }
                    Tab::Sheets => {
                        if let Some(d) = db.sheets.get_mut(i) {
                            sheet_form(ui, d, &mut cx);
                        }
                    }
                    Tab::Sounds => {
                        if let Some(d) = db.sounds.get_mut(i) {
                            sound_form(ui, d, &mut cx);
                        }
                    }
                    _ => {}
                }
            });
        });
    }
    if let Some((rel, frame, character, sel)) = paint.take() {
        ed.open_pixel(&rel, frame, if character { crate::pixel::Layout::Character } else { crate::pixel::Layout::Plain });
        if ed.px.doc.as_ref().is_some_and(|d| d.rel == rel) {
            ed.px.sel_frame = sel;
        }
    }
    if let Some((prop, old, new)) = state_renamed.take() {
        ed.rename_prop_state(&prop, &old, &new);
    }
    if list_changed || before.0 != ed.project.db || before.1 != ed.project.config {
        ed.defs_dirty = true;
        if before.0.items != ed.project.db.items || before.0.sounds != ed.project.db.sounds {
            ed.refs.items = ed.project.db.items.iter().map(|i| i.id.clone()).collect();
            ed.refs.sounds = ed.project.db.sounds.iter().map(|i| i.id.clone()).collect();
        }
    }
    if let Some((kind, old, new)) = ed.db.rename.request.take() {
        ed.rename_id(kind, &old, &new);
    }
}

/// Shows an id with a "Rename" button; renaming updates every reference in the project.
pub fn rename_row(ui: &mut egui::Ui, kind: RefKind, id: &str, rn: &mut RenameUi) {
    ui.horizontal(|ui| {
        ui.label("Id");
        match &mut rn.buf {
            None => {
                ui.monospace(id);
                if ui.small_button("Rename…").on_hover_text("Renames the id and updates all references in maps, triggers and definitions").clicked() {
                    rn.buf = Some(id.to_string());
                }
            }
            Some(buf) => {
                let r = ui.add(egui::TextEdit::singleline(buf).desired_width(140.0));
                let enter = r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                if ui.button("Apply").clicked() || enter {
                    rn.request = Some((kind, id.to_string(), buf.trim().to_string()));
                    rn.buf = None;
                } else if ui.button("Cancel").clicked() {
                    rn.buf = None;
                }
            }
        }
    });
}

fn name_row(ui: &mut egui::Ui, name: &mut String) {
    ui.horizontal(|ui| {
        ui.label("Name");
        ui.text_edit_singleline(name);
    });
}

fn hitbox(ui: &mut egui::Ui, hb: &mut [f32; 2]) {
    ui.horizontal(|ui| {
        ui.label("Hitbox w/h (px)");
        ui.add(egui::DragValue::new(&mut hb[0]).range(1.0..=256.0));
        ui.add(egui::DragValue::new(&mut hb[1]).range(1.0..=256.0));
    });
}

/// Editor for an optional weapon. Characters only use ranged weapons.
fn weapon_ui(ui: &mut egui::Ui, id: &str, w: &mut Option<Weapon>, cx: &mut Cx, character: bool) {
    let mut on = w.is_some();
    // Items choose "Weapon" in the Equipment section; characters toggle their ranged attack here.
    if character && ui.checkbox(&mut on, "Shoots projectiles").changed() {
        *w = on.then(|| {
            if character {
                Weapon::ranged(1, 1.5, Ranged { projectile: SpriteRef::default(), speed: 360.0, range: 6.0, ammo: String::new() })
            } else {
                Weapon::melee(1, 0.4)
            }
        });
    }
    let Some(w) = w else { return };
    egui::Grid::new(format!("{id}/grid")).num_columns(2).show(ui, |ui| {
        ui.label("Damage");
        ui.add(egui::DragValue::new(&mut w.damage).range(0..=999));
        ui.end_row();
        ui.label("Cooldown (s)");
        ui.add(egui::DragValue::new(&mut w.cooldown).range(0.05..=10.0).speed(0.05));
        ui.end_row();
        if !character {
            ui.label("Type");
            ui.horizontal(|ui| {
                let mut ranged = w.ranged.is_some();
                ui.radio_value(&mut ranged, false, "Melee (swing)");
                ui.radio_value(&mut ranged, true, "Ranged (projectiles)");
                if ranged != w.ranged.is_some() {
                    w.ranged = ranged.then(|| Ranged { projectile: SpriteRef::default(), speed: 440.0, range: 8.0, ammo: String::new() });
                }
            });
            ui.end_row();
        }
        if w.ranged.is_none() {
            ui.label("Reach (tiles)");
            ui.add(egui::DragValue::new(&mut w.reach).range(0.3..=6.0).speed(0.05)).on_hover_text("How far the swing reaches");
            ui.end_row();
            ui.label("Arc (degrees)");
            ui.add(egui::DragValue::new(&mut w.arc).range(5.0..=180.0)).on_hover_text("Half-angle of the swing: 20 = narrow thrust, 90 = everything in front");
            ui.end_row();
        }
        ui.label("Knockback");
        ui.add(egui::DragValue::new(&mut w.knockback).range(0.0..=5.0).speed(0.05)).on_hover_text("Multiplier");
        ui.end_row();
        if let Some(r) = &mut w.ranged {
            ui.label("Speed (px/s)");
            ui.add(egui::DragValue::new(&mut r.speed).range(10.0..=2000.0));
            ui.end_row();
            ui.label("Range (tiles)");
            ui.add(egui::DragValue::new(&mut r.range).range(0.5..=64.0).speed(0.1));
            ui.end_row();
            if !character {
                ui.label("Ammo item");
                ui.horizontal(|ui| {
                    widgets::suggest(ui, &format!("{id}/ammo"), &mut r.ammo, &cx.refs.items, 110.0);
                    ui.weak("(empty = unlimited)");
                });
                ui.end_row();
            }
        }
    });
    if let Some(r) = &mut w.ranged {
        ui.label("Projectile sprite (drawn pointing right, rotated in flight):");
        widgets::sprite_picker(ui, &format!("{id}/proj"), &mut r.projectile, cx.tex, cx.assets);
    }
}

fn item_form(ui: &mut egui::Ui, d: &mut ItemDef, cx: &mut Cx) {
    ui.heading("Item");
    rename_row(ui, RefKind::Item, &d.id, cx.rename);
    name_row(ui, &mut d.name);
    ui.label("Description");
    ui.add(egui::TextEdit::multiline(&mut d.description).desired_rows(2).desired_width(f32::INFINITY));
    ui.checkbox(&mut d.consumable, "Consumed when used (if a trigger ran)");
    ui.horizontal(|ui| {
        ui.label("Stack size");
        ui.add(egui::DragValue::new(&mut d.max_stack).range(1..=9999)).on_hover_text("How many fit into one inventory slot");
    });
    ui.separator();
    ui.heading("Equipment");
    ui.weak("Equipment goes into a character-sheet slot when used from the inventory (weapons also with Q) instead of running 'On use' triggers.");
    let mut kind = match (&d.weapon, &d.armor) {
        (Some(_), _) => 1,
        (None, Some(a)) if a.slot == EquipSlot::Head => 2,
        (None, Some(_)) => 3,
        _ => 0,
    };
    let old = kind;
    ui.horizontal(|ui| {
        ui.radio_value(&mut kind, 0, "None");
        ui.radio_value(&mut kind, 1, "Weapon");
        ui.radio_value(&mut kind, 2, "Helmet");
        ui.radio_value(&mut kind, 3, "Body armor");
    });
    if kind != old {
        d.weapon = (kind == 1).then(|| Weapon::melee(1, 0.4));
        d.armor = match kind {
            2 | 3 => {
                let slot = if kind == 2 { EquipSlot::Head } else { EquipSlot::Body };
                Some(d.armor.take().map(|a| Armor { slot, ..a }).unwrap_or(Armor { slot, defense: 1, speed: 0.0, overlay: String::new() }))
            }
            _ => None,
        };
        if kind != 0 {
            d.max_stack = 1;
        }
    }
    if d.weapon.is_some() {
        weapon_ui(ui, "item_weapon", &mut d.weapon, cx, false);
    }
    if let Some(a) = &mut d.armor {
        egui::Grid::new("armor").num_columns(2).show(ui, |ui| {
            ui.label("Defense");
            ui.add(egui::DragValue::new(&mut a.defense).range(-99..=99)).on_hover_text("Subtracted from incoming damage (1 heart = 2 HP)");
            ui.end_row();
            ui.label("Speed change");
            let mut pct = a.speed * 100.0;
            if ui.add(egui::DragValue::new(&mut pct).range(-90.0..=100.0).suffix(" %")).changed() {
                a.speed = pct / 100.0;
            }
            ui.end_row();
            ui.label("Overlay sheet");
            let sheets: Vec<String> = {
                let mut v: Vec<String> = cx.assets.sheets.keys().cloned().collect();
                v.sort();
                v
            };
            widgets::suggest(ui, "armor_overlay", &mut a.overlay, &sheets, 130.0);
            ui.end_row();
        });
        ui.weak("The overlay sheet uses the same layout as the player's sheet and is drawn on top of it (leave empty for none).");
        if !a.overlay.is_empty() {
            let t = (macroquad::time::get_time() * 6.0) as u32 % 4;
            ui.horizontal(|ui| {
                for row in 0..4 {
                    widgets::frame_thumb(ui, cx.tex, cx.assets, &a.overlay, row * 4 + t, 48.0, false);
                }
            });
        }
    }
    ui.separator();
    ui.heading("Icon");
    widgets::sprite_picker(ui, "item_icon", &mut d.icon, cx.tex, cx.assets);
    ui.separator();
    ui.heading("On use");
    ui.weak("Runs when the item is used from the inventory and the entity in front of the player didn't react to it.");
    widgets::triggers_ui(ui, &format!("item/{}", d.id), &mut d.on_use, cx.refs);
    for t in &mut d.on_use {
        if !matches!(t.on, TriggerOn::UseItem { .. }) {
            t.on = TriggerOn::UseItem { item: String::new() };
        }
    }
}

fn prop_form(ui: &mut egui::Ui, d: &mut PropDef, cx: &mut Cx) {
    ui.heading("Prop");
    rename_row(ui, RefKind::Prop, &d.id, cx.rename);
    name_row(ui, &mut d.name);
    hitbox(ui, &mut d.hitbox);
    ui.separator();
    prop_states_ui(ui, d, cx);
    ui.separator();
    let states = d.state_names();
    ui.heading("Container");
    let mut on = d.container.is_some();
    if ui.checkbox(&mut on, "Lootable container (chest, barrel, ...)").changed() {
        d.container = on.then(ContainerSpec::default);
    }
    if let Some(spec) = &mut d.container {
        ui.weak("Default for every placed instance; instances can customize size and contents in the map inspector.");
        widgets::container_ui(ui, "prop_container", spec, cx.refs, cx.tex, cx.assets, cx.db, &states);
    }
    ui.separator();
    ui.heading("Lock");
    let mut on = d.lock.is_some();
    if ui.checkbox(&mut on, "Lockable (needs a key item)").changed() {
        d.lock = on.then(|| LockSpec { key: cx.refs.items.iter().find(|i| i.contains("key")).cloned().unwrap_or_default(), ..Default::default() });
    }
    let prop = d.clone();
    if let Some(lock) = &mut d.lock {
        ui.weak("Default for every placed instance; instances can use a different key or no lock in the map inspector.");
        widgets::lock_ui(ui, "prop_lock", lock, cx.refs, cx.tex, cx.assets, cx.db, Some(&prop));
    }
    ui.separator();
    ui.heading("Default triggers");
    ui.weak("Run for every placed instance, in addition to the instance's own triggers.");
    widgets::triggers_ui(ui, &format!("prop/{}", d.id), &mut d.triggers, cx.refs);
}

/// The prop's states in one place: name, sprite, whether it blocks movement.
fn prop_states_ui(ui: &mut egui::Ui, d: &mut PropDef, cx: &mut Cx) {
    ui.heading("States");
    ui.weak("A prop always shows one of its states. Placed props start in the default state (or the one chosen in the map inspector); a lock, a container, the 'Set prop state' action or set_state(target, \"name\") in scripts switch to another.");
    // All states as one list (index 0 = the default state).
    let mut all: Vec<PropState> = std::iter::once(PropState { name: d.default_state.clone(), sprite: d.sprite.clone(), solid: d.solid }).chain(d.states.iter().cloned()).collect();
    let used: Vec<String> = [
        d.lock.as_ref().and_then(|l| match &l.on_unlock {
            OnUnlock::ToState { state } => Some(state.clone()),
            _ => None,
        }),
        d.container.as_ref().map(|c| c.open_state.clone()).filter(|s| !s.is_empty()),
    ]
    .into_iter()
    .flatten()
    .collect();
    if *cx.state_sel >= all.len() {
        *cx.state_sel = 0;
    }
    enum Op {
        Up(usize),
        Down(usize),
        Delete(usize),
        MakeDefault(usize),
        Rename(usize, String),
    }
    let mut op = None;
    let n = all.len();
    for (i, st) in all.iter_mut().enumerate() {
        let selected = *cx.state_sel == i;
        let frame = egui::Frame::group(ui.style()).fill(if selected { Color32::from_rgb(40, 52, 74) } else { Color32::TRANSPARENT });
        frame.show(ui, |ui| {
            ui.horizontal(|ui| {
                if widgets::sprite_thumb(ui, cx.tex, cx.assets, &st.sprite, 44.0, selected).on_hover_text("Select to change its sprite").clicked() {
                    *cx.state_sel = i;
                }
                ui.vertical(|ui| {
                    ui.horizontal(|ui| {
                        match cx.state_rename {
                            Some((k, buf)) if *k == i => {
                                let r = ui.add(egui::TextEdit::singleline(buf).desired_width(110.0));
                                let clean: String = buf.trim().to_lowercase().replace(' ', "_");
                                let taken = all_names_except(d, i).contains(&clean);
                                let ok = !clean.is_empty() && !taken;
                                if (ui.add_enabled(ok, egui::Button::new("OK")).clicked() || (ok && r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)))) && clean != st.name {
                                    op = Some(Op::Rename(i, clean));
                                } else if ui.button("Cancel").clicked() {
                                    *cx.state_rename = None;
                                }
                                if taken {
                                    ui.colored_label(Color32::LIGHT_RED, "name taken");
                                }
                            }
                            _ => {
                                if ui.selectable_label(selected, egui::RichText::new(&st.name).strong()).clicked() {
                                    *cx.state_sel = i;
                                }
                                if ui.small_button("✏").on_hover_text("Rename (placed props using it follow)").clicked() {
                                    *cx.state_rename = Some((i, st.name.clone()));
                                }
                            }
                        }
                        if i == 0 {
                            ui.weak("default");
                        }
                        if used.contains(&st.name) {
                            ui.weak("· used by the lock / container");
                        }
                    });
                    ui.horizontal(|ui| {
                        ui.checkbox(&mut st.solid, "blocks movement");
                        if i > 0 {
                            if ui.small_button("Make default").on_hover_text("Placed props without a chosen state start in this one").clicked() {
                                op = Some(Op::MakeDefault(i));
                            }
                            if i > 1 && ui.small_button("⏶").clicked() {
                                op = Some(Op::Up(i));
                            }
                            if i + 1 < n && ui.small_button("⏷").clicked() {
                                op = Some(Op::Down(i));
                            }
                            let in_use = used.contains(&st.name);
                            if ui.add_enabled(!in_use, egui::Button::new("✖").small()).on_hover_text(if in_use { "Used by the lock or container" } else { "Delete this state" }).clicked() {
                                op = Some(Op::Delete(i));
                            }
                        }
                    });
                });
            });
        });
    }
    let mut renamed = None;
    match op {
        Some(Op::Up(i)) => all.swap(i, i - 1),
        Some(Op::Down(i)) => all.swap(i, i + 1),
        Some(Op::Delete(i)) => {
            all.remove(i);
            *cx.state_sel = 0;
        }
        Some(Op::MakeDefault(i)) => {
            all.swap(0, i);
            *cx.state_sel = 0;
        }
        Some(Op::Rename(i, new)) => {
            renamed = Some((all[i].name.clone(), new.clone()));
            all[i].name = new;
            *cx.state_rename = None;
        }
        None => {}
    }
    ui.horizontal(|ui| {
        if ui.button("+ Add state").on_hover_text("A new state showing the next frame of the sheet; pick its sprite below").clicked() {
            let last = all.last().unwrap();
            let sprite = SpriteRef { frame: last.sprite.frame + last.sprite.cells().0, ..last.sprite.clone() };
            let names: Vec<String> = all.iter().map(|s| s.name.clone()).collect();
            let name = (2..).map(|k| format!("state_{k}")).find(|c| !names.contains(c)).unwrap();
            all.push(PropState { name: name.clone(), sprite, solid: all[0].solid });
            *cx.state_sel = all.len() - 1;
            *cx.state_rename = Some((all.len() - 1, name));
        }
        ui.weak("e.g. closed / open / destroyed, dormant / glowing, on / off");
    });
    // The selected state's sprite.
    let sel = (*cx.state_sel).min(all.len() - 1);
    ui.add_space(4.0);
    ui.label(egui::RichText::new(format!("Sprite of '{}'", all[sel].name)).strong());
    widgets::sprite_picker(ui, &format!("prop_state_{sel}"), &mut all[sel].sprite, cx.tex, cx.assets);
    if let Some(sheet) = cx.db.sheet(&all[sel].sprite.sheet) {
        if ui.button("✏ Edit pixels").on_hover_text("Open the sheet in the pixel editor at this frame").clicked() {
            *cx.paint = Some((sheet.image.clone(), (sheet.frame_w, sheet.frame_h), false, all[sel].sprite.frame));
        }
    }
    // Write back.
    let first = all.remove(0);
    d.default_state = first.name;
    d.sprite = first.sprite;
    d.solid = first.solid;
    d.states = all;
    if let Some((old, new)) = renamed {
        // References inside this definition follow immediately; maps afterwards.
        if let Some(l) = &mut d.lock {
            if let OnUnlock::ToState { state } = &mut l.on_unlock {
                if *state == old {
                    *state = new.clone();
                }
            }
        }
        if let Some(c) = &mut d.container {
            if c.open_state == old {
                c.open_state = new.clone();
            }
        }
        rename_state_in_triggers(&mut d.triggers, &old, &new);
        *cx.state_renamed = Some((d.id.clone(), old, new));
    }
}

fn all_names_except(d: &PropDef, i: usize) -> Vec<String> {
    d.state_names().into_iter().enumerate().filter(|(k, _)| *k != i).map(|(_, n)| n).collect()
}

/// Renames a prop state in "Set prop state" actions and "Prop state" conditions aimed at "self".
pub fn rename_state_in_triggers(ts: &mut [rust2d_engine::events::Trigger], old: &str, new: &str) {
    use rust2d_engine::events::{Action, Cond};
    fn actions(acts: &mut [Action], old: &str, new: &str) {
        for a in acts {
            match a {
                Action::SetState { target, state } if (target == "self" || target.is_empty()) && state == old => *state = new.to_string(),
                Action::If { conditions, then, otherwise } => {
                    conds(conditions, old, new);
                    actions(then, old, new);
                    actions(otherwise, old, new);
                }
                _ => {}
            }
        }
    }
    fn conds(cs: &mut [rust2d_engine::events::Condition], old: &str, new: &str) {
        for c in cs {
            if let Cond::PropState { target, state } = &mut c.cond {
                if (target == "self" || target.is_empty()) && state == old {
                    *state = new.to_string();
                }
            }
        }
    }
    for t in ts {
        conds(&mut t.conditions, old, new);
        actions(&mut t.actions, old, new);
    }
}

fn character_form(ui: &mut egui::Ui, d: &mut CharacterDef, cx: &mut Cx) {
    ui.heading("Character");
    rename_row(ui, RefKind::Character, &d.id, cx.rename);
    name_row(ui, &mut d.name);
    let mut sheets: Vec<&String> = cx.assets.sheets.keys().collect();
    sheets.sort();
    egui::Grid::new("chr").num_columns(2).show(ui, |ui| {
        ui.label("Sprite sheet");
        egui::ComboBox::from_id_salt("chr_sheet").selected_text(d.sheet.as_str()).show_ui(ui, |ui| {
            for s in sheets {
                ui.selectable_value(&mut d.sheet, s.clone(), s.as_str());
            }
        });
        ui.end_row();
        ui.label("Walk frames");
        ui.add(egui::DragValue::new(&mut d.walk_frames).range(1..=16));
        ui.end_row();
        ui.label("Speed (px/s)");
        ui.add(egui::DragValue::new(&mut d.speed).range(1.0..=500.0));
        ui.end_row();
        ui.label("Behavior");
        ui.horizontal(|ui| {
            let mut kind = match d.behavior {
                Behavior::Static => 0,
                Behavior::Wander { .. } => 1,
                Behavior::Chase { .. } => 2,
            };
            let old = kind;
            ui.radio_value(&mut kind, 0, "Static");
            ui.radio_value(&mut kind, 1, "Wander");
            ui.radio_value(&mut kind, 2, "Chase player");
            if kind != old {
                d.behavior = match kind {
                    0 => Behavior::Static,
                    1 => Behavior::Wander { radius: 3.0 },
                    _ => Behavior::Chase { sight: 4.0 },
                };
            }
            match &mut d.behavior {
                Behavior::Wander { radius } => {
                    ui.label("radius");
                    ui.add(egui::DragValue::new(radius).range(0.5..=64.0).speed(0.1));
                }
                Behavior::Chase { sight } => {
                    ui.label("sight");
                    ui.add(egui::DragValue::new(sight).range(0.5..=64.0).speed(0.1)).on_hover_text("Tiles; wanders near home when the player is farther away");
                }
                Behavior::Static => {}
            }
        });
        ui.end_row();
    });
    hitbox(ui, &mut d.hitbox);
    ui.separator();
    ui.heading("Combat");
    egui::Grid::new("chr_combat").num_columns(2).show(ui, |ui| {
        ui.label("Max HP");
        ui.add(egui::DragValue::new(&mut d.max_hp).range(0..=999)).on_hover_text("0 = can't be damaged");
        ui.end_row();
        ui.label("Damage");
        ui.add(egui::DragValue::new(&mut d.damage).range(0..=999));
        ui.end_row();
        ui.label("Defense");
        ui.add(egui::DragValue::new(&mut d.defense).range(-99..=99)).on_hover_text("Subtracted from incoming damage (at least 1 always gets through). Player: base defense before armor.");
        ui.end_row();
        ui.label("Hostile");
        ui.checkbox(&mut d.hostile, "hurts the player on contact");
        ui.end_row();
    });
    ui.weak("1 heart = 2 HP. For the player character, Damage is the bare-handed attack strength. Defeated characters fire 'Defeated' triggers.");
    ui.label("Ranged attack (hostile characters shoot at the player when in range and in sight):");
    weapon_ui(ui, "chr_weapon", &mut d.weapon, cx, true);
    ui.separator();
    ui.weak("Sheet layout: 4 rows (down, left, right, up) × walk frames; frame 0 of a row is the idle pose.");
    // Animated preview of all directions.
    let frames = d.walk_frames.max(1);
    let t = (macroquad::time::get_time() * 8.0) as u32 % frames;
    ui.horizontal(|ui| {
        for row in 0..4 {
            widgets::frame_thumb(ui, cx.tex, cx.assets, &d.sheet, row * frames + t, 64.0, false);
        }
    });
    ui.ctx().request_repaint();
    ui.separator();
    ui.heading("Default triggers");
    widgets::triggers_ui(ui, &format!("chr/{}", d.id), &mut d.triggers, cx.refs);
}

fn tileset_form(ui: &mut egui::Ui, d: &mut TilesetDef, cx: &mut Cx) {
    let tile = cx.assets.tile_size;
    ui.heading("Tileset");
    rename_row(ui, RefKind::Tileset, &d.id, cx.rename);
    ui.horizontal(|ui| {
        ui.label("Image");
        ui.text_edit_singleline(&mut d.image);
        if ui.button("✏ Edit pixels").on_hover_text("Open the image in the pixel editor").clicked() {
            *cx.paint = Some((d.image.clone(), (tile as u32, tile as u32), false, 0));
        }
        if ui.button("Browse…").clicked() {
            *cx.browse = Some(Purpose::TilesetImage(d.id.clone()));
        }
    });
    ui.weak(format!("Path relative to the project; files from elsewhere are copied into tilesets/. Tiles are {tile}px squares. After editing the path by hand, use ⟳ Reload assets."));
    let (Some(t), Some(ts)) = (cx.tex.tilesets.get(&d.id), cx.assets.tilesets.get_mut(&d.id)) else {
        ui.colored_label(Color32::LIGHT_RED, "Not loaded — reload assets.");
        return;
    };

    // ---- animations
    ui.separator();
    ui.heading("Animated tiles");
    ui.weak("Wherever the base tile is placed it cycles through the frames. Pick an animation below, then click tiles in the image to append frames.");
    let mut remove = None;
    let uv_of = |i: u32| {
        let (c, r) = (i % ts.columns, i / ts.columns);
        let (w, h) = (ts.image.width as f32, ts.image.height as f32);
        egui::Rect::from_min_max(pos2(c as f32 * tile / w, r as f32 * tile / h), pos2((c + 1) as f32 * tile / w, (r + 1) as f32 * tile / h))
    };
    let now = macroquad::time::get_time();
    for (k, a) in d.animations.iter_mut().enumerate() {
        ui.horizontal(|ui| {
            ui.radio_value(cx.anim_target, Some(k), "");
            let (rect, _) = ui.allocate_exact_size(vec2(32.0, 32.0), Sense::hover());
            ui.painter().image(t.id(), rect, uv_of(a.frame_at(now)), Color32::WHITE);
            ui.label("tile");
            ui.add(egui::DragValue::new(&mut a.tile).range(0..=ts.count.saturating_sub(1)));
            ui.label("frames");
            let mut text = a.frames.iter().map(|f| f.to_string()).collect::<Vec<_>>().join(", ");
            if ui.add(egui::TextEdit::singleline(&mut text).desired_width(140.0)).changed() {
                a.frames = text.split(',').filter_map(|f| f.trim().parse().ok()).filter(|&f| f < ts.count).collect();
            }
            ui.label("fps");
            ui.add(egui::DragValue::new(&mut a.fps).range(0.1..=60.0).speed(0.1));
            if ui.small_button("✖").clicked() {
                remove = Some(k);
            }
        });
    }
    if let Some(k) = remove {
        d.animations.remove(k);
        *cx.anim_target = None;
    }
    ui.horizontal(|ui| {
        if ui.button("+ Animation").clicked() {
            d.animations.push(TileAnim { tile: 0, frames: vec![], fps: 4.0 });
            *cx.anim_target = Some(d.animations.len() - 1);
        }
        if cx.anim_target.is_some() && ui.button("Done (back to solidity)").clicked() {
            *cx.anim_target = None;
        }
    });
    ui.ctx().request_repaint();

    // ---- tile grid
    ui.separator();
    match *cx.anim_target {
        Some(k) => ui.label(format!("Click tiles to append frames to animation #{} (the first click also sets its base tile).", k + 1)),
        None => ui.label("Click tiles to toggle solidity (red = blocks movement)."),
    };
    let scale = 3.0;
    let size = vec2(ts.image.width as f32 * scale, ts.image.height as f32 * scale);
    let cell = tile * scale;
    egui::ScrollArea::both().id_salt("ts_solid").show(ui, |ui| {
        let (rect, resp) = ui.allocate_exact_size(size, Sense::click());
        let p = ui.painter_at(rect);
        p.rect_filled(rect, 0.0, Color32::from_gray(30));
        p.image(t.id(), rect, egui::Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
        let animated: Vec<u32> = d.animations.iter().map(|a| a.tile).collect();
        for i in 0..ts.count {
            let r = egui::Rect::from_min_size(rect.min + vec2((i % ts.columns) as f32 * cell, (i / ts.columns) as f32 * cell), vec2(cell, cell));
            p.rect_stroke(r, 0.0, Stroke::new(1.0_f32, Color32::from_white_alpha(20)), StrokeKind::Inside);
            if d.solid.contains(&i) {
                p.rect_filled(r, 0.0, Color32::from_rgba_unmultiplied(255, 0, 0, 70));
            }
            if animated.contains(&i) {
                p.rect_stroke(r, 0.0, Stroke::new(2.0_f32, Color32::LIGHT_BLUE), StrokeKind::Inside);
            }
            p.text(r.left_top() + vec2(2.0, 1.0), egui::Align2::LEFT_TOP, i.to_string(), egui::FontId::monospace(9.0), Color32::from_white_alpha(120));
        }
        if resp.clicked() {
            if let Some(pos) = resp.interact_pointer_pos() {
                let rel = pos - rect.min;
                let i = (rel.y / cell) as u32 * ts.columns + (rel.x / cell) as u32;
                if i < ts.count {
                    match cx.anim_target.and_then(|k| d.animations.get_mut(k)) {
                        Some(a) => {
                            if a.frames.is_empty() {
                                a.tile = i;
                            }
                            a.frames.push(i);
                        }
                        None => {
                            if let Some(k) = d.solid.iter().position(|&s| s == i) {
                                d.solid.remove(k);
                            } else {
                                d.solid.push(i);
                                d.solid.sort();
                            }
                            // Keep runtime flags in sync without a full reload.
                            if let Some(v) = ts.solid.get_mut(i as usize) {
                                *v = d.solid.contains(&i);
                            }
                        }
                    }
                }
            }
        }
    });
    if ts.anims != d.animations {
        ts.set_animations(&d.animations);
    }
}

fn sheet_form(ui: &mut egui::Ui, d: &mut SheetDef, cx: &mut Cx) {
    ui.heading("Sprite sheet");
    rename_row(ui, RefKind::Sheet, &d.id, cx.rename);
    egui::Grid::new("sheet").num_columns(2).show(ui, |ui| {
        ui.label("Image");
        ui.horizontal(|ui| {
            ui.text_edit_singleline(&mut d.image);
            if ui.button("✏ Edit pixels").on_hover_text("Open the image in the pixel editor").clicked() {
                let character = cx.db.characters.iter().any(|c| c.sheet == d.id);
                *cx.paint = Some((d.image.clone(), (d.frame_w, d.frame_h), character, 0));
            }
            if ui.button("Browse…").clicked() {
                *cx.browse = Some(Purpose::SheetImage(d.id.clone()));
            }
        });
        ui.end_row();
        ui.label("Frame size");
        ui.horizontal(|ui| {
            ui.add(egui::DragValue::new(&mut d.frame_w).range(1..=512));
            ui.add(egui::DragValue::new(&mut d.frame_h).range(1..=512));
        });
        ui.end_row();
    });
    ui.weak("After changing the image or frame size, use ⟳ Reload assets.");
    ui.separator();
    let mut f = u32::MAX;
    widgets::frame_grid(ui, "sheet_prev", &d.id, &mut f, cx.tex, cx.assets);
}

fn sound_form(ui: &mut egui::Ui, d: &mut SoundDef, cx: &mut Cx) {
    ui.heading("Sound");
    rename_row(ui, RefKind::Sound, &d.id, cx.rename);
    egui::Grid::new("sound").num_columns(2).show(ui, |ui| {
        ui.label("File");
        ui.horizontal(|ui| {
            ui.text_edit_singleline(&mut d.file);
            if ui.button("Browse…").clicked() {
                *cx.browse = Some(Purpose::SoundFile(d.id.clone()));
            }
        });
        ui.end_row();
        ui.label("Volume");
        ui.add(egui::Slider::new(&mut d.volume, 0.0..=1.0));
        ui.end_row();
    });
    ui.weak("WAV or OGG, path relative to the project. Use ⟳ Reload assets after changing the file or volume.");
    ui.horizontal(|ui| {
        if cx.audio.has(&d.id) {
            if ui.button("▶ Play").clicked() {
                cx.audio.play(&d.id);
            }
        } else {
            ui.colored_label(Color32::LIGHT_RED, "Not loaded — reload assets (see terminal for errors).");
        }
    });
    ui.separator();
    ui.weak("Use sounds with the 'Play sound' / 'Play music' actions, as map music (map properties), or for built-in game events (Project tab).");
}

fn project_ui(ed: &mut Editor, ui: &mut egui::Ui) {
    let c = &mut ed.project.config;
    ui.heading("Project");
    egui::Grid::new("proj").num_columns(2).show(ui, |ui| {
        ui.label("Name");
        ui.text_edit_singleline(&mut c.name);
        ui.end_row();
        ui.label("Tile size");
        ui.weak(format!("{} px (fixed)", rust2d_engine::project::TILE_SIZE));
        ui.end_row();
        ui.label("Start map");
        widgets::suggest(ui, "start_map", &mut c.start_map, &ed.refs.maps, 140.0);
        ui.end_row();
        ui.label("Start spawn");
        let spawns = ed.refs.spawns.get(&c.start_map).cloned().unwrap_or_default();
        widgets::suggest(ui, "start_spawn", &mut c.start_spawn, &spawns, 140.0);
        ui.end_row();
        ui.label("Pixel scale");
        ui.add(egui::DragValue::new(&mut c.scale).range(1.0..=8.0).speed(0.1));
        ui.end_row();
        ui.label("Inventory slots");
        ui.add(egui::DragValue::new(&mut c.inventory_size).range(1..=200));
        ui.end_row();
        ui.label("Player character");
        let ids: Vec<String> = ed.project.db.characters.iter().map(|d| d.id.clone()).collect();
        widgets::suggest(ui, "player", &mut c.player, &ids, 140.0);
        ui.end_row();
    });
    ui.weak("Tile size changes need a restart of the editor.");
    ui.separator();
    ui.heading("Game sounds");
    ui.weak("Played by built-in events. Leave empty for silence.");
    egui::Grid::new("sfx").num_columns(2).show(ui, |ui| {
        for (label, v) in c.sfx.fields_mut() {
            ui.label(label);
            widgets::suggest(ui, &format!("sfx_{label}"), v, &ed.refs.sounds, 140.0);
            ui.end_row();
        }
    });
    ui.separator();
    ui.label(format!("Project folder: {}", ed.project.root.display()));
}

fn scripts_ui(ed: &mut Editor, ui: &mut egui::Ui) {
    let dir = ed.project.scripts_dir();
    let mut files: Vec<_> = std::fs::read_dir(&dir)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e == "rhai"))
        .collect();
    files.sort();
    let st = &mut ed.db;
    let mut save = false;
    egui::SidePanel::left("scripts_left").default_width(200.0).show_inside(ui, |ui| {
        for f in &files {
            let name = f.file_name().unwrap().to_string_lossy();
            if ui.selectable_label(st.script_file.as_ref() == Some(f), name).clicked() && st.script_file.as_ref() != Some(f) {
                st.script_file = Some(f.clone());
                st.script_text = std::fs::read_to_string(f).unwrap_or_default();
                st.script_dirty = false;
            }
        }
        ui.separator();
        ui.horizontal(|ui| {
            ui.add(egui::TextEdit::singleline(&mut st.new_script).hint_text("new_file").desired_width(110.0));
            if ui.button("+ New").clicked() && !st.new_script.is_empty() {
                let p = dir.join(format!("{}.rhai", st.new_script.trim_end_matches(".rhai")));
                let _ = std::fs::create_dir_all(&dir);
                if !p.exists() {
                    let _ = std::fs::write(&p, "// New script\n\nfn my_function() {\n    say(\"Hello!\");\n}\n");
                }
                st.script_text = std::fs::read_to_string(&p).unwrap_or_default();
                st.script_file = Some(p);
                st.new_script.clear();
            }
        });
        ui.separator();
        ui.label("Functions:");
        for f in &ed.refs.functions {
            ui.monospace(f);
        }
    });
    egui::CentralPanel::default().show_inside(ui, |ui| {
        let Some(f) = st.script_file.clone() else {
            ui.label("Select a script file. Functions can be called from triggers with the 'Run script' action, or used as 'Script' conditions (returning bool).");
            return;
        };
        ui.horizontal(|ui| {
            ui.strong(f.file_name().unwrap().to_string_lossy());
            if st.script_dirty {
                ui.colored_label(Color32::YELLOW, "•");
            }
            save = ui.button("💾 Save & check").clicked();
        });
        let resp = egui::ScrollArea::vertical().id_salt("script_edit").show(ui, |ui| {
            ui.add(
                egui::TextEdit::multiline(&mut st.script_text)
                    .code_editor()
                    .desired_width(f32::INFINITY)
                    .desired_rows(30),
            )
        });
        if resp.inner.changed() {
            st.script_dirty = true;
        }
        if ui.input(|i| i.modifiers.ctrl && i.key_pressed(egui::Key::S)) {
            save = true;
        }
    });
    if save {
        if let Some(f) = ed.db.script_file.clone() {
            match std::fs::write(&f, &ed.db.script_text) {
                Ok(()) => {
                    ed.db.script_dirty = false;
                    let mut host = rust2d_engine::script::ScriptHost::new();
                    host.load_dir(&dir);
                    ed.refresh_refs();
                    if host.errors.is_empty() {
                        ed.set_status("Script saved, no errors");
                    } else {
                        ed.set_status(format!("Script error: {}", host.errors.join("; ")));
                    }
                }
                Err(e) => ed.set_status(format!("Saving script failed: {e}")),
            }
        }
    }
}
