//! Asset bundles and the shared asset library.
//!
//! A [`Bundle`] is a set of definitions (tilesets, sprite sheets, sounds, items, props,
//! characters) together with the image / sound files they need. Bundles are
//! - collected from a project with all their dependencies ([`Bundle::collect`]),
//! - stored as `.r2dpack` packages (a zip with `manifest.ron` and `files/…`),
//! - rescaled between tile sizes (e.g. 32 px library art into a 16 px project),
//! - merged into a project or the library: identical assets are reused, clashing ids and
//!   file names get a numeric suffix and every reference inside the bundle follows.
//!
//! The library itself is an ordinary project folder (without maps) under
//! `~/.local/share/rust2d/library`, so it is loaded and saved like any project.

use std::collections::{BTreeMap, BTreeSet};
use std::io::{Cursor, Read, Write};
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};

use crate::defs::*;
use crate::imageio::Rgba;
use crate::project::{Project, ProjectConfig, TILE_SIZE};
use crate::refactor::{self, RefKind};

pub const PACK_EXT: &str = "r2dpack";
const PACK_VERSION: u32 = 1;

/// Kind and id of one asset.
pub type AssetRef = (RefKind, String);

/// Asset kinds in dependency order (things later in the list may refer to earlier ones).
pub const KINDS: [RefKind; 6] = [RefKind::Tileset, RefKind::Sheet, RefKind::Sound, RefKind::Item, RefKind::Prop, RefKind::Character];

pub fn kind_label(k: RefKind) -> &'static str {
    match k {
        RefKind::Tileset => "Tileset",
        RefKind::Sheet => "Sprite sheet",
        RefKind::Sound => "Sound",
        RefKind::Item => "Item",
        RefKind::Prop => "Prop",
        RefKind::Character => "Character",
        RefKind::Map => "Map",
    }
}

#[derive(Serialize, Deserialize)]
struct Manifest {
    version: u32,
    name: String,
    tile_size: u32,
    #[serde(default)]
    tilesets: Vec<TilesetDef>,
    #[serde(default)]
    sheets: Vec<SheetDef>,
    #[serde(default)]
    sounds: Vec<SoundDef>,
    #[serde(default)]
    items: Vec<ItemDef>,
    #[serde(default)]
    props: Vec<PropDef>,
    #[serde(default)]
    characters: Vec<CharacterDef>,
}

#[derive(Clone, Debug, Default)]
pub struct Bundle {
    pub name: String,
    /// Tile size the art was made for.
    pub tile_size: u32,
    pub db: Database,
    /// Files by project-relative path.
    pub files: BTreeMap<String, Vec<u8>>,
}

/// What a merge did.
#[derive(Clone, Debug, Default)]
pub struct MergeReport {
    pub added: Vec<AssetRef>,
    /// Already present with identical content.
    pub existing: Vec<AssetRef>,
    /// (kind, id in the bundle, id it got).
    pub renamed: Vec<(RefKind, String, String)>,
    /// Files written.
    pub files: Vec<String>,
}

impl MergeReport {
    pub fn summary(&self) -> String {
        let mut s = format!("{} added, {} already present", self.added.len(), self.existing.len());
        if !self.renamed.is_empty() {
            let list: Vec<String> = self.renamed.iter().map(|(_, a, b)| format!("{a} → {b}")).collect();
            s += &format!(", renamed (id taken): {}", list.join(", "));
        }
        s
    }
}

pub fn exists(db: &Database, a: &AssetRef) -> bool {
    let id = a.1.as_str();
    match a.0 {
        RefKind::Tileset => db.tileset(id).is_some(),
        RefKind::Sheet => db.sheet(id).is_some(),
        RefKind::Sound => db.sounds.iter().any(|d| d.id == id),
        RefKind::Item => db.item(id).is_some(),
        RefKind::Prop => db.prop(id).is_some(),
        RefKind::Character => db.character(id).is_some(),
        RefKind::Map => false,
    }
}

/// Assets `a` refers to directly (only kinds that live in a bundle).
pub fn deps(db: &Database, a: &AssetRef) -> Vec<AssetRef> {
    let refs = match a.0 {
        RefKind::Item => db.item(&a.1).map(refactor::refs_of_item),
        RefKind::Prop => db.prop(&a.1).map(refactor::refs_of_prop),
        RefKind::Character => db.character(&a.1).map(refactor::refs_of_character),
        _ => None,
    };
    refs.unwrap_or_default().into_iter().filter(|r| r.0 != RefKind::Map).collect()
}

