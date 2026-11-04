//! Character model chooser: pick the look (sprite sheet) of a character definition or of a
//! single placed NPC — from the project's character sheets or from the asset library. A
//! library model is copied into the project (just its sheet) when it is chosen.

use std::cell::RefCell;

use anyhow::{Context, Result};
use egui_macroquad::egui::{self, pos2, vec2, Color32, Rect, Sense, Stroke, StrokeKind};
use rust2d_engine::defs::SpriteRef;
use rust2d_engine::library::Bundle;
use rust2d_engine::project::TILE_SIZE;
use rust2d_engine::refactor::RefKind;

use crate::app::Editor;
use crate::doc::Objects;
use crate::widgets;

/// What gets the new model.
#[derive(Clone, Debug, PartialEq)]
pub enum ModelTarget {
    /// A character definition (every instance without its own model).
    Def(String),
    /// One placed character on the open map (entity uid).
    Instance(u32),
    /// A look of the character creator (index; past the end = add a new one).
    Look(usize),
}

/// A model to use.
#[derive(Clone, Debug, PartialEq)]
pub enum ModelPick {
    /// A sprite sheet of the project.
    Project(String),
    /// A character of the asset library (its sheet is copied into the project).
    Library(String),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Gender {
    All,
    Male,
    Female,
}

pub struct ModelChooser {
    target: ModelTarget,
    library: bool,
    search: String,
    gender: Gender,
    pick: Option<ModelPick>,
}

thread_local! {
    static REQUEST: RefCell<Option<ModelTarget>> = const { RefCell::new(None) };
}

/// Opens the model chooser for `target` (called from forms that can't reach the editor).
pub fn request_model(target: ModelTarget) {
    REQUEST.with(|r| *r.borrow_mut() = Some(target));
}

/// Library characters count as female when their id ends in `_f` or the name says so
/// ("(female)", "Lady", "Heroine", ...).
fn is_female(id: &str, name: &str) -> bool {
    let n = name.to_lowercase();
    let words = ["female", "lady", "woman", "girl", "queen", "princess", "heroine", "sorceress", "witch", "priestess"];
    id.ends_with("_f") || id.contains("_f_") || words.iter().any(|w| n.split(|c: char| !c.is_alphanumeric()).any(|x| x == *w))
}

const THUMB: f32 = 64.0;

impl Editor {
    /// The sheet the target currently shows.
    fn model_of(&self, t: &ModelTarget) -> Option<String> {
        match t {
            ModelTarget::Def(id) => self.project.db.character(id).map(|d| d.sheet.clone()),
            ModelTarget::Instance(uid) => {
                let e = self.doc.as_ref()?.map.entities.iter().find(|e| e.uid == *uid)?;
                if e.sheet.is_empty() {
                    self.project.db.character(&e.def).map(|d| d.sheet.clone())
                } else {
                    Some(e.sheet.clone())
                }
            }
            ModelTarget::Look(i) => self.project.config.creation.looks.get(*i).map(|l| l.sheet.clone()),
        }
    }

    /// Project sheets that are character models (used by a character or a placed NPC).
    fn project_models(&self) -> Vec<String> {
        let mut v: Vec<String> = self.project.db.characters.iter().map(|c| c.sheet.clone()).collect();
        if let Some(doc) = &self.doc {
            v.extend(doc.map.entities.iter().filter(|e| !e.sheet.is_empty()).map(|e| e.sheet.clone()));
        }
        v.retain(|s| !s.is_empty() && self.assets.sheets.contains_key(s));
        v.sort();
        v.dedup();
        v
    }

