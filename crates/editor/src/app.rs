//! Editor state and egui panels.

use std::collections::HashMap;
use std::path::PathBuf;

use egui_macroquad::egui::{self, pos2, vec2, Color32, Sense, Stroke, StrokeKind};
use macroquad::prelude::{get_time, Vec2};
use rust2d_engine::entity::{EntityInstance, EntityKind, Facing};
use rust2d_engine::map::{Edge, EdgeLink, Map, Respawn, Tile, Zone, ZoneKind, MAX_MAP_SIZE};
use rust2d_engine::audio::Audio;
use rust2d_engine::defs::{ContainerSpec, LockSpec, ShopSpec, SpriteRef};

/// Definitions remembered in the entity palette's "Recent" row.
const RECENT_MAX: usize = 10;
use rust2d_engine::project::Project;
use rust2d_engine::refactor::{self, RefKind};
use rust2d_engine::render::Assets;
use rust2d_engine::script::ScriptHost;

use crate::database::DbState;
use crate::files;
use crate::doc::{MapDoc, Objects};
use crate::widgets::{self, Refs, Textures};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Map,
    Database,
    /// The shared asset library.
    Library,
    /// The pixel editor.
    Pixel,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Tool {
    /// Default: click selects entities / zones, drag moves them. Never paints.
    Select,
    Brush,
    Rect,
    Fill,
    Erase,
    Pick,
    Collision,
    Entity,
    Zone,
    /// Drag an exit area, pick the target map, drag the arrival area there.
    Link,
}

impl Tool {
    pub const ALL: [(Tool, &'static str, &'static str); 10] = [
        (Tool::Select, "Select", "V"),
        (Tool::Brush, "Brush", "B"),
        (Tool::Rect, "Rect", "R"),
        (Tool::Fill, "Fill", "F"),
        (Tool::Erase, "Erase", "E"),
        (Tool::Pick, "Pick", "I"),
        (Tool::Collision, "Collision", "C"),
        (Tool::Entity, "Entities", "N"),
        (Tool::Zone, "Zones", "Z"),
        (Tool::Link, "Transition", "T"),
    ];
    pub fn paints_tiles(self) -> bool {
        matches!(self, Tool::Brush | Tool::Rect | Tool::Fill | Tool::Erase | Tool::Pick)
    }
}

/// Selected rectangle of tiles in a tileset (the stamp).
#[derive(Clone, Debug)]
pub struct Brush {
    pub tileset: String,
    pub x: u32,
    pub y: u32,
    pub w: u32,
    pub h: u32,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Selection {
    None,
    Entity(u32),
    Zone(u32),
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ZoneTemplate {
    Spawn,
    Portal,
    Trigger,
    Spawner,
    Transition,
}

pub struct ViewOpts {
    pub grid: bool,
    pub collision: bool,
    pub zones: bool,
    pub dim_layers: bool,
    pub minimap: bool,
}

pub struct Cam {
    pub center: Vec2,
    pub zoom: f32,
}

pub struct NewMapDialog {
    pub name: String,
    pub width: u32,
    pub height: u32,
    pub fill: bool,
}

pub enum Pending {
    Open(String),
    /// Open a map and focus a zone (by name; "" = none).
    OpenAt(String, String),
    New,
    Quit,
    OpenProject(PathBuf),
    NewProject(PathBuf),
    /// Back to the welcome screen without a project.
    CloseProject,
}

thread_local! {
    /// "Go to" requests from inspector widgets (map, zone name), handled after the panel.
    static GOTO: std::cell::RefCell<Option<(String, String)>> = const { std::cell::RefCell::new(None) };
}

/// Answer to the "Unsaved changes" question.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Choice {
    None,
    Save,
    Discard,
    Cancel,
}

pub struct Editor {
    /// Transition tool draft.
    pub link: Option<crate::maps::LinkDraft>,
    pub quick: crate::maps::QuickSwitch,
    /// Open character model chooser.
    pub model: Option<crate::models::ModelChooser>,
    /// Debug: screen points to report the egui layer under (script command `probe X Y`).
    pub probe: Vec<egui::Pos2>,
    /// Camera (center, zoom) per map, restored when switching back.
    pub map_cams: HashMap<String, (Vec2, f32)>,
    /// Last click on a portal (uid, time) to detect double-clicks.
    pub last_portal_click: Option<(u32, f64)>,
    /// False on the welcome screen: `project` is then an empty placeholder.
    pub project_open: bool,
    pub lib: crate::library_ui::LibState,
    pub px: crate::pixel::PixelState,
    pub wizard: Option<crate::wizard::Wizard>,
    pub export: Option<crate::export::ExportDialog>,
    pub project: Project,
    pub assets: Assets,
    pub audio: Audio,
    /// Inline "rename map" field in the map properties.
    pub map_rename: Option<String>,
    pub tex: Textures,
    pub tex_dirty: bool,
    pub doc: Option<MapDoc>,
    pub maps: Vec<String>,
    pub mode: Mode,
    pub tool: Tool,
    pub layer: usize,
    pub brush: Brush,
    pub cam: Cam,
    pub sel: Selection,
    pub place_kind: EntityKind,
    /// Definition placed by clicking empty space ("" = nothing: clicks only select).
    pub place_def: String,
    /// Entity tool palette: kind tab (None = all kinds) and search text.
    pub place_tab: Option<EntityKind>,
    pub place_filter: String,
    /// Recently placed / picked definitions, newest first.
    pub place_recent: Vec<(EntityKind, String)>,
    pub zone_template: ZoneTemplate,
    pub view: ViewOpts,
    pub status: (String, f64),
    pub new_map: Option<NewMapDialog>,
    pub resize: Option<(u32, u32)>,
    pub pending: Option<Pending>,
    pub defs_dirty: bool,
    pub db: DbState,
    pub refs: Refs,
    pub pointer_over_ui: bool,
    pub keyboard_busy: bool,
    /// Scroll the palette / map entity list to the armed definition / selection once.
    pub palette_scroll_to_armed: bool,
    pub on_map_scroll: bool,
    pub canvas_rect: egui::Rect,
    pub hover_tile: Option<(i32, i32)>,
    pub hover_world: Vec2,
    /// Where the left button went down (world), to tell clicks from drags.
    pub press_world: Vec2,
    /// Select tool: objects under the last click and whether the clicked one was already
    /// selected (a plain click then cycles to the next one).
    pub click_stack: Option<(Vec<Selection>, bool)>,
    pub drag: Option<crate::canvas::Drag>,
    pub quit: bool,
    pub palette_drag: Option<(u32, u32)>,
    /// Native file dialog currently open.
    pub file_dialog: Option<files::PendingDialog>,
    /// A form asked for a file dialog this frame.
    pub browse_request: Option<files::Purpose>,
}

impl Editor {
    /// An editor with the project at `root` open.
    pub fn new(root: PathBuf) -> anyhow::Result<Self> {
        // Projects from before tiles were fixed at 32 px are converted first (with a backup),
        // restoring library art that was shrunken for them.
        let mut converted = None;
        if rust2d_engine::migrate::stored_tile_size(&root)? != rust2d_engine::project::TILE_SIZE {
            let lib = rust2d_engine::library::open_library(&rust2d_engine::library::library_dir(), Some(crate::library_ui::STARTER)).ok();
            converted = rust2d_engine::migrate::upgrade(&root, lib.as_ref())?;
        }
        let project = Project::load(&root)?;
        let mut ed = Self::with_project(project, true);
        let first = if ed.maps.contains(&ed.project.config.start_map) {
            Some(ed.project.config.start_map.clone())
        } else {
            ed.maps.first().cloned()
        };
        if let Some(m) = first {
            ed.open_map(&m);
        }
        ed.refresh_refs();
        if !ed.assets.errors.is_empty() {
            ed.set_status(format!("{} asset error(s), see terminal", ed.assets.errors.len()));
        }
        files::remember_recent(&root);
        if let Some(c) = converted {
            println!("[editor] {}", c.summary());
            ed.set_status(c.summary());
        }
        Ok(ed)
    }

    /// The welcome screen: no project open.
    pub fn empty() -> Self {
        let project = Project { root: PathBuf::new(), config: rust2d_engine::project::ProjectConfig::new(""), db: Default::default() };
        Self::with_project(project, false)
    }

    fn with_project(project: Project, project_open: bool) -> Self {
        let assets = Assets::load(&project);
        let audio = Audio::load(&project);
        let maps = project.list_maps();
        let brush_ts = project.db.tilesets.first().map(|t| t.id.clone()).unwrap_or_default();
        Editor {
            link: None,
            quick: Default::default(),
            model: None,
            probe: Vec::new(),
            map_cams: HashMap::new(),
            last_portal_click: None,
            project_open,
            assets,
            audio,
            map_rename: None,
            tex: Textures::default(),
            tex_dirty: true,
            doc: None,
            maps,
            mode: Mode::Map,
            tool: Tool::Select,
            layer: 0,
            brush: Brush { tileset: brush_ts, x: 0, y: 0, w: 1, h: 1 },
            cam: Cam { center: Vec2::ZERO, zoom: 2.0 },
            sel: Selection::None,
            place_kind: EntityKind::Prop,
            place_def: String::new(),
            place_tab: None,
            place_filter: String::new(),
            place_recent: Vec::new(),
            zone_template: ZoneTemplate::Spawn,
            view: ViewOpts { grid: true, collision: false, zones: true, dim_layers: false, minimap: true },
            status: (String::new(), 0.0),
            new_map: None,
            resize: None,
            pending: None,
            defs_dirty: false,
            db: DbState::default(),
            lib: Default::default(),
            px: Default::default(),
            wizard: None,
            export: None,
            refs: Refs::default(),
            pointer_over_ui: false,
            keyboard_busy: false,
            palette_scroll_to_armed: false,
            on_map_scroll: false,
            canvas_rect: egui::Rect::NOTHING,
            hover_tile: None,
            hover_world: Vec2::ZERO,
            press_world: Vec2::ZERO,
            click_stack: None,
            drag: None,
            quit: false,
            palette_drag: None,
            file_dialog: None,
            browse_request: None,
            project,
        }
    }

    /// Closes the project (after unsaved changes were handled) and shows the welcome screen.
    fn close_project(&mut self) {
        self.audio.stop_music();
        let rect = self.canvas_rect;
        *self = Editor::empty();
        self.canvas_rect = rect;
    }

    pub fn ts(&self) -> f32 {
        self.assets.tile_size
    }

    pub fn set_status(&mut self, s: impl Into<String>) {
        let s = s.into();
        println!("[editor] {s}");
        self.status = (s, get_time());
    }

    // ------------------------------------------------------------ files

    pub fn open_map(&mut self, name: &str) {
        // Remember where we looked on the map we leave.
        if let Some(d) = &self.doc {
            self.map_cams.insert(d.map.name.clone(), (self.cam.center, self.cam.zoom));
        }
        match self.project.load_map(name) {
            Ok(map) => {
                self.load_doc(map);
                if let Some(&(center, zoom)) = self.map_cams.get(name) {
                    self.cam.center = center;
                    self.cam.zoom = zoom;
                }
                self.set_status(format!("Opened {name}"));
            }
            Err(e) => self.set_status(format!("Failed to open {name}: {e:#}")),
        }
    }

