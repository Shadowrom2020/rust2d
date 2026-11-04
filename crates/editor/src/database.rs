//! Definition editors: items, props, characters, tilesets, sprite sheets, sounds, project and scripts.

use egui_macroquad::egui::{self, pos2, vec2, Color32, Sense, Stroke, StrokeKind};

use crate::ui::{self, ph};
use rust2d_engine::audio::Audio;
use rust2d_engine::defs::*;
use rust2d_engine::events::TriggerOn;
use rust2d_engine::imageio::Rgba;
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
    Quests,
    Project,
    Scripts,
    Structures,
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
    /// The open script as on disk; `script_dirty` is only set while the text differs from it.
    pub script_saved: String,
    pub script_dirty: bool,
    pub new_script: String,
    /// Character index to move the script editor's cursor to (and scroll to) next frame.
    pub script_jump: Option<usize>,
    /// Script editor (completion popup, pending cursor moves).
    pub script_ed: crate::script_editor::EditorState,
    pub rename: RenameUi,
    /// Tileset editor: animation that clicked tiles are appended to (None = toggle solidity).
    pub anim_target: Option<usize>,
    /// Prop editor: selected state (its sprite is edited below the list) and an open rename.
    pub state_sel: usize,
    pub state_rename: Option<(usize, String)>,
    /// Items / props / characters: the pixel editor is shown under the form.
    pub art_open: bool,
    /// What the embedded editor last focused (to select its frame once).
    pub art_focus: Option<(String, u32)>,
    /// Tilesets tab: clicks on the tile grid toggle light tiles.
    pub tile_light_pick: bool,
    /// The structure editor (Structures tab).
    pub st_edit: crate::structures::StructEdit,
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
    art_open: &'a mut bool,
    /// Art actions handled after the form (they need the whole editor).
    art: &'a mut Option<ArtAction>,
    st_edit: &'a mut crate::structures::StructEdit,
    tile_light_pick: &'a mut bool,
}

/// An id not yet used in `ids`, based on `prefix`.
fn unique(prefix: &str, ids: &[String]) -> String {
    (1..).map(|n| format!("{prefix}_{n}")).find(|c| !ids.contains(c)).unwrap()
}

