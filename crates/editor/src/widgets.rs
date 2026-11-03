//! Reusable egui widgets: textures, sprite pickers and the trigger editor.

use std::collections::HashMap;

use egui_macroquad::egui::{self, pos2, vec2, Color32, Rect, Sense, Stroke, StrokeKind, TextureHandle, Ui};
use rust2d_engine::defs::{ContainerSpec, LockSpec, OnUnlock, PropDef, SpriteRef};
use rust2d_engine::inventory::ItemStack;
use rust2d_engine::entity::Facing;
use rust2d_engine::events::{Action, CmpOp, Cond, Condition, Trigger, TriggerOn, VarOp};
use rust2d_engine::render::Assets;

/// egui copies of tileset and sprite sheet images.
#[derive(Default)]
pub struct Textures {
    pub tilesets: HashMap<String, TextureHandle>,
    pub sheets: HashMap<String, TextureHandle>,
}

impl Textures {
    pub fn build(ctx: &egui::Context, assets: &Assets) -> Self {
        let load = |name: &str, img: &macroquad::texture::Image| {
            let ci = egui::ColorImage::from_rgba_unmultiplied([img.width as usize, img.height as usize], &img.bytes);
            ctx.load_texture(name, ci, egui::TextureOptions::NEAREST)
        };
        Textures {
            tilesets: assets.tilesets.iter().map(|(id, t)| (id.clone(), load(&format!("ts:{id}"), &t.image))).collect(),
            sheets: assets.sheets.iter().map(|(id, s)| (id.clone(), load(&format!("sh:{id}"), &s.image))).collect(),
        }
    }
}

/// Paints one frame of a sheet into `rect`.
pub fn paint_frame(ui: &Ui, tex: &Textures, assets: &Assets, sheet: &str, frame: u32, rect: Rect) -> bool {
    paint_sprite(ui, tex, assets, &SpriteRef::new(sheet, frame), rect)
}

/// Paints a (possibly multi-cell) sprite into `rect`, keeping its aspect ratio, bottom-centered.
pub fn paint_sprite(ui: &Ui, tex: &Textures, assets: &Assets, sprite: &SpriteRef, rect: Rect) -> bool {
    let (Some(t), Some(s)) = (tex.sheets.get(&sprite.sheet), assets.sheets.get(&sprite.sheet)) else { return false };
    let (cw, ch) = sprite.cells();
    let src = s.region(sprite.frame, cw, ch);
    let (w, h) = (s.image.width as f32, s.image.height as f32);
    let uv = Rect::from_min_max(pos2(src.x / w, src.y / h), pos2((src.x + src.w) / w, (src.y + src.h) / h));
    // Keep aspect ratio, bottom-centered.
    let scale = (rect.width() / src.w).min(rect.height() / src.h);
    let size = vec2(src.w * scale, src.h * scale);
    let r = Rect::from_min_size(pos2(rect.center().x - size.x / 2.0, rect.max.y - size.y), size);
    ui.painter().image(t.id(), r, uv, Color32::WHITE);
    true
}

/// A sprite thumbnail as a widget (multi-cell sprites are scaled to fit).
pub fn sprite_thumb(ui: &mut Ui, tex: &Textures, assets: &Assets, sprite: &SpriteRef, size: f32, selected: bool) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(vec2(size, size), Sense::click());
    let bg = if selected { Color32::from_rgb(70, 90, 140) } else { Color32::from_gray(40) };
    ui.painter().rect_filled(rect, 2.0, bg);
    if !paint_sprite(ui, tex, assets, sprite, rect.shrink(2.0)) {
        ui.painter().text(rect.center(), egui::Align2::CENTER_CENTER, "?", egui::FontId::default(), Color32::RED);
    }
    if resp.hovered() {
        ui.painter().rect_stroke(rect, 2.0, Stroke::new(1.0_f32, Color32::WHITE), StrokeKind::Inside);
    }
    let (w, h) = sprite.cells();
    resp.on_hover_text(format!("frame {} ({w}×{h} cells)", sprite.frame))
}