    /// Gives `target` the model `pick`; library models are copied into the project first.
    pub fn apply_model(&mut self, target: &ModelTarget, pick: &ModelPick) -> Result<String> {
        let (sheet, frames, label, female) = match pick {
            ModelPick::Project(s) => {
                self.project.db.sheet(s).with_context(|| format!("no sprite sheet '{s}' in the project"))?;
                (s.clone(), None, s.clone(), is_female(s, s))
            }
            ModelPick::Library(cid) => {
                self.lib.ensure_loaded();
                let lib = self.lib.lib.as_ref().context("the asset library is not available")?;
                let c = lib.db.character(cid).with_context(|| format!("no character '{cid}' in the library"))?.clone();
                let mut b = Bundle::collect(&lib.db, &lib.root, TILE_SIZE, "library", &[(RefKind::Sheet, c.sheet.clone())])?;
                b.rescale(TILE_SIZE)?;
                let root = self.project.root.clone();
                let report = b.merge_into(&mut self.project.db, &root)?;
                // The id may have been taken by a different sheet.
                let id = report.renamed.iter().find(|(k, old, _)| *k == RefKind::Sheet && *old == c.sheet).map_or(c.sheet.clone(), |r| r.2.clone());
                if !report.added.is_empty() {
                    self.defs_dirty = true;
                }
                (id, Some(c.walk_frames), c.name.clone(), is_female(&c.id, &c.name))
            }
        };
        let msg = match target {
            ModelTarget::Def(id) => {
                let d = self.project.db.characters.iter_mut().find(|d| d.id == *id).with_context(|| format!("no character '{id}'"))?;
                d.sheet = sheet;
                if let Some(f) = frames {
                    d.walk_frames = f;
                }
                self.defs_dirty = true;
                format!("{id} now looks like {label}. Save the definitions to keep it.")
            }
            ModelTarget::Look(i) => {
                use rust2d_engine::{project::PlayerLook, state::Sex};
                let looks = &mut self.project.config.creation.looks;
                match looks.get_mut(*i) {
                    Some(l) => l.sheet = sheet,
                    None => looks.push(PlayerLook { sheet, sex: if female { Sex::Female } else { Sex::Male }, label: label.clone() }),
                }
                self.defs_dirty = true;
                format!("Character creator look: {label}. Save the definitions to keep it.")
            }
            ModelTarget::Instance(uid) => {
                let doc = self.doc.as_mut().context("no map open")?;
                let before = Objects::of(&doc.map);
                let e = doc.map.entities.iter_mut().find(|e| e.uid == *uid).with_context(|| format!("no entity #{uid}"))?;
                let def_sheet = self.project.db.character(&e.def).map(|d| d.sheet.clone()).unwrap_or_default();
                e.sheet = if sheet == def_sheet { String::new() } else { sheet };
                doc.record_objects(before, "model");
                format!("#{uid} now looks like {label}.")
            }
        };
        self.reload_assets();
        Ok(msg)
    }

