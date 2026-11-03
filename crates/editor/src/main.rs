//! rust2d editor: maps, entities, zones, transitions, definitions and scripts.
//!
//! Usage: rust2d-editor [PROJECT_DIR]   (without a folder the welcome screen shows)

mod app;
mod canvas;
mod database;
mod doc;
mod export;
mod files;
mod library_ui;
mod pixel;
mod wizard;
mod widgets;

use std::collections::VecDeque;

use app::{Editor, Mode, Pending, Selection, Tool};
use macroquad::prelude::*;

fn window_conf() -> Conf {
    Conf {
        window_title: "rust2d editor".to_owned(),
        window_width: 1600,
        window_height: 900,
        high_dpi: true,
        window_resizable: true,
        ..Default::default()
    }
}

#[macroquad::main(window_conf)]
async fn main() {
    // With a folder argument that project opens directly; otherwise the welcome screen shows.
    let mut ed = match std::env::args().nth(1) {
        Some(root) => Editor::new(root.clone().into()).unwrap_or_else(|e| {
            let mut ed = Editor::empty();
            ed.set_status(format!("Cannot open project '{root}': {e:#}"));
            ed
        }),
        None => Editor::empty(),
    };
    prevent_quit();
    // Debug aid: RUST2D_EDITOR_SCRIPT="tool entity; select entity 3; wait 0.5; shot a.png; quit"
    let mut script: VecDeque<Vec<String>> = std::env::var("RUST2D_EDITOR_SCRIPT")
        .map(|s| s.split(';').map(|c| c.split_whitespace().map(str::to_string).collect::<Vec<_>>()).filter(|c| !c.is_empty()).collect())
        .unwrap_or_default();
    let mut wait = 0.3f32;
    let mut shot: Option<String> = None;
    let mut mouse_frames: VecDeque<canvas::Mouse> = VecDeque::new();
    loop {
        if is_quit_requested() {
            ed.request(Pending::Quit);
        }
        if !script.is_empty() || !mouse_frames.is_empty() {
            wait -= get_frame_time();
            while wait <= 0.0 && shot.is_none() && mouse_frames.is_empty() {
                let Some(cmd) = script.pop_front() else { break };
                run_script_cmd(&mut ed, &cmd, &mut wait, &mut shot, &mut mouse_frames);
            }
        }
        canvas::inject_mouse(mouse_frames.pop_front());
        egui_macroquad::ui(|ctx| ed.ui(ctx));
        ed.update_canvas();
        ed.draw_canvas();
        egui_macroquad::draw();
        if let Some(p) = shot.take() {
            get_screen_data().export_png(&p);
        }
        if ed.quit {
            break;
        }
        next_frame().await;
    }
}