/// A frame thumbnail as a widget.
pub fn frame_thumb(ui: &mut Ui, tex: &Textures, assets: &Assets, sheet: &str, frame: u32, size: f32, selected: bool) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(vec2(size, size), Sense::click());
    let bg = if selected { Color32::from_rgb(70, 90, 140) } else { Color32::from_gray(40) };
    ui.painter().rect_filled(rect, 2.0, bg);
    if !paint_frame(ui, tex, assets, sheet, frame, rect.shrink(2.0)) {
        ui.painter().text(rect.center(), egui::Align2::CENTER_CENTER, "?", egui::FontId::default(), Color32::RED);
    }
    if resp.hovered() {
        ui.painter().rect_stroke(rect, 2.0, Stroke::new(1.0_f32, Color32::WHITE), StrokeKind::Inside);
    }
    resp.on_hover_text(format!("frame {frame}"))
}

/// Sheet combo + frame grid.
/// Editor for a key lock: key item, consumption, effect and messages.
/// `prop`: the prop definition (its states are offered as "switch to" targets and previewed).
#[allow(clippy::too_many_arguments)]
pub fn lock_ui(ui: &mut Ui, id: &str, lock: &mut LockSpec, refs: &Refs, tex: &Textures, assets: &Assets, db: &rust2d_engine::defs::Database, prop: Option<&PropDef>) {
    let sprite = prop.map(|p| &p.sprite);
    ui.horizontal(|ui| {
        ui.label("Key item");
        let (rect, _) = ui.allocate_exact_size(vec2(22.0, 22.0), Sense::hover());
        if let Some(d) = db.item(&lock.key) {
            paint_frame(ui, tex, assets, &d.icon.sheet, d.icon.frame, rect);
        }
        suggest(ui, &format!("{id}/key"), &mut lock.key, &refs.items, 120.0);
        if lock.key.is_empty() {
            ui.weak("(no key: opens when used, e.g. doors)");
        } else if db.item(&lock.key).is_none() {
            ui.colored_label(Color32::LIGHT_RED, "unknown item");
        }
    });
    ui.add_enabled_ui(!lock.key.is_empty(), |ui| {
        ui.horizontal(|ui| {
            ui.label("On unlocking the key");
            ui.radio_value(&mut lock.consume_key, true, "is used up");
            ui.radio_value(&mut lock.consume_key, false, "stays in the inventory");
        });
    });
    ui.horizontal(|ui| {
        ui.label("When unlocked");
        let mut k = match lock.on_unlock {
            OnUnlock::Nothing => 0,
            OnUnlock::Disappear => 1,
            OnUnlock::Open { .. } => 3,
            OnUnlock::ToState { .. } => 2,
        };
        let old = k;
        ui.radio_value(&mut k, 0, "stays (chests open)");
        ui.radio_value(&mut k, 1, "disappears");
        ui.radio_value(&mut k, 2, "switches state (doors, gates)");
        if old == 3 {
            ui.radio_value(&mut k, 3, "old frame offset");
        }
        if k != old {
            lock.on_unlock = match k {
                0 => OnUnlock::Nothing,
                1 => OnUnlock::Disappear,
                3 => OnUnlock::Open { frame: 1 },
                _ => {
                    // The first non-default state, ideally one called "open".
                    let names = prop.map(|p| p.state_names()).unwrap_or_default();
                    let pick = names.iter().skip(1).find(|n| n.contains("open")).or(names.get(1)).cloned().unwrap_or_default();
                    OnUnlock::ToState { state: pick }
                }
            };
        }
    });
    if let OnUnlock::ToState { state } = &mut lock.on_unlock {
        let names = prop.map(|p| p.state_names()).unwrap_or_default();
        ui.horizontal(|ui| {
            ui.label("Switch to state");
            egui::ComboBox::from_id_salt(format!("{id}/to_state")).selected_text(if state.is_empty() { "(choose)" } else { state.as_str() }).show_ui(ui, |ui| {
                for n in &names {
                    ui.selectable_value(state, n.clone(), n);
                }
            });
            if names.len() < 2 {
                ui.colored_label(Color32::YELLOW, "The prop has only one state: add an 'open' state in Database → Props.");
            }
        });
        if let Some(p) = prop {
            let target = p.state_index(state).map(|i| p.state_look(i));
            ui.horizontal(|ui| {
                let looks = [(p.default_state.as_str(), Some((&p.sprite, p.solid))), (state.as_str(), target)];
                for (label, look) in looks {
                    ui.vertical(|ui| {
                        let (r, _) = ui.allocate_exact_size(vec2(56.0, 56.0), Sense::hover());
                        ui.painter().rect_filled(r, 3.0, Color32::from_gray(45));
                        if !look.is_some_and(|(sp, _)| paint_sprite(ui, tex, assets, sp, r.shrink(3.0))) {
                            ui.painter().text(r.center(), egui::Align2::CENTER_CENTER, "?", egui::FontId::proportional(20.0), Color32::GRAY);
                        }
                        ui.weak(label);
                    });
                }
                if target.is_some_and(|(_, solid)| solid) {
                    ui.colored_label(Color32::YELLOW, "This state blocks movement: the opened prop can't be walked through.");
                }
            });
        }
    }
    if let OnUnlock::Open { frame } = &mut lock.on_unlock {
        ui.horizontal(|ui| {
            ui.label("Open frame offset");
            ui.add(egui::DragValue::new(frame).range(0..=255));
            ui.weak("(also stops blocking movement)");
        });
        // Closed / open preview, so a wrong offset (another prop's picture, an empty cell) shows.
        if let Some(s) = sprite {
            let open = SpriteRef { frame: s.frame + *frame, ..s.clone() };
            ui.horizontal(|ui| {
                for (label, sp) in [("closed", s), ("open", &open)] {
                    ui.vertical(|ui| {
                        let (r, _) = ui.allocate_exact_size(vec2(56.0, 56.0), Sense::hover());
                        ui.painter().rect_filled(r, 3.0, Color32::from_gray(45));
                        if !paint_sprite(ui, tex, assets, sp, r.shrink(3.0)) {
                            ui.painter().text(r.center(), egui::Align2::CENTER_CENTER, "?", egui::FontId::proportional(20.0), Color32::GRAY);
                        }
                        ui.weak(label);
                    });
                }
                let empty = assets.sheets.get(&s.sheet).is_some_and(|sh| {
                    let r = sh.region(open.frame, open.cells().0, open.cells().1);
                    let (x0, y0) = (r.x as u32, r.y as u32);
                    let (w, h) = (sh.image.width as u32, sh.image.height as u32);
                    x0 >= w || y0 >= h || (0..r.h as u32).all(|y| (0..r.w as u32).all(|x| x0 + x >= w || y0 + y >= h || sh.image.get_pixel(x0 + x, y0 + y).a == 0.0))
                });
                if empty {
                    ui.colored_label(Color32::YELLOW, "The open frame is empty: draw it in the pixel editor or pick another offset.");
                }
            });
        }
    }
    let disappears = lock.on_unlock == OnUnlock::Disappear;
    ui.add_enabled_ui(!disappears, |ui| {
        let label = if lock.key.is_empty() { "Can be closed again" } else { "Can be locked again with the key" };
        ui.checkbox(&mut lock.relock, label).on_hover_text(
            "Interacting with the opened prop (or using the key on it) locks it again if the player carries the key. \
             Containers are only relocked by using the key on them. 'Locked again' triggers run afterwards.",
        );
        if disappears {
            ui.weak("(not possible for props that disappear)");
        } else if lock.relock && lock.consume_key && !lock.key.is_empty() {
            ui.colored_label(Color32::YELLOW, "The key is used up on unlocking: relocking needs another key.");
        }
    });
    ui.horizontal(|ui| {
        ui.label("Locked message");
        ui.add(egui::TextEdit::singleline(&mut lock.locked_text).hint_text("It's locked. You need the <key>.").desired_width(240.0));
    });
    ui.horizontal(|ui| {
        ui.label("Unlock message");
        ui.add(egui::TextEdit::singleline(&mut lock.unlock_text).hint_text("Unlocked with the <key>.").desired_width(240.0));
    });
    if lock.can_relock() {
        ui.horizontal(|ui| {
            ui.label("Relock message");
            let hint = if lock.key.is_empty() { "Closed." } else { "Locked with the <key>." };
            ui.add(egui::TextEdit::singleline(&mut lock.lock_text).hint_text(hint).desired_width(240.0));
        });
    }
    ui.weak("Interacting (or using the key on it) checks the inventory for the key. 'Unlocked' triggers run when it opens, 'Locked again' triggers when it is relocked.");
}