/// All assets in a database, in dependency order.
pub fn all_assets(db: &Database) -> Vec<AssetRef> {
    let mut out = Vec::new();
    out.extend(db.tilesets.iter().map(|d| (RefKind::Tileset, d.id.clone())));
    out.extend(db.sheets.iter().map(|d| (RefKind::Sheet, d.id.clone())));
    out.extend(db.sounds.iter().map(|d| (RefKind::Sound, d.id.clone())));
    out.extend(db.items.iter().map(|d| (RefKind::Item, d.id.clone())));
    out.extend(db.props.iter().map(|d| (RefKind::Prop, d.id.clone())));
    out.extend(db.characters.iter().map(|d| (RefKind::Character, d.id.clone())));
    out
}

/// Assets in `db` that refer to `a`.
pub fn dependents(db: &Database, a: &AssetRef) -> Vec<AssetRef> {
    all_assets(db).into_iter().filter(|b| deps(db, b).contains(a)).collect()
}

fn with_suffix(path: &str, n: u32) -> String {
    match path.rfind('.') {
        Some(dot) if !path[dot..].contains('/') => format!("{}_{n}{}", &path[..dot], &path[dot..]),
        _ => format!("{path}_{n}"),
    }
}

/// Paths inside a package must stay inside the project.
fn safe_rel(p: &str) -> bool {
    !p.is_empty() && !p.starts_with('/') && !p.contains('\\') && p.split('/').all(|c| !c.is_empty() && c != "." && c != "..")
}

impl Bundle {
    /// Collects `wanted` plus everything they depend on from a project-like store.
    pub fn collect(db: &Database, root: &Path, tile_size: u32, name: &str, wanted: &[AssetRef]) -> Result<Bundle> {
        let mut seen: BTreeSet<AssetRef> = BTreeSet::new();
        let mut stack: Vec<AssetRef> = wanted.to_vec();
        while let Some(a) = stack.pop() {
            if a.0 == RefKind::Map || seen.contains(&a) || !exists(db, &a) {
                continue;
            }
            stack.extend(deps(db, &a));
            seen.insert(a);
        }
        let has = |k: RefKind, id: &str| seen.contains(&(k, id.to_string()));
        let out = Database {
            tilesets: db.tilesets.iter().filter(|d| has(RefKind::Tileset, &d.id)).cloned().collect(),
            sheets: db.sheets.iter().filter(|d| has(RefKind::Sheet, &d.id)).cloned().collect(),
            sounds: db.sounds.iter().filter(|d| has(RefKind::Sound, &d.id)).cloned().collect(),
            items: db.items.iter().filter(|d| has(RefKind::Item, &d.id)).cloned().collect(),
            props: db.props.iter().filter(|d| has(RefKind::Prop, &d.id)).cloned().collect(),
            characters: db.characters.iter().filter(|d| has(RefKind::Character, &d.id)).cloned().collect(),
            quests: Vec::new(),
        };
        let mut files = BTreeMap::new();
        let paths = out.tilesets.iter().map(|d| &d.image).chain(out.sheets.iter().map(|d| &d.image)).chain(out.sounds.iter().map(|d| &d.file));
        for rel in paths {
            if rel.is_empty() || files.contains_key(rel) {
                continue;
            }
            let bytes = std::fs::read(root.join(rel)).with_context(|| format!("reading {rel}"))?;
            files.insert(rel.clone(), bytes);
        }
        Ok(Bundle { name: name.to_string(), tile_size, db: out, files })
    }

    pub fn is_empty(&self) -> bool {
        all_assets(&self.db).is_empty()
    }

    /// Writes a `.r2dpack` (zip) package.
    pub fn to_pack(&self) -> Result<Vec<u8>> {
        let m = Manifest {
            version: PACK_VERSION,
            name: self.name.clone(),
            tile_size: self.tile_size,
            tilesets: self.db.tilesets.clone(),
            sheets: self.db.sheets.clone(),
            sounds: self.db.sounds.clone(),
            items: self.db.items.clone(),
            props: self.db.props.clone(),
            characters: self.db.characters.clone(),
        };
        let cfg = ron::ser::PrettyConfig::default().struct_names(false);
        let manifest = ron::ser::to_string_pretty(&m, cfg)?;
        let mut zw = zip::ZipWriter::new(Cursor::new(Vec::new()));
        let opts = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
        zw.start_file("manifest.ron", opts)?;
        zw.write_all(manifest.as_bytes())?;
        for (rel, bytes) in &self.files {
            zw.start_file(format!("files/{rel}"), opts)?;
            zw.write_all(bytes)?;
        }
        Ok(zw.finish()?.into_inner())
    }