    fn load_doc(&mut self, map: Map) {
        let ts = self.ts();
        self.cam.center = macroquad::math::vec2(map.width as f32 * ts / 2.0, map.height as f32 * ts / 2.0);
        self.layer = self.layer.min(map.layers.len().saturating_sub(1));
        self.sel = Selection::None;
        self.drag = None;
        self.doc = Some(MapDoc::new(map, &self.assets));
        self.fit_view();
        self.refresh_refs();
    }

    pub fn fit_view(&mut self) {
        let Some(doc) = &self.doc else { return };
        let ts = self.ts();
        let (w, h) = (doc.map.width as f32 * ts, doc.map.height as f32 * ts);
        self.cam.center = macroquad::math::vec2(w / 2.0, h / 2.0);
        let r = self.canvas_rect;
        let (cw, ch) = if r.is_positive() && r.width() > 50.0 {
            (r.width(), r.height())
        } else {
            (macroquad::window::screen_width() * 0.6, macroquad::window::screen_height() * 0.8)
        };
        self.cam.zoom = ((cw / w).min(ch / h) * 0.9).clamp(0.02, 4.0);
        if r.is_positive() {
            // Center the map inside the canvas area rather than the whole window.
            let sc = macroquad::math::vec2(macroquad::window::screen_width() / 2.0, macroquad::window::screen_height() / 2.0);
            let off = (macroquad::math::vec2(r.center().x, r.center().y) - sc) / self.cam.zoom;
            self.cam.center -= off;
        }
    }

    pub fn save_map(&mut self) {
        let Some(doc) = &mut self.doc else { return };
        doc.end_stroke();
        match self.project.save_map(&doc.map) {
            Ok(()) => {
                doc.dirty = false;
                let name = doc.map.name.clone();
                if !self.maps.contains(&name) {
                    self.maps.push(name.clone());
                    self.maps.sort();
                }
                self.refresh_refs();
                self.set_status(format!("Saved map {name}"));
            }
            Err(e) => self.set_status(format!("Save failed: {e:#}")),
        }
    }

    pub fn save_defs(&mut self) {
        match self.project.save_defs().and_then(|_| self.project.save_config()) {
            Ok(()) => {
                self.defs_dirty = false;
                self.set_status("Saved definitions");
            }
            Err(e) => self.set_status(format!("Saving definitions failed: {e:#}")),
        }
    }

    pub fn save_all(&mut self) {
        if self.doc.as_ref().is_some_and(|d| d.dirty) {
            self.save_map();
        }
        if self.defs_dirty {
            self.save_defs();
        }
        if self.px.is_dirty() {
            self.save_pixel();
        }
    }

    pub fn reload_assets(&mut self) {
        self.assets = Assets::load(&self.project);
        self.audio.stop_music();
        self.audio = Audio::load(&self.project);
        self.tex_dirty = true;
        if let Some(doc) = &mut self.doc {
            doc.rebuild_overview(&self.assets);
        }
        self.refresh_refs();
        let n = self.assets.errors.len() + self.audio.errors.len();
        self.set_status(if n == 0 { "Assets reloaded".to_string() } else { format!("Assets reloaded with {n} error(s)") });
    }

    /// Rebuilds dropdown suggestions (items, maps, spawns, script functions, ...).
    pub fn refresh_refs(&mut self) {
        let mut spawns: HashMap<String, Vec<String>> = HashMap::new();
        let mut transitions: HashMap<String, Vec<(String, String, String)>> = HashMap::new();
        for m in &self.maps {
            let zones = if self.doc.as_ref().is_some_and(|d| &d.map.name == m) {
                self.doc.as_ref().unwrap().map.zones.clone()
            } else {
                self.project.load_map(m).map(|m| m.zones).unwrap_or_default()
            };
            spawns.insert(m.clone(), zones.iter().filter(|z| z.kind == ZoneKind::Spawn).map(|z| z.name.clone()).collect());
            transitions.insert(
                m.clone(),
                zones
                    .iter()
                    .filter_map(|z| match &z.kind {
                        ZoneKind::Transition { target_map, target_zone } => Some((z.name.clone(), target_map.clone(), target_zone.clone())),
                        _ => None,
                    })
                    .collect(),
            );
        }
        let mut scripts = ScriptHost::new();
        scripts.load_dir(&self.project.scripts_dir());
        let mut targets = vec!["self".to_string(), "player".to_string()];
        let mut flags = Vec::new();
        if let Some(doc) = &self.doc {
            targets.extend(doc.map.entities.iter().filter(|e| !e.name.is_empty()).map(|e| e.name.clone()));
            collect_flags(&doc.map, &mut flags);
        }
        flags.sort();
        flags.dedup();
        self.refs = Refs {
            items: self.project.db.items.iter().map(|i| i.id.clone()).collect(),
            maps: self.maps.clone(),
            targets,
            functions: scripts.function_names(),
            spawns,
            transitions,
            current_map: self.doc.as_ref().map(|d| d.map.name.clone()).unwrap_or_default(),
            flags,
            sounds: self.project.db.sounds.iter().map(|s| s.id.clone()).collect(),
            characters: self.project.db.characters.iter().map(|c| c.id.clone()).collect(),
            states: {
                let mut v: Vec<String> = self.project.db.props.iter().flat_map(|p| p.state_names()).collect();
                v.sort();
                v.dedup();
                v
            },
            quests: self.project.db.quests.iter().map(|q| q.id.clone()).collect(),
            quest_stages: self.project.db.quests.iter().map(|q| (q.id.clone(), q.stages.clone())).collect(),
        };
    }

    /// Renames a definition id (or a map) and rewrites every reference in the project:
    /// definitions, project config, all map files and the open map. Saves immediately.
    /// Follows a renamed prop state in placed props (open map and maps on disk): their start
    /// state, lock / container overrides and their own triggers aimed at "self".
    pub fn rename_prop_state(&mut self, prop: &str, old: &str, new: &str) {
        let fix = |e: &mut EntityInstance| {
            if e.kind != EntityKind::Prop || e.def != prop {
                return false;
            }
            let before = format!("{:?}{:?}{:?}{:?}", e.state, e.lock, e.container, e.triggers);
            if e.state == old {
                e.state = new.to_string();
            }
            if let Some(l) = &mut e.lock {
                if let rust2d_engine::defs::OnUnlock::ToState { state } = &mut l.on_unlock {
                    if state == old {
                        *state = new.to_string();
                    }
                }
            }
            if let Some(c) = &mut e.container {
                if c.open_state == old {
                    c.open_state = new.to_string();
                }
            }
            crate::database::rename_state_in_triggers(&mut e.triggers, old, new);
            before != format!("{:?}{:?}{:?}{:?}", e.state, e.lock, e.container, e.triggers)
        };
        let mut changed = 0;
        let open = self.doc.as_ref().map(|d| d.map.name.clone());
        if let Some(doc) = &mut self.doc {
            for e in &mut doc.map.entities {
                if fix(e) {
                    changed += 1;
                    doc.dirty = true;
                }
            }
        }
        for m in self.maps.clone() {
            if Some(&m) == open.as_ref() {
                continue;
            }
            let Ok(mut map) = self.project.load_map(&m) else { continue };
            let n = map.entities.iter_mut().map(|e| fix(e) as usize).sum::<usize>();
            if n > 0 && self.project.save_map(&map).is_ok() {
                changed += n;
            }
        }
        self.defs_dirty = true;
        self.set_status(format!("Renamed state '{old}' → '{new}' of '{prop}' ({changed} placed prop(s) updated). Save the definitions to keep it."));
    }

    pub fn rename_id(&mut self, kind: RefKind, old: &str, new: &str) {
        let new = new.trim();
        if new == old || new.is_empty() {
            return;
        }
        if !valid_name(new) {
            self.set_status("Ids may only contain letters, digits, '_' and '-'");
            return;
        }
        let db = &mut self.project.db;
        let taken = match kind {
            RefKind::Tileset => db.tilesets.iter().any(|d| d.id == new),
            RefKind::Sheet => db.sheets.iter().any(|d| d.id == new),
            RefKind::Item => db.items.iter().any(|d| d.id == new),
            RefKind::Prop => db.props.iter().any(|d| d.id == new),
            RefKind::Character => db.characters.iter().any(|d| d.id == new),
            RefKind::Sound => db.sounds.iter().any(|d| d.id == new),
            RefKind::Map => self.maps.iter().any(|m| m == new),
        };
        if taken {
            self.set_status(format!("'{new}' is already used"));
            return;
        }
        // The thing itself.
        macro_rules! rename_def {
            ($list:expr) => {
                if let Some(d) = $list.iter_mut().find(|d| d.id == old) {
                    d.id = new.to_string();
                }
            };
        }
        match kind {
            RefKind::Tileset => rename_def!(db.tilesets),
            RefKind::Sheet => rename_def!(db.sheets),
            RefKind::Item => rename_def!(db.items),
            RefKind::Prop => rename_def!(db.props),
            RefKind::Character => rename_def!(db.characters),
            RefKind::Sound => rename_def!(db.sounds),
            RefKind::Map => {
                if let Err(e) = self.project.rename_map_file(old, new) {
                    self.set_status(format!("Rename failed: {e:#}"));
                    return;
                }
                for m in &mut self.maps {
                    if m == old {
                        *m = new.to_string();
                    }
                }
                self.maps.sort();
            }
        }
        let mut refs = refactor::rename_in_db(&mut self.project.db, &mut self.project.config, kind, old, new);

        // Every map on disk, then the open one (which may have unsaved edits).
        let mut maps_changed = 0;
        let mut errors = Vec::new();
        for name in self.maps.clone() {
            match self.project.load_map(&name) {
                Ok(mut map) => {
                    let n = refactor::rename_in_map(&mut map, kind, old, new);
                    if n > 0 {
                        refs += n;
                        maps_changed += 1;
                        if let Err(e) = self.project.save_map(&map) {
                            errors.push(format!("{name}: {e:#}"));
                        }
                    }
                }
                Err(e) => errors.push(format!("{name}: {e:#}")),
            }
        }
        if let Some(doc) = &mut self.doc {
            if refactor::rename_in_map(&mut doc.map, kind, old, new) > 0 {
                // Old snapshots still contain the old id.
                doc.clear_history();
            }
        }

        // Editor state that refers to ids.
        match kind {
            RefKind::Tileset if self.brush.tileset == old => self.brush.tileset = new.to_string(),
            RefKind::Prop | RefKind::Character | RefKind::Item if self.place_def == old => self.place_def = new.to_string(),
            _ => {}
        }
        self.save_defs();
        if matches!(kind, RefKind::Tileset | RefKind::Sheet | RefKind::Sound) {
            self.reload_assets();
        }
        self.refresh_refs();
        let mut msg = format!("Renamed '{old}' → '{new}': {refs} reference(s) updated in {maps_changed} map file(s)");
        if matches!(kind, RefKind::Item | RefKind::Sound | RefKind::Map) {
            msg.push_str(" (strings inside scripts are not changed)");
        }
        if !errors.is_empty() {
            msg = format!("{msg}; errors: {}", errors.join("; "));
        }
        self.set_status(msg);
    }