/// Editor for a container: size, starting contents, lock conditions and open frame.
/// `states`: the prop's state names (for the "when opened" state).
#[allow(clippy::too_many_arguments)]
pub fn container_ui(ui: &mut Ui, id: &str, spec: &mut ContainerSpec, refs: &Refs, tex: &Textures, assets: &Assets, db: &rust2d_engine::defs::Database, states: &[String]) {
    ui.horizontal(|ui| {
        ui.label("Slots");
        ui.add(egui::DragValue::new(&mut spec.size).range(1..=64));
        ui.label("when opened, switch to");
        let label = if spec.open_state.is_empty() { "(no change)".to_string() } else { spec.open_state.clone() };
        egui::ComboBox::from_id_salt(format!("{id}/open_state")).selected_text(label).show_ui(ui, |ui| {
            ui.selectable_value(&mut spec.open_state, String::new(), "(no change)");
            for s in states {
                ui.selectable_value(&mut spec.open_state, s.clone(), s);
            }
        });
        if !spec.open_state.is_empty() {
            spec.open_frame = 0;
        } else if spec.open_frame > 0 {
            ui.weak(format!("(old frame offset {})", spec.open_frame));
        }
    });
    ui.label("Contents (filled into the slots in order; big counts split by the item's stack size):");
    let mut remove = None;
    for (k, s) in spec.items.iter_mut().enumerate() {
        ui.horizontal(|ui| {
            let (rect, _) = ui.allocate_exact_size(vec2(22.0, 22.0), Sense::hover());
            if let Some(d) = db.item(&s.item) {
                paint_frame(ui, tex, assets, &d.icon.sheet, d.icon.frame, rect);
            }
            ui.add(egui::DragValue::new(&mut s.count).range(1..=9999));
            ui.label("x");
            suggest(ui, &format!("{id}/c{k}"), &mut s.item, &refs.items, 110.0);
            if !s.item.is_empty() && db.item(&s.item).is_none() {
                ui.colored_label(Color32::LIGHT_RED, "unknown item");
            }
            if ui.small_button("✖").clicked() {
                remove = Some(k);
            }
        });
    }
    if let Some(k) = remove {
        spec.items.remove(k);
    }
    ui.horizontal(|ui| {
        if ui.button("+ Item").clicked() {
            spec.items.push(ItemStack::new(refs.items.first().map_or("", |s| s.as_str()), 1));
        }
        // Slots needed vs. available.
        let needed: u32 = spec.items.iter().map(|s| s.count.div_ceil(db.item(&s.item).map_or(99, |d| d.max_stack.max(1)))).sum();
        if needed > spec.size {
            ui.colored_label(Color32::LIGHT_RED, format!("needs {needed} slots, only {} — extra items are dropped", spec.size));
        } else {
            ui.weak(format!("{needed}/{} slots used", spec.size));
        }
    });
    ui.label("Opens only if all of:");
    ui.indent(format!("{id}/cond"), |ui| conditions_ui(ui, &format!("{id}/cond"), &mut spec.conditions, refs));
    ui.horizontal(|ui| {
        ui.label("Locked message");
        ui.add(egui::TextEdit::singleline(&mut spec.locked_text).hint_text("It's locked.").desired_width(220.0));
    });
}