    /// Reads a `.r2dpack` package.
    pub fn from_pack(bytes: &[u8]) -> Result<Bundle> {
        let mut za = zip::ZipArchive::new(Cursor::new(bytes)).context("not an asset package")?;
        let mut manifest = String::new();
        za.by_name("manifest.ron").context("package has no manifest.ron")?.read_to_string(&mut manifest)?;
        let m: Manifest = ron::from_str(&manifest).context("parsing manifest.ron")?;
        if m.version > PACK_VERSION {
            bail!("package version {} is newer than this editor supports ({PACK_VERSION})", m.version);
        }
        let mut files = BTreeMap::new();
        for i in 0..za.len() {
            let mut f = za.by_index(i)?;
            let name = f.name()?.to_string();
            let Some(rel) = name.strip_prefix("files/").map(str::to_string) else { continue };
            if f.is_dir() {
                continue;
            }
            if !safe_rel(&rel) {
                bail!("unsafe path in package: {rel}");
            }
            let mut data = Vec::new();
            f.read_to_end(&mut data)?;
            files.insert(rel, data);
        }
        let db = Database { tilesets: m.tilesets, sheets: m.sheets, sounds: m.sounds, items: m.items, props: m.props, quests: Vec::new(), characters: m.characters };
        let b = Bundle { name: m.name, tile_size: m.tile_size, db, files };
        for rel in b.db.tilesets.iter().map(|d| &d.image).chain(b.db.sheets.iter().map(|d| &d.image)).chain(b.db.sounds.iter().map(|d| &d.file)) {
            if !rel.is_empty() && !b.files.contains_key(rel) {
                bail!("package is missing {rel}");
            }
        }
        Ok(b)
    }

    /// Rescales the art and pixel-based values (frame sizes, hitboxes, speeds) for a project
    /// with tile size `to` (nearest-neighbour, so 16 → 32 is lossless).
    pub fn rescale(&mut self, to: u32) -> Result<()> {
        if to == 0 || self.tile_size == 0 || self.tile_size == to {
            self.tile_size = to.max(self.tile_size);
            return Ok(());
        }
        let f = to as f32 / self.tile_size as f32;
        let sc = |v: u32| ((v as f32 * f).round() as u32).max(1);
        let images: BTreeSet<String> = self.db.tilesets.iter().map(|d| d.image.clone()).chain(self.db.sheets.iter().map(|d| d.image.clone())).collect();
        for rel in images {
            let Some(bytes) = self.files.get(&rel) else { continue };
            let img = Rgba::decode(bytes).with_context(|| format!("rescaling {rel} (only PNG images can be rescaled)"))?;
            let out = img.resized(sc(img.w), sc(img.h));
            self.files.insert(rel, out.encode()?);
        }
        for s in &mut self.db.sheets {
            s.frame_w = sc(s.frame_w);
            s.frame_h = sc(s.frame_h);
        }
        let ranged = |w: &mut Option<Weapon>| {
            if let Some(r) = w.as_mut().and_then(|w| w.ranged.as_mut()) {
                r.speed *= f;
            }
        };
        for d in &mut self.db.items {
            ranged(&mut d.weapon);
        }
        for d in &mut self.db.props {
            d.hitbox = [d.hitbox[0] * f, d.hitbox[1] * f];
        }
        for d in &mut self.db.characters {
            d.hitbox = [d.hitbox[0] * f, d.hitbox[1] * f];
            d.speed *= f;
            ranged(&mut d.weapon);
        }
        self.tile_size = to;
        Ok(())
    }