    pub fn play(&mut self, here: bool) {
        self.save_all();
        let exe = std::env::current_exe().ok().map(|p| p.with_file_name("rust2d-game"));
        let Some(exe) = exe.filter(|p| p.exists()) else {
            self.set_status("rust2d-game not found next to the editor; run `cargo build -p rust2d-game`");
            return;
        };
        let mut cmd = std::process::Command::new(exe);
        cmd.arg(&self.project.root);
        if !here {
            cmd.arg("--no-title");
        }
        if here {
            if let Some(doc) = &self.doc {
                cmd.args(["--map", &doc.map.name]);
                let spawn = match self.sel {
                    Selection::Zone(uid) => doc.map.zones.iter().find(|z| z.uid == uid && z.kind == ZoneKind::Spawn),
                    _ => None,
                };
                match spawn {
                    Some(z) => cmd.args(["--spawn", &z.name]),
                    None => cmd.args(["--pos", &format!("{},{}", self.cam.center.x, self.cam.center.y)]),
                };
            }
        }
        // A game built from a different engine may not understand this editor's map data
        // (e.g. `cargo run -p rust2d-editor` does not rebuild rust2d-game).
        let game_build = std::process::Command::new(cmd.get_program())
            .arg("--engine-build")
            .output()
            .ok()
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string());
        let stale = game_build.as_deref() != Some(rust2d_engine::ENGINE_BUILD);
        match cmd.spawn() {
            Ok(_) if stale => self.set_status(
                "Play-test started, but rust2d-game was built from a different engine version; if it fails, run `cargo build -p rust2d-game` (or `cargo build --workspace`)",
            ),
            Ok(_) => self.set_status("Play-test started"),
            Err(e) => self.set_status(format!("Failed to start game: {e}")),
        }
    }

    // ------------------------------------------------------------ files

    /// Replaces the editor state with another project.
    pub fn switch_project(&mut self, root: PathBuf) {
        // (Callers go through `request`, so unsaved changes were saved or discarded.)
        if !root.join("project.ron").exists() {
            self.set_status(format!("{} is not a rust2d project (no project.ron)", root.display()));
            return;
        }
        match Editor::new(root.clone()) {
            Ok(mut e) => {
                self.audio.stop_music();
                e.canvas_rect = self.canvas_rect;
                *self = e;
                self.fit_view();
                self.set_status(format!("Opened project {}", root.display()));
            }
            Err(e) => self.set_status(format!("Could not open project: {e:#}")),
        }
    }

    pub fn open_file_dialog(&mut self, purpose: files::Purpose) {
        if self.file_dialog.is_some() {
            self.set_status("A file dialog is already open");
            return;
        }
        let start = match purpose {
            files::Purpose::OpenProject | files::Purpose::NewProject => match self.project.root.parent() {
                Some(p) if self.project_open => Some(p.to_path_buf()),
                _ => std::env::var_os("HOME").map(PathBuf::from),
            },
            _ => Some(self.project.root.clone()),
        };
        self.file_dialog = Some(files::PendingDialog::open(purpose, start));
    }

    /// Applies the result of a file dialog (also used directly by the debug script).
    pub fn handle_file(&mut self, purpose: files::Purpose, path: PathBuf) {
        use files::Purpose as P;
        let root = self.project.root.clone();
        let import = |sub: &str| files::import_asset(&root, &path, sub);
        match purpose {
            P::ImportPackage { into_library } => {
                let r = self.import_package(into_library, &path);
                self.show_result(r);
            }
            P::ExportPackage { from_library, assets } => {
                let r = self.export_package(from_library, &assets, &path);
                self.show_result(r);
            }
            P::ExportGameFolder => {
                if let Some(d) = &mut self.export {
                    d.out = path.display().to_string();
                }
            }
            P::OpenProject => self.request(Pending::OpenProject(path)),
            P::NewProject => self.request(Pending::NewProject(path)),
            P::TilesetImage(ref id) | P::SheetImage(ref id) | P::SoundFile(ref id) => {
                let id = id.clone();
                let sub = match purpose {
                    P::TilesetImage(_) => "tilesets",
                    P::SheetImage(_) => "sprites",
                    _ => "sounds",
                };
                let rel = match import(sub) {
                    Ok(rel) => rel,
                    Err(e) => {
                        self.set_status(format!("Import failed: {e:#}"));
                        return;
                    }
                };
                let db = &mut self.project.db;
                match purpose {
                    P::TilesetImage(_) => db.tilesets.iter_mut().filter(|d| d.id == id).for_each(|d| d.image = rel.clone()),
                    P::SheetImage(_) => db.sheets.iter_mut().filter(|d| d.id == id).for_each(|d| d.image = rel.clone()),
                    _ => db.sounds.iter_mut().filter(|d| d.id == id).for_each(|d| d.file = rel.clone()),
                }
                self.defs_dirty = true;
                self.reload_assets();
                self.set_status(format!("Using {rel}"));
            }
        }
    }

    /// Shows the outcome of a library / package action in the status bar and the library page.
    pub fn show_result(&mut self, r: anyhow::Result<String>) {
        let msg = match r {
            Ok(m) => m,
            Err(e) => format!("Error: {e:#}"),
        };
        self.set_status(msg.clone());
        self.lib.report = Some(msg);
    }

    fn poll_file_dialog(&mut self) {
        if let Some(req) = self.browse_request.take() {
            self.open_file_dialog(req);
        }
        let Some(d) = &self.file_dialog else { return };
        let Some(result) = d.poll() else { return };
        let purpose = d.purpose.clone();
        self.file_dialog = None;
        if let Some(path) = result {
            self.handle_file(purpose, path);
        }
    }

    // ------------------------------------------------------------ helpers

    /// Brush tile at stamp offset (dx, dy), registering the tileset on the map.
    pub fn stamp_tile(&mut self, dx: u32, dy: u32) -> Tile {
        let Some(doc) = &mut self.doc else { return Tile::EMPTY };
        let Some(ts) = self.assets.tilesets.get(&self.brush.tileset) else { return Tile::EMPTY };
        let slot = doc.map.tileset_slot(&self.brush.tileset);
        let (x, y) = (self.brush.x + dx % self.brush.w, self.brush.y + dy % self.brush.h);
        Tile::new(slot, y * ts.columns + x)
    }

    /// Snaps an entity's feet position to the grid. Props an even number of tiles wide are
    /// centered on a tile edge, odd ones on a tile center (so they cover whole tiles).
    pub fn snap_entity(&self, world: Vec2, free: bool, tiles_wide: u32) -> Vec2 {
        let ts = self.ts();
        if free {
            return world.round();
        }
        let ty = (world.y / ts).floor();
        let x = if tiles_wide.is_multiple_of(2) { (world.x / ts).round() * ts } else { ((world.x / ts).floor() + 0.5) * ts };
        macroquad::math::vec2(x, (ty + 1.0) * ts - 1.0)
    }

    /// Sprite of an entity definition (props / items; characters use their idle frame).
    pub fn def_sprite(&self, kind: EntityKind, def: &str) -> Option<rust2d_engine::defs::SpriteRef> {
        let db = &self.project.db;
        match kind {
            EntityKind::Prop => db.prop(def).map(|d| d.sprite.clone()),
            EntityKind::Item => db.item(def).map(|d| d.icon.clone()),
            EntityKind::Character => db.character(def).map(|d| rust2d_engine::defs::SpriteRef::new(&d.sheet, 0)),
        }
    }

    /// Width of an entity definition's sprite in whole tiles (for snapping).
    pub fn def_tiles_wide(&self, kind: EntityKind, def: &str) -> u32 {
        let ts = self.ts();
        self.def_sprite(kind, def).map_or(1, |s| (self.assets.sprite_size(&s, ts).x / ts).round().max(1.0) as u32)
    }

    pub fn place_entity(&mut self, pos: Vec2) {
        if self.place_def.is_empty() {
            self.set_status("Choose a definition to place first");
            return;
        }
        let Some(doc) = &mut self.doc else { return };
        let before = Objects::of(&doc.map);
        let uid = doc.map.alloc_uid();
        doc.map.entities.push(EntityInstance::new(uid, self.place_kind, &self.place_def, pos.x, pos.y));
        doc.record_objects(before, "place");
        self.sel = Selection::Entity(uid);
        self.remember_placed();
    }

    /// Arms a definition for placing (empty `def` = stop placing).
    pub fn arm(&mut self, kind: EntityKind, def: &str) {
        self.place_kind = kind;
        self.place_def = def.to_string();
        if def.is_empty() {
            self.set_status("Placing off: clicks select");
        } else {
            self.remember_placed();
        }
    }

    pub fn disarm(&mut self) {
        if !self.place_def.is_empty() {
            self.arm(self.place_kind, "");
        }
    }

    fn remember_placed(&mut self) {
        let cur = (self.place_kind, self.place_def.clone());
        self.place_recent.retain(|r| *r != cur);
        self.place_recent.insert(0, cur);
        self.place_recent.truncate(RECENT_MAX);
    }

    /// Definitions shown in the entity palette (current tab + search): (kind, id, name, sprite).
    pub fn palette_entries(&self) -> Vec<(EntityKind, String, String, SpriteRef)> {
        let db = &self.project.db;
        let q = self.place_filter.trim().to_lowercase();
        EntityKind::ALL
            .into_iter()
            .filter(|k| self.place_tab.is_none_or(|t| t == *k))
            .flat_map(|k| -> Vec<(EntityKind, String, String, SpriteRef)> {
                match k {
                    EntityKind::Prop => db.props.iter().map(|d| (k, d.id.clone(), d.name.clone(), d.sprite.clone())).collect(),
                    EntityKind::Character => db.characters.iter().map(|d| (k, d.id.clone(), d.name.clone(), SpriteRef::new(&d.sheet, 0))).collect(),
                    EntityKind::Item => db.items.iter().map(|d| (k, d.id.clone(), d.name.clone(), d.icon.clone())).collect(),
                }
            })
            .filter(|(_, id, name, _)| q.is_empty() || id.to_lowercase().contains(&q) || name.to_lowercase().contains(&q))
            .collect()
    }

    /// Arms the next / previous palette entry (`[` / `]`).
    pub fn cycle_place(&mut self, step: i32) {
        let entries = self.palette_entries();
        if entries.is_empty() {
            return;
        }
        let n = entries.len() as i32;
        let cur = entries.iter().position(|(k, id, ..)| *k == self.place_kind && *id == self.place_def);
        let i = match cur {
            Some(i) => (i as i32 + step).rem_euclid(n),
            None if step > 0 => 0,
            None => n - 1,
        } as usize;
        let (k, id, name, _) = entries[i].clone();
        self.arm(k, &id);
        self.set_status(format!("Placing {name}"));
    }

    pub fn delete_selection(&mut self) {
        let Some(doc) = &mut self.doc else { return };
        let before = Objects::of(&doc.map);
        match self.sel {
            Selection::Entity(uid) => doc.map.entities.retain(|e| e.uid != uid),
            Selection::Zone(uid) => doc.map.zones.retain(|z| z.uid != uid),
            Selection::None => return,
        }
        doc.record_objects(before, "delete");
        self.sel = Selection::None;
    }