pub fn sprite_picker(ui: &mut Ui, id: &str, sprite: &mut SpriteRef, tex: &Textures, assets: &Assets) {
    let mut sheets: Vec<&String> = assets.sheets.keys().collect();
    sheets.sort();
    ui.horizontal(|ui| {
        ui.label("Sheet");
        egui::ComboBox::from_id_salt(format!("{id}/sheet"))
            .selected_text(sprite.sheet.as_str())
            .show_ui(ui, |ui| {
                for s in sheets {
                    ui.selectable_value(&mut sprite.sheet, s.clone(), s.as_str());
                }
            });
        ui.label("Frame");
        ui.add(egui::DragValue::new(&mut sprite.frame).range(0..=4096));
        ui.label("Size (cells)");
        if sprite.w == 0 {
            sprite.w = 1;
        }
        if sprite.h == 0 {
            sprite.h = 1;
        }
        ui.add(egui::DragValue::new(&mut sprite.w).range(1..=16));
        ui.label("×");
        ui.add(egui::DragValue::new(&mut sprite.h).range(1..=16));
    });
    region_picker(ui, id, sprite, tex, assets);
}

/// The whole sheet with a grid: click a cell, or drag over several cells for a multi-cell sprite.
pub fn region_picker(ui: &mut Ui, id: &str, sprite: &mut SpriteRef, tex: &Textures, assets: &Assets) {
    let (Some(t), Some(s)) = (tex.sheets.get(&sprite.sheet), assets.sheets.get(&sprite.sheet)) else {
        ui.colored_label(Color32::LIGHT_RED, "unknown sheet");
        return;
    };
    ui.weak("Click a cell, or drag over several cells for a bigger sprite (e.g. 2×2 gate, 3×4 tree).");
    let rows = s.count.div_ceil(s.columns.max(1));
    let scale = ((ui.available_width() - 8.0) / s.image.width as f32).clamp(1.0, 3.0).floor();
    let (cw, ch) = (s.frame_w * scale, s.frame_h * scale);
    let size = vec2(s.image.width as f32 * scale, s.image.height as f32 * scale);
    egui::ScrollArea::both().id_salt(format!("{id}/region")).max_height(320.0).show(ui, |ui| {
        let (rect, resp) = ui.allocate_exact_size(size, Sense::click_and_drag());
        let p = ui.painter_at(rect);
        p.rect_filled(rect, 0.0, Color32::from_gray(30));
        p.image(t.id(), rect, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
        for c in 1..s.columns {
            let x = rect.min.x + c as f32 * cw;
            p.line_segment([pos2(x, rect.min.y), pos2(x, rect.max.y)], Stroke::new(1.0_f32, Color32::from_white_alpha(25)));
        }
        for r in 1..rows {
            let y = rect.min.y + r as f32 * ch;
            p.line_segment([pos2(rect.min.x, y), pos2(rect.max.x, y)], Stroke::new(1.0_f32, Color32::from_white_alpha(25)));
        }
        let cell_at = |pos: egui::Pos2| {
            let rel = pos - rect.min;
            (((rel.x / cw).floor().max(0.0) as u32).min(s.columns - 1), ((rel.y / ch).floor().max(0.0) as u32).min(rows - 1))
        };
        let drag_id = egui::Id::new(format!("{id}/region_start"));
        if resp.drag_started() || resp.clicked() {
            if let Some(pos) = resp.interact_pointer_pos() {
                ui.memory_mut(|m| m.data.insert_temp(drag_id, cell_at(pos)));
            }
        }
        if resp.dragged() || resp.clicked() {
            if let (Some(pos), Some(start)) = (resp.interact_pointer_pos(), ui.memory(|m| m.data.get_temp::<(u32, u32)>(drag_id))) {
                let end = cell_at(pos);
                let (x0, y0) = (start.0.min(end.0), start.1.min(end.1));
                sprite.frame = y0 * s.columns + x0;
                sprite.w = start.0.abs_diff(end.0) + 1;
                sprite.h = start.1.abs_diff(end.1) + 1;
            }
        }
        let (c, r) = (sprite.frame % s.columns.max(1), sprite.frame / s.columns.max(1));
        let (w, h) = sprite.cells();
        let sel = Rect::from_min_size(rect.min + vec2(c as f32 * cw, r as f32 * ch), vec2(w as f32 * cw, h as f32 * ch));
        p.rect_stroke(sel, 0.0, Stroke::new(2.0_f32, Color32::YELLOW), StrokeKind::Inside);
    });
}

pub fn frame_grid(ui: &mut Ui, id: &str, sheet: &str, frame: &mut u32, tex: &Textures, assets: &Assets) {
    let Some(s) = assets.sheets.get(sheet) else {
        ui.colored_label(Color32::LIGHT_RED, "unknown sheet");
        return;
    };
    egui::ScrollArea::vertical().id_salt(format!("{id}/frames")).max_height(160.0).show(ui, |ui| {
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = vec2(2.0, 2.0);
            for f in 0..s.count {
                if frame_thumb(ui, tex, assets, sheet, f, 36.0, f == *frame).clicked() {
                    *frame = f;
                }
            }
        });
    });
}