    /// Renames an asset inside the bundle (its definition and every reference to it).
    fn rename(&mut self, kind: RefKind, old: &str, new: &str) {
        let set = |id: &mut String| {
            if id == old {
                *id = new.to_string();
            }
        };
        match kind {
            RefKind::Tileset => self.db.tilesets.iter_mut().for_each(|d| set(&mut d.id)),
            RefKind::Sheet => self.db.sheets.iter_mut().for_each(|d| set(&mut d.id)),
            RefKind::Sound => self.db.sounds.iter_mut().for_each(|d| set(&mut d.id)),
            RefKind::Item => self.db.items.iter_mut().for_each(|d| set(&mut d.id)),
            RefKind::Prop => self.db.props.iter_mut().for_each(|d| set(&mut d.id)),
            RefKind::Character => self.db.characters.iter_mut().for_each(|d| set(&mut d.id)),
            RefKind::Map => {}
        }
        let mut cfg = ProjectConfig::new("");
        refactor::rename_in_db(&mut self.db, &mut cfg, kind, old, new);
    }

    /// Is the bundle's version of `a` identical to the one in `db`?
    fn same_as(&self, db: &Database, a: &AssetRef) -> bool {
        let id = a.1.as_str();
        match a.0 {
            RefKind::Tileset => db.tileset(id) == self.db.tileset(id),
            RefKind::Sheet => db.sheet(id) == self.db.sheet(id),
            RefKind::Sound => db.sounds.iter().find(|d| d.id == id) == self.db.sounds.iter().find(|d| d.id == id),
            RefKind::Item => db.item(id) == self.db.item(id),
            RefKind::Prop => db.prop(id) == self.db.prop(id),
            RefKind::Character => db.character(id) == self.db.character(id),
            RefKind::Map => true,
        }
    }

    /// Adds only the assets whose ids `db` doesn't have yet (existing assets, possibly edited
    /// by the user, are left alone; new assets referring to them use the existing ones).
    pub fn add_missing(mut self, db: &mut Database, root: &Path) -> Result<MergeReport> {
        let new = |k: RefKind, id: &str| !exists(db, &(k, id.to_string()));
        self.db.tilesets.retain(|d| new(RefKind::Tileset, &d.id));
        self.db.sheets.retain(|d| new(RefKind::Sheet, &d.id));
        self.db.sounds.retain(|d| new(RefKind::Sound, &d.id));
        self.db.items.retain(|d| new(RefKind::Item, &d.id));
        self.db.props.retain(|d| new(RefKind::Prop, &d.id));
        self.db.characters.retain(|d| new(RefKind::Character, &d.id));
        let used = files_of(&self.db);
        self.files.retain(|rel, _| used.contains(rel));
        self.merge_into(db, root)
    }

    /// Adds the bundle to a project (or the library) at `root`. Files are written right away;
    /// the caller saves the definitions.
    pub fn merge_into(mut self, db: &mut Database, root: &Path) -> Result<MergeReport> {
        let mut report = MergeReport::default();
        // 1. Files: reuse identical files, never overwrite different ones.
        let mut moved: BTreeMap<String, String> = BTreeMap::new();
        for (rel, bytes) in &self.files {
            if !safe_rel(rel) {
                bail!("unsafe path: {rel}");
            }
            let mut target = rel.clone();
            let mut n = 2;
            loop {
                let p = root.join(&target);
                if !p.exists() {
                    if let Some(dir) = p.parent() {
                        std::fs::create_dir_all(dir)?;
                    }
                    std::fs::write(&p, bytes).with_context(|| format!("writing {target}"))?;
                    report.files.push(target.clone());
                    break;
                }
                if std::fs::read(&p)? == *bytes {
                    break;
                }
                target = with_suffix(rel, n);
                n += 1;
            }
            if target != *rel {
                moved.insert(rel.clone(), target);
            }
        }
        let fix = |p: &mut String| {
            if let Some(n) = moved.get(p) {
                *p = n.clone();
            }
        };
        self.db.tilesets.iter_mut().for_each(|d| fix(&mut d.image));
        self.db.sheets.iter_mut().for_each(|d| fix(&mut d.image));
        self.db.sounds.iter_mut().for_each(|d| fix(&mut d.file));

        // 2. Ids: an id that exists with different content is renamed (and the rename is
        // followed by the bundle's references). Repeat until stable, since a rename can make
        // dependents differ too.
        let mut renamed_to: BTreeSet<AssetRef> = BTreeSet::new();
        loop {
            let mut changed = false;
            for a in all_assets(&self.db) {
                if renamed_to.contains(&a) || !exists(db, &a) || self.same_as(db, &a) {
                    continue;
                }
                let mut n = 2;
                let new = loop {
                    let cand = format!("{}_{n}", a.1);
                    let r = (a.0, cand.clone());
                    if !exists(db, &r) && !exists(&self.db, &r) {
                        break cand;
                    }
                    n += 1;
                };
                self.rename(a.0, &a.1, &new);
                report.renamed.push((a.0, a.1.clone(), new.clone()));
                renamed_to.insert((a.0, new));
                changed = true;
            }
            if !changed {
                break;
            }
        }

        // 3. Add what is new.
        for a in all_assets(&self.db) {
            if exists(db, &a) {
                report.existing.push(a);
                continue;
            }
            let id = a.1.as_str();
            match a.0 {
                RefKind::Tileset => db.tilesets.extend(self.db.tileset(id).cloned()),
                RefKind::Sheet => db.sheets.extend(self.db.sheet(id).cloned()),
                RefKind::Sound => db.sounds.extend(self.db.sounds.iter().find(|d| d.id == id).cloned()),
                RefKind::Item => db.items.extend(self.db.item(id).cloned()),
                RefKind::Prop => db.props.extend(self.db.prop(id).cloned()),
                RefKind::Character => db.characters.extend(self.db.character(id).cloned()),
                RefKind::Map => {}
            }
            report.added.push(a);
        }
        Ok(report)
    }
}