    pub fn duplicate_selection(&mut self) {
        let ts = self.ts();
        let Some(doc) = &mut self.doc else { return };
        let before = Objects::of(&doc.map);
        match self.sel {
            Selection::Entity(uid) => {
                let Some(mut e) = doc.map.entity(uid).cloned() else { return };
                e.uid = doc.map.alloc_uid();
                e.x += ts;
                if !e.name.is_empty() {
                    e.name = format!("{}_copy", e.name);
                }
                self.sel = Selection::Entity(e.uid);
                doc.map.entities.push(e);
            }
            Selection::Zone(uid) => {
                let Some(mut z) = doc.map.zones.iter().find(|z| z.uid == uid).cloned() else { return };
                z.uid = doc.map.alloc_uid();
                z.x = (z.x + 1).min(doc.map.width - 1);
                z.name = format!("{}_copy", z.name);
                self.sel = Selection::Zone(z.uid);
                doc.map.zones.push(z);
            }
            Selection::None => return,
        }
        doc.record_objects(before, "duplicate");
    }

    // ------------------------------------------------------------ egui

    pub fn ui(&mut self, ctx: &egui::Context) {
        for p in std::mem::take(&mut self.probe) {
            let layer = ctx.layer_id_at(p);
            let ppp = ctx.pixels_per_point();
            println!("[probe] {p:?}: layer {layer:?} (pixels per point {ppp})");
        }
        self.poll_file_dialog();
        if self.tex_dirty {
            self.tex = Textures::build(ctx, &self.assets);
            self.tex_dirty = false;
        }
        self.menu_bar(ctx);
        if !self.project_open {
            self.status_bar(ctx);
            self.welcome(ctx);
            self.dialogs(ctx);
            self.canvas_rect = ctx.available_rect();
            self.pointer_over_ui = true;
            self.keyboard_busy = ctx.wants_keyboard_input();
            return;
        }
        self.status_bar(ctx);
        match self.mode {
            Mode::Map => {
                egui::SidePanel::left("left").resizable(true).default_width(290.0).show(ctx, |ui| {
                    egui::ScrollArea::vertical().id_salt("left_scroll").show(ui, |ui| self.left_panel(ui));
                });
                egui::SidePanel::right("right").resizable(true).default_width(340.0).show(ctx, |ui| {
                    egui::ScrollArea::vertical().id_salt("right_scroll").show(ui, |ui| self.inspector(ui));
                });
            }
            Mode::Database => {
                egui::CentralPanel::default().show(ctx, |ui| crate::database::ui(self, ui));
            }
            Mode::Library => self.library_ui(ctx),
            Mode::Pixel => self.pixel_ui(ctx),
        }
        self.wizard_window(ctx);
        self.map_windows(ctx);
        self.model_window(ctx);
        self.handle_transition_request();
        if let Some((m, zone)) = GOTO.with(|g| g.borrow_mut().take()) {
            self.goto_map(&m, Some(&zone));
        }
        self.export_window(ctx);
        self.canvas_rect = ctx.available_rect();
        self.dialogs(ctx);
        self.pointer_over_ui = ctx.is_pointer_over_area() || ctx.wants_pointer_input();
        self.keyboard_busy = ctx.wants_keyboard_input();
    }

