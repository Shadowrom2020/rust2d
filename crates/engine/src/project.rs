//! A project directory: config, definitions, maps and scripts.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{de::DeserializeOwned, Deserialize, Serialize};

use crate::defs::Database;
use crate::map::{io as map_io, Map};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ProjectConfig {
    pub name: String,
    /// Always [`TILE_SIZE`]. Projects saved with another size are converted by
    /// [`crate::migrate::upgrade`] (the editor does this when opening them).
    #[serde(default = "default_tile_size")]
    pub tile_size: u32,
    pub start_map: String,
    pub start_spawn: String,
    /// Pixel scale used by the game window.
    #[serde(default = "default_scale")]
    pub scale: f32,
    /// Player character definition id.
    pub player: String,
    /// Sounds played by built-in game events.
    #[serde(default)]
    pub sfx: GameSfx,
    /// Number of player inventory slots.
    #[serde(default = "default_inventory_size")]
    pub inventory_size: u32,
}

/// Edge length of every tile in pixels. Sprites, hitboxes and speeds are designed for it.
pub const TILE_SIZE: u32 = 32;

fn default_tile_size() -> u32 {
    TILE_SIZE
}

impl ProjectConfig {
    /// A fresh configuration with the given name.
    pub fn new(name: &str) -> Self {
        ProjectConfig {
            name: name.to_string(),
            tile_size: TILE_SIZE,
            start_map: "start".into(),
            start_spawn: "start".into(),
            scale: default_scale(),
            player: String::new(),
            sfx: GameSfx::default(),
            inventory_size: default_inventory_size(),
        }
    }
}

fn default_inventory_size() -> u32 {
    crate::inventory::DEFAULT_CAPACITY as u32
}

/// Sound ids for built-in game events (empty = silent).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct GameSfx {
    pub pickup: String,
    pub attack: String,
    pub hit: String,
    pub hurt: String,
    pub defeat: String,
    pub dialog: String,
    pub portal: String,
    pub open_container: String,
    pub unlock: String,
}

impl GameSfx {
    pub fn fields_mut(&mut self) -> [(&'static str, &mut String); 9] {
        [
            ("pickup", &mut self.pickup),
            ("attack", &mut self.attack),
            ("hit", &mut self.hit),
            ("hurt", &mut self.hurt),
            ("defeat", &mut self.defeat),
            ("dialog", &mut self.dialog),
            ("portal", &mut self.portal),
            ("open container", &mut self.open_container),
            ("unlock", &mut self.unlock),
        ]
    }
}

fn default_scale() -> f32 {
    1.5
}

static SAVES_DIR: std::sync::OnceLock<PathBuf> = std::sync::OnceLock::new();

/// Keeps save games in `dir` instead of the project folder (set once, at startup).
pub fn set_saves_dir(dir: PathBuf) {
    let _ = SAVES_DIR.set(dir);
}

pub struct Project {
    pub root: PathBuf,
    pub config: ProjectConfig,
    pub db: Database,
}

const DEFS: [&str; 7] = ["tilesets", "sheets", "items", "props", "characters", "sounds", "quests"];

fn read_ron<T: DeserializeOwned + Default>(path: &Path) -> Result<T> {
    if !path.exists() {
        return Ok(T::default());
    }
    let s = std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
    ron::from_str(&s).with_context(|| format!("parsing {}", path.display()))
}

pub fn write_ron<T: Serialize>(path: &Path, value: &T) -> Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let cfg = ron::ser::PrettyConfig::default().struct_names(false);
    let s = ron::ser::to_string_pretty(value, cfg)?;
    std::fs::write(path, s).with_context(|| format!("writing {}", path.display()))
}

impl Project {
    pub fn load(root: &Path) -> Result<Project> {
        let root = root.to_path_buf();
        let cfg_path = root.join("project.ron");
        let config: ProjectConfig = ron::from_str(
            &std::fs::read_to_string(&cfg_path).with_context(|| format!("reading {}", cfg_path.display()))?,
        )
        .with_context(|| format!("parsing {}", cfg_path.display()))?;
        let d = |n: &str| root.join("defs").join(format!("{n}.ron"));
        let db = Database {
            tilesets: read_ron(&d(DEFS[0]))?,
            sheets: read_ron(&d(DEFS[1]))?,
            items: read_ron(&d(DEFS[2]))?,
            props: read_ron(&d(DEFS[3]))?,
            characters: read_ron(&d(DEFS[4]))?,
            sounds: read_ron(&d(DEFS[5]))?,
            quests: read_ron(&d(DEFS[6]))?,
        };
        let mut db = db;
        db.upgrade_prop_states();
        Ok(Project { root, config, db })
    }

    pub fn save_config(&self) -> Result<()> {
        write_ron(&self.root.join("project.ron"), &self.config)
    }

    pub fn save_defs(&self) -> Result<()> {
        let d = |n: &str| self.root.join("defs").join(format!("{n}.ron"));
        write_ron(&d(DEFS[0]), &self.db.tilesets)?;
        write_ron(&d(DEFS[1]), &self.db.sheets)?;
        write_ron(&d(DEFS[2]), &self.db.items)?;
        write_ron(&d(DEFS[3]), &self.db.props)?;
        write_ron(&d(DEFS[4]), &self.db.characters)?;
        write_ron(&d(DEFS[5]), &self.db.sounds)?;
        write_ron(&d(DEFS[6]), &self.db.quests)
    }

    pub fn path(&self, rel: &str) -> PathBuf {
        self.root.join(rel)
    }
    pub fn maps_dir(&self) -> PathBuf {
        self.root.join("maps")
    }
    pub fn scripts_dir(&self) -> PathBuf {
        self.root.join("scripts")
    }
    /// `<root>/saves`, unless [`set_saves_dir`] chose another folder (released games).
    pub fn saves_dir(&self) -> PathBuf {
        SAVES_DIR.get().cloned().unwrap_or_else(|| self.root.join("saves"))
    }
    pub fn map_path(&self, name: &str) -> PathBuf {
        self.maps_dir().join(format!("{name}.map"))
    }

    pub fn list_maps(&self) -> Vec<String> {
        let mut out: Vec<String> = std::fs::read_dir(self.maps_dir())
            .into_iter()
            .flatten()
            .flatten()
            .filter_map(|e| {
                let p = e.path();
                if p.extension()? != "map" {
                    return None;
                }
                Some(p.file_stem()?.to_string_lossy().into_owned())
            })
            .collect();
        out.sort();
        out
    }

    pub fn load_map(&self, name: &str) -> Result<Map> {
        let mut m = map_io::load(&self.map_path(name))?;
        m.name = name.to_string();
        for e in &mut m.entities {
            self.db.upgrade_instance(e);
        }
        Ok(m)
    }

    pub fn save_map(&self, map: &Map) -> Result<()> {
        map_io::save(map, &self.map_path(&map.name))
    }

    /// Renames a map file (references are updated with `refactor`).
    pub fn rename_map_file(&self, old: &str, new: &str) -> Result<()> {
        let (from, to) = (self.map_path(old), self.map_path(new));
        if to.exists() {
            anyhow::bail!("a map named '{new}' already exists");
        }
        std::fs::rename(from, to)?;
        Ok(())
    }
}
