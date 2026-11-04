//! The shared asset library: the Asset Library window's Browse view (`rust2d-editor --library`,
//! see `library_window.rs`: import / export packages, remove, jump to editing) and the main
//! editor's "Add from library" dialog. Both use the same thumbnail grid.

use std::collections::{BTreeSet, HashMap};
use std::path::Path;

use anyhow::{Context, Result};
use egui_macroquad::egui::{self, pos2, vec2, Color32, Rect, Sense, Stroke, StrokeKind, TextureHandle};
use rust2d_engine::defs::{Database, SpriteRef};
use rust2d_engine::imageio::Rgba;
use rust2d_engine::library::{self, kind_label, AssetRef, Bundle, KINDS};
use rust2d_engine::project::Project;
use rust2d_engine::refactor::RefKind;

use crate::app::Editor;
use crate::files::Purpose;

/// Assets the library starts with (built from the example project by `gen_example`).
pub const STARTER: &[u8] = include_bytes!("../assets/starter.r2dpack");

struct Tex {
    handle: TextureHandle,
    w: f32,
    h: f32,
}

/// The "Add from library" dialog of the main editor.
#[derive(Default)]
pub struct AddDialog {
    /// Opened for one kind of asset (e.g. from the Tilesets tab); `None` = all kinds.
    pub kind: Option<RefKind>,
    /// Kind tab inside the dialog when it was opened for all kinds.
    pub filter: Option<RefKind>,
    pub picked: BTreeSet<AssetRef>,
    pub search: String,
}

#[derive(Default)]
pub struct LibState {
    /// The library as a project-like store (main editor: loaded for the add dialog and the
    /// model chooser; the Asset Library window uses `Editor::project` instead).
    pub lib: Option<Project>,
    pub error: Option<String>,
    tex: HashMap<String, Tex>,
    pub tex_dirty: bool,
    pub filter: Option<RefKind>,
    pub search: String,
    pub selected: BTreeSet<AssetRef>,
    pub report: Option<String>,
    /// Assets waiting for the user to confirm their removal (the confirmation dialog is open).
    pub confirm_remove: Option<Vec<AssetRef>>,
    pub add: Option<AddDialog>,
}

impl LibState {
    pub fn ensure_loaded(&mut self) {
        if self.lib.is_some() || self.error.is_some() {
            return;
        }
        match library::open_library(&library::library_dir(), Some(STARTER)) {
            Ok(p) => {
                self.lib = Some(p);
                self.tex_dirty = true;
            }
            Err(e) => self.error = Some(format!("Cannot open the asset library at {}: {e:#}", library::library_dir().display())),
        }
    }

    /// Reads the library from disk again (it may have been changed in the Asset Library window).
    pub fn reload(&mut self) {
        self.lib = None;
        self.error = None;
        self.ensure_loaded();
        if let Some(lib) = &self.lib {
            self.selected.retain(|a| library::exists(&lib.db, a));
        }
    }

    pub(crate) fn textures(&mut self, ctx: &egui::Context) {
        if let Some(lib) = self.lib.take() {
            self.textures_of(ctx, &lib);
            self.lib = Some(lib);
        }
    }

    /// Thumbnail textures of a store's tilesets and sprite sheets (rebuilt when `tex_dirty`).
    pub(crate) fn textures_of(&mut self, ctx: &egui::Context, store: &Project) {
        if !self.tex_dirty {
            return;
        }
        self.tex_dirty = false;
        self.tex.clear();
        let images = store.db.tilesets.iter().map(|t| (format!("ts:{}", t.id), &t.image)).chain(store.db.sheets.iter().map(|s| (format!("sh:{}", s.id), &s.image)));
        for (key, rel) in images {
            let Ok(img) = std::fs::read(store.root.join(rel)).map_err(anyhow::Error::from).and_then(|b| Rgba::decode(&b)) else { continue };
            let ci = egui::ColorImage::from_rgba_unmultiplied([img.w as usize, img.h as usize], &img.px);
            let handle = ctx.load_texture(format!("lib:{key}"), ci, egui::TextureOptions::NEAREST);
            self.tex.insert(key, Tex { handle, w: img.w as f32, h: img.h as f32 });
        }
    }