    /// Start screen when no project is open.
    fn welcome(&mut self, ctx: &egui::Context) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.vertical_centered(|ui| {
                ui.add_space(80.0);
                ui.heading(egui::RichText::new("rust2d editor").size(34.0));
                ui.label("Maps, characters, props, items and events for top-down RPGs.");
                ui.add_space(24.0);
                if ui.add_sized([260.0, 36.0], egui::Button::new("✨ New project…")).on_hover_text("Choose an empty folder for the project").clicked() {
                    self.request(Pending::NewProject(PathBuf::new()));
                }
                ui.add_space(6.0);
                if ui.add_sized([260.0, 36.0], egui::Button::new("📂 Open project…")).on_hover_text("Choose a folder containing project.ron").clicked() {
                    self.open_file_dialog(files::Purpose::OpenProject);
                }
                let recent = files::recent_projects();
                if !recent.is_empty() {
                    ui.add_space(24.0);
                    ui.label(egui::RichText::new("Recent projects").strong());
                    for p in recent {
                        let name = p.file_name().map_or(p.display().to_string(), |n| n.to_string_lossy().into_owned());
                        if ui.add_sized([360.0, 28.0], egui::Button::new(&name)).on_hover_text(p.display().to_string()).clicked() {
                            self.request(Pending::OpenProject(p));
                        }
                    }
                }
                ui.add_space(24.0);
                ui.weak("Tip: the asset library (📂 Library, once a project is open) has ready-made tiles, characters, props and items.");
            });
        });
    }

    fn menu_bar(&mut self, ctx: &egui::Context) {
        if !self.project_open {
            egui::TopBottomPanel::top("menu").show(ctx, |ui| {
                egui::menu::bar(ui, |ui| {
                    ui.menu_button("File", |ui| {
                        if ui.button("New project…").clicked() {
                            self.request(Pending::NewProject(PathBuf::new()));
                            ui.close_menu();
                        }
                        if ui.button("Open project…").clicked() {
                            self.open_file_dialog(files::Purpose::OpenProject);
                            ui.close_menu();
                        }
                        ui.separator();
                        if ui.button("Quit").clicked() {
                            self.request(Pending::Quit);
                            ui.close_menu();
                        }
                    });
                    ui.separator();
                    ui.label(egui::RichText::new("(no project)").weak());
                });
            });
            return;
        }
        egui::TopBottomPanel::top("menu").show(ctx, |ui| {
            egui::menu::bar(ui, |ui| {
                ui.menu_button("File", |ui| {
                    if ui.button("New project…").clicked() {
                        self.request(Pending::NewProject(PathBuf::new()));
                        ui.close_menu();
                    }
                    if ui.button("Open project…").clicked() {
                        self.open_file_dialog(files::Purpose::OpenProject);
                        ui.close_menu();
                    }
                    if ui.button("Close project").clicked() {
                        self.request(Pending::CloseProject);
                        ui.close_menu();
                    }
                    ui.separator();
                    if ui.button("New map…").clicked() {
                        self.request(Pending::New);
                        ui.close_menu();
                    }
                    ui.menu_button("Open map", |ui| {
                        for m in self.maps.clone() {
                            if ui.button(&m).clicked() {
                                self.request(Pending::Open(m));
                                ui.close_menu();
                            }
                        }
                    });
                    ui.separator();
                    if ui.button("Save map   Ctrl+S").clicked() {
                        self.save_all();
                        ui.close_menu();
                    }
                    if ui.button("Save definitions").clicked() {
                        self.save_defs();
                        ui.close_menu();
                    }
                    ui.separator();
                    if ui.button("📦 Export game (standalone)…").clicked() {
                        self.open_export();
                        ui.close_menu();
                    }
                    if ui.button("Import asset package into project…").clicked() {
                        self.open_file_dialog(files::Purpose::ImportPackage { into_library: false });
                        ui.close_menu();
                    }
                    if ui.button("New asset…").clicked() {
                        self.wizard = Some(crate::wizard::Wizard::new(&self.project));
                        ui.close_menu();
                    }
                    ui.separator();
                    if ui.button("Reload assets & scripts").clicked() {
                        self.reload_assets();
                        ui.close_menu();
                    }
                    ui.separator();
                    if ui.button("Quit").clicked() {
                        self.request(Pending::Quit);
                        ui.close_menu();
                    }
                });
                ui.menu_button("Edit", |ui| {
                    let (u, r) = self.doc.as_ref().map_or((false, false), |d| (d.can_undo(), d.can_redo()));
                    if ui.add_enabled(u, egui::Button::new("Undo   Ctrl+Z")).clicked() {
                        if let Some(d) = &mut self.doc {
                            d.undo(&self.assets);
                        }
                        ui.close_menu();
                    }
                    if ui.add_enabled(r, egui::Button::new("Redo   Ctrl+Y")).clicked() {
                        if let Some(d) = &mut self.doc {
                            d.redo(&self.assets);
                        }
                        ui.close_menu();
                    }
                    ui.separator();
                    if ui.add_enabled(self.sel != Selection::None, egui::Button::new("Duplicate   Ctrl+D")).clicked() {
                        self.duplicate_selection();
                        ui.close_menu();
                    }
                    if ui.add_enabled(self.sel != Selection::None, egui::Button::new("Delete   Del")).clicked() {
                        self.delete_selection();
                        ui.close_menu();
                    }
                });
                ui.menu_button("View", |ui| {
                    ui.checkbox(&mut self.view.grid, "Grid   G");
                    ui.checkbox(&mut self.view.collision, "Collision overlay");
                    ui.checkbox(&mut self.view.zones, "Zones & edges");
                    ui.checkbox(&mut self.view.dim_layers, "Dim inactive layers");
                    ui.checkbox(&mut self.view.minimap, "Minimap");
                    if ui.button("Fit map   Home").clicked() {
                        self.fit_view();
                        ui.close_menu();
                    }
                });
                ui.menu_button("Play", |ui| {
                    if ui.button("Play here   F5").clicked() {
                        self.play(true);
                        ui.close_menu();
                    }
                    if ui.button("Play from start   F6").clicked() {
                        self.play(false);
                        ui.close_menu();
                    }
                });
                ui.separator();
                ui.selectable_value(&mut self.mode, Mode::Map, "🗺 Map");
                ui.selectable_value(&mut self.mode, Mode::Database, "📚 Database");
                ui.selectable_value(&mut self.mode, Mode::Library, "📂 Library");
                let px_label = if self.px.is_dirty() { "✏ Pixel editor •" } else { "✏ Pixel editor" };
                ui.selectable_value(&mut self.mode, Mode::Pixel, px_label);
                ui.separator();
                self.map_dropdown(ui);
                if self.defs_dirty {
                    ui.colored_label(Color32::YELLOW, "defs •");
                }
            });
        });
    }

    fn status_bar(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::bottom("status").show(ctx, |ui| {
            ui.horizontal(|ui| {
                if let (Some(doc), Some((x, y))) = (&self.doc, self.hover_tile) {
                    ui.monospace(format!("tile {x},{y}"));
                    if doc.map.in_bounds(x, y) {
                        let t = doc.map.layers.get(self.layer).map(|l| l.get(x as u32, y as u32));
                        if let Some((slot, i)) = t.and_then(|t| t.parts()) {
                            ui.monospace(format!("[{}#{}]", doc.map.tilesets.get(slot).map_or("?", |s| s.as_str()), i));
                        }
                    }
                    ui.separator();
                }
                ui.monospace(format!("zoom {:.0}%", self.cam.zoom * 100.0));
                if let Some(doc) = &self.doc {
                    ui.separator();
                    ui.monospace(format!("{}x{}  ents {}  zones {}", doc.map.width, doc.map.height, doc.map.entities.len(), doc.map.zones.len()));
                }
                ui.separator();
                ui.monospace(format!("{} fps", macroquad::time::get_fps()));
                if self.file_dialog.is_some() {
                    ui.separator();
                    ui.colored_label(Color32::YELLOW, "File dialog open…");
                }
                if get_time() - self.status.1 < 6.0 {
                    ui.separator();
                    ui.label(&self.status.0);
                }
            });
        });
    }

    /// Applies the answer to the "Unsaved changes" question for `p`.
    pub fn settle_pending(&mut self, p: Pending, choice: Choice) {
        match choice {
            Choice::None => self.pending = Some(p),
            Choice::Cancel => {}
            Choice::Save | Choice::Discard => {
                if choice == Choice::Save {
                    self.save_all();
                } else {
                    if let Some(d) = &mut self.doc {
                        d.dirty = false;
                    }
                    self.defs_dirty = false;
                    self.px.doc = None;
                }
                if self.unsaved_for(&p) {
                    // Saving failed: stay (the status bar shows the error).
                    self.set_status("Some changes could not be saved; nothing was closed");
                } else {
                    self.run_pending(p);
                }
            }
        }
    }

    /// Whether anything `p` would throw away is still unsaved.
    fn unsaved_for(&self, p: &Pending) -> bool {
        let leaving_project = matches!(p, Pending::Quit | Pending::OpenProject(_) | Pending::NewProject(_) | Pending::CloseProject);
        self.doc.as_ref().is_some_and(|d| d.dirty) || (leaving_project && (self.defs_dirty || self.px.is_dirty()))
    }

    pub fn request(&mut self, p: Pending) {
        if self.unsaved_for(&p) {
            self.pending = Some(p);
        } else {
            self.run_pending(p);
        }
    }

    fn run_pending(&mut self, p: Pending) {
        match p {
            Pending::OpenProject(dir) => self.switch_project(dir),
            // Unsaved work is settled at this point; now choose the folder.
            Pending::NewProject(dir) if dir.as_os_str().is_empty() => self.open_file_dialog(files::Purpose::NewProject),
            Pending::NewProject(dir) => match files::create_project(&dir) {
                Ok(()) => {
                    // Opening replaces the whole editor state, closing the old project.
                    self.close_project();
                    self.switch_project(dir);
                    self.set_status("New project created. Add art from 📂 Library (or Database → Tilesets), then paint a map.");
                }
                Err(e) => self.set_status(format!("Could not create project: {e:#}")),
            },
            Pending::Open(m) => self.open_map(&m),
            Pending::OpenAt(m, zone) => {
                self.open_map(&m);
                if !zone.is_empty() {
                    self.focus_zone(&zone);
                }
            }
            Pending::New => {
                self.new_map = Some(NewMapDialog { name: "new_map".into(), width: 64, height: 64, fill: true });
            }
            Pending::Quit => self.quit = true,
            Pending::CloseProject => {
                self.close_project();
                self.set_status("Project closed");
            }
        }
    }

    fn dialogs(&mut self, ctx: &egui::Context) {
        if let Some(p) = self.pending.take() {
            let mut choice = Choice::None;
            let mut what = Vec::new();
            if let Some(d) = self.doc.as_ref().filter(|d| d.dirty) {
                what.push(format!("map '{}'", d.map.name));
            }
            if self.defs_dirty {
                what.push("definitions".to_string());
            }
            if let Some(d) = self.px.doc.as_ref().filter(|d| d.dirty) {
                what.push(format!("image {}", d.rel));
            }
            egui::Window::new("Unsaved changes").collapsible(false).resizable(false).anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0]).show(ctx, |ui| {
                ui.label(format!("Unsaved changes: {}.", what.join(", ")));
                ui.horizontal(|ui| {
                    if ui.button("Save").clicked() {
                        choice = Choice::Save;
                    }
                    if ui.button("Discard").clicked() {
                        choice = Choice::Discard;
                    }
                    if ui.button("Cancel").clicked() {
                        choice = Choice::Cancel;
                    }
                });
            });
            match choice {
                Choice::None => self.pending = Some(p),
                c => self.settle_pending(p, c),
            }
        }

        if let Some(mut d) = self.new_map.take() {
            let mut open = true;
            let mut create = false;
            egui::Window::new("New map").collapsible(false).resizable(false).anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0]).show(ctx, |ui| {
                egui::Grid::new("newmap").show(ui, |ui| {
                    ui.label("Name");
                    ui.text_edit_singleline(&mut d.name);
                    ui.end_row();
                    ui.label("Width");
                    ui.add(egui::DragValue::new(&mut d.width).range(1..=MAX_MAP_SIZE));
                    ui.end_row();
                    ui.label("Height");
                    ui.add(egui::DragValue::new(&mut d.height).range(1..=MAX_MAP_SIZE));
                    ui.end_row();
                });
                ui.checkbox(&mut d.fill, "Fill ground with the current brush tile");
                let valid = valid_name(&d.name);
                let exists = self.maps.contains(&d.name);
                if !valid {
                    ui.colored_label(Color32::LIGHT_RED, "Use letters, digits, '_' or '-'");
                } else if exists {
                    ui.colored_label(Color32::LIGHT_RED, "A map with this name exists");
                }
                ui.horizontal(|ui| {
                    if ui.add_enabled(valid && !exists, egui::Button::new("Create")).clicked() {
                        create = true;
                    }
                    if ui.button("Cancel").clicked() {
                        open = false;
                    }
                });
            });
            if create {
                let mut map = Map::new(&d.name, d.width, d.height);
                self.doc = None;
                if d.fill {
                    if let Some(ts) = self.assets.tilesets.get(&self.brush.tileset) {
                        let slot = map.tileset_slot(&self.brush.tileset);
                        let t = Tile::new(slot, self.brush.y * ts.columns + self.brush.x);
                        for y in 0..map.height {
                            for x in 0..map.width {
                                map.layers[0].set(x, y, t);
                            }
                        }
                    }
                }
                self.load_doc(map);
                self.save_map();
            } else if open {
                self.new_map = Some(d);
            }
        }

        if let Some((mut w, mut h)) = self.resize.take() {
            let mut open = true;
            let mut apply = false;
            egui::Window::new("Resize map").collapsible(false).resizable(false).anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0]).show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label("Width");
                    ui.add(egui::DragValue::new(&mut w).range(1..=MAX_MAP_SIZE));
                    ui.label("Height");
                    ui.add(egui::DragValue::new(&mut h).range(1..=MAX_MAP_SIZE));
                });
                ui.weak("The top-left area is kept. Undoable.");
                ui.horizontal(|ui| {
                    apply = ui.button("Apply").clicked();
                    open = !ui.button("Cancel").clicked();
                });
            });
            if apply {
                if let Some(doc) = &mut self.doc {
                    let before = doc.map.clone();
                    doc.map.resize(w, h);
                    doc.record_map(before, &self.assets);
                }
            } else if open {
                self.resize = Some((w, h));
            }
        }
    }

    // ------------------------------------------------------------ left panel

    fn left_panel(&mut self, ui: &mut egui::Ui) {
        ui.heading("Tools");
        egui::Grid::new("tools").num_columns(2).show(ui, |ui| {
            for (i, (tool, label, key)) in Tool::ALL.iter().enumerate() {
                if ui.selectable_label(self.tool == *tool, *label).on_hover_text(format!("Shortcut: {key}")).clicked() {
                    self.tool = *tool;
                }
                if i % 2 == 1 {
                    ui.end_row();
                }
            }
        });
        ui.separator();

        if let Some(doc) = &mut self.doc {
            ui.heading("Layers");
            let n = doc.map.layers.len();
            let mut layer_op: Option<(&str, usize)> = None;
            for i in 0..n {
                let l = &mut doc.map.layers[i];
                ui.horizontal(|ui| {
                    ui.checkbox(&mut l.visible, "").on_hover_text("Visible");
                    if ui.selectable_label(self.layer == i, format!("{}. {}", i + 1, l.name)).clicked() {
                        self.layer = i;
                    }
                    if l.above_entities {
                        ui.weak("overhead");
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if n > 1 && ui.small_button("✖").on_hover_text("Delete layer").clicked() {
                            layer_op = Some(("del", i));
                        }
                        if i + 1 < n && ui.small_button("⏷").clicked() {
                            layer_op = Some(("down", i));
                        }
                        if i > 0 && ui.small_button("⏶").clicked() {
                            layer_op = Some(("up", i));
                        }
                    });
                });
            }
            if let Some(l) = doc.map.layers.get_mut(self.layer) {
                ui.horizontal(|ui| {
                    ui.label("Name");
                    ui.add(egui::TextEdit::singleline(&mut l.name).desired_width(110.0));
                    ui.checkbox(&mut l.above_entities, "overhead").on_hover_text("Drawn above characters, never collides");
                });
            }
            if ui.button("+ Layer").clicked() {
                layer_op = Some(("add", 0));
            }
            if let Some((op, i)) = layer_op {
                let before = doc.map.clone();
                match op {
                    "add" => {
                        let name = format!("Layer {}", n + 1);
                        doc.map.add_layer(&name, false);
                        self.layer = n;
                    }
                    "del" => {
                        doc.map.layers.remove(i);
                        self.layer = self.layer.min(doc.map.layers.len() - 1);
                    }
                    "up" => {
                        doc.map.layers.swap(i, i - 1);
                        self.layer = i - 1;
                    }
                    _ => {
                        doc.map.layers.swap(i, i + 1);
                        self.layer = i + 1;
                    }
                }
                doc.record_map(before, &self.assets);
            }
            ui.separator();
        }

        match self.tool {
            Tool::Select => {
                ui.heading("Select");
                ui.label("Click an entity or zone to select it, drag to move it. Click again to cycle through objects on the same spot. Clicking empty space or right-click deselects.");
                ui.weak("Nothing is painted with this tool. Pick a tile below (or press B / R / F) to paint; Esc returns here.");
                ui.separator();
                self.palette(ui);
            }
            t if t.paints_tiles() => self.palette(ui),
            Tool::Collision => {
                ui.heading("Collision");
                ui.label("Left: force solid\nRight: force walkable\nShift+Left: reset to tileset default");
                ui.weak("Red = blocked, green = walkable override. Tileset tiles flagged solid block automatically (edit flags in Database → Tilesets).");
            }
            Tool::Entity => self.entity_palette(ui),
            Tool::Link => {
                ui.heading("Transition");
                ui.label("Link two maps with a pair of transition zones, e.g. where a road leaves one map and enters the other:");
                ui.label("1. Drag over the exit (the road end).");
                ui.label("2. Choose the target map.");
                ui.label("3. On the target map, drag the other end.");
                ui.weak("Each end leads to the other; players arrive one tile inside the other end (so they don't bounce back). Transition zones can also be drawn with the Zones tool and linked in the inspector (Leads to map / Other end, ✏ Draw the other end, ↗ Go to, Unlink). Portals and spawn points still work as before.");
            }
            Tool::Zone => {
                ui.heading("Zones");
                ui.label("Drag on the map to create:");
                ui.radio_value(&mut self.zone_template, ZoneTemplate::Spawn, "Spawn point");
                ui.radio_value(&mut self.zone_template, ZoneTemplate::Portal, "Portal (teleport on enter)");
                ui.radio_value(&mut self.zone_template, ZoneTemplate::Trigger, "Trigger area (events)");
                ui.radio_value(&mut self.zone_template, ZoneTemplate::Spawner, "Enemy spawner (invisible)");
                ui.radio_value(&mut self.zone_template, ZoneTemplate::Transition, "Map transition (link it to another map in the inspector)");
                ui.weak("Click a zone to select it, drag to move. Map-edge transitions are set in the map properties (right panel, nothing selected).");
            }
            _ => {}
        }
    }

    fn palette(&mut self, ui: &mut egui::Ui) {
        ui.heading("Tileset");
        let mut ids: Vec<String> = self.project.db.tilesets.iter().map(|t| t.id.clone()).collect();
        ids.sort();
        egui::ComboBox::from_id_salt("brush_ts").selected_text(self.brush.tileset.as_str()).show_ui(ui, |ui| {
            for id in &ids {
                if ui.selectable_label(*id == self.brush.tileset, id).clicked() {
                    self.brush = Brush { tileset: id.clone(), x: 0, y: 0, w: 1, h: 1 };
                }
            }
        });
        let (Some(tex), Some(ts)) = (self.tex.tilesets.get(&self.brush.tileset), self.assets.tilesets.get(&self.brush.tileset)) else {
            ui.label("No tileset");
            return;
        };
        ui.weak("Click a tile, or drag to select a multi-tile stamp. Right-click on the map picks a tile.");
        let tsz = self.assets.tile_size;
        let scale = ((ui.available_width() - 8.0) / ts.image.width as f32).clamp(1.0, 3.0).floor();
        let size = vec2(ts.image.width as f32 * scale, ts.image.height as f32 * scale);
        let cell = tsz * scale;
        let rows = ts.count.div_ceil(ts.columns);
        egui::ScrollArea::both().id_salt("palette").max_height(420.0).show(ui, |ui| {
            let (rect, resp) = ui.allocate_exact_size(size, Sense::click_and_drag());
            let p = ui.painter_at(rect);
            p.rect_filled(rect, 0.0, Color32::from_gray(30));
            p.image(tex.id(), rect, egui::Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
            let cell_at = |pos: egui::Pos2| -> (u32, u32) {
                let rel = pos - rect.min;
                (((rel.x / cell).floor().max(0.0) as u32).min(ts.columns - 1), ((rel.y / cell).floor().max(0.0) as u32).min(rows - 1))
            };
            if let Some(pos) = resp.interact_pointer_pos() {
                let (cx, cy) = cell_at(pos);
                if (resp.drag_started() || resp.clicked() || self.palette_drag.is_none())
                    && (resp.drag_started() || resp.clicked()) {
                        self.palette_drag = Some((cx, cy));
                    }
                if let Some((sx, sy)) = self.palette_drag {
                    self.brush.x = sx.min(cx);
                    self.brush.y = sy.min(cy);
                    self.brush.w = sx.abs_diff(cx) + 1;
                    self.brush.h = sy.abs_diff(cy) + 1;
                    // Picking a tile is the explicit way to start painting.
                    if matches!(self.tool, Tool::Erase | Tool::Pick | Tool::Select) {
                        self.tool = Tool::Brush;
                    }
                }
            }
            if resp.drag_stopped() || resp.clicked() {
                self.palette_drag = None;
            }
            for &s in &self.project.db.tileset(&self.brush.tileset).map(|d| d.solid.clone()).unwrap_or_default() {
                let r = egui::Rect::from_min_size(rect.min + vec2((s % ts.columns) as f32 * cell, (s / ts.columns) as f32 * cell), vec2(cell, cell));
                p.line_segment([r.right_top() + vec2(-6.0, 1.0), r.right_top() + vec2(-1.0, 1.0)], Stroke::new(2.0_f32, Color32::RED));
            }
            let sel = egui::Rect::from_min_size(
                rect.min + vec2(self.brush.x as f32 * cell, self.brush.y as f32 * cell),
                vec2(self.brush.w as f32 * cell, self.brush.h as f32 * cell),
            );
            p.rect_stroke(sel, 0.0, Stroke::new(2.0_f32, Color32::YELLOW), StrokeKind::Inside);
        });
        ui.label(format!("Stamp {}x{} at ({}, {})", self.brush.w, self.brush.h, self.brush.x, self.brush.y));
    }

    fn entity_palette(&mut self, ui: &mut egui::Ui) {
        // What a click on empty space does.
        ui.horizontal(|ui| {
            ui.heading("Place");
            if !self.place_def.is_empty() && ui.button("✖ Stop placing").on_hover_text("Right-click or Esc on the map").clicked() {
                self.disarm();
            }
        });
        let armed = self.def_sprite(self.place_kind, &self.place_def).zip(self.def_name(self.place_kind, &self.place_def));
        ui.horizontal(|ui| match armed {
            Some((sprite, name)) => {
                widgets::sprite_thumb(ui, &self.tex, &self.assets, &sprite, 28.0, true);
                ui.label(egui::RichText::new(format!("{name}  ({})", self.place_kind.label().to_lowercase())).strong());
            }
            None => {
                ui.weak("Nothing: clicks select / move. Pick a definition below.");
            }
        });
        ui.weak("Click empty space to place (Shift = no snapping) · right-click / Esc: stop placing · [ ] or PgUp / PgDn: previous / next · click to select, drag to move, Del deletes, Ctrl+D duplicates.");

        // Recently used definitions: one click to switch back.
        if !self.place_recent.is_empty() {
            ui.horizontal_wrapped(|ui| {
                ui.label("Recent");
                for (k, id) in self.place_recent.clone() {
                    let Some(sprite) = self.def_sprite(k, &id) else { continue };
                    let on = self.place_kind == k && self.place_def == id;
                    let name = self.def_name(k, &id).unwrap_or_else(|| id.clone());
                    if widgets::sprite_thumb(ui, &self.tex, &self.assets, &sprite, 32.0, on).on_hover_text(&name).clicked() {
                        if on { self.disarm() } else { self.arm(k, &id) }
                    }
                }
            });
        }

        ui.horizontal(|ui| {
            ui.label("🔍");
            let r = ui.add(egui::TextEdit::singleline(&mut self.place_filter).hint_text("search name or id").desired_width(150.0));
            if r.changed() {
                self.palette_scroll_to_armed = false;
            }
            if !self.place_filter.is_empty() && ui.small_button("✖").on_hover_text("Clear search").clicked() {
                self.place_filter.clear();
            }
        });
        ui.horizontal(|ui| {
            if ui.selectable_label(self.place_tab.is_none(), "All").clicked() {
                self.place_tab = None;
            }
            for k in EntityKind::ALL {
                if ui.selectable_label(self.place_tab == Some(k), k.label()).clicked() {
                    self.place_tab = Some(k);
                }
            }
        });
        let entries = self.palette_entries();
        let mut clicked: Option<(EntityKind, String)> = None;
        egui::ScrollArea::vertical().id_salt("place_list").max_height(330.0).auto_shrink([false, true]).show(ui, |ui| {
            if entries.is_empty() {
                ui.weak("No matching definitions.");
            }
            let mut last_kind = None;
            for (k, id, name, sprite) in &entries {
                if self.place_tab.is_none() && last_kind != Some(*k) {
                    ui.label(egui::RichText::new(format!("{}s", k.label())).strong());
                    last_kind = Some(*k);
                }
                let on = self.place_kind == *k && self.place_def == *id;
                let row = ui.horizontal(|ui| {
                    let a = widgets::sprite_thumb(ui, &self.tex, &self.assets, sprite, 32.0, on);
                    let b = ui.selectable_label(on, format!("{name}  ·  {id}"));
                    a.clicked() || b.clicked()
                });
                if row.inner {
                    clicked = Some((*k, id.clone()));
                }
                if on && self.palette_scroll_to_armed {
                    row.response.scroll_to_me(None);
                }
            }
        });
        self.palette_scroll_to_armed = false;
        if let Some((k, id)) = clicked {
            // Clicking the armed definition again stops placing.
            if self.place_kind == k && self.place_def == id { self.disarm() } else { self.arm(k, &id) }
        }
        if ui.button("Edit definitions…").clicked() {
            self.mode = Mode::Database;
        }

        // Entities already on the map: click to select and jump there.
        ui.separator();
        let q = self.place_filter.trim().to_lowercase();
        let Some(doc) = &self.doc else { return };
        let list: Vec<(u32, String, f32, f32)> = doc
            .map
            .entities
            .iter()
            .filter(|e| self.place_tab.is_none_or(|t| t == e.kind))
            .filter(|e| q.is_empty() || e.def.to_lowercase().contains(&q) || e.name.to_lowercase().contains(&q))
            .map(|e| (e.uid, e.label(), e.x, e.y))
            .collect();
        let total = doc.map.entities.len();
        let mut select = None;
        egui::CollapsingHeader::new(format!("On this map ({} of {total})", list.len())).id_salt("on_map").default_open(true).show(ui, |ui| {
            egui::ScrollArea::vertical().id_salt("on_map_list").max_height(220.0).auto_shrink([false, true]).show(ui, |ui| {
                for (uid, label, x, y) in &list {
                    let on = self.sel == Selection::Entity(*uid);
                    let r = ui.selectable_label(on, label);
                    if r.clicked() {
                        select = Some((*uid, *x, *y));
                    }
                    if on && self.on_map_scroll {
                        r.scroll_to_me(None);
                    }
                }
            });
        });
        self.on_map_scroll = false;
        if let Some((uid, x, y)) = select {
            self.sel = Selection::Entity(uid);
            self.cam.center = macroquad::math::vec2(x, y);
        }
    }

    /// Display name of a definition.
    pub fn def_name(&self, kind: EntityKind, def: &str) -> Option<String> {
        let db = &self.project.db;
        match kind {
            EntityKind::Prop => db.prop(def).map(|d| d.name.clone()),
            EntityKind::Character => db.character(def).map(|d| d.name.clone()),
            EntityKind::Item => db.item(def).map(|d| d.name.clone()),
        }
    }

    // ------------------------------------------------------------ inspector

    fn inspector(&mut self, ui: &mut egui::Ui) {
        let Some(doc) = &mut self.doc else {
            ui.label("No map open. Use File → New map.");
            return;
        };
        let before = Objects::of(&doc.map);
        let ts = self.assets.tile_size;
        let refs = &self.refs;
        let mut delete = false;
        let mut duplicate = false;
        let mut arm: Option<(EntityKind, String)> = None;
        let mut rename_map: Option<(String, String)> = None;
        match self.sel {
            Selection::Entity(uid) => {
                let db = &self.project.db;
                let Some(e) = doc.map.entities.iter_mut().find(|e| e.uid == uid) else {
                    self.sel = Selection::None;
                    return;
                };
                ui.horizontal(|ui| {
                    ui.heading(format!("{} #{}", e.kind.label(), e.uid));
                    if ui.button("Place more like this").on_hover_text("Use this definition for placing (Entities tool)").clicked() {
                        arm = Some((e.kind, e.def.clone()));
                    }
                });
                egui::Grid::new("ent").num_columns(2).show(ui, |ui| {
                    ui.label("Definition");
                    egui::ComboBox::from_id_salt("ent_def").selected_text(e.def.as_str()).show_ui(ui, |ui| {
                        for id in db.ids(e.kind) {
                            ui.selectable_value(&mut e.def, id.clone(), id);
                        }
                    });
                    ui.end_row();
                    ui.label("Name");
                    ui.add(egui::TextEdit::singleline(&mut e.name).hint_text("for actions/scripts"));
                    ui.end_row();
                    ui.label("Position");
                    ui.horizontal(|ui| {
                        ui.add(egui::DragValue::new(&mut e.x).speed(1.0));
                        ui.add(egui::DragValue::new(&mut e.y).speed(1.0));
                        ui.weak(format!("tile {},{}", (e.x / ts).floor(), (e.y / ts).floor()));
                    });
                    ui.end_row();
                    ui.label("Facing");
                    egui::ComboBox::from_id_salt("ent_face").selected_text(e.facing.label()).show_ui(ui, |ui| {
                        for f in Facing::ALL {
                            ui.selectable_value(&mut e.facing, f, f.label());
                        }
                    });
                    ui.end_row();
                    ui.label("Active");
                    ui.checkbox(&mut e.active, "visible at start");
                    ui.end_row();
                    if e.kind == EntityKind::Character {
                        ui.label("Model");
                        ui.horizontal(|ui| {
                            let def_sheet = db.character(&e.def).map(|d| d.sheet.clone()).unwrap_or_default();
                            let sheet = if e.sheet.is_empty() { def_sheet.clone() } else { e.sheet.clone() };
                            let frames = db.character(&e.def).map_or(4, |d| d.walk_frames.max(1));
                            let thumb = widgets::frame_thumb(ui, &self.tex, &self.assets, &sheet, e.facing as u32 * frames, 40.0, false);
                            if thumb.clicked() {
                                crate::models::request_model(crate::models::ModelTarget::Instance(e.uid));
                            }
                            ui.vertical(|ui| {
                                if e.sheet.is_empty() {
                                    ui.weak(format!("{sheet} (definition's)"));
                                } else {
                                    ui.label(format!("{sheet} (own)"));
                                }
                                ui.horizontal(|ui| {
                                    if ui.button("Change…").on_hover_text("Choose another look for this NPC only — from the project or the asset library").clicked() {
                                        crate::models::request_model(crate::models::ModelTarget::Instance(e.uid));
                                    }
                                    if !e.sheet.is_empty() && ui.button("Reset").on_hover_text("Use the definition's model again").clicked() {
                                        e.sheet.clear();
                                    }
                                });
                            });
                        });
                        ui.end_row();
                    }
                    if let Some(d) = (e.kind == EntityKind::Prop).then(|| db.prop(&e.def)).flatten() {
                        ui.label("State");
                        ui.horizontal(|ui| {
                            let cur = if e.state.is_empty() { d.default_state.clone() } else { e.state.clone() };
                            egui::ComboBox::from_id_salt("ent_state").selected_text(&cur).show_ui(ui, |ui| {
                                for (k, n) in d.state_names().into_iter().enumerate() {
                                    let label = if k == 0 { format!("{n} (default)") } else { n.clone() };
                                    if ui.selectable_label(n == cur, label).clicked() {
                                        e.state = if k == 0 { String::new() } else { n };
                                        e.frame = 0;
                                    }
                                }
                            });
                            if d.state_index(&cur).is_none() {
                                ui.colored_label(Color32::LIGHT_RED, "unknown state");
                            }
                        });
                        ui.end_row();
                        if e.frame > 0 {
                            ui.label("Frame offset");
                            ui.horizontal(|ui| {
                                ui.add(egui::DragValue::new(&mut e.frame).range(0..=255));
                                ui.weak("(old; choose a state instead)");
                            });
                            ui.end_row();
                        }
                    }
                    if e.kind == EntityKind::Item {
                        ui.label("Count");
                        ui.add(egui::DragValue::new(&mut e.count).range(1..=9999));
                        ui.end_row();
                    }
                });
                ui.horizontal(|ui| {
                    duplicate = ui.button("Duplicate").clicked();
                    delete = ui.button("Delete").clicked();
                });
                if e.kind == EntityKind::Character {
                    ui.separator();
                    ui.heading("Merchant");
                    let def_shop = db.character(&e.def).and_then(|d| d.shop.clone());
                    let mut set: Option<Option<ShopSpec>> = None;
                    match &mut e.shop {
                        Some(spec) => {
                            widgets::shop_ui(ui, &format!("shop{uid}"), spec, refs, &self.tex, &self.assets, db);
                            let label = if def_shop.is_some() { "Reset to the definition's stock" } else { "Not a merchant" };
                            if ui.button(label).clicked() {
                                set = Some(None);
                            }
                        }
                        None => match def_shop {
                            Some(d) => {
                                ui.weak(format!("Uses the definition's stock: {} item(s).", d.items.len()));
                                if ui.button("Customize stock & prices").clicked() {
                                    set = Some(Some(d));
                                }
                            }
                            None => {
                                ui.weak("Not a merchant.");
                                if ui.button("Make this a merchant").clicked() {
                                    set = Some(Some(ShopSpec::default()));
                                }
                            }
                        },
                    }
                    if let Some(v) = set {
                        e.shop = v;
                    }
                }
                if e.kind == EntityKind::Prop {
                    ui.separator();
                    ui.heading("Container");
                    let def_spec = db.prop(&e.def).and_then(|d| d.container.clone());
                    let mut set: Option<Option<ContainerSpec>> = None;
                    match &mut e.container {
                        Some(spec) => {
                            let states = db.prop(&e.def).map(|d| d.state_names()).unwrap_or_default();
                            widgets::container_ui(ui, &format!("cont{uid}"), spec, refs, &self.tex, &self.assets, db, &states);
                            let label = if def_spec.is_some() { "Reset to the definition's container" } else { "Not a container" };
                            if ui.button(label).clicked() {
                                set = Some(None);
                            }
                        }
                        None => match def_spec {
                            Some(d) => {
                                ui.weak(format!("Uses the definition's container: {} slots, {} starting stack(s).", d.size, d.items.len()));
                                if ui.button("Customize size & contents").clicked() {
                                    set = Some(Some(d));
                                }
                            }
                            None => {
                                ui.weak("Not a container.");
                                if ui.button("Make this a container").clicked() {
                                    set = Some(Some(ContainerSpec::default()));
                                }
                            }
                        },
                    }
                    if let Some(v) = set {
                        e.container = v;
                    }

                    ui.separator();
                    ui.heading("Lock");
                    let def_lock = db.prop(&e.def).and_then(|d| d.lock.clone());
                    let def_prop = db.prop(&e.def).cloned();
                    let mut set_lock: Option<Option<LockSpec>> = None;
                    match &mut e.lock {
                        Some(lock) => {
                            widgets::lock_ui(ui, &format!("lock{uid}"), lock, refs, &self.tex, &self.assets, db, def_prop.as_ref());
                            let label = if def_lock.is_some() { "Use the definition's lock" } else { "Remove lock" };
                            if ui.button(label).clicked() {
                                set_lock = Some(None);
                            }
                        }
                        None => match def_lock {
                            Some(l) => {
                                let key = db.item(&l.key).map_or(l.key.clone(), |d| d.name.clone());
                                ui.weak(format!("Uses the definition's lock (key: {key})."));
                                if ui.button("Customize lock (other key, effect, messages)").clicked() {
                                    set_lock = Some(Some(l));
                                }
                            }
                            None => {
                                ui.weak("Not locked.");
                                if ui.button("Make this lockable").clicked() {
                                    let key = refs.items.iter().find(|i| i.contains("key")).cloned().unwrap_or_default();
                                    set_lock = Some(Some(LockSpec { key, ..Default::default() }));
                                }
                            }
                        },
                    }
                    if let Some(v) = set_lock {
                        e.lock = v;
                    }
                }
                ui.separator();
                let n_def = db.def_triggers(e.kind, &e.def).len();
                ui.heading("Triggers");
                if e.kind == EntityKind::Item {
                    ui.weak("Interacting picks the item up; triggers below run too.");
                }
                if n_def > 0 {
                    ui.weak(format!("+ {n_def} trigger(s) from the definition (Database)."));
                }
                widgets::triggers_ui(ui, &format!("e{uid}"), &mut e.triggers, refs);
            }
            Selection::Zone(uid) => {
                let Some(z) = doc.map.zones.iter_mut().find(|z| z.uid == uid) else {
                    self.sel = Selection::None;
                    return;
                };
                ui.heading(format!("{} zone #{}", z.kind.label(), z.uid));
                zone_fields(ui, z, refs, doc.map.width, doc.map.height);
                ui.horizontal(|ui| {
                    duplicate = ui.button("Duplicate").clicked();
                    delete = ui.button("Delete").clicked();
                });
                ui.separator();
                ui.heading("Triggers");
                ui.weak("Zones react to Enter, Leave and Interact (pressing E while inside).");
                widgets::triggers_ui(ui, &format!("z{uid}"), &mut z.triggers, refs);
            }
            Selection::None => {
                let map = &mut doc.map;
                ui.heading(format!("Map: {}", map.name));
                ui.horizontal(|ui| {
                    ui.label(format!("{} x {} tiles", map.width, map.height));
                    if ui.button("Resize…").clicked() {
                        self.resize = Some((map.width, map.height));
                    }
                });
                ui.horizontal(|ui| {
                    ui.label("Background");
                    ui.color_edit_button_srgb(&mut map.background);
                });
                ui.horizontal(|ui| {
                    ui.label("Music");
                    widgets::suggest(ui, "map_music", &mut map.music, &refs.sounds, 130.0);
                    if !map.music.is_empty() && ui.small_button("▶").on_hover_text("Preview").clicked() {
                        self.audio.play_music(&map.music);
                    }
                    if self.audio.current_music().is_some() && ui.small_button("■").on_hover_text("Stop").clicked() {
                        self.audio.stop_music();
                    }
                });
                ui.horizontal(|ui| match &mut self.map_rename {
                    None => {
                        if ui.button("Rename map…").on_hover_text("Renames the file and updates portals, edge links, teleports and the project start").clicked() {
                            self.map_rename = Some(map.name.clone());
                        }
                    }
                    Some(buf) => {
                        ui.add(egui::TextEdit::singleline(buf).desired_width(120.0));
                        if ui.button("Apply").clicked() {
                            rename_map = Some((map.name.clone(), buf.clone()));
                        }
                        if ui.button("Cancel").clicked() {
                            self.map_rename = None;
                        }
                    }
                });
                ui.separator();
                ui.heading("Edge transitions");
                ui.weak("Walking into a linked edge moves the player to the target map, keeping the position along the edge (+offset).");
                for e in Edge::ALL {
                    let link = map.edges.get_mut(e);
                    ui.push_id(e.label(), |ui| {
                        ui.horizontal(|ui| {
                            let mut on = link.is_some();
                            if ui.checkbox(&mut on, e.label()).changed() {
                                *link = on.then(|| EdgeLink::new(refs.maps.first().map_or("", |s| s.as_str())));
                            }
                            if let Some(l) = link {
                                widgets::suggest(ui, "edge_map", &mut l.target_map, &refs.maps, 90.0);
                                if ui.small_button("↗").on_hover_text(format!("Go to {}", l.target_map)).clicked() {
                                    GOTO.with(|g| *g.borrow_mut() = Some((l.target_map.clone(), String::new())));
                                }
                            }
                        });
                        if let Some(l) = link {
                            ui.horizontal(|ui| {
                                ui.add_space(24.0);
                                ui.label("offset");
                                ui.add(egui::DragValue::new(&mut l.offset).range(-1024..=1024)).on_hover_text("Tiles added along the edge");
                                ui.label("depth");
                                ui.add(egui::DragValue::new(&mut l.depth).range(1..=16)).on_hover_text("Thickness of the transition strip in tiles");
                            });
                        }
                    });
                }
                ui.separator();
                ui.heading("Map triggers");
                ui.weak("Map enter and custom events.");
                widgets::triggers_ui(ui, "map", &mut map.triggers, refs);
                ui.separator();
                ui.collapsing(format!("Entities ({})", map.entities.len()), |ui| {
                    for e in &map.entities {
                        if ui.selectable_label(false, e.label()).clicked() {
                            self.sel = Selection::Entity(e.uid);
                            self.cam.center = macroquad::math::vec2(e.x, e.y);
                        }
                    }
                });
                ui.collapsing(format!("Zones ({})", map.zones.len()), |ui| {
                    for z in &map.zones {
                        if ui.selectable_label(false, format!("#{} {} ({})", z.uid, z.name, z.kind.label())).clicked() {
                            self.sel = Selection::Zone(z.uid);
                            let r = z.pixel_rect(ts);
                            self.cam.center = macroquad::math::vec2(r.x + r.w / 2.0, r.y + r.h / 2.0);
                        }
                    }
                });
            }
        }
        doc.record_objects(before, "inspector");
        if let Some((old, new)) = rename_map {
            self.map_rename = None;
            if doc.dirty {
                self.save_map();
            }
            self.rename_id(RefKind::Map, &old, &new);
        }
        if duplicate {
            self.duplicate_selection();
        }
        if delete {
            self.delete_selection();
        }
        if let Some((k, def)) = arm {
            self.tool = Tool::Entity;
            self.arm(k, &def);
            self.palette_scroll_to_armed = true;
        }
    }
}