/// Values offered by dropdowns in the trigger editor.
#[derive(Default, Clone)]
pub struct Refs {
    pub items: Vec<String>,
    pub maps: Vec<String>,
    /// "self", "player" and entity names on the current map.
    pub targets: Vec<String>,
    pub functions: Vec<String>,
    /// map -> spawn names
    pub spawns: HashMap<String, Vec<String>>,
    pub flags: Vec<String>,
    pub sounds: Vec<String>,
    pub characters: Vec<String>,
    /// All prop state names (for "Set prop state" / "Prop state").
    pub states: Vec<String>,
}

/// Single-line text field with a dropdown of suggestions.
pub fn suggest(ui: &mut Ui, id: &str, value: &mut String, options: &[String], width: f32) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 1.0;
        ui.add(egui::TextEdit::singleline(value).desired_width(width));
        if !options.is_empty() {
            ui.push_id(id, |ui| {
                ui.menu_button("⏷", |ui| {
                    egui::ScrollArea::vertical().max_height(240.0).show(ui, |ui| {
                        for o in options {
                            if ui.selectable_label(o == value, o).clicked() {
                                *value = o.clone();
                                ui.close_menu();
                            }
                        }
                    });
                });
            });
        }
    });
}

fn labeled(ui: &mut Ui, label: &str, add: impl FnOnce(&mut Ui)) {
    ui.horizontal(|ui| {
        ui.label(label);
        add(ui);
    });
}