    /// Paints a sprite of a library sheet into `rect` (aspect kept, bottom-centered).
    pub(crate) fn paint_sprite(&self, ui: &egui::Ui, sprite: &SpriteRef, rect: Rect) -> bool {
        self.lib.as_ref().is_some_and(|lib| self.paint_sprite_in(ui, &lib.db, sprite, rect))
    }

    fn paint_sprite_in(&self, ui: &egui::Ui, db: &Database, sprite: &SpriteRef, rect: Rect) -> bool {
        let (Some(t), Some(s)) = (self.tex.get(&format!("sh:{}", sprite.sheet)), db.sheet(&sprite.sheet)) else { return false };
        let (fw, fh) = (s.frame_w.max(1) as f32, s.frame_h.max(1) as f32);
        let cols = (t.w / fw).floor().max(1.0) as u32;
        let (cw, ch) = sprite.cells();
        let (x, y) = ((sprite.frame % cols) as f32 * fw, (sprite.frame / cols) as f32 * fh);
        let (w, h) = (fw * cw as f32, fh * ch as f32);
        let uv = Rect::from_min_max(pos2(x / t.w, y / t.h), pos2((x + w) / t.w, (y + h) / t.h));
        let scale = (rect.width() / w).min(rect.height() / h);
        let size = vec2(w * scale, h * scale);
        let r = Rect::from_min_size(pos2(rect.center().x - size.x / 2.0, rect.max.y - size.y), size);
        ui.painter().image(t.handle.id(), r, uv, Color32::WHITE);
        true
    }