fn zone_fields(ui: &mut egui::Ui, z: &mut Zone, refs: &Refs, mw: u32, mh: u32) {
    egui::Grid::new("zone").num_columns(2).show(ui, |ui| {
        ui.label("Name");
        ui.text_edit_singleline(&mut z.name);
        ui.end_row();
        ui.label("Kind");
        let mut k = match z.kind {
            ZoneKind::Spawn => 0,
            ZoneKind::Portal { .. } => 1,
            ZoneKind::Trigger => 2,
            ZoneKind::Spawner { .. } => 3,
            ZoneKind::Transition { .. } => 4,
        };
        let old = k;
        ui.vertical(|ui| {
            ui.horizontal(|ui| {
                ui.radio_value(&mut k, 0, "Spawn");
                ui.radio_value(&mut k, 1, "Portal");
                ui.radio_value(&mut k, 4, "Map transition");
            });
            ui.horizontal(|ui| {
                ui.radio_value(&mut k, 2, "Trigger");
                ui.radio_value(&mut k, 3, "Enemy spawner");
            });
        });
        if k != old {
            z.kind = match k {
                0 => ZoneKind::Spawn,
                1 => ZoneKind::Portal { target_map: refs.maps.first().cloned().unwrap_or_default(), target_spawn: String::new() },
                2 => ZoneKind::Trigger,
                4 => ZoneKind::Transition { target_map: String::new(), target_zone: String::new() },
                _ => default_spawner(refs),
            };
        }
        ui.end_row();
        ui.label("Tile x / y");
        ui.horizontal(|ui| {
            ui.add(egui::DragValue::new(&mut z.x).range(0..=mw - 1));
            ui.add(egui::DragValue::new(&mut z.y).range(0..=mh - 1));
        });
        ui.end_row();
        ui.label("Size w / h");
        ui.horizontal(|ui| {
            ui.add(egui::DragValue::new(&mut z.w).range(1..=mw));
            ui.add(egui::DragValue::new(&mut z.h).range(1..=mh));
        });
        ui.end_row();
        let zone_name = z.name.clone();
        if let ZoneKind::Transition { target_map, target_zone } = &mut z.kind {
            ui.label("Leads to map");
            egui::ComboBox::from_id_salt("tr_map").selected_text(if target_map.is_empty() { "(not linked)" } else { target_map.as_str() }).show_ui(ui, |ui| {
                for m in &refs.maps {
                    if ui.selectable_label(target_map == m, m).clicked() {
                        *target_map = m.clone();
                        target_zone.clear();
                    }
                }
            });
            ui.end_row();
            if !target_map.is_empty() {
                ui.label("Other end");
                let others: Vec<(String, String, String)> = refs.transitions.get(target_map.as_str()).cloned().unwrap_or_default();
                egui::ComboBox::from_id_salt("tr_zone").selected_text(if target_zone.is_empty() { "(choose)" } else { target_zone.as_str() }).show_ui(ui, |ui| {
                    for (name, _, _) in &others {
                        ui.selectable_value(target_zone, name.clone(), name);
                    }
                });
                ui.end_row();
                // Status: does the other end exist and lead back here?
                let me = refs.current_map.clone();
                let other = others.iter().find(|(n, _, _)| n == target_zone);
                ui.label("");
                match other {
                    Some((_, m, z)) if *m == me && *z == zone_name => {
                        ui.colored_label(Color32::LIGHT_GREEN, "✔ linked both ways");
                    }
                    Some(_) => {
                        ui.horizontal(|ui| {
                            ui.colored_label(Color32::YELLOW, "the other end leads elsewhere");
                            if ui.button("Link back").on_hover_text("Make the other end lead here").clicked() {
                                crate::maps::request_transition(crate::maps::TransitionReq::LinkBack { zone: zone_name.clone(), map: target_map.clone(), other: target_zone.clone() });
                            }
                        });
                    }
                    None if target_zone.is_empty() => {
                        ui.colored_label(Color32::YELLOW, "choose the other end, or draw it");
                    }
                    None => {
                        ui.colored_label(Color32::LIGHT_RED, format!("'{target_map}' has no transition '{target_zone}'"));
                    }
                }
                ui.end_row();
            }
            ui.label("");
            ui.horizontal_wrapped(|ui| {
                let map_for_draw = if target_map.is_empty() { refs.maps.iter().find(|m| **m != refs.current_map).cloned().unwrap_or_default() } else { target_map.clone() };
                if !map_for_draw.is_empty() && ui.button(format!("✏ Draw the other end on {map_for_draw}")).on_hover_text("Opens that map: drag the other end there; both ends get linked").clicked() {
                    crate::maps::request_transition(crate::maps::TransitionReq::DrawOther { zone: zone_name.clone(), map: map_for_draw });
                }
                if !target_map.is_empty() {
                    if ui.button(format!("↗ Go to {target_map}")).clicked() {
                        GOTO.with(|g| *g.borrow_mut() = Some((target_map.clone(), target_zone.clone())));
                    }
                    if ui.button("Unlink").on_hover_text("Also unlinks the other end if it leads here").clicked() {
                        crate::maps::request_transition(crate::maps::TransitionReq::Unlink { zone: zone_name.clone() });
                    }
                }
            });
            ui.end_row();
        }
        if let ZoneKind::Portal { target_map, target_spawn } = &mut z.kind {
            ui.label("Target map");
            widgets::suggest(ui, "pmap", target_map, &refs.maps, 120.0);
            ui.end_row();
            ui.label("Target spawn");
            let spawns = refs.spawns.get(target_map.as_str()).cloned().unwrap_or_default();
            widgets::suggest(ui, "pspawn", target_spawn, &spawns, 120.0);
            ui.end_row();
            ui.label("");
            if ui.button(format!("↗ Go to {target_map}")).on_hover_text("Open the target map at the target spawn (or double-click the portal)").clicked() {
                GOTO.with(|g| *g.borrow_mut() = Some((target_map.clone(), target_spawn.clone())));
            }
            ui.end_row();
        }
        if let ZoneKind::Spawner { character, count, respawn } = &mut z.kind {
            ui.label("Character");
            widgets::suggest(ui, "sp_char", character, &refs.characters, 120.0);
            ui.end_row();
            ui.label("How many");
            ui.add(egui::DragValue::new(count).range(1..=100)).on_hover_text("Characters alive at the same time");
            ui.end_row();
            ui.label("Respawn");
            ui.vertical(|ui| {
                let mut mode = match respawn {
                    Respawn::Never => 0,
                    Respawn::OnEnter => 1,
                    Respawn::After { .. } => 2,
                };
                let old = mode;
                ui.radio_value(&mut mode, 0, "Never (defeated ones stay dead, saved)");
                ui.radio_value(&mut mode, 1, "Each time the map is entered");
                ui.radio_value(&mut mode, 2, "After a delay");
                if mode != old {
                    *respawn = match mode {
                        0 => Respawn::Never,
                        1 => Respawn::OnEnter,
                        _ => Respawn::After { seconds: 30.0 },
                    };
                }
                if let Respawn::After { seconds } = respawn {
                    ui.horizontal(|ui| {
                        ui.label("seconds");
                        ui.add(egui::DragValue::new(seconds).range(0.5..=3600.0).speed(0.5));
                    });
                }
            });
            ui.end_row();
        }
    });
    match z.kind {
        ZoneKind::Spawn => {
            ui.weak("Spawn points are referenced by name from portals, teleports and the project start.");
        }
        ZoneKind::Spawner { .. } => {
            ui.weak("Invisible in the game. Characters appear on random walkable tiles inside the zone (never right next to the player). Their 'Defeated' triggers come from the character definition.");
        }
        _ => {}
    }
}