/// Removes assets from a project-like store and deletes files nothing refers to anymore.
/// Refuses (with the list of users) if other remaining assets depend on them.
pub fn remove_assets(project: &mut Project, assets: &[AssetRef]) -> Result<()> {
    let gone: BTreeSet<&AssetRef> = assets.iter().collect();
    for a in assets {
        let users: Vec<String> = dependents(&project.db, a).into_iter().filter(|u| !gone.contains(u)).map(|u| u.1).collect();
        if !users.is_empty() {
            bail!("{} '{}' is used by: {}", kind_label(a.0), a.1, users.join(", "));
        }
    }
    let files_before: BTreeSet<String> = files_of(&project.db);
    let db = &mut project.db;
    let keep = |k: RefKind, id: &str| !gone.contains(&(k, id.to_string()));
    db.tilesets.retain(|d| keep(RefKind::Tileset, &d.id));
    db.sheets.retain(|d| keep(RefKind::Sheet, &d.id));
    db.sounds.retain(|d| keep(RefKind::Sound, &d.id));
    db.items.retain(|d| keep(RefKind::Item, &d.id));
    db.props.retain(|d| keep(RefKind::Prop, &d.id));
    db.characters.retain(|d| keep(RefKind::Character, &d.id));
    let files_after = files_of(&project.db);
    for f in files_before.difference(&files_after) {
        if safe_rel(f) {
            let _ = std::fs::remove_file(project.root.join(f));
        }
    }
    project.save_defs()
}

fn files_of(db: &Database) -> BTreeSet<String> {
    db.tilesets.iter().map(|d| d.image.clone()).chain(db.sheets.iter().map(|d| d.image.clone())).chain(db.sounds.iter().map(|d| d.file.clone())).collect()
}

/// Folder of the shared asset library: `$RUST2D_LIBRARY`, else
/// `$XDG_DATA_HOME/rust2d/library`, else `~/.local/share/rust2d/library`.
pub fn library_dir() -> PathBuf {
    if let Some(p) = std::env::var_os("RUST2D_LIBRARY") {
        return PathBuf::from(p);
    }
    let data = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/share")))
        .unwrap_or_else(|| PathBuf::from("."));
    data.join("rust2d/library")
}