pub fn triggers_ui(ui: &mut Ui, salt: &str, triggers: &mut Vec<Trigger>, refs: &Refs) {
    let mut remove = None;
    let mut swap = None;
    let n = triggers.len();
    for (i, t) in triggers.iter_mut().enumerate() {
        let id = format!("{salt}/t{i}");
        let title = format!(
            "On {}{}  ·  {} cond, {} act{}",
            t.on.label(),
            match &t.on {
                TriggerOn::UseItem { item } if !item.is_empty() => format!(" [{item}]"),
                TriggerOn::Custom { name } => format!(" [{name}]"),
                _ => String::new(),
            },
            t.conditions.len(),
            t.actions.len(),
            if t.once { " · once" } else { "" }
        );
        egui::CollapsingHeader::new(title).id_salt(&id).default_open(n <= 2).show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label("On");
                egui::ComboBox::from_id_salt(format!("{id}/on")).selected_text(t.on.label()).show_ui(ui, |ui| {
                    for tpl in TriggerOn::templates() {
                        let same = std::mem::discriminant(&tpl) == std::mem::discriminant(&t.on);
                        if ui.selectable_label(same, tpl.label()).clicked() && !same {
                            t.on = tpl;
                        }
                    }
                });
                match &mut t.on {
                    TriggerOn::UseItem { item } => {
                        let mut opts = vec![String::new()];
                        opts.extend(refs.items.iter().cloned());
                        suggest(ui, &format!("{id}/item"), item, &opts, 90.0);
                    }
                    TriggerOn::Custom { name } => {
                        ui.add(egui::TextEdit::singleline(name).hint_text("event name").desired_width(110.0));
                    }
                    _ => {}
                }
                ui.checkbox(&mut t.once, "once");
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.small_button("✖").on_hover_text("Delete trigger").clicked() {
                        remove = Some(i);
                    }
                    if i + 1 < n && ui.small_button("⏷").clicked() {
                        swap = Some((i, i + 1));
                    }
                    if i > 0 && ui.small_button("⏶").clicked() {
                        swap = Some((i - 1, i));
                    }
                });
            });
            if matches!(t.on, TriggerOn::UseItem { .. }) {
                ui.weak("(empty item = any item)");
            }
            ui.label("If all of:");
            ui.indent(format!("{id}/ci"), |ui| conditions_ui(ui, &id, &mut t.conditions, refs));
            ui.label("Do:");
            ui.indent(format!("{id}/ai"), |ui| actions_ui(ui, &id, &mut t.actions, refs));
        });
    }
    if let Some(i) = remove {
        triggers.remove(i);
    }
    if let Some((a, b)) = swap {
        triggers.swap(a, b);
    }
    ui.menu_button("+ Trigger", |ui| {
        for tpl in TriggerOn::templates() {
            if ui.button(tpl.label()).clicked() {
                triggers.push(Trigger::new(tpl));
                ui.close_menu();
            }
        }
    });
}

