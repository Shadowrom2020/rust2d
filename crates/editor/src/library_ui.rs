//! Library mode: browse the shared asset library, add assets to the project, import and
//! export `.r2dpack` packages.

use std::collections::{BTreeSet, HashMap};
use std::path::Path;

use anyhow::{Context, Result};
use egui_macroquad::egui::{self, pos2, vec2, Color32, Rect, Sense, Stroke, StrokeKind, TextureHandle};
use rust2d_engine::defs::SpriteRef;
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

#[derive(Default)]
pub struct LibState {
    pub lib: Option<Project>,
    pub error: Option<String>,
    tex: HashMap<String, Tex>,
    tex_dirty: bool,
    pub filter: Option<RefKind>,
    pub search: String,
    pub selected: BTreeSet<AssetRef>,
    pub report: Option<String>,
    /// Assets waiting for the user to confirm their removal (the confirmation dialog is open).
    pub confirm_remove: Option<Vec<AssetRef>>,
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

    pub fn reload(&mut self) {
        self.lib = None;
        self.error = None;
        self.ensure_loaded();
        if let Some(lib) = &self.lib {
            self.selected.retain(|a| library::exists(&lib.db, a));
        }
    }

    pub(crate) fn textures(&mut self, ctx: &egui::Context) {
        if !self.tex_dirty {
            return;
        }
        self.tex_dirty = false;
        self.tex.clear();
        let Some(lib) = &self.lib else { return };
        let images = lib.db.tilesets.iter().map(|t| (format!("ts:{}", t.id), &t.image)).chain(lib.db.sheets.iter().map(|s| (format!("sh:{}", s.id), &s.image)));
        for (key, rel) in images {
            let Ok(img) = std::fs::read(lib.root.join(rel)).map_err(anyhow::Error::from).and_then(|b| Rgba::decode(&b)) else { continue };
            let ci = egui::ColorImage::from_rgba_unmultiplied([img.w as usize, img.h as usize], &img.px);
            let handle = ctx.load_texture(format!("lib:{key}"), ci, egui::TextureOptions::NEAREST);
            self.tex.insert(key, Tex { handle, w: img.w as f32, h: img.h as f32 });
        }
    }

    /// Paints a sprite of a library sheet into `rect` (aspect kept, bottom-centered).
    pub(crate) fn paint_sprite(&self, ui: &egui::Ui, sprite: &SpriteRef, rect: Rect) -> bool {
        let (Some(lib), Some(t)) = (&self.lib, self.tex.get(&format!("sh:{}", sprite.sheet))) else { return false };
        let Some(s) = lib.db.sheet(&sprite.sheet) else { return false };
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

    fn paint_thumb(&self, ui: &egui::Ui, a: &AssetRef, rect: Rect) {
        let Some(lib) = &self.lib else { return };
        let db = &lib.db;
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
            (_, Some(s)) => self.paint_sprite(ui, &s, rect),
            _ => false,
        };
        if !drawn {
            let glyph = if a.0 == RefKind::Sound { "♪" } else { "?" };
            ui.painter().text(rect.center(), egui::Align2::CENTER_CENTER, glyph, egui::FontId::proportional(28.0), Color32::GRAY);
        }
    }

    fn name_of(&self, a: &AssetRef) -> String {
        let Some(lib) = &self.lib else { return a.1.clone() };
        let db = &lib.db;
        match a.0 {
            RefKind::Item => db.item(&a.1).map(|d| d.name.clone()),
            RefKind::Prop => db.prop(&a.1).map(|d| d.name.clone()),
            RefKind::Character => db.character(&a.1).map(|d| d.name.clone()),
            _ => None,
        }
        .unwrap_or_else(|| a.1.clone())
    }
}