    fn paint_thumb(&self, ui: &egui::Ui, db: &Database, a: &AssetRef, rect: Rect) {
        if a.0 == RefKind::Structure {
            if let Some(d) = db.structure(&a.1) {
                let tileset = |id: &str| self.tex.get(&format!("ts:{id}")).map(|t| (t.handle.id(), vec2(t.w, t.h)));
                let sprite = |s: &SpriteRef, r: Rect| self.paint_sprite_in(ui, db, s, r);
                crate::structures::paint_structure(ui, rect, d, db, &tileset, &sprite);
            }
            return;
        }
        let sprite = match a.0 {
            RefKind::Item => db.item(&a.1).map(|d| d.icon.clone()),
            RefKind::Prop => db.prop(&a.1).map(|d| d.sprite.clone()),
            RefKind::Character => db.character(&a.1).map(|d| SpriteRef::new(&d.sheet, 0)),
            RefKind::Sheet => Some(SpriteRef::new(&a.1, 0)),
            _ => None,
        };
        let drawn = match (a.0, sprite) {
            (RefKind::Tileset, _) => self.tex.get(&format!("ts:{}", a.1)).is_some_and(|t| {
                let scale = (rect.width() / t.w).min(rect.height() / t.h);
                let r = Rect::from_center_size(rect.center(), vec2(t.w, t.h) * scale);
                ui.painter().image(t.handle.id(), r, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
                true
            }),
            (_, Some(s)) => self.paint_sprite_in(ui, db, &s, rect),
            _ => false,
        };
        if !drawn {
            let glyph = if a.0 == RefKind::Sound { "♪" } else { "?" };
            ui.painter().text(rect.center(), egui::Align2::CENTER_CENTER, glyph, egui::FontId::proportional(28.0), Color32::GRAY);
        }
    }

    /// A wrapping grid of asset cards (thumbnail, name, kind; `note` adds a small tag like
    /// "in project"). Returns the clicked and the double-clicked card.
    fn grid(&self, ui: &mut egui::Ui, db: &Database, shown: &[AssetRef], on: &BTreeSet<AssetRef>, note: &dyn Fn(&AssetRef) -> Option<String>) -> (Option<AssetRef>, Option<AssetRef>) {
        let (mut clicked, mut double) = (None, None);
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = vec2(8.0, 8.0);
            for a in shown {
                let (rect, resp) = ui.allocate_exact_size(vec2(118.0, 112.0), Sense::click());
                let sel = on.contains(a);
                let bg = if sel { Color32::from_rgb(52, 64, 90) } else if resp.hovered() { Color32::from_gray(48) } else { Color32::from_gray(38) };
                ui.painter().rect_filled(rect, 4.0, bg);
                ui.painter().rect_stroke(rect, 4.0, Stroke::new(if sel { 2.0_f32 } else { 1.0 }, if sel { Color32::YELLOW } else { Color32::from_gray(60) }), StrokeKind::Inside);
                let thumb = Rect::from_min_size(rect.min + vec2(23.0, 6.0), vec2(72.0, 64.0));
                self.paint_thumb(ui, db, a, thumb);
                let name = name_of(db, a);
                let short: String = if name.chars().count() > 16 { name.chars().take(15).collect::<String>() + "…" } else { name.clone() };
                ui.painter().text(pos2(rect.center().x, rect.min.y + 84.0), egui::Align2::CENTER_CENTER, short, egui::FontId::proportional(13.0), Color32::WHITE);
                let sub = match note(a) {
                    Some(n) => format!("{} · {n}", kind_label(a.0)),
                    None => kind_label(a.0).to_string(),
                };
                ui.painter().text(pos2(rect.center().x, rect.min.y + 100.0), egui::Align2::CENTER_CENTER, sub, egui::FontId::proportional(11.0), Color32::GRAY);
                let deps: Vec<String> = library::deps(db, a).into_iter().map(|d| format!("{} {}", kind_label(d.0).to_lowercase(), d.1)).collect();
                let tip = if deps.is_empty() { format!("{name}\nid: {}", a.1) } else { format!("{name}\nid: {}\nalso brings: {}", a.1, deps.join(", ")) };
                let resp = resp.on_hover_text(tip);
                if resp.double_clicked() {
                    double = Some(a.clone());
                } else if resp.clicked() {
                    clicked = Some(a.clone());
                }
            }
        });
        (clicked, double)
    }
}

/// Display name of an asset (items, props and characters have one; others show their id).
fn name_of(db: &Database, a: &AssetRef) -> String {
    match a.0 {
        RefKind::Item => db.item(&a.1).map(|d| d.name.clone()),
        RefKind::Prop => db.prop(&a.1).map(|d| d.name.clone()),
        RefKind::Character => db.character(&a.1).map(|d| d.name.clone()),
        _ => None,
    }
    .unwrap_or_else(|| a.1.clone())
}

/// Assets of `db` of the kind `k` (all with `None`) matching a search text (id or name).
fn filtered(db: &Database, k: Option<RefKind>, search: &str) -> Vec<AssetRef> {
    let q = search.trim().to_lowercase();
    library::all_assets(db)
        .into_iter()
        .filter(|a| k.is_none_or(|k| k == a.0))
        .filter(|a| q.is_empty() || a.1.to_lowercase().contains(&q) || name_of(db, a).to_lowercase().contains(&q))
        .collect()
}

/// "tilesets", "sprite sheets", …
fn plural(k: RefKind) -> String {
    format!("{}s", kind_label(k).to_lowercase())
}

/// Parses "item", "prop", "character", "sheet", "tileset", "sound" or "structure".
pub fn parse_kind(s: &str) -> Option<RefKind> {
    Some(match s {
        "item" => RefKind::Item,
        "prop" => RefKind::Prop,
        "character" => RefKind::Character,
        "sheet" => RefKind::Sheet,
        "tileset" => RefKind::Tileset,
        "sound" => RefKind::Sound,
        "structure" => RefKind::Structure,
        _ => return None,
    })
}