pub fn conditions_ui(ui: &mut Ui, salt: &str, conds: &mut Vec<Condition>, refs: &Refs) {
    let mut remove = None;
    for (i, c) in conds.iter_mut().enumerate() {
        let id = format!("{salt}/c{i}");
        ui.horizontal(|ui| {
            ui.checkbox(&mut c.negate, "not");
            match &mut c.cond {
                Cond::Flag { name } => {
                    ui.label("flag");
                    suggest(ui, &id, name, &refs.flags, 110.0);
                }
                Cond::Var { name, op, value } => {
                    ui.label("var");
                    ui.add(egui::TextEdit::singleline(name).desired_width(80.0));
                    egui::ComboBox::from_id_salt(format!("{id}/op")).width(40.0).selected_text(op.symbol()).show_ui(ui, |ui| {
                        for o in CmpOp::ALL {
                            ui.selectable_value(op, o, o.symbol());
                        }
                    });
                    ui.add(egui::DragValue::new(value));
                }
                Cond::HasItem { item, count } => {
                    ui.label("has");
                    ui.add(egui::DragValue::new(count).range(1..=9999));
                    suggest(ui, &id, item, &refs.items, 90.0);
                }
                Cond::Script { function } => {
                    ui.label("script");
                    suggest(ui, &id, function, &refs.functions, 110.0);
                    ui.label("()");
                }
                Cond::PropState { target, state } => {
                    ui.label("prop");
                    suggest(ui, &format!("{id}/t"), target, &refs.targets, 90.0);
                    ui.label("is in state");
                    suggest(ui, &format!("{id}/s"), state, &refs.states, 90.0);
                }
                Cond::PlayerHp { op, value } => {
                    ui.label("player HP");
                    egui::ComboBox::from_id_salt(format!("{id}/op")).width(40.0).selected_text(op.symbol()).show_ui(ui, |ui| {
                        for o in CmpOp::ALL {
                            ui.selectable_value(op, o, o.symbol());
                        }
                    });
                    ui.add(egui::DragValue::new(value));
                }
            }
            if ui.small_button("✖").clicked() {
                remove = Some(i);
            }
        });
    }
    if let Some(i) = remove {
        conds.remove(i);
    }
    ui.push_id(format!("{salt}/addc"), |ui| {
        ui.menu_button("+ Condition", |ui| {
            for tpl in Cond::templates() {
                if ui.button(tpl.label()).clicked() {
                    conds.push(Condition::new(tpl));
                    ui.close_menu();
                }
            }
        });
    });
}

pub fn actions_ui(ui: &mut Ui, salt: &str, actions: &mut Vec<Action>, refs: &Refs) {
    let mut remove = None;
    let mut swap = None;
    let n = actions.len();
    for (i, a) in actions.iter_mut().enumerate() {
        let id = format!("{salt}/a{i}");
        egui::Frame::group(ui.style()).inner_margin(4.0).show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.strong(format!("{}.", i + 1));
                ui.strong(a.label());
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.small_button("✖").clicked() {
                        remove = Some(i);
                    }
                    if i + 1 < n && ui.small_button("⏷").clicked() {
                        swap = Some((i, i + 1));
                    }
                    if i > 0 && ui.small_button("⏶").clicked() {
                        swap = Some((i - 1, i));
                    }
                });
            });
            action_fields(ui, &id, a, refs);
        });
    }
    if let Some(i) = remove {
        actions.remove(i);
    }
    if let Some((a, b)) = swap {
        actions.swap(a, b);
    }
    ui.push_id(format!("{salt}/adda"), |ui| {
        ui.menu_button("+ Action", |ui| {
            for tpl in Action::templates() {
                if ui.button(tpl.label()).clicked() {
                    actions.push(tpl);
                    ui.close_menu();
                }
            }
        });
    });
}

fn target_field(ui: &mut Ui, id: &str, target: &mut String, refs: &Refs) {
    labeled(ui, "target", |ui| suggest(ui, id, target, &refs.targets, 100.0));
}