fn run_script_cmd(ed: &mut Editor, cmd: &[String], wait: &mut f32, shot: &mut Option<String>, mouse: &mut VecDeque<canvas::Mouse>) {
    let arg = |i: usize| cmd.get(i).map(String::as_str).unwrap_or("");
    let num = |i: usize| arg(i).parse::<f32>().unwrap_or(0.0);
    // Screen position of the center of tile (x, y).
    let at = |ed: &Editor, x: f32, y: f32| {
        let ts = ed.ts();
        ed.camera().world_to_screen(vec2((x + 0.5) * ts, (y + 0.5) * ts))
    };
    let frame = |pos: Vec2, b: usize, down: bool, pressed: bool, released: bool| {
        let mut m = canvas::Mouse { pos, ..Default::default() };
        m.down[b] = down;
        m.pressed[b] = pressed;
        m.released[b] = released;
        m
    };
    match arg(0) {
        // click X Y [right]  /  drag X0 Y0 X1 Y1   (tile coordinates)
        "click" => {
            let b = if arg(3) == "right" { 1 } else { 0 };
            let p = at(ed, num(1), num(2));
            mouse.push_back(frame(p, b, true, true, false));
            mouse.push_back(frame(p, b, false, false, true));
            mouse.push_back(frame(p, b, false, false, false));
        }
        "drag" => {
            let (a, b) = (at(ed, num(1), num(2)), at(ed, num(3), num(4)));
            mouse.push_back(frame(a, 0, true, true, false));
            for i in 1..=12 {
                mouse.push_back(frame(a.lerp(b, i as f32 / 12.0), 0, true, false, false));
            }
            mouse.push_back(frame(b, 0, false, false, true));
            mouse.push_back(frame(b, 0, false, false, false));
        }
        "brush" => {
            ed.brush.x = num(1) as u32;
            ed.brush.y = num(2) as u32;
            ed.brush.w = (num(3) as u32).max(1);
            ed.brush.h = (num(4) as u32).max(1);
        }
        "layer" => ed.layer = num(1) as usize,
        // containeradd ITEM COUNT / containersize N: edit the selected entity's container
        "containeradd" | "containersize" => {
            if let (Some(doc), Selection::Entity(uid)) = (&mut ed.doc, ed.sel) {
                let before = doc::Objects::of(&doc.map);
                let def = ed.project.db.clone();
                if let Some(e) = doc.map.entities.iter_mut().find(|e| e.uid == uid) {
                    let default = def.container_of(e).cloned().unwrap_or_default();
                    let spec = e.container.get_or_insert(default);
                    if arg(0) == "containeradd" {
                        spec.items.push(rust2d_engine::inventory::ItemStack::new(arg(1), num(2) as u32));
                    } else {
                        spec.size = num(1) as u32;
                    }
                }
                doc.record_objects(before, "script");
            }
        }
        // spawnermode never|enter|SECONDS  (selected zone)
        "spawnermode" => {
            use rust2d_engine::map::{Respawn, ZoneKind};
            if let (Some(doc), Selection::Zone(uid)) = (&mut ed.doc, ed.sel) {
                if let Some(ZoneKind::Spawner { respawn, .. }) = doc.map.zones.iter_mut().find(|z| z.uid == uid).map(|z| &mut z.kind) {
                    *respawn = match arg(1) {
                        "never" => Respawn::Never,
                        "enter" => Respawn::OnEnter,
                        s => Respawn::After { seconds: s.parse().unwrap_or(10.0) },
                    };
                    doc.dirty = true;
                }
            }
        }
        "place" => {
            ed.place_kind = match arg(1) {
                "character" => rust2d_engine::entity::EntityKind::Character,
                "item" => rust2d_engine::entity::EntityKind::Item,
                _ => rust2d_engine::entity::EntityKind::Prop,
            };
            ed.place_def = arg(2).to_string();
        }
        // unplace: stop placing (like right-click / Esc)
        "unplace" => ed.disarm(),
        // cycle +1|-1: next / previous definition in the palette
        "cycle" => ed.cycle_place(num(1) as i32),
        // filter TEXT / tabkind all|prop|character|item
        "filter" => ed.place_filter = arg(1).to_string(),
        "zonekind" => {
            ed.zone_template = match arg(1) {
                "portal" => app::ZoneTemplate::Portal,
                "trigger" => app::ZoneTemplate::Trigger,
                "spawner" => app::ZoneTemplate::Spawner,
                _ => app::ZoneTemplate::Spawn,
            }
        }
        "undo" => {
            if let Some(d) = &mut ed.doc {
                d.undo(&ed.assets);
            }
        }
        "redo" => {
            if let Some(d) = &mut ed.doc {
                d.redo(&ed.assets);
            }
        }
        "save" => ed.save_all(),
        // Same code paths as the file dialogs, without the dialog.
        "import" => {
            let purpose = match arg(1) {
                "tileset" => files::Purpose::TilesetImage(arg(2).into()),
                "sheet" => files::Purpose::SheetImage(arg(2).into()),
                _ => files::Purpose::SoundFile(arg(2).into()),
            };
            ed.handle_file(purpose, arg(3).into());
        }
        "newproject" => ed.handle_file(files::Purpose::NewProject, arg(1).into()),
        "openproject" => ed.handle_file(files::Purpose::OpenProject, arg(1).into()),
        "closeproject" => ed.request(Pending::CloseProject),
        // exportgame OUT_DIR [NAME]  /  exportdialog
        "exportgame" => {
            let name = if arg(2).is_empty() { ed.project.config.name.clone() } else { cmd[2..].join(" ") };
            match ed.run_export(&name, arg(1), true) {
                Ok(m) => println!("[export] {m}"),
                Err(e) => println!("[export] error: {e}"),
            }
        }
        "exportdialog" => ed.open_export(),
        // pending save|discard|cancel: answers the "Unsaved changes" question
        "pending" => {
            if let Some(p) = ed.pending.take() {
                let c = match arg(1) {
                    "save" => app::Choice::Save,
                    "discard" => app::Choice::Discard,
                    _ => app::Choice::Cancel,
                };
                ed.settle_pending(p, c);
            }
        }
        "status" => println!("[status] open={} project={} pending={} map={:?} dirty={} msg={}", ed.project_open, ed.project.root.display(), ed.pending.is_some(), ed.doc.as_ref().map(|d| d.map.name.clone()), ed.doc.as_ref().is_some_and(|d| d.dirty), ed.status.0),
        "filedialog" => ed.open_file_dialog(files::Purpose::OpenProject),
        "rename" => {
            use rust2d_engine::refactor::RefKind;
            let kind = match arg(1) {
                "tileset" => RefKind::Tileset,
                "sheet" => RefKind::Sheet,
                "item" => RefKind::Item,
                "prop" => RefKind::Prop,
                "character" => RefKind::Character,
                "sound" => RefKind::Sound,
                _ => RefKind::Map,
            };
            ed.rename_id(kind, arg(2), arg(3));
        }
        "newmap" => {
            ed.new_map = Some(app::NewMapDialog { name: arg(1).into(), width: num(2) as u32, height: num(3) as u32, fill: true });
        }
        "dump" => {
            if let Some(d) = &ed.doc {
                let m = &d.map;
                let tile = |l: usize, x: u32, y: u32| m.layers[l].get(x, y).parts().map_or("-".to_string(), |(_, i)| i.to_string());
                println!(
                    "[dump] map={} {}x{} dirty={} sel={:?} ents={} zones={:?} tile(L{},{},{})={} col={}",
                    m.name,
                    m.width,
                    m.height,
                    d.dirty,
                    ed.sel,
                    m.entities.iter().map(|e| format!("{}@{},{}", e.def, e.x, e.y)).collect::<Vec<_>>().join(" "),
                    m.zones.iter().map(|z| format!("{}:{}({},{} {}x{})", z.name, z.kind.label(), z.x, z.y, z.w, z.h)).collect::<Vec<_>>(),
                    num(1),
                    num(2),
                    num(3),
                    tile(num(1) as usize, num(2) as u32, num(3) as u32),
                    m.collision[(num(3) as u32 * m.width + num(2) as u32) as usize],
                );
            }
        }
        "wait" => *wait = num(1),
        "shot" => *shot = Some(arg(1).to_string()),
        "quit" => ed.quit = true,
        "open" => ed.open_map(arg(1)),
        "mode" => {
            ed.mode = match arg(1) {
                "db" => Mode::Database,
                "library" => Mode::Library,
                "pixel" => Mode::Pixel,
                _ => Mode::Map,
            }
        }
        // Library / packages: libsel KIND ID · libadd · tolib KIND ID · libexport PATH ·
        // libimport PATH (into the library) · projimport PATH (into the project)
        "libsel" => {
            ed.lib.ensure_loaded();
            if let Some(k) = library_ui::parse_kind(arg(1)) {
                ed.lib.selected.insert((k, arg(2).to_string()));
            }
        }
        "libadd" => {
            let sel: Vec<_> = ed.lib.selected.iter().cloned().collect();
            let r = ed.library_to_project(&sel);
            ed.show_result(r);
            println!("[script] {}", ed.lib.report.clone().unwrap_or_default());
        }
        "tolib" => {
            if let Some(k) = library_ui::parse_kind(arg(1)) {
                let r = ed.project_to_library(&[(k, arg(2).to_string())]);
                ed.show_result(r);
                println!("[script] {}", ed.lib.report.clone().unwrap_or_default());
            }
        }
        "libexport" => {
            let sel: Vec<_> = ed.lib.selected.iter().cloned().collect();
            let r = ed.export_package(true, &sel, std::path::Path::new(arg(1)));
            ed.show_result(r);
            println!("[script] {}", ed.lib.report.clone().unwrap_or_default());
        }
        "libimport" | "projimport" => {
            let r = ed.import_package(arg(0) == "libimport", std::path::Path::new(arg(1)));
            ed.show_result(r);
            println!("[script] {}", ed.lib.report.clone().unwrap_or_default());
        }
        // newasset KIND ID [W H]: blank image (prop: tiles, sheet/character: frame px)
        "newasset" => {
            if let Some(k) = wizard::WKind::parse(arg(1)) {
                let mut w = wizard::Wizard::new(&ed.project);
                w.kind = k;
                w.id = arg(2).to_string();
                if num(3) > 0.0 {
                    w.cells = (num(3) as u32, num(4) as u32);
                    w.frame = (num(3) as u32, num(4) as u32);
                    w.grid = (num(3) as u32, num(4) as u32);
                }
                let r = ed.create_asset(&w);
                ed.show_result(r);
                println!("[script] {}", ed.lib.report.clone().unwrap_or_default());
            }
        }
        "wizard" => ed.wizard = Some(wizard::Wizard::new(&ed.project)),
        // Pixel editor: pxopen REL FW FH · pxtool NAME · pxcolor RRGGBB · pxat X Y [right] · pxsave
        "pxopen" => ed.open_pixel(arg(1), (num(2) as u32, num(3) as u32), pixel::Layout::Plain),
        "pxtool" => {
            if let Some(t) = pixel::PxTool::parse(arg(1)) {
                ed.px.tool = t;
            }
        }
        "pxcolor" => {
            if let Ok(v) = u32::from_str_radix(arg(1), 16) {
                ed.px.colors[0] = [(v >> 16) as u8, (v >> 8) as u8, v as u8, 255];
            }
        }
        "pxat" => ed.px.apply_at(if arg(3) == "right" { 1 } else { 0 }, (num(1) as i32, num(2) as i32)),
        "pxsave" => ed.save_pixel(),
        "tab" => {
            use database::Tab;
            ed.db.tab = match arg(1) {
                "props" => Tab::Props,
                "characters" => Tab::Characters,
                "tilesets" => Tab::Tilesets,
                "sheets" => Tab::Sheets,
                "project" => Tab::Project,
                "scripts" => Tab::Scripts,
                _ => Tab::Items,
            };
            ed.db.sel = num(2) as usize;
        }
        "tool" => {
            ed.tool = match arg(1) {
                "select" => Tool::Select,
                "entity" => Tool::Entity,
                "zone" => Tool::Zone,
                "collision" => Tool::Collision,
                "rect" => Tool::Rect,
                "fill" => Tool::Fill,
                "erase" => Tool::Erase,
                _ => Tool::Brush,
            }
        }
        "select" => {
            ed.sel = match arg(1) {
                "entity" => Selection::Entity(num(2) as u32),
                "zone" => Selection::Zone(num(2) as u32),
                _ => Selection::None,
            }
        }
        "view" => {
            ed.cam.center = vec2(num(1), num(2));
            ed.cam.zoom = num(3).max(0.01);
        }
        "fit" => ed.fit_view(),
        "collision" => ed.view.collision = arg(1) != "off",
        other => eprintln!("[editor script] unknown command '{other}'"),
    }
}