pub fn default_spawner(refs: &Refs) -> ZoneKind {
    let character = refs.characters.iter().find(|c| c.contains("slime") || c.contains("enemy")).or(refs.characters.first()).cloned().unwrap_or_default();
    ZoneKind::Spawner { character, count: 3, respawn: Respawn::OnEnter }
}

fn valid_name(s: &str) -> bool {
    !s.is_empty() && s.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

/// Flag names used anywhere on a map (for suggestions).
fn collect_flags(map: &Map, out: &mut Vec<String>) {
    use rust2d_engine::events::{Action, Cond, Trigger};
    fn actions(a: &[Action], out: &mut Vec<String>) {
        for a in a {
            match a {
                Action::SetFlag { name, .. } => out.push(name.clone()),
                Action::If { conditions, then, otherwise } => {
                    for c in conditions {
                        if let Cond::Flag { name } = &c.cond {
                            out.push(name.clone());
                        }
                    }
                    actions(then, out);
                    actions(otherwise, out);
                }
                _ => {}
            }
        }
    }
    let mut trig = |ts: &[Trigger]| {
        for t in ts {
            for c in &t.conditions {
                if let Cond::Flag { name } = &c.cond {
                    out.push(name.clone());
                }
            }
            actions(&t.actions, out);
        }
    };
    trig(&map.triggers);
    for e in &map.entities {
        trig(&e.triggers);
    }
    for z in &map.zones {
        trig(&z.triggers);
    }
}