fn with_ext(path: &Path) -> std::path::PathBuf {
    if path.extension().is_some_and(|e| e == library::PACK_EXT) {
        path.to_path_buf()
    } else {
        let mut p = path.as_os_str().to_owned();
        p.push(format!(".{}", library::PACK_EXT));
        p.into()
    }
}

impl Editor {
    /// The library as a store: the open project in the Asset Library window, else the
    /// library loaded next to the project.
    fn lib_store_mut(&mut self) -> Option<&mut Project> {
        if self.library_mode {
            Some(&mut self.project)
        } else {
            self.lib.ensure_loaded();
            self.lib.lib.as_mut()
        }
    }

    fn lib_store(&mut self) -> Option<&Project> {
        self.lib_store_mut().map(|p| &*p)
    }

    /// Adds library assets (with their dependencies) to the project.
    pub fn library_to_project(&mut self, assets: &[AssetRef]) -> Result<String> {
        self.lib.ensure_loaded();
        let lib = self.lib.lib.as_ref().context("library not available")?;
        let mut b = Bundle::collect(&lib.db, &lib.root, rust2d_engine::project::TILE_SIZE, "library", assets)?;
        b.rescale(rust2d_engine::project::TILE_SIZE)?;
        let root = self.project.root.clone();
        let report = b.merge_into(&mut self.project.db, &root)?;
        self.defs_dirty = true;
        self.reload_assets();
        Ok(format!("Added to the project: {}. Save the definitions to keep them.", report.summary()))
    }

    /// Copies project assets (with their dependencies) into the library.
    pub fn project_to_library(&mut self, assets: &[AssetRef]) -> Result<String> {
        self.lib.ensure_loaded();
        let b = Bundle::collect(&self.project.db, &self.project.root, rust2d_engine::project::TILE_SIZE, &self.project.config.name, assets)?;
        let lib = self.lib.lib.as_mut().context("library not available")?;
        let report = library::add_to_library(lib, b)?;
        self.lib.tex_dirty = true;
        Ok(format!("Library: {}", report.summary()))
    }

    pub fn export_package(&mut self, from_library: bool, assets: &[AssetRef], path: &Path) -> Result<String> {
        let b = if from_library {
            let lib = self.lib_store().context("library not available")?;
            Bundle::collect(&lib.db, &lib.root, rust2d_engine::project::TILE_SIZE, "rust2d assets", assets)?
        } else {
            Bundle::collect(&self.project.db, &self.project.root, rust2d_engine::project::TILE_SIZE, &self.project.config.name, assets)?
        };
        let path = with_ext(path);
        std::fs::write(&path, b.to_pack()?).with_context(|| format!("writing {}", path.display()))?;
        Ok(format!("Exported {} asset(s) to {}", library::all_assets(&b.db).len(), path.display()))
    }

    pub fn import_package(&mut self, into_library: bool, path: &Path) -> Result<String> {
        let bytes = std::fs::read(path).with_context(|| format!("reading {}", path.display()))?;
        let mut b = Bundle::from_pack(&bytes)?;
        if into_library {
            let lib = self.lib_store_mut().context("library not available")?;
            let report = library::add_to_library(lib, b)?;
            self.after_library_write();
            Ok(format!("Imported into the library: {}", report.summary()))
        } else {
            b.rescale(rust2d_engine::project::TILE_SIZE)?;
            let root = self.project.root.clone();
            let report = b.merge_into(&mut self.project.db, &root)?;
            self.defs_dirty = true;
            self.reload_assets();
            Ok(format!("Imported into the project: {}. Save the definitions to keep them.", report.summary()))
        }
    }

    /// The library's definitions were just written to disk (import / remove save them).
    fn after_library_write(&mut self) {
        self.lib.tex_dirty = true;
        if self.library_mode {
            self.mark_library_saved();
            self.reload_assets();
        }
    }