    /// Shows the model chooser window (opened via [`request_model`]).
    pub fn model_window(&mut self, ctx: &egui::Context) {
        if let Some(t) = REQUEST.with(|r| r.borrow_mut().take()) {
            self.model = Some(ModelChooser { target: t, library: false, search: String::new(), gender: Gender::All, pick: None });
        }
        let Some(mut st) = self.model.take() else { return };
        if st.library || matches!(st.pick, Some(ModelPick::Library(_))) {
            self.lib.ensure_loaded();
            self.lib.textures(ctx);
        }
        let title = match &st.target {
            ModelTarget::Def(id) => format!("Model of '{id}'"),
            ModelTarget::Instance(uid) => format!("Model of #{uid} (only this one)"),
            ModelTarget::Look(_) => "Look for the character creator".to_string(),
        };
        let current = self.model_of(&st.target).unwrap_or_default();
        let mut open = true;
        let mut done: Option<bool> = None;
        egui::Window::new(title).id(egui::Id::new("model_chooser")).open(&mut open).collapsible(false).default_size(vec2(760.0, 560.0)).show(ctx, |ui| {
            // Preview of the pick (or the current model) in all four directions + buttons.
            ui.horizontal(|ui| {
                let shown = st.pick.clone().unwrap_or_else(|| ModelPick::Project(current.clone()));
                for row in 0..4u32 {
                    let (r, _) = ui.allocate_exact_size(vec2(56.0, 72.0), Sense::hover());
                    ui.painter().rect_filled(r, 3.0, Color32::from_gray(45));
                    self.paint_model(ui, &shown, row, r.shrink(3.0));
                }
                ui.vertical(|ui| {
                    ui.label(match &st.pick {
                        None => format!("Current: {current}"),
                        Some(ModelPick::Project(s)) => format!("Project sheet: {s}"),
                        Some(ModelPick::Library(c)) => format!("Library: {} (copied into the project)", self.lib_char_name(c)),
                    });
                    ui.horizontal(|ui| {
                        if ui.add_enabled(st.pick.is_some(), egui::Button::new("\u{E182} Use this model")).clicked() {
                            done = Some(true);
                        }
                        if ui.button("Cancel").clicked() {
                            done = Some(false);
                        }
                        if let ModelTarget::Instance(uid) = st.target {
                            let custom = self.doc.as_ref().and_then(|d| d.map.entities.iter().find(|e| e.uid == uid)).is_some_and(|e| !e.sheet.is_empty());
                            if custom && ui.button("Use the definition's model").clicked() {
                                if let Some(doc) = &mut self.doc {
                                    let before = Objects::of(&doc.map);
                                    if let Some(e) = doc.map.entities.iter_mut().find(|e| e.uid == uid) {
                                        e.sheet.clear();
                                    }
                                    doc.record_objects(before, "model");
                                }
                                done = Some(false);
                            }
                        }
                    });
                });
            });
            ui.separator();
            ui.horizontal(|ui| {
                ui.selectable_value(&mut st.library, false, "This project");
                ui.selectable_value(&mut st.library, true, "Asset library");
                ui.separator();
                ui.add(egui::TextEdit::singleline(&mut st.search).hint_text("\u{E30C} search").desired_width(160.0));
                if st.library {
                    ui.selectable_value(&mut st.gender, Gender::All, "All");
                    ui.selectable_value(&mut st.gender, Gender::Male, "Male");
                    ui.selectable_value(&mut st.gender, Gender::Female, "Female");
                }
            });
            let q = st.search.to_lowercase();
            // (pick, label, tooltip)
            let entries: Vec<(ModelPick, String, String)> = if st.library {
                match &self.lib.lib {
                    None => {
                        ui.colored_label(Color32::LIGHT_RED, self.lib.error.clone().unwrap_or_else(|| "The asset library is not available.".into()));
                        Vec::new()
                    }
                    Some(lib) => {
                        let mut v: Vec<_> = lib
                            .db
                            .characters
                            .iter()
                            .filter(|c| match st.gender {
                                Gender::All => true,
                                Gender::Male => !is_female(&c.id, &c.name),
                                Gender::Female => is_female(&c.id, &c.name),
                            })
                            .filter(|c| q.is_empty() || c.name.to_lowercase().contains(&q) || c.id.contains(&q))
                            .map(|c| {
                                let in_project = self.project.db.sheet(&c.sheet).is_some();
                                let tip = format!("{} ({})\nsheet: {}{}", c.name, c.id, c.sheet, if in_project { "\nalready in the project" } else { "" });
                                (ModelPick::Library(c.id.clone()), c.name.clone(), tip)
                            })
                            .collect();
                        v.sort_by(|a, b| a.1.cmp(&b.1));
                        v
                    }
                }
            } else {
                self.project_models()
                    .into_iter()
                    .filter(|s| q.is_empty() || s.to_lowercase().contains(&q))
                    .map(|s| {
                        let users: Vec<String> = self.project.db.characters.iter().filter(|c| c.sheet == s).map(|c| c.name.clone()).collect();
                        let tip = if users.is_empty() { s.clone() } else { format!("{s}\nused by: {}", users.join(", ")) };
                        (ModelPick::Project(s.clone()), s, tip)
                    })
                    .collect()
            };
            if entries.is_empty() && (self.lib.lib.is_some() || !st.library) {
                ui.weak("Nothing matches.");
            }
            egui::ScrollArea::vertical().id_salt("model_grid").max_height(380.0).show(ui, |ui| {
                ui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().item_spacing = vec2(6.0, 6.0);
                    for (pick, label, tip) in &entries {
                        let (r, resp) = ui.allocate_exact_size(vec2(THUMB + 20.0, THUMB + 40.0), Sense::click());
                        let img = Rect::from_min_size(r.min + vec2(10.0, 2.0), vec2(THUMB, THUMB + 16.0));
                        let on = st.pick.as_ref() == Some(pick);
                        let is_current = matches!(pick, ModelPick::Project(s) if *s == current);
                        ui.painter().rect_filled(r, 4.0, if resp.hovered() { Color32::from_gray(70) } else { Color32::from_gray(48) });
                        self.paint_model(ui, pick, 0, img);
                        let label = label.replace(" (female)", " (f)").replace(" (male)", " (m)");
                        let short: String = label.chars().take(15).collect();
                        ui.painter().text(pos2(r.center().x, r.max.y - 10.0), egui::Align2::CENTER_CENTER, short, egui::FontId::proportional(12.0), Color32::LIGHT_GRAY);
                        if on {
                            ui.painter().rect_stroke(r, 4.0, Stroke::new(2.0_f32, Color32::YELLOW), StrokeKind::Inside);
                        } else if is_current {
                            ui.painter().rect_stroke(r, 4.0, Stroke::new(1.5_f32, Color32::from_rgb(120, 200, 255)), StrokeKind::Inside);
                        }
                        let resp = resp.on_hover_text(tip);
                        if resp.clicked() || resp.double_clicked() {
                            st.pick = Some(pick.clone());
                        }
                        if resp.double_clicked() {
                            done = Some(true);
                        }
                    }
                });
            });
            ui.weak("Click to preview, double-click (or \u{E182}) to use. Library models are copied into the project's sprite sheets.");
        });
        match done {
            Some(true) => {
                if let Some(p) = st.pick.clone() {
                    let r = self.apply_model(&st.target, &p);
                    self.show_result(r);
                }
            }
            Some(false) => {}
            None if open => self.model = Some(st),
            None => {}
        }
    }

    /// Script hook: switch the open chooser to the library (or project) tab and preview a model.
    pub fn model_script(&mut self, library: bool, pick: Option<ModelPick>) {
        if let Some(t) = REQUEST.with(|r| r.borrow_mut().take()) {
            self.model = Some(ModelChooser { target: t, library: false, search: String::new(), gender: Gender::All, pick: None });
        }
        if let Some(st) = &mut self.model {
            st.library = library;
            if pick.is_some() {
                st.pick = pick;
            }
        }
    }

    /// Script hook: use the previewed model of the open chooser.
    pub fn model_script_use(&mut self) {
        let Some(st) = self.model.take() else { return };
        if let Some(p) = st.pick {
            let r = self.apply_model(&st.target, &p);
            self.show_result(r);
        }
    }

    fn lib_char_name(&self, id: &str) -> String {
        self.lib.lib.as_ref().and_then(|l| l.db.character(id)).map_or(id.to_string(), |c| c.name.clone())
    }

    /// Paints the idle frame of direction `row` (0 down, 1 left, 2 right, 3 up) of a model.
    fn paint_model(&self, ui: &egui::Ui, pick: &ModelPick, row: u32, rect: Rect) {
        match pick {
            ModelPick::Project(s) => {
                let frames = self.project.db.characters.iter().find(|c| c.sheet == *s).map_or(4, |c| c.walk_frames.max(1));
                widgets::paint_frame(ui, &self.tex, &self.assets, s, row * frames, rect);
            }
            ModelPick::Library(c) => {
                if let Some(d) = self.lib.lib.as_ref().and_then(|l| l.db.character(c)) {
                    self.lib.paint_sprite(ui, &SpriteRef::new(&d.sheet, row * d.walk_frames.max(1)), rect);
                }
            }
        }
    }
}