thread_local! {
    /// The Database view belongs to the Asset Library window (set each frame by `ui`):
    /// deleting goes through the library's remove confirmation instead of `list_ui`.
    static LIBRARY_MODE: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// Icon of a Database tab.
fn tab_icon(t: Tab) -> &'static str {
    match t {
        Tab::Items => ph::SWORD,
        Tab::Props => ph::TREASURE_CHEST,
        Tab::Characters => ph::USERS,
        Tab::Tilesets => ph::GRID_FOUR,
        Tab::Sheets => ph::IMAGE,
        Tab::Sounds => ph::SPEAKER_HIGH,
        Tab::Structures => ph::HOUSE,
        Tab::Quests => ph::SCROLL,
        Tab::Project => ph::GEAR,
        Tab::Scripts => ph::CODE,
    }
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
        if ui.button(ui::t(ph::PLUS, "New")).on_hover_text("A new definition with default values").clicked() {
            items.push(make(&ids));
            *sel = items.len() - 1;
            changed = true;
        }
        if ui.add_enabled(*sel < items.len(), egui::Button::new(ui::t(ph::COPY, "Duplicate"))).on_hover_text("A copy of the selected definition with a new id").clicked() {
            let mut c = items[*sel].clone();
            let id = id_of(&mut c);
            *id = unique(id, &ids);
            ids.push(id.clone());
            items.push(c);
            *sel = items.len() - 1;
            changed = true;
        }
        let library = LIBRARY_MODE.with(|m| m.get());
        if !library && ui.add_enabled(*sel < items.len(), egui::Button::new(ui::t(ph::TRASH, "Delete"))).on_hover_text("Removes the definition (references in maps are not removed)").clicked() {
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
    let library = ed.library_mode;
    LIBRARY_MODE.with(|m| m.set(library));
    if library && matches!(ed.db.tab, Tab::Quests | Tab::Project | Tab::Scripts) {
        ed.db.tab = Tab::Items;
    }
    ui.horizontal(|ui| {
        let tabs = [
            (Tab::Items, "Items"),
            (Tab::Props, "Props"),
            (Tab::Characters, "Characters"),
            (Tab::Tilesets, "Tilesets"),
            (Tab::Sheets, "Sprite sheets"),
            (Tab::Sounds, "Sounds"),
            (Tab::Structures, "Structures"),
            (Tab::Quests, "Quests"),
            (Tab::Project, "Project"),
            (Tab::Scripts, "Scripts"),
        ];
        for (t, l) in tabs {
            if library && matches!(t, Tab::Quests | Tab::Project | Tab::Scripts) {
                continue; // the library only holds assets
            }
            if ui.selectable_label(ed.db.tab == t, ui::t(tab_icon(t), l)).clicked() && ed.db.tab != t {
                ed.db.tab = t;
                ed.db.sel = 0;
                ed.db.rename = RenameUi::default();
                ed.db.anim_target = None;
            }
        }
        ui.separator();
        if ui.button(ui::t(ph::FLOPPY_DISK, "Save definitions")).on_hover_text("Writes defs/*.ron (Ctrl+S saves everything)").clicked() {
            ed.save_defs();
        }
        if ui.button(ui::t(ph::ARROW_CLOCKWISE, "Reload assets")).on_hover_text("Re-reads images, sounds and scripts from disk").clicked() {
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
            Tab::Structures => db.structures.get(i).map(|d| (RefKind::Structure, d.id.clone())),
            _ => None,
        };
        if !library {
            let kind = match ed.db.tab {
                Tab::Items => Some(RefKind::Item),
                Tab::Props => Some(RefKind::Prop),
                Tab::Characters => Some(RefKind::Character),
                Tab::Tilesets => Some(RefKind::Tileset),
                Tab::Sheets => Some(RefKind::Sheet),
                Tab::Sounds => Some(RefKind::Sound),
                Tab::Structures => Some(RefKind::Structure),
                _ => None,
            };
            if kind.is_some() {
                ed.add_from_library_button(ui, kind);
            }
        }
        if let Some(a) = current {
            if library {
                if ui.button(ui::t(ph::TRASH, "Delete…")).on_hover_text(format!("Removes '{}' from the library (asks first)", a.1)).clicked() {
                    ed.lib.confirm_remove = Some(vec![a.clone()]);
                }
            } else if ui.button(ui::t(ph::UPLOAD_SIMPLE, "Add to library")).on_hover_text(format!("Copies '{}' and everything it needs into the shared asset library", a.1)).clicked() {
                let r = ed.project_to_library(std::slice::from_ref(&a));
                ed.show_result(r);
            }
            if ui.button(ui::t(ph::PACKAGE, "Export…")).on_hover_text(format!("Saves '{}' and everything it needs as a .r2dpack file", a.1)).clicked() {
                ed.open_file_dialog(Purpose::ExportPackage { from_library: false, assets: vec![a] });
            }
        }
        if ui.button(ui::t(ph::SPARKLE, "New asset…")).on_hover_text("A prop, item, character, tileset, sprite sheet or structure (a wizard asks the details)").clicked() {
            ed.wizard = Some(crate::wizard::Wizard::new(&ed.project));
        }
    });
    ui.separator();

    let before = (ed.project.db.clone(), ed.project.config.clone());
    let mut paint: Option<(String, (u32, u32), bool, u32)> = None;
    let mut state_renamed: Option<(String, String, String)> = None;
    let mut art: Option<ArtAction> = None;
    let mut art_target: Option<(String, (u32, u32), bool, u32)> = None;
    let mut list_changed = false;
    let mut place_structure: Option<String> = None;
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
                        value: 0,
        light: None,
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
        light: None,
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
                        shop: None,
                        loot: vec![],
        light: None,
                    },
                    |d| &mut d.id,
                ),
                Tab::Tilesets => list_ui(
                    ui,
                    &mut db.tilesets,
                    sel,
                    |d| d.id.clone(),
                    |ids| TilesetDef { id: unique("tileset", ids), image: "tilesets/new.png".into(), solid: vec![], animations: vec![], lights: vec![] },
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
                Tab::Structures => list_ui(
                    ui,
                    &mut db.structures,
                    sel,
                    |d| format!("{}  ({})", d.name, d.id),
                    |ids| StructureDef { id: unique("structure", ids), name: "New structure".into(), w: 6, h: 5, ..Default::default() },
                    |d| &mut d.id,
                ),
                Tab::Quests => list_ui(
                    ui,
                    &mut db.quests,
                    sel,
                    |d| format!("{}  ({})", d.title, d.id),
                    |ids| rust2d_engine::quest::QuestDef { id: unique("quest", ids), title: "New quest".into(), description: String::new(), stages: vec!["First objective".into()] },
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
        let mut embed: Option<(String, (u32, u32), bool)> = None;
        egui::CentralPanel::default().show_inside(ui, |ui| {
            // Sprite sheets: a compact form on top, the pixel editor below it.
            let art_tab = matches!(tab, Tab::Items | Tab::Props | Tab::Characters) && ed.db.art_open;
            let form_height = if tab == Tab::Sheets {
                210.0
            } else if art_tab {
                (ui.available_height() * 0.45).max(220.0)
            } else {
                f32::INFINITY
            };
            egui::ScrollArea::vertical().id_salt("db_form").max_height(form_height).show(ui, |ui| {
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
                    art_open: &mut ed.db.art_open,
                    art: &mut art,
                    st_edit: &mut ed.db.st_edit,
                    tile_light_pick: &mut ed.db.tile_light_pick,
                };
                let i = ed.db.sel;
                let db = &mut ed.project.db;
                match tab {
                    Tab::Items => {
                        if let Some(d) = db.items.get_mut(i) {
                            item_form(ui, d, &mut cx);
                            art_target = art_of(&db_snapshot, &d.icon, false);
                        }
                    }
                    Tab::Props => {
                        if let Some(d) = db.props.get_mut(i) {
                            prop_form(ui, d, &mut cx);
                            let k = (*cx.state_sel).min(d.states.len());
                            art_target = art_of(&db_snapshot, d.state_look(k).0, false);
                        }
                    }
                    Tab::Characters => {
                        if let Some(d) = db.characters.get_mut(i) {
                            character_form(ui, d, &mut cx);
                            art_target = art_of(&db_snapshot, &SpriteRef::new(&d.sheet, 0), true);
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
                            let character = db_snapshot.characters.iter().any(|c| c.sheet == d.id);
                            embed = Some((d.image.clone(), (d.frame_w, d.frame_h), character));
                        }
                    }
                    Tab::Sounds => {
                        if let Some(d) = db.sounds.get_mut(i) {
                            sound_form(ui, d, &mut cx);
                        }
                    }
                    Tab::Quests => {
                        if let Some(d) = db.quests.get_mut(i) {
                            quest_form(ui, d, &mut cx);
                        }
                    }
                    Tab::Structures => {
                        if let Some(d) = db.structures.get_mut(i) {
                            if structure_form(ui, d, &mut cx) {
                                place_structure = Some(d.id.clone());
                            }
                        }
                    }
                    _ => {}
                }
            });
            if let Some((rel, frame, character)) = embed.take() {
                ui.separator();
                let h = ui.available_height().max(260.0);
                let layout = if character { crate::pixel::Layout::Character } else { crate::pixel::Layout::Plain };
                ed.pixel_embedded(ui, &rel, frame, layout, h);
            }
            // Items / props / characters: their art under the form.
            if art_tab {
                ui.separator();
                match art_target.clone() {
                    Some((rel, frame, character, sel)) => {
                        let h = ui.available_height().max(260.0);
                        let layout = if character { crate::pixel::Layout::Character } else { crate::pixel::Layout::Plain };
                        ed.pixel_embedded(ui, &rel, frame, layout, h);
                        // Select the definition's frame when the focus changes.
                        let focus = Some((rel.clone(), sel));
                        if ed.db.art_focus != focus && ed.px.doc.as_ref().is_some_and(|d| d.rel == rel) {
                            ed.px.sel_frame = sel;
                            ed.db.art_focus = focus;
                        }
                    }
                    None => {
                        ui.weak("No sprite sheet chosen yet: choose a sprite, or create a blank one with the buttons above.");
                    }
                }
            }
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
    if let Some(a) = art.take() {
        ed.art_action(a, tab);
    }
    if let Some(id) = place_structure {
        ed.mode = crate::app::Mode::Map;
        ed.arm_structure(&id);
    }
    if list_changed || before.0 != ed.project.db || before.1 != ed.project.config {
        ed.defs_dirty = true;
        if before.0.quests != ed.project.db.quests {
            ed.refs.quests = ed.project.db.quests.iter().map(|q| q.id.clone()).collect();
            ed.refs.quest_stages = ed.project.db.quests.iter().map(|q| (q.id.clone(), q.stages.clone())).collect();
        }
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
        if !character {
            ui.label("Hands");
            ui.checkbox(&mut w.two_handed, "Two-handed").on_hover_text("Nothing can be held in the left hand (shield, lantern) while this weapon is equipped");
            ui.end_row();
        }
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
        ui.label("Value");
        ui.add(egui::DragValue::new(&mut d.value).range(0..=999_999))
            .on_hover_text("Base trade price; merchants use it when their stock has no own price (0 = worthless, can't be sold)");
    });
    ui.separator();
    ui.heading("Equipment");
    ui.weak("Equipment goes into a character-sheet slot when used from the inventory (weapons also with Q) instead of running 'On use' triggers.");
    let mut kind = match (&d.weapon, &d.armor) {
        (Some(_), _) => 1,
        (None, Some(a)) if a.slot == EquipSlot::Head => 2,
        (None, Some(a)) if a.slot == EquipSlot::Offhand => 4,
        (None, Some(_)) => 3,
        _ => 0,
    };
    let old = kind;
    ui.horizontal_wrapped(|ui| {
        ui.radio_value(&mut kind, 0, "None");
        ui.radio_value(&mut kind, 1, "Weapon");
        ui.radio_value(&mut kind, 2, "Helmet");
        ui.radio_value(&mut kind, 3, "Body armor");
        ui.radio_value(&mut kind, 4, "Left hand").on_hover_text("A shield (defense) or a light (lantern, torch: set its light below). Not usable together with a two-handed weapon.");
    });
    if kind != old {
        d.weapon = (kind == 1).then(|| Weapon::melee(1, 0.4));
        d.armor = match kind {
            2..=4 => {
                let slot = match kind {
                    2 => EquipSlot::Head,
                    3 => EquipSlot::Body,
                    _ => EquipSlot::Offhand,
                };
                Some(d.armor.take().map(|a| Armor { slot, ..a }).unwrap_or(Armor { slot, defense: 1, speed: 0.0, overlay: String::new() }))
            }
            _ => None,
        };
        if kind != 0 && kind != 4 {
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
    ui.heading("Light");
    widgets::light_ui(ui, "item_light", &mut d.light, "Gives light (while equipped, e.g. in the left hand, or lying on the ground)");
    ui.separator();
    ui.heading("Icon");
    widgets::sprite_field(ui, "item_icon", &mut d.icon, cx.tex, cx.assets, cx.db, false);
    art_buttons(ui, cx, "icon", (1, 1));
    ui.separator();
    ui.heading("On use");
    ui.weak("Runs when the item is used from the inventory and the entity in front of the player didn't react to it.");
    widgets::triggers_ui(ui, &format!("item/{}", d.id), &mut d.on_use, cx.refs, rust2d_engine::events::TriggerOwner::ItemUse);
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
    ui.heading("Light");
    ui.weak("Light per state, e.g. a lamp that is lit in its \"lit\" state and dark in \"off\". Placed props can have their own light (map inspector).");
    let names = d.state_names();
    for (i, name) in names.iter().enumerate() {
        let slot = if i == 0 { &mut d.light } else { &mut d.states[i - 1].light };
        ui.push_id(("prop_light", i), |ui| {
            widgets::light_ui(ui, "prop_light", slot, &format!("'{name}' gives light"));
        });
    }
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
    widgets::triggers_ui(ui, &format!("prop/{}", d.id), &mut d.triggers, cx.refs, rust2d_engine::events::TriggerOwner::Prop);
}

/// The prop's states in one place: name, sprite, whether it blocks movement.
fn prop_states_ui(ui: &mut egui::Ui, d: &mut PropDef, cx: &mut Cx) {
    ui.heading("States");
    ui.weak("A prop always shows one of its states. Placed props start in the default state (or the one chosen in the map inspector); a lock, a container, the 'Set prop state' action or set_state(target, \"name\") in scripts switch to another.");
    // All states as one list (index 0 = the default state).
    let mut all: Vec<PropState> = std::iter::once(PropState { name: d.default_state.clone(), sprite: d.sprite.clone(), solid: d.solid, light: None }).chain(d.states.iter().cloned()).collect();
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
                                if ui.small_button("\u{E3B4}").on_hover_text("Rename (placed props using it follow)").clicked() {
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
                            if i > 1 && ui.small_button("\u{E13C}").clicked() {
                                op = Some(Op::Up(i));
                            }
                            if i + 1 < n && ui.small_button("\u{E136}").clicked() {
                                op = Some(Op::Down(i));
                            }
                            let in_use = used.contains(&st.name);
                            if ui.add_enabled(!in_use, egui::Button::new("\u{E4F6}").small()).on_hover_text(if in_use { "Used by the lock or container" } else { "Delete this state" }).clicked() {
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
            all.push(PropState { name: name.clone(), sprite, solid: all[0].solid, light: None });
            *cx.state_sel = all.len() - 1;
            *cx.state_rename = Some((all.len() - 1, name));
        }
        ui.weak("e.g. closed / open / destroyed, dormant / glowing, on / off");
    });
    // The selected state's sprite.
    let sel = (*cx.state_sel).min(all.len() - 1);
    ui.add_space(4.0);
    ui.label(egui::RichText::new(format!("Sprite of '{}'", all[sel].name)).strong());
    widgets::sprite_field(ui, &format!("prop_state_{sel}"), &mut all[sel].sprite, cx.tex, cx.assets, cx.db, true);
    art_buttons(ui, cx, "frame for this state", all[sel].sprite.cells());
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
        ui.horizontal(|ui| {
            egui::ComboBox::from_id_salt("chr_sheet").selected_text(d.sheet.as_str()).show_ui(ui, |ui| {
                for s in &sheets {
                    ui.selectable_value(&mut d.sheet, s.to_string(), s.as_str());
                }
            });
            if ui.button("\u{E4C2} Choose model…").on_hover_text("Pick a look from this project or the asset library (library models are copied into the project)").clicked() {
                crate::models::request_model(crate::models::ModelTarget::Def(d.id.clone()));
            }
            let mut on = *cx.art_open;
            if ui.toggle_value(&mut on, "\u{E6C8} Edit art").on_hover_text("Show the pixel editor for this character's sheet").changed() {
                *cx.art_open = on;
            }
            ui.menu_button("+ New sheet", |ui| {
                ui.label("A sheet for this character: 4 walk frames per row, rows Down / Left / Right / Up.");
                if ui.button("Blank (32×48 frames)").clicked() {
                    *cx.art = Some(ArtAction::CharacterSheet { template: None });
                    ui.close_menu();
                }
                ui.separator();
                ui.label("Start from a copy of:");
                for s in &sheets {
                    let used = cx.db.characters.iter().any(|c| &c.sheet == *s);
                    if used && ui.button(s.as_str()).clicked() {
                        *cx.art = Some(ArtAction::CharacterSheet { template: Some(s.to_string()) });
                        ui.close_menu();
                    }
                }
            });
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
    ui.heading("Loot");
    if d.max_hp <= 0 {
        ui.weak("This character can't be defeated (Max HP 0), so its loot never drops.");
    } else {
        ui.weak("What the character can drop when defeated. Placed characters and spawners can use their own table instead.");
    }
    widgets::loot_ui(ui, "chr_loot", &mut d.loot, cx.refs, cx.tex, cx.assets, cx.db);
    ui.separator();
    ui.heading("Light");
    widgets::light_ui(ui, "chr_light", &mut d.light, "Carries a light (e.g. a lantern)");
    ui.weak("Placed characters can carry their own light instead (map inspector).");
    ui.separator();
    ui.heading("Merchant");
    let mut on = d.shop.is_some();
    if ui.checkbox(&mut on, "Merchant (trades items with the player)").changed() {
        d.shop = on.then(ShopSpec::default);
    }
    if let Some(spec) = &mut d.shop {
        ui.weak("Default stock for every placed instance; instances can have their own stock and prices in the map inspector.");
        widgets::shop_ui(ui, "chr_shop", spec, cx.refs, cx.tex, cx.assets, cx.db);
    }
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
    widgets::triggers_ui(ui, &format!("chr/{}", d.id), &mut d.triggers, cx.refs, rust2d_engine::events::TriggerOwner::Character);
}

fn quest_form(ui: &mut egui::Ui, d: &mut rust2d_engine::quest::QuestDef, _cx: &mut Cx) {
    ui.heading("Quest");
    egui::Grid::new("quest").num_columns(2).show(ui, |ui| {
        ui.label("Id");
        ui.add(egui::TextEdit::singleline(&mut d.id).desired_width(200.0))
            .on_hover_text("Used by 'Quest' actions / conditions and scripts (start_quest(\"id\")). Changing it does not update them.");
        ui.end_row();
        ui.label("Title");
        ui.add(egui::TextEdit::singleline(&mut d.title).desired_width(320.0));
        ui.end_row();
    });
    ui.label("Description (shown in the journal)");
    ui.add(egui::TextEdit::multiline(&mut d.description).desired_rows(3).desired_width(f32::INFINITY));
    ui.separator();
    ui.heading("Stages");
    ui.weak("One objective per stage. The journal shows the objectives reached so far: earlier ones ticked, the current one open. \
             Triggers move the quest on with the Quest action (start, advance to the next stage, set stage, complete, fail) \
             and check it with the Quest condition; scripts use start_quest / advance_quest / set_quest_stage / complete_quest / fail_quest \
             and quest_stage / quest_active / quest_done. Write {name} to show the value of variable 'name', e.g. \"Defeat slimes ({slimes}/5)\".");
    let n = d.stages.len();
    let mut swap = None;
    let mut remove = None;
    for (k, s) in d.stages.iter_mut().enumerate() {
        ui.horizontal(|ui| {
            ui.label(format!("{k}"));
            ui.add(egui::TextEdit::multiline(s).desired_rows(1).desired_width(460.0));
            if ui.add_enabled(k > 0, egui::Button::new("^").small()).on_hover_text("Move up").clicked() {
                swap = Some(k - 1);
            }
            if ui.add_enabled(k + 1 < n, egui::Button::new("v").small()).on_hover_text("Move down").clicked() {
                swap = Some(k);
            }
            if ui.small_button("\u{E4F6}").on_hover_text("Remove (later stages move up: check your triggers)").clicked() {
                remove = Some(k);
            }
        });
    }
    if let Some(k) = swap {
        d.stages.swap(k, k + 1);
    }
    if let Some(k) = remove {
        d.stages.remove(k);
    }
    if ui.button("+ Stage").clicked() {
        d.stages.push(String::new());
    }
}

fn tileset_form(ui: &mut egui::Ui, d: &mut TilesetDef, cx: &mut Cx) {
    let tile = cx.assets.tile_size;
    ui.heading("Tileset");
    rename_row(ui, RefKind::Tileset, &d.id, cx.rename);
    ui.horizontal(|ui| {
        ui.label("Image");
        ui.text_edit_singleline(&mut d.image);
        if ui.button("\u{E3B4} Edit pixels").on_hover_text("Open the image in the pixel editor").clicked() {
            *cx.paint = Some((d.image.clone(), (tile as u32, tile as u32), false, 0));
        }
        if ui.button("Browse…").clicked() {
            *cx.browse = Some(Purpose::TilesetImage(d.id.clone()));
        }
    });
    ui.weak(format!("Path relative to the project; files from elsewhere are copied into tilesets/. Tiles are {tile}px squares. After editing the path by hand, use \u{E036} Reload assets."));
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
            if ui.small_button("\u{E4F6}").clicked() {
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

    // ---- light tiles
    ui.separator();
    ui.heading("Light tiles");
    ui.weak("Tiles that give light wherever they are placed (wall torches, lamps, glowing windows). Lights only show on maps that are dark (Map › Lighting).");
    let mut remove = None;
    for (k, tl) in d.lights.iter_mut().enumerate() {
        ui.push_id(("tile_light", k), |ui| {
            ui.horizontal(|ui| {
                let (rect, _) = ui.allocate_exact_size(vec2(32.0, 32.0), Sense::hover());
                ui.painter().image(t.id(), rect, uv_of(tl.tile), Color32::WHITE);
                ui.label(format!("tile {}", tl.tile));
                if ui.small_button(ui::ph::TRASH).on_hover_text("No light for this tile").clicked() {
                    remove = Some(k);
                }
            });
            let mut l = Some(tl.light.clone());
            widgets::light_ui(ui, "tile_light", &mut l, "gives light");
            match l {
                Some(l) => tl.light = l,
                None => remove = Some(k),
            }
        });
    }
    if let Some(k) = remove {
        d.lights.remove(k);
    }
    if ui.selectable_label(*cx.tile_light_pick, ui::t(ui::ph::LIGHTBULB, "Pick light tiles in the grid")).on_hover_text("While on, clicking a tile below adds or removes its light").clicked() {
        *cx.tile_light_pick = !*cx.tile_light_pick;
        *cx.anim_target = None;
    }

    // ---- tile grid
    ui.separator();
    match (*cx.anim_target, *cx.tile_light_pick) {
        (Some(k), _) => ui.label(format!("Click tiles to append frames to animation #{} (the first click also sets its base tile).", k + 1)),
        (None, true) => ui.label("Click tiles to add or remove their light (yellow bulb = gives light)."),
        (None, false) => ui.label("Click tiles to toggle solidity (red = blocks movement)."),
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
            if d.lights.iter().any(|l| l.tile == i) {
                p.text(r.right_top() + vec2(-3.0, 2.0), egui::Align2::RIGHT_TOP, ui::ph::LIGHTBULB, egui::FontId::proportional(16.0), Color32::YELLOW);
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
                        None if *cx.tile_light_pick => match d.lights.iter().position(|l| l.tile == i) {
                            Some(k) => {
                                d.lights.remove(k);
                            }
                            None => d.lights.push(rust2d_engine::defs::TileLight { tile: i, light: rust2d_engine::defs::LightSpec::torch(4.0) }),
                        },
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
            if ui.button("\u{E3B4} Edit pixels").on_hover_text("Open the image in the pixel editor").clicked() {
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
    // Who uses this sheet.
    let db = cx.db;
    let mut users: Vec<String> = Vec::new();
    users.extend(db.items.iter().filter(|i| i.icon.sheet == d.id || i.armor.as_ref().is_some_and(|a| a.overlay == d.id)).map(|i| format!("item {}", i.name)));
    users.extend(db.props.iter().filter(|p| p.sprite.sheet == d.id || p.states.iter().any(|s| s.sprite.sheet == d.id)).map(|p| format!("prop {}", p.name)));
    users.extend(db.characters.iter().filter(|c| c.sheet == d.id).map(|c| format!("character {}", c.name)));
    ui.horizontal_wrapped(|ui| {
        ui.label("Used by:");
        if users.is_empty() {
            ui.weak("nothing yet");
        } else {
            ui.weak(users.join(", "));
        }
    });
    ui.weak("Edit the image below: draw, add frames (Canvas … Resize), copy / mirror frames. Ctrl+S saves it and updates it everywhere.");
}

/// Structure form: id, name, category, a one-line summary and the structure editor. Returns true
/// when "Place on map" was clicked.
fn structure_form(ui: &mut egui::Ui, d: &mut StructureDef, cx: &mut Cx) -> bool {
    let library = LIBRARY_MODE.with(|m| m.get());
    let mut place = false;
    ui.horizontal(|ui| {
        ui.heading(if d.name.is_empty() { d.id.as_str() } else { d.name.as_str() });
        ui.add_space(8.0);
        rename_row(ui, RefKind::Structure, &d.id, cx.rename);
    });
    ui.horizontal_wrapped(|ui| {
        ui.label("Name");
        ui.add(egui::TextEdit::singleline(&mut d.name).desired_width(220.0));
        ui.add_space(12.0);
        ui.label("Category");
        ui.add(egui::TextEdit::singleline(&mut d.category).desired_width(140.0).hint_text("e.g. Buildings"))
            .on_hover_text("Groups structures in the map palette, e.g. Buildings, Walls & fences, Rooms, Outdoor");
        let mut cats: Vec<String> = cx.db.structures.iter().map(|s| s.category.clone()).filter(|c| !c.is_empty()).collect();
        cats.sort();
        cats.dedup();
        egui::ComboBox::from_id_salt("st_form_cat").selected_text("").width(24.0).show_ui(ui, |ui| {
            for c in cats {
                ui.selectable_value(&mut d.category, c.clone(), c);
            }
        });
    });
    ui.horizontal_wrapped(|ui| {
        let missing: Vec<&String> = d.tilesets.iter().filter(|t| cx.db.tileset(t).is_none()).collect();
        let tilesets = if d.tilesets.is_empty() { "no tiles yet".to_string() } else { format!("tilesets: {}", d.tilesets.join(", ")) };
        let props = match d.entities.len() {
            0 => String::new(),
            1 => " · 1 prop".to_string(),
            n => format!(" · {n} props"),
        };
        ui.weak(format!("{} × {} tiles · {tilesets}{props}", d.w, d.h));
        if !missing.is_empty() {
            ui.colored_label(Color32::YELLOW, format!("\u{E4E0} not in this project: {}", missing.iter().map(|s| s.as_str()).collect::<Vec<_>>().join(", ")));
        }
        if !library {
            place = ui.button("\u{E31A} Place on map").on_hover_text("Switches to the map with the Structures tool and this structure picked").clicked();
        }
    });
    ui.separator();
    crate::structures::structure_editor(ui, d, cx.st_edit, cx.db, cx.tex, cx.assets);
    if !library {
        ui.weak("You can also capture an area of a map: Structures tool (H), \u{E69C} New from map area…");
    }
    place
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
    ui.weak("WAV or OGG, path relative to the project. Use \u{E036} Reload assets after changing the file or volume.");
    ui.horizontal(|ui| {
        if cx.audio.has(&d.id) {
            if ui.button("\u{E3D0} Play").clicked() {
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
    egui::ScrollArea::vertical().id_salt("project_scroll").show(ui, |ui| project_form(ed, ui));
}

fn project_form(ed: &mut Editor, ui: &mut egui::Ui) {
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
        ui.label("Day length");
        ui.add(egui::DragValue::new(&mut c.day_minutes).range(0.0..=1440.0).speed(0.5).suffix(" min"))
            .on_hover_text("Real minutes one in-game day lasts on outdoor maps (0 = the clock stands still)");
        ui.end_row();
        ui.label("New games start at");
        ui.add(egui::DragValue::new(&mut c.start_hour).range(0.0..=23.99).speed(0.25).suffix(" h"));
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
    ui.heading("Character creator");
    let cc = &mut c.creation;
    ui.checkbox(&mut cc.enabled, "Show the character creator when a new game starts")
        .on_hover_text("The player types a name, chooses male / female and one of the looks below. Play-tests from the editor skip it and use the default name.");
    ui.horizontal(|ui| {
        ui.label("Player character's sex");
        egui::ComboBox::from_id_salt("creation_sex").selected_text(cc.sex.label()).show_ui(ui, |ui| {
            for v in rust2d_engine::project::SexChoice::ALL {
                ui.selectable_value(&mut cc.sex, v, v.label());
            }
        });
        ui.weak(match cc.sex {
            rust2d_engine::project::SexChoice::Both => "chosen in the character creator (male when it is off)",
            _ => "fixed by the game; the creator only offers looks of this sex",
        });
    });
    ui.horizontal(|ui| {
        ui.label("Default name");
        ui.add(egui::TextEdit::singleline(&mut cc.default_name).hint_text("(player character's name)").desired_width(160.0));
    });
    ui.label("Looks (sprite sheets in the player character's layout; the player picks among those of the chosen sex):");
    let mut remove = None;
    let mut swap = None;
    let n = cc.looks.len();
    for (k, l) in cc.looks.iter_mut().enumerate() {
        ui.horizontal(|ui| {
            if widgets::frame_thumb(ui, &ed.tex, &ed.assets, &l.sheet, 0, 44.0, false).on_hover_text("Choose another look").clicked() {
                crate::models::request_model(crate::models::ModelTarget::Look(k));
            }
            ui.vertical(|ui| {
                ui.horizontal(|ui| {
                    egui::ComboBox::from_id_salt(format!("look_sex{k}")).width(80.0).selected_text(l.sex.label()).show_ui(ui, |ui| {
                        for s in rust2d_engine::state::Sex::ALL {
                            ui.selectable_value(&mut l.sex, s, s.label());
                        }
                    });
                    ui.add(egui::TextEdit::singleline(&mut l.label).hint_text(l.sheet.as_str()).desired_width(160.0));
                    if ui.add_enabled(k > 0, egui::Button::new("^").small()).clicked() {
                        swap = Some(k - 1);
                    }
                    if ui.add_enabled(k + 1 < n, egui::Button::new("v").small()).clicked() {
                        swap = Some(k);
                    }
                    if ui.small_button("\u{E4F6}").clicked() {
                        remove = Some(k);
                    }
                });
                ui.horizontal(|ui| {
                    ui.weak(format!("sheet: {}", l.sheet));
                    if !ed.assets.sheets.contains_key(&l.sheet) {
                        ui.colored_label(Color32::LIGHT_RED, "missing sheet");
                    }
                });
            });
        });
    }
    if let Some(k) = swap {
        cc.looks.swap(k, k + 1);
    }
    if let Some(k) = remove {
        cc.looks.remove(k);
    }
    if ui.button("\u{E4C2} + Add a look…").on_hover_text("From this project or the asset library").clicked() {
        crate::models::request_model(crate::models::ModelTarget::Look(n));
    }
    let needed: Vec<rust2d_engine::state::Sex> = match cc.sex.fixed() {
        Some(x) => vec![x],
        None => rust2d_engine::state::Sex::ALL.to_vec(),
    };
    let missing: Vec<&str> = needed.iter().filter(|x| !cc.looks.iter().any(|l| l.sex == **x)).map(|x| x.label()).collect();
    if cc.enabled && !missing.is_empty() {
        ui.colored_label(Color32::YELLOW, format!("No {} look yet: the player character's own look is used.", missing.join(" / ").to_lowercase()));
    }
    ui.weak("Dialogs address the player with {player} (name) and {sir|madam} (male|female); conditions have 'Player sex'; scripts have player_name(), player_sex(), is_female(), is_male(), text(\"...\").");
    ui.separator();
    ui.label(format!("Project folder: {}", ed.project.root.display()));
}

/// `fn name(` definitions in a script, with the character index where each starts.
fn script_functions(text: &str) -> Vec<(String, usize)> {
    let mut out = Vec::new();
    let mut chars = 0;
    for line in text.split_inclusive('\n') {
        let t = line.trim_start();
        if let Some(rest) = t.strip_prefix("fn ").or_else(|| t.strip_prefix("private fn ")) {
            let name: String = rest.trim_start().chars().take_while(|c| c.is_alphanumeric() || *c == '_').collect();
            if !name.is_empty() {
                out.push((name, chars + (line.chars().count() - t.chars().count())));
            }
        }
        chars += line.chars().count();
    }
    out
}

impl Editor {
    /// Opens a script in Database → Scripts (saving the open one first if it was edited),
    /// optionally with the cursor at character `jump`.
    pub fn open_script(&mut self, f: &std::path::Path, jump: Option<usize>) {
        if self.db.script_file.as_deref() != Some(f) {
            self.save_script_if_dirty();
            self.db.script_text = std::fs::read_to_string(f).unwrap_or_default();
            self.db.script_saved = self.db.script_text.clone();
            self.db.script_file = Some(f.to_path_buf());
            self.db.script_dirty = false;
        }
        self.db.script_jump = jump;
    }

    fn save_script_if_dirty(&mut self) {
        if !self.db.script_dirty {
            return;
        }
        if let Some(f) = self.db.script_file.clone() {
            match std::fs::write(&f, &self.db.script_text) {
                Ok(()) => {
                    self.db.script_dirty = false;
                    self.db.script_saved = self.db.script_text.clone();
                    self.set_status(format!("Saved {}", f.file_name().unwrap_or_default().to_string_lossy()));
                }
                Err(e) => self.set_status(format!("Saving script failed: {e}")),
            }
        }
    }
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
    // Always show a script: the first one when none (or a deleted one) is open.
    if ed.db.script_file.as_ref().is_none_or(|f| !files.contains(f)) {
        if let Some(f) = files.first().cloned() {
            ed.db.script_dirty = false;
            ed.open_script(&f, None);
        }
    }
    let mut open: Option<(std::path::PathBuf, Option<usize>)> = None;
    let mut create: Option<std::path::PathBuf> = None;
    let mut save = false;
    // Ids used in API calls that don't exist (checked live, the open file from the editor text).
    let texts: Vec<(std::path::PathBuf, String)> = files
        .iter()
        .map(|f| (f.clone(), if ed.db.script_file.as_ref() == Some(f) { ed.db.script_text.clone() } else { std::fs::read_to_string(f).unwrap_or_default() }))
        .collect();
    let issues: Vec<(std::path::PathBuf, usize, String, String)> = texts
        .iter()
        .flat_map(|(f, text)| {
            rust2d_engine::script_refs::check_script(text, &ed.project.db, &ed.refs.maps, &ed.refs.spawns)
                .into_iter()
                .map(move |i| (f.clone(), i.line, i.message, text.clone()))
        })
        .collect();
    egui::SidePanel::left("scripts_left").default_width(220.0).show_inside(ui, |ui| {
        let st = &mut ed.db;
        ui.weak("Click a file or one of its functions to edit it.");
        egui::ScrollArea::vertical().id_salt("script_files").max_height(ui.available_height() - 40.0).show(ui, |ui| {
            for f in &files {
                let name = f.file_name().unwrap().to_string_lossy().to_string();
                let current = st.script_file.as_ref() == Some(f);
                let row = ui.with_layout(egui::Layout::top_down_justified(egui::Align::LEFT), |ui| ui.selectable_label(current, format!("\u{E23A} {name}")));
                if row.inner.clicked() {
                    open = Some((f.clone(), None));
                }
                // Its functions (from the editor text for the open file, so new ones show at once).
                let text = if current { st.script_text.clone() } else { std::fs::read_to_string(f).unwrap_or_default() };
                for (func, at) in script_functions(&text) {
                    ui.horizontal(|ui| {
                        ui.add_space(14.0);
                        let label = egui::RichText::new(format!("{func}()")).monospace();
                        let r = ui.with_layout(egui::Layout::top_down_justified(egui::Align::LEFT), |ui| ui.selectable_label(false, label));
                        if r.inner.on_hover_text(format!("Go to fn {func} in {name}")).clicked() {
                            open = Some((f.clone(), Some(at)));
                        }
                    });
                }
                ui.add_space(4.0);
            }
            if files.is_empty() {
                ui.weak("No scripts yet.");
            }
            ui.separator();
            if issues.is_empty() {
                ui.weak("\u{E182} No unknown ids in script calls");
            } else {
                ui.colored_label(Color32::YELLOW, format!("Problems ({})", issues.len()))
                    .on_hover_text("Ids passed to script functions (give_item, teleport, start_quest, play_sound, set_state, ...) that don't exist in this project");
                for (f, line, msg, text) in &issues {
                    let name = f.file_name().unwrap_or_default().to_string_lossy();
                    let label = egui::RichText::new(format!("{name}:{line}  {msg}")).small().color(Color32::LIGHT_RED);
                    if ui.add(egui::Label::new(label).sense(egui::Sense::click())).on_hover_text("Go to this line").clicked() {
                        // Character index of the line's start.
                        let at: usize = text.split_inclusive('\n').take(line - 1).map(|l| l.chars().count()).sum();
                        open = Some((f.clone(), Some(at)));
                    }
                }
            }
        });
        ui.separator();
        ui.horizontal(|ui| {
            ui.add(egui::TextEdit::singleline(&mut st.new_script).hint_text("new file name").desired_width(120.0));
            if ui.button("+ New").on_hover_text("Creates scripts/<name>.rhai (without a name: script_N.rhai)").clicked() {
                let base = st.new_script.trim().trim_end_matches(".rhai").to_string();
                let name = if base.is_empty() {
                    (1..).map(|n| format!("script_{n}")).find(|n| !dir.join(format!("{n}.rhai")).exists()).unwrap()
                } else {
                    base
                };
                create = Some(dir.join(format!("{name}.rhai")));
                st.new_script.clear();
            }
        });
    });
    if let Some(p) = create {
        let _ = std::fs::create_dir_all(&dir);
        if !p.exists() {
            let _ = std::fs::write(&p, "// New script\n\nfn my_function() {\n    say(\"Hello!\");\n}\n");
        }
        ed.refresh_refs();
        open = Some((p, None));
    }
    if let Some((f, jump)) = open {
        ed.open_script(&f, jump);
    }
    // What completion offers: names from triggers and maps plus those used in scripts.
    let mut flags = ed.refs.all_flags.clone();
    let mut vars = ed.refs.vars.clone();
    let mut functions: Vec<String> = Vec::new();
    for (_, text) in &texts {
        functions.extend(crate::script_editor::defined_functions(text));
        for a in rust2d_engine::script_refs::string_args(text) {
            match (a.func.as_str(), a.arg) {
                ("flag" | "set_flag", 0) => flags.push(a.value),
                ("get_var" | "set_var" | "add_var" | "queue_set_var", 0) | ("choice", 2) => vars.push(a.value),
                _ => {}
            }
        }
    }
    for v in [&mut flags, &mut vars, &mut functions] {
        v.sort();
        v.dedup();
    }
    let mut targets = vec!["self".to_string(), "player".to_string()];
    targets.extend(ed.refs.entity_names.iter().cloned());
    let problem_spans: Vec<(usize, usize)> = rust2d_engine::script_refs::check_script(&ed.db.script_text, &ed.project.db, &ed.refs.maps, &ed.refs.spawns)
        .into_iter()
        .map(|i| i.span)
        .collect();
    let mut goto: Option<String> = None;
    let st = &mut ed.db;
    egui::CentralPanel::default().show_inside(ui, |ui| {
        let Some(f) = st.script_file.clone() else {
            ui.label("No script files yet: create one on the left. Functions can be called from triggers with the 'Run script' action, or used as 'Script' conditions (returning bool).");
            return;
        };
        ui.horizontal(|ui| {
            ui.strong(f.file_name().unwrap().to_string_lossy());
            if st.script_dirty {
                ui.colored_label(Color32::YELLOW, "• edited");
            }
            save = ui.button("\u{E248} Save & check").on_hover_text("Ctrl+S").clicked();
        });
        if let Some(ci) = st.script_jump.take() {
            st.script_ed.set_cursor = Some(ci);
            st.script_ed.scroll_to = Some(ci);
        }
        let known = crate::script_editor::Known {
            db: &ed.project.db,
            maps: &ed.refs.maps,
            spawns: &ed.refs.spawns,
            targets: &targets,
            flags: &flags,
            vars: &vars,
            functions: &functions,
        };
        let res = crate::script_editor::show(ui, &mut st.script_text, &mut st.script_ed, &known, &problem_spans);
        if res.changed {
            st.script_dirty = st.script_text != st.script_saved;
        }
        goto = res.goto;
        if ui.input(|i| i.modifiers.ctrl && i.key_pressed(egui::Key::S)) {
            save = true;
        }
    });
    // Ctrl+click on a function: open its file at the definition.
    if let Some(name) = goto {
        let found = texts.iter().find_map(|(f, text)| {
            let text = if ed.db.script_file.as_ref() == Some(f) { ed.db.script_text.clone() } else { text.clone() };
            let at = script_functions(&text).into_iter().find(|(n, _)| *n == name)?.1;
            Some((f.clone(), at))
        });
        if let Some((f, at)) = found {
            ed.open_script(&f, Some(at));
        }
    }
    if save {
        ed.db.script_dirty = true;
        ed.save_script_if_dirty();
        if !ed.db.script_dirty {
            let mut host = rust2d_engine::script::ScriptHost::new();
            host.load_dir(&dir);
            ed.refresh_refs();
            let unknown = if issues.is_empty() { String::new() } else { format!("; {} unknown id(s), see Problems", issues.len()) };
            if host.errors.is_empty() {
                ed.set_status(format!("Script saved, no errors{unknown}"));
            } else {
                ed.set_status(format!("Script error: {}{unknown}", host.errors.join("; ")));
            }
        }
    }
}

/// Art actions of the item / prop / character forms.
pub enum ArtAction {
    /// Give the current item icon / prop state a blank frame (`cells` big) to draw in.
    NewFrame { cells: (u32, u32) },
    /// Give the current character its own sheet (blank or a copy of `template`).
    CharacterSheet { template: Option<String> },
}

/// The image the embedded editor should show for a sprite: (path, frame size, character
/// layout, frame to select).
fn art_of(db: &Database, sprite: &SpriteRef, character: bool) -> Option<(String, (u32, u32), bool, u32)> {
    let s = db.sheet(&sprite.sheet)?;
    Some((s.image.clone(), (s.frame_w, s.frame_h), character, sprite.frame))
}

/// "Edit art" toggle and "New blank …" for item icons and prop states.
fn art_buttons(ui: &mut egui::Ui, cx: &mut Cx, what: &str, cells: (u32, u32)) {
    ui.horizontal(|ui| {
        let mut on = *cx.art_open;
        if ui.toggle_value(&mut on, "\u{E6C8} Edit art").on_hover_text("Show the pixel editor under this form, at this sprite").changed() {
            *cx.art_open = on;
        }
        if ui.button(format!("+ New blank {what}")).on_hover_text("Uses an empty frame of the sheet (or adds a row of frames) and opens it for drawing").clicked() {
            *cx.art = Some(ArtAction::NewFrame { cells });
        }
    });
}

impl Editor {
    /// Carries out an art action of the item / prop / character forms.
    pub fn art_action(&mut self, a: ArtAction, tab: Tab) {
        let i = self.db.sel;
        let res: anyhow::Result<String> = (|| match a {
            ArtAction::NewFrame { cells } => {
                let cur = match tab {
                    Tab::Items => self.project.db.items.get(i).map(|d| d.icon.clone()),
                    Tab::Props => self.project.db.props.get(i).map(|d| d.state_look(self.db.state_sel.min(d.states.len())).0.clone()),
                    _ => None,
                };
                let Some(cur) = cur else { anyhow::bail!("nothing selected") };
                let sheet = if self.project.db.sheet(&cur.sheet).is_some() { cur.sheet.clone() } else { self.default_sheet(tab)? };
                let frame = self.blank_frame(&sheet, cells)?;
                let sprite = if cells == (1, 1) { SpriteRef::new(&sheet, frame) } else { SpriteRef::block(&sheet, frame, cells.0, cells.1) };
                match tab {
                    Tab::Items => self.project.db.items[i].icon = sprite,
                    _ => {
                        let d = &mut self.project.db.props[i];
                        let k = self.db.state_sel.min(d.states.len());
                        if k == 0 {
                            d.sprite = sprite;
                        } else {
                            d.states[k - 1].sprite = sprite;
                        }
                    }
                }
                self.defs_dirty = true;
                self.db.art_open = true;
                self.db.art_focus = None;
                Ok(format!("Blank frame {frame} of sheet '{sheet}' is ready: draw it below, Ctrl+S saves the image"))
            }
            ArtAction::CharacterSheet { template } => {
                let Some(d) = self.project.db.characters.get(i).cloned() else { anyhow::bail!("nothing selected") };
                let mut id = d.id.clone();
                let mut n = 2;
                while self.project.db.sheet(&id).is_some() {
                    id = format!("{}_{n}", d.id);
                    n += 1;
                }
                let mut rel = format!("sprites/{id}.png");
                let mut n = 2;
                while self.project.root.join(&rel).exists() {
                    rel = format!("sprites/{id}_{n}.png");
                    n += 1;
                }
                let (bytes, fw, fh) = match &template {
                    Some(t) => {
                        let s = self.project.db.sheet(t).cloned().ok_or_else(|| anyhow::anyhow!("unknown sheet {t}"))?;
                        (std::fs::read(self.project.root.join(&s.image))?, s.frame_w, s.frame_h)
                    }
                    None => {
                        let ts = rust2d_engine::project::TILE_SIZE;
                        (Rgba::new(ts * 4, ts * 3 / 2 * 4).encode()?, ts, ts * 3 / 2)
                    }
                };
                let path = self.project.root.join(&rel);
                std::fs::create_dir_all(path.parent().unwrap())?;
                std::fs::write(&path, bytes)?;
                self.project.db.sheets.push(SheetDef { id: id.clone(), image: rel.clone(), frame_w: fw, frame_h: fh });
                self.project.db.characters[i].sheet = id.clone();
                self.defs_dirty = true;
                self.reload_assets();
                self.db.art_open = true;
                self.db.art_focus = None;
                Ok(format!("New sheet '{id}' ({rel}) for {}: draw it below, Ctrl+S saves the image", d.name))
            }
        })();
        match res {
            Ok(m) => self.set_status(m),
            Err(e) => self.set_status(format!("Could not create the art: {e:#}")),
        }
    }

    /// A sheet for new item icons / prop frames when the definition has none yet: an existing
    /// "items" / "props" sheet, else a new blank one.
    fn default_sheet(&mut self, tab: Tab) -> anyhow::Result<String> {
        let want = if tab == Tab::Items { "items" } else { "props" };
        if self.project.db.sheet(want).is_some() {
            return Ok(want.to_string());
        }
        let id = if tab == Tab::Items { "icons" } else { "prop_art" };
        if self.project.db.sheet(id).is_none() {
            let ts = rust2d_engine::project::TILE_SIZE;
            let rel = format!("sprites/{id}.png");
            let path = self.project.root.join(&rel);
            std::fs::create_dir_all(path.parent().unwrap())?;
            if !path.exists() {
                std::fs::write(&path, Rgba::new(ts * 8, ts).encode()?)?;
            }
            self.project.db.sheets.push(SheetDef { id: id.into(), image: rel, frame_w: ts, frame_h: ts });
            self.defs_dirty = true;
            self.reload_assets();
        }
        Ok(id.to_string())
    }

    /// First fully transparent `cells`-sized frame block of a sheet; if there is none, a row
    /// of frames is added to the image. Returns the block's top-left frame.
    fn blank_frame(&mut self, sheet: &str, cells: (u32, u32)) -> anyhow::Result<u32> {
        let def = self.project.db.sheet(sheet).cloned().ok_or_else(|| anyhow::anyhow!("unknown sheet {sheet}"))?;
        if self.px.doc.as_ref().is_some_and(|d| d.rel == def.image && d.is_dirty()) {
            anyhow::bail!("save or revert the open image {} first", def.image);
        }
        let path = self.project.root.join(&def.image);
        let mut img = Rgba::decode(&std::fs::read(&path)?)?;
        let (fw, fh) = (def.frame_w.max(1), def.frame_h.max(1));
        let (cols, rows) = ((img.w / fw).max(1), img.h / fh);
        let (cw, ch) = (cells.0.max(1), cells.1.max(1));
        if cw > cols {
            anyhow::bail!("the sheet is only {cols} frames wide");
        }
        let empty = |img: &Rgba, c: u32, r: u32| {
            (0..ch * fh).all(|y| (0..cw * fw).all(|x| img.get((c * fw + x) as i32, (r * fh + y) as i32)[3] == 0))
        };
        for r in 0..rows.saturating_sub(ch - 1) {
            for c in 0..=cols - cw {
                if empty(&img, c, r) {
                    return Ok(r * cols + c);
                }
            }
        }
        // Full: add rows at the bottom.
        img = img.with_canvas(img.w, (rows + ch) * fh);
        std::fs::write(&path, img.encode()?)?;
        self.reload_assets();
        if let Some(d) = self.px.doc.as_ref().filter(|d| d.rel == def.image) {
            let (rel, frame, layout) = (d.rel.clone(), d.frame, d.layout);
            self.load_pixel(&rel, frame, layout);
        }
        Ok(rows * cols)
    }
}