    /// Removes assets from the library right away (after the confirmation dialog).
    pub fn remove_from_library(&mut self, assets: &[AssetRef]) {
        let r = match self.lib_store_mut() {
            Some(lib) => library::remove_assets(lib, assets).map(|_| format!("Removed {} asset(s) from the library", assets.len())),
            None => Ok(String::new()),
        };
        if r.is_ok() {
            self.lib.selected.clear();
            self.after_library_write();
            self.db.sel = 0;
        }
        self.show_result(r);
    }

    /// The "Remove from library?" dialog: lists the assets and what still uses them.
    pub(crate) fn confirm_remove_dialog(&mut self, ctx: &egui::Context) {
        let Some(list) = self.lib.confirm_remove.clone() else { return };
        let unsaved = self.library_mode && self.defs_dirty;
        let Some(lib) = self.lib_store() else {
            self.lib.confirm_remove = None;
            return;
        };
        // Remaining library assets that depend on the ones to remove (removal would be refused).
        let mut users: Vec<String> = list
            .iter()
            .flat_map(|a| library::dependents(&lib.db, a))
            .filter(|u| !list.contains(u))
            .map(|u| format!("{} '{}'", kind_label(u.0), u.1))
            .collect();
        users.sort();
        users.dedup();
        let mut choice: Option<bool> = None;
        let modal = egui::Modal::new(egui::Id::new("lib_confirm_remove")).show(ctx, |ui| {
            ui.set_max_width(460.0);
            ui.heading(format!("Remove {} asset(s) from the library?", list.len()));
            ui.add_space(4.0);
            egui::ScrollArea::vertical().max_height(220.0).show(ui, |ui| {
                for a in &list {
                    ui.label(format!("• {} '{}'", kind_label(a.0), a.1));
                }
            });
            ui.add_space(4.0);
            if users.is_empty() {
                ui.label("Their definitions and image / sound files are deleted from the library folder. This can't be undone. Projects that already use them keep their own copies.");
                if unsaved {
                    ui.colored_label(Color32::YELLOW, "Your other unsaved library edits are saved as well.");
                }
            } else {
                ui.colored_label(Color32::LIGHT_RED, format!("Can't remove: still used by {}. Select those too, or remove them first.", users.join(", ")));
            }
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                let remove = egui::Button::new(egui::RichText::new("🗑 Remove").color(Color32::WHITE)).fill(Color32::from_rgb(150, 40, 40));
                if ui.add_enabled(users.is_empty(), remove).clicked() {
                    choice = Some(true);
                }
                if ui.button("Cancel").clicked() {
                    choice = Some(false);
                }
            });
        });
        if modal.should_close() && choice.is_none() {
            choice = Some(false); // Esc or a click outside cancels
        }
        match choice {
            Some(true) => {
                self.lib.confirm_remove = None;
                self.remove_from_library(&list);
            }
            Some(false) => self.lib.confirm_remove = None,
            None => {}
        }
    }

    /// Browse view of the Asset Library window: all library assets, import / export / remove,
    /// and a jump to editing an asset.
    pub fn library_ui(&mut self, ctx: &egui::Context) {
        self.lib.textures_of(ctx, &self.project);
        let mut edit: Option<AssetRef> = None;
        let mut new_structure = false;
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.heading("Asset library");
                ui.weak(self.project.root.display().to_string());
            });
            let sel: Vec<AssetRef> = self.lib.selected.iter().cloned().collect();
            let any = !sel.is_empty();
            ui.horizontal_wrapped(|ui| {
                if ui.add_enabled(sel.len() == 1, egui::Button::new("✏ Edit")).on_hover_text("Edit the selected asset (or double-click it)").clicked() {
                    edit = sel.first().cloned();
                }
                if ui.button("✨ New asset…").on_hover_text("Create a prop, item, character, tileset or sprite sheet in the library (draw it in the pixel editor)").clicked() {
                    self.wizard = Some(crate::wizard::Wizard::new(&self.project));
                }
                if ui.button("🏠 New structure").on_hover_text("A multi-tile structure (house, wall, room…), assembled from tileset stamps").clicked() {
                    new_structure = true;
                }
                ui.separator();
                if ui.button("📥 Import package…").on_hover_text("Adds a .r2dpack file to the library").clicked() {
                    self.open_file_dialog(Purpose::ImportPackage { into_library: true });
                }
                if ui.add_enabled(any, egui::Button::new("📦 Export package…")).on_hover_text("Saves the selected assets and everything they need as a .r2dpack file").clicked() {
                    self.open_file_dialog(Purpose::ExportPackage { from_library: true, assets: sel.clone() });
                }
                if ui.add_enabled(any, egui::Button::new("🗑 Remove from library…")).clicked() {
                    self.lib.confirm_remove = Some(sel.clone());
                }
            });
            ui.horizontal_wrapped(|ui| {
                if ui.selectable_label(self.lib.filter.is_none(), "All").clicked() {
                    self.lib.filter = None;
                }
                for k in KINDS {
                    if ui.selectable_label(self.lib.filter == Some(k), format!("{}s", kind_label(k))).clicked() {
                        self.lib.filter = Some(k);
                    }
                }
                ui.separator();
                ui.label("🔍");
                ui.add(egui::TextEdit::singleline(&mut self.lib.search).hint_text("search").desired_width(160.0));
            });
            let db = &self.project.db;
            let shown = filtered(db, self.lib.filter, &self.lib.search);
            ui.horizontal(|ui| {
                ui.label(format!("{} shown · {} selected", shown.len(), self.lib.selected.len()));
                if ui.small_button("Select shown").clicked() {
                    self.lib.selected.extend(shown.iter().cloned());
                }
                if ui.small_button("Clear selection").clicked() {
                    self.lib.selected.clear();
                }
            });
            if let Some(r) = &self.lib.report {
                ui.colored_label(if r.starts_with("Error") { Color32::LIGHT_RED } else { Color32::LIGHT_GREEN }, r);
            }
            ui.separator();
            let (clicked, double) = egui::ScrollArea::vertical().id_salt("lib_grid").show(ui, |ui| self.lib.grid(ui, db, &shown, &self.lib.selected, &|_| None)).inner;
            if let Some(a) = clicked {
                if !self.lib.selected.remove(&a) {
                    self.lib.selected.insert(a);
                }
            }
            if double.is_some() {
                edit = double;
            }
        });
        if let Some(a) = edit {
            self.edit_library_asset(&a);
        }
        if new_structure {
            self.new_structure();
        }
    }

    /// Opens the "Add from library" dialog (re-reading the library, which the Asset Library
    /// window may have changed). `kind` limits it to one kind of asset.
    pub fn open_add_dialog(&mut self, kind: Option<RefKind>) {
        self.lib.reload();
        self.lib.tex_dirty = true;
        self.lib.add = Some(AddDialog { kind, ..Default::default() });
    }

    /// Adds the assets picked in the "Add from library" dialog to the project and closes it.
    pub fn add_picked_from_library(&mut self) {
        let Some(d) = self.lib.add.take() else { return };
        let picked: Vec<AssetRef> = d.picked.into_iter().collect();
        if picked.is_empty() {
            return;
        }
        let r = self.library_to_project(&picked);
        self.show_result(r);
    }

    /// The main editor's "Add from library" dialog.
    pub fn add_from_library_dialog(&mut self, ctx: &egui::Context) {
        let Some(mut d) = self.lib.add.take() else { return };
        self.lib.textures(ctx);
        let (mut close, mut add) = (false, false);
        let width = (ctx.screen_rect().width() * 0.8).clamp(400.0, 900.0);
        let height = ctx.screen_rect().height() * 0.5;
        let modal = egui::Modal::new(egui::Id::new("lib_add")).show(ctx, |ui| {
            ui.set_width(width);
            ui.heading(match d.kind {
                Some(k) => format!("Add {} from the library", plural(k)),
                None => "Add from the library".to_string(),
            });
            let Some(lib) = &self.lib.lib else {
                ui.colored_label(Color32::LIGHT_RED, self.lib.error.clone().unwrap_or_else(|| "The asset library is not available.".into()));
                close = ui.button("Close").clicked();
                return;
            };
            ui.horizontal_wrapped(|ui| {
                if d.kind.is_none() {
                    if ui.selectable_label(d.filter.is_none(), "All").clicked() {
                        d.filter = None;
                    }
                    for k in KINDS {
                        if ui.selectable_label(d.filter == Some(k), format!("{}s", kind_label(k))).clicked() {
                            d.filter = Some(k);
                        }
                    }
                    ui.separator();
                }
                ui.label("🔍");
                ui.add(egui::TextEdit::singleline(&mut d.search).hint_text("search").desired_width(160.0));
            });
            let shown = filtered(&lib.db, d.kind.or(d.filter), &d.search);
            ui.horizontal(|ui| {
                ui.label(format!("{} in the library · {} picked", shown.len(), d.picked.len()));
                if ui.small_button("Pick all shown").clicked() {
                    d.picked.extend(shown.iter().cloned());
                }
                if ui.small_button("Clear").clicked() {
                    d.picked.clear();
                }
            });
            ui.separator();
            let project = &self.project.db;
            let note = |a: &AssetRef| library::exists(project, a).then(|| "in project".to_string());
            let (clicked, double) = egui::ScrollArea::vertical()
                .id_salt("lib_add_grid")
                .max_height(height)
                .auto_shrink([false, true])
                .show(ui, |ui| {
                    if shown.is_empty() {
                        ui.weak("Nothing in the library matches.");
                    }
                    self.lib.grid(ui, &lib.db, &shown, &d.picked, &note)
                })
                .inner;
            if let Some(a) = clicked.or(double.clone()) {
                if !d.picked.remove(&a) {
                    d.picked.insert(a);
                }
            }
            if let Some(a) = double {
                // Double-click: add just this one right away.
                d.picked = BTreeSet::from([a]);
                add = true;
            }
            ui.separator();
            let mut brings: Vec<String> = d
                .picked
                .iter()
                .flat_map(|a| library::deps(&lib.db, a))
                .filter(|a| !d.picked.contains(a) && !library::exists(project, a))
                .map(|a| format!("{} {}", kind_label(a.0).to_lowercase(), a.1))
                .collect();
            brings.sort();
            brings.dedup();
            if !brings.is_empty() {
                ui.weak(format!("Also brings what they need: {}", brings.join(", ")));
            }
            if d.picked.iter().any(|a| library::exists(project, a)) {
                ui.weak("Assets marked 'in project' are skipped if identical; a different one with the same id is added under a new id.");
            }
            ui.horizontal(|ui| {
                let label = format!("➕ Add {} to project", d.picked.len());
                if ui.add_enabled(!d.picked.is_empty(), egui::Button::new(label)).clicked() {
                    add = true;
                }
                if ui.button("Cancel").clicked() {
                    close = true;
                }
            });
        });
        if modal.should_close() && !add {
            close = true;
        }
        if add {
            self.lib.add = Some(d);
            self.add_picked_from_library();
        } else if !close {
            self.lib.add = Some(d);
        }
    }

    /// "📥 Add from library…" button for `kind` (`None`: all kinds).
    pub fn add_from_library_button(&mut self, ui: &mut egui::Ui, kind: Option<RefKind>) {
        let what = kind.map_or("assets".to_string(), plural);
        if ui.button("📥 Add from library…").on_hover_text(format!("Choose {what} from the shared asset library to copy into this project")).clicked() {
            self.open_add_dialog(kind);
        }
    }
}