/// Opens the library at `dir`, creating it (filled with `starter`, a `.r2dpack`) if needed.
pub fn open_library(dir: &Path, starter: Option<&[u8]>) -> Result<Project> {
    let stamp_file = dir.join(STARTER_STAMP);
    let stamp = starter.map(|b| format!("{:016x}", fnv1a(b)));
    if !dir.join("project.ron").exists() {
        std::fs::create_dir_all(dir).with_context(|| format!("creating {}", dir.display()))?;
        let bundle = starter.map(Bundle::from_pack).transpose().context("reading the starter assets")?;
        let mut lib = Project { root: dir.to_path_buf(), config: ProjectConfig::new("Asset library"), db: Database::default() };
        if let Some(mut b) = bundle {
            b.rescale(TILE_SIZE)?;
            b.merge_into(&mut lib.db, dir)?;
        }
        lib.save_config()?;
        lib.save_defs()?;
        if let Some(s) = &stamp {
            std::fs::write(&stamp_file, s)?;
        }
    }
    // Libraries from before tiles were fixed at 32 px.
    crate::migrate::upgrade(dir, None)?;
    let mut lib = Project::load(dir)?;
    // A newer starter set (new editor version): add the assets the library doesn't have yet.
    if let (Some(bytes), Some(s)) = (starter, &stamp) {
        if std::fs::read_to_string(&stamp_file).ok().as_deref() != Some(s.as_str()) {
            let mut b = Bundle::from_pack(bytes).context("reading the starter assets")?;
            b.rescale(TILE_SIZE)?;
            let report = b.add_missing(&mut lib.db, dir)?;
            if !report.added.is_empty() {
                lib.save_defs()?;
            }
            std::fs::write(&stamp_file, s)?;
        }
    }
    Ok(lib)
}

/// File in the library recording which starter set was last applied.
const STARTER_STAMP: &str = "starter.stamp";

/// 64-bit FNV-1a hash (a cheap fingerprint of the starter pack).
fn fnv1a(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf29ce484222325u64, |h, &b| (h ^ b as u64).wrapping_mul(0x100000001b3))
}