fn action_fields(ui: &mut Ui, id: &str, a: &mut Action, refs: &Refs) {
    match a {
        Action::Dialog { speaker, text } => {
            labeled(ui, "speaker", |ui| {
                ui.add(egui::TextEdit::singleline(speaker).desired_width(160.0));
            });
            ui.add(egui::TextEdit::multiline(text).desired_rows(2).desired_width(f32::INFINITY));
        }
        Action::Choice { speaker, text, options, var } => {
            labeled(ui, "speaker", |ui| {
                ui.add(egui::TextEdit::singleline(speaker).desired_width(160.0));
            });
            ui.add(egui::TextEdit::multiline(text).desired_rows(2).desired_width(f32::INFINITY));
            ui.label("Options (one per line):");
            let mut joined = options.join("\n");
            if ui.add(egui::TextEdit::multiline(&mut joined).desired_rows(2).desired_width(f32::INFINITY)).changed() {
                *options = joined.lines().map(str::to_string).collect();
            }
            labeled(ui, "store index in var", |ui| {
                ui.add(egui::TextEdit::singleline(var).desired_width(100.0));
            });
        }
        Action::GiveItem { item, count } | Action::TakeItem { item, count } => {
            ui.horizontal(|ui| {
                ui.add(egui::DragValue::new(count).range(1..=9999));
                ui.label("x");
                suggest(ui, id, item, &refs.items, 110.0);
            });
        }
        Action::SetFlag { name, value } => {
            ui.horizontal(|ui| {
                suggest(ui, id, name, &refs.flags, 120.0);
                ui.checkbox(value, "set");
            });
        }
        Action::SetVar { name, op, value } => {
            ui.horizontal(|ui| {
                ui.add(egui::TextEdit::singleline(name).desired_width(90.0));
                egui::ComboBox::from_id_salt(format!("{id}/op")).width(50.0).selected_text(match op {
                    VarOp::Set => "=",
                    VarOp::Add => "+=",
                }).show_ui(ui, |ui| {
                    ui.selectable_value(op, VarOp::Set, "=");
                    ui.selectable_value(op, VarOp::Add, "+=");
                });
                ui.add(egui::DragValue::new(value));
            });
        }
        Action::Teleport { map, spawn } => {
            labeled(ui, "map", |ui| suggest(ui, &format!("{id}/m"), map, &refs.maps, 110.0));
            let spawns = refs.spawns.get(map.as_str()).cloned().unwrap_or_default();
            labeled(ui, "spawn", |ui| suggest(ui, &format!("{id}/s"), spawn, &spawns, 110.0));
        }
        Action::SetActive { target, active } => {
            target_field(ui, id, target, refs);
            ui.checkbox(active, "active");
        }
        Action::SetState { target, state } => {
            target_field(ui, id, target, refs);
            labeled(ui, "state", |ui| suggest(ui, &format!("{id}/state"), state, &refs.states, 100.0));
        }
        Action::SetFrame { target, frame } => {
            target_field(ui, id, target, refs);
            labeled(ui, "frame offset", |ui| {
                ui.add(egui::DragValue::new(frame).range(0..=255));
            });
        }
        Action::Move { target, dx, dy } => {
            target_field(ui, id, target, refs);
            ui.horizontal(|ui| {
                ui.label("by tiles dx");
                ui.add(egui::DragValue::new(dx).range(-64..=64));
                ui.label("dy");
                ui.add(egui::DragValue::new(dy).range(-64..=64));
            });
        }
        Action::Face { target, facing } => {
            target_field(ui, id, target, refs);
            egui::ComboBox::from_id_salt(format!("{id}/f")).selected_text(facing.label()).show_ui(ui, |ui| {
                for f in Facing::ALL {
                    ui.selectable_value(facing, f, f.label());
                }
            });
        }
        Action::Wait { seconds } => {
            labeled(ui, "seconds", |ui| {
                ui.add(egui::DragValue::new(seconds).range(0.0..=60.0).speed(0.05));
            });
        }
        Action::Emit { name } => {
            labeled(ui, "event", |ui| {
                ui.add(egui::TextEdit::singleline(name).desired_width(140.0));
            });
        }
        Action::RunScript { function } => {
            labeled(ui, "function", |ui| suggest(ui, id, function, &refs.functions, 130.0));
        }
        Action::PlaySound { sound } | Action::PlayMusic { sound } => {
            labeled(ui, "sound", |ui| suggest(ui, id, sound, &refs.sounds, 130.0));
            if matches!(a, Action::PlayMusic { .. }) {
                ui.weak("(empty = stop music)");
            }
        }
        Action::SetLocked { target, locked } => {
            target_field(ui, id, target, refs);
            ui.checkbox(locked, "locked");
            ui.weak("(unlocking applies the lock's effect and runs 'Unlocked' triggers; locking closes it again and runs 'Locked again' triggers)");
        }
        Action::Heal { amount } => {
            labeled(ui, "hit points", |ui| {
                ui.add(egui::DragValue::new(amount).range(-99..=99));
            });
            ui.weak("(negative = damage)");
        }
        Action::If { conditions, then, otherwise } => {
            ui.label("If all of:");
            ui.indent(format!("{id}/ic"), |ui| conditions_ui(ui, id, conditions, refs));
            ui.label("Then:");
            ui.indent(format!("{id}/it"), |ui| actions_ui(ui, &format!("{id}/then"), then, refs));
            ui.label("Else:");
            ui.indent(format!("{id}/ie"), |ui| actions_ui(ui, &format!("{id}/else"), otherwise, refs));
        }
    }
}