/// Parses "item", "prop", "character", "sheet", "tileset" or "sound".
pub fn parse_kind(s: &str) -> Option<RefKind> {
    Some(match s {
        "item" => RefKind::Item,
        "prop" => RefKind::Prop,
        "character" => RefKind::Character,
        "sheet" => RefKind::Sheet,
        "tileset" => RefKind::Tileset,
        "sound" => RefKind::Sound,
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
            self.lib.ensure_loaded();
            let lib = self.lib.lib.as_ref().context("library not available")?;
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
            self.lib.ensure_loaded();
            let lib = self.lib.lib.as_mut().context("library not available")?;
            let report = library::add_to_library(lib, b)?;
            self.lib.tex_dirty = true;
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

    /// Removes assets from the library right away (after the confirmation dialog).
    pub fn remove_from_library(&mut self, assets: &[AssetRef]) {
        let r = match self.lib.lib.as_mut() {
            Some(lib) => library::remove_assets(lib, assets).map(|_| format!("Removed {} asset(s) from the library", assets.len())),
            None => Ok(String::new()),
        };
        if r.is_ok() {
            self.lib.selected.clear();
            self.lib.tex_dirty = true;
        }
        self.show_result(r);
    }

    /// The "Remove from library?" dialog: lists the assets and what still uses them.
    fn confirm_remove_dialog(&mut self, ctx: &egui::Context) {
        let Some(list) = self.lib.confirm_remove.clone() else { return };
        let Some(lib) = &self.lib.lib else {
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

    pub fn library_ui(&mut self, ctx: &egui::Context) {
        self.confirm_remove_dialog(ctx);
        self.lib.ensure_loaded();
        self.lib.textures(ctx);
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.heading("Asset library");
                ui.weak(library::library_dir().display().to_string());
            });
            if let Some(e) = &self.lib.error {
                ui.colored_label(Color32::LIGHT_RED, e);
                if ui.button("Retry").clicked() {
                    self.lib.reload();
                }
                return;
            }
            let sel: Vec<AssetRef> = self.lib.selected.iter().cloned().collect();
            let any = !sel.is_empty();
            ui.horizontal_wrapped(|ui| {
                if ui.add_enabled(any, egui::Button::new("➕ Add to project")).on_hover_text("Copies the selected assets and everything they need into the project").clicked() {
                    let r = self.library_to_project(&sel);
                    self.show_result(r);
                }
                if ui.add_enabled(any, egui::Button::new("📦 Export package…")).clicked() {
                    self.open_file_dialog(Purpose::ExportPackage { from_library: true, assets: sel.clone() });
                }
                if ui.button("📥 Import package…").on_hover_text("Adds a .r2dpack file to the library").clicked() {
                    self.open_file_dialog(Purpose::ImportPackage { into_library: true });
                }
                if ui.add_enabled(any, egui::Button::new("🗑 Remove from library…")).clicked() {
                    self.lib.confirm_remove = Some(sel.clone());
                }
                ui.separator();
                if ui.button("✨ New asset…").on_hover_text("Create a prop, item, character, tileset or sprite sheet in the project (draw it in the pixel editor)").clicked() {
                    self.wizard = Some(crate::wizard::Wizard::new(&self.project));
                }
                if ui.button("⟳ Reload").clicked() {
                    self.lib.reload();
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
            let Some(lib) = &self.lib.lib else { return };
            let q = self.lib.search.trim().to_lowercase();
            let shown: Vec<AssetRef> = library::all_assets(&lib.db)
                .into_iter()
                .filter(|a| self.lib.filter.is_none_or(|k| k == a.0))
                .filter(|a| q.is_empty() || a.1.to_lowercase().contains(&q) || self.lib.name_of(a).to_lowercase().contains(&q))
                .collect();
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
            let mut toggle = None;
            egui::ScrollArea::vertical().id_salt("lib_grid").show(ui, |ui| {
                ui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().item_spacing = vec2(8.0, 8.0);
                    for a in &shown {
                        let (rect, resp) = ui.allocate_exact_size(vec2(118.0, 112.0), Sense::click());
                        let on = self.lib.selected.contains(a);
                        let bg = if on { Color32::from_rgb(52, 64, 90) } else if resp.hovered() { Color32::from_gray(48) } else { Color32::from_gray(38) };
                        ui.painter().rect_filled(rect, 4.0, bg);
                        ui.painter().rect_stroke(rect, 4.0, Stroke::new(if on { 2.0_f32 } else { 1.0 }, if on { Color32::YELLOW } else { Color32::from_gray(60) }), StrokeKind::Inside);
                        let thumb = Rect::from_min_size(rect.min + vec2(23.0, 6.0), vec2(72.0, 64.0));
                        self.lib.paint_thumb(ui, a, thumb);
                        let name = self.lib.name_of(a);
                        let short: String = if name.chars().count() > 16 { name.chars().take(15).collect::<String>() + "…" } else { name.clone() };
                        ui.painter().text(pos2(rect.center().x, rect.min.y + 84.0), egui::Align2::CENTER_CENTER, short, egui::FontId::proportional(13.0), Color32::WHITE);
                        ui.painter().text(pos2(rect.center().x, rect.min.y + 100.0), egui::Align2::CENTER_CENTER, kind_label(a.0), egui::FontId::proportional(11.0), Color32::GRAY);
                        let deps: Vec<String> = library::deps(&lib.db, a).into_iter().map(|d| format!("{} {}", kind_label(d.0).to_lowercase(), d.1)).collect();
                        let tip = if deps.is_empty() { format!("{name}\nid: {}", a.1) } else { format!("{name}\nid: {}\nalso brings: {}", a.1, deps.join(", ")) };
                        if resp.on_hover_text(tip).clicked() {
                            toggle = Some(a.clone());
                        }
                    }
                });
            });
            if let Some(a) = toggle {
                if !self.lib.selected.remove(&a) {
                    self.lib.selected.insert(a);
                }
            }
        });
    }
}