/// Adds a bundle to the library (rescaled to the library's tile size) and saves it.
pub fn add_to_library(lib: &mut Project, mut bundle: Bundle) -> Result<MergeReport> {
    bundle.rescale(TILE_SIZE)?;
    let root = lib.root.clone();
    let report = bundle.merge_into(&mut lib.db, &root)?;
    lib.save_defs()?;
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(name: &str) -> PathBuf {
        let p = std::env::temp_dir().join(format!("rust2d_lib_test_{name}_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&p);
        std::fs::create_dir_all(&p).unwrap();
        p
    }

    fn png(w: u32, h: u32, c: [u8; 4]) -> Vec<u8> {
        let mut img = Rgba::new(w, h);
        img.px.as_chunks_mut::<4>().0.iter_mut().for_each(|p| *p = c);
        img.encode().unwrap()
    }

    /// A source project: an "arrows" item, a "bow" using them, both on sheet "items".
    fn source(test: &str) -> (PathBuf, Database) {
        let root = tmp(&format!("src_{test}"));
        std::fs::create_dir_all(root.join("sprites")).unwrap();
        std::fs::write(root.join("sprites/items.png"), png(32, 16, [255, 0, 0, 255])).unwrap();
        let item = |id: &str| ItemDef {
            id: id.into(),
            name: id.into(),
            description: String::new(),
            icon: SpriteRef::new("items", 0),
            consumable: false,
            on_use: vec![],
            weapon: None,
            armor: None,
            max_stack: 99,
            value: 0,
        };
        let mut bow = item("bow");
        bow.weapon = Some(Weapon::ranged(1, 0.5, Ranged { projectile: SpriteRef::new("items", 1), speed: 100.0, range: 6.0, ammo: "arrows".into() }));
        let db = Database {
            sheets: vec![SheetDef { id: "items".into(), image: "sprites/items.png".into(), frame_w: 16, frame_h: 16 }],
            items: vec![item("arrows"), bow],
            ..Default::default()
        };
        (root, db)
    }

    #[test]
    fn collect_pack_and_merge() {
        let (root, db) = source("merge");
        // Collecting the bow brings its ammo and the sheet along.
        let b = Bundle::collect(&db, &root, 16, "bow", &[(RefKind::Item, "bow".into())]).unwrap();
        assert_eq!(b.db.items.len(), 2);
        assert_eq!(b.db.sheets.len(), 1);
        assert!(b.files.contains_key("sprites/items.png"));
        let b = Bundle::from_pack(&b.to_pack().unwrap()).unwrap();
        assert_eq!(b.db.items.len(), 2);

        // Into an empty project: everything is added.
        let dest = tmp("dest");
        let mut d = Database::default();
        let r = b.clone().merge_into(&mut d, &dest).unwrap();
        assert_eq!(r.added.len(), 3);
        assert!(dest.join("sprites/items.png").exists());
        // Again: all identical, nothing new.
        let r = b.clone().merge_into(&mut d, &dest).unwrap();
        assert_eq!((r.added.len(), r.existing.len()), (0, 3));

        // A project with a different "items" sheet image: the file and the sheet get new
        // names, and the items (which now differ) are renamed with their references.
        let other = tmp("other");
        std::fs::create_dir_all(other.join("sprites")).unwrap();
        std::fs::write(other.join("sprites/items.png"), png(32, 16, [0, 255, 0, 255])).unwrap();
        let mut d2 = Database { sheets: db.sheets.clone(), items: db.items.clone(), ..Default::default() };
        let r = b.merge_into(&mut d2, &other).unwrap();
        assert!(other.join("sprites/items_2.png").exists());
        assert_eq!(d2.sheet("items_2").unwrap().image, "sprites/items_2.png");
        let bow2 = d2.item("bow_2").unwrap();
        assert_eq!(bow2.icon.sheet, "items_2");
        assert_eq!(bow2.weapon.as_ref().unwrap().ranged.as_ref().unwrap().ammo, "arrows_2");
        assert_eq!(r.renamed.len(), 3);
        for p in [root, dest, other] {
            let _ = std::fs::remove_dir_all(p);
        }
    }

    #[test]
    fn rescale_doubles_art_and_sizes() {
        let (root, mut db) = source("rescale");
        db.props.push(PropDef {
            id: "crate".into(),
            name: "Crate".into(),
            sprite: SpriteRef::new("items", 0),
            solid: true,
            hitbox: [12.0, 6.0],
            triggers: vec![],
            container: None,
            lock: None,
            default_state: default_state_name(),
            states: vec![],
        });
        let mut b = Bundle::collect(&db, &root, 16, "x", &all_assets(&db)).unwrap();
        b.rescale(32).unwrap();
        let img = Rgba::decode(&b.files["sprites/items.png"]).unwrap();
        assert_eq!((img.w, img.h), (64, 32));
        assert_eq!(b.db.sheets[0].frame_w, 32);
        assert_eq!(b.db.props[0].hitbox, [24.0, 12.0]);
        assert_eq!(b.db.item("bow").unwrap().weapon.as_ref().unwrap().ranged.as_ref().unwrap().speed, 200.0);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn library_and_removal() {
        let (root, db) = source("lib");
        let pack = Bundle::collect(&db, &root, 16, "starter", &all_assets(&db)).unwrap().to_pack().unwrap();
        let dir = tmp("lib");
        let _ = std::fs::remove_dir_all(&dir);
        let mut lib = open_library(&dir, Some(&pack)).unwrap();
        // The 16px pack was enlarged to the fixed 32px tiles.
        assert_eq!(lib.config.tile_size, 32);
        assert_eq!(lib.db.sheet("items").unwrap().frame_w, 32);
        assert_eq!(lib.db.items.len(), 2);
        // The sheet is still used by the items.
        assert!(remove_assets(&mut lib, &[(RefKind::Sheet, "items".into())]).is_err());
        // A newer starter set: only assets with new ids are added, existing ones stay as they are.
        let mut db2 = db.clone();
        db2.items[0].name = "Changed arrows".into();
        db2.sheets.push(SheetDef { id: "extra".into(), image: "sprites/extra.png".into(), frame_w: 16, frame_h: 16 });
        std::fs::write(root.join("sprites/extra.png"), png(16, 16, [1, 2, 3, 255])).unwrap();
        let pack2 = Bundle::collect(&db2, &root, 16, "starter", &all_assets(&db2)).unwrap().to_pack().unwrap();
        let mut lib = open_library(&dir, Some(&pack2)).unwrap();
        assert!(lib.db.sheet("extra").is_some(), "new sheet added");
        assert_eq!(lib.db.item("arrows").unwrap().name, "arrows", "existing item untouched");
        assert_eq!(lib.db.items.len(), 2);
        let everything = all_assets(&lib.db);
        remove_assets(&mut lib, &everything).unwrap();
        assert!(!dir.join("sprites/items.png").exists());
        assert!(Project::load(&dir).unwrap().db.items.is_empty());
        assert!(!safe_rel("../x") && !safe_rel("/etc/x") && safe_rel("sprites/a.png"));
        let _ = std::fs::remove_dir_all(root);
        let _ = std::fs::remove_dir_all(dir);
    }
}
