//! Linux packages of an exported game (File → Export game), built from the export folder
//! (`<slug>/<slug>` + `<slug>/data/`):
//!
//! - **AppImage** (`<slug>-x86_64.AppImage`): one executable file that runs on most
//!   distributions. It is the official AppImage runtime (downloaded once, pinned by hash, cached
//!   in `~/.cache/rust2d/`) followed by a squashfs image of the game, written here with
//!   `backhand` - no AppImage tools needed.
//! - **Flatpak** (`<slug>.flatpak`): a single-file bundle on the freedesktop runtime that
//!   players install with `flatpak install --user <slug>.flatpak` (Flatpak fetches the runtime
//!   from Flathub). Built with the `flatpak` command (build-finish, build-export, build-bundle);
//!   the runtime doesn't have to be installed on the creator's machine.
//!
//! Inside both, the game executable sits next to its `data/` folder, so it finds the game the
//! same way as in the plain export; saves go to the player's data folder (in Flatpak's
//! sandbox: `~/.var/app/<app id>/data/<slug>/saves`). Both get a desktop entry and an icon (the
//! player character, or a picture chosen in the export dialog).

use std::io::Cursor;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{bail, Context, Result};
use backhand::compression::Compressor;
use backhand::{FilesystemCompressor, FilesystemWriter, NodeHeader};
use rust2d_engine::entity::Facing;
use rust2d_engine::imageio::Rgba;
use rust2d_engine::project::Project;
use rust2d_engine::release::DATA_DIR;
use rust2d_engine::render::{character, recolor};

/// The AppImage runtime (type 2, static: needs no libfuse on the player's machine, only
/// `fusermount`; without FUSE, `--appimage-extract-and-run` works).
const RUNTIME_VERSION: &str = "20251108";
const RUNTIME_URL: &str = "https://github.com/AppImage/type2-runtime/releases/download/20251108/runtime-x86_64";
const RUNTIME_SHA256: &str = "2fca8b443c92510f1483a883f60061ad09b46b978b2631c807cd873a47ec260d";

/// Flatpak runtimes offered (freedesktop SDK branches; glibc 2.42 / 2.44).
pub const FLATPAK_RUNTIMES: [&str; 2] = ["25.08", "26.08"];
const FLATHUB_REPO: &str = "https://dl.flathub.org/repo/flathub.flatpakrepo";

/// Size of the generated icon.
const ICON: u32 = 256;

// ------------------------------------------------------------------ AppImage

/// Builds `out_dir/<slug>-x86_64.AppImage` from the export `folder`.
pub fn appimage(folder: &Path, slug: &str, name: &str, icon: &[u8], out_dir: &Path) -> Result<PathBuf> {
    let runtime = appimage_runtime()?;
    let squashfs = appimage_squashfs(folder, slug, name, icon)?;
    let path = out_dir.join(format!("{slug}-x86_64.AppImage"));
    std::fs::write(&path, [runtime.as_slice(), squashfs.as_slice()].concat()).with_context(|| format!("writing {}", path.display()))?;
    set_executable(&path)?;
    Ok(path)
}

/// The AppDir as a squashfs image:
/// ```text
/// AppRun -> usr/bin/<slug>     <slug>.desktop     <slug>.png     .DirIcon -> <slug>.png
/// usr/bin/<slug>               usr/bin/data/...   usr/share/applications, usr/share/icons
/// ```
fn appimage_squashfs(folder: &Path, slug: &str, name: &str, icon: &[u8]) -> Result<Vec<u8>> {
    let mtime = now() as u32;
    let dir = NodeHeader::new(0o755, 0, 0, mtime);
    let file = NodeHeader::new(0o644, 0, 0, mtime);
    let exe = NodeHeader::new(0o755, 0, 0, mtime);
    let mut fs = FilesystemWriter::default();
    fs.set_compressor(FilesystemCompressor::new(Compressor::Gzip, None)?);
    fs.set_time(mtime);
    fs.set_root_mode(0o755);
    let desktop = desktop_entry(name, slug, slug);
    // backhand keeps readers until write(): give it owned bytes.
    let mut files: Vec<(String, Vec<u8>, NodeHeader)> = vec![
        (format!("{slug}.desktop"), desktop.clone().into_bytes(), file),
        (format!("{slug}.png"), icon.to_vec(), file),
        (format!("usr/share/applications/{slug}.desktop"), desktop.into_bytes(), file),
        (format!("usr/share/icons/hicolor/{ICON}x{ICON}/apps/{slug}.png"), icon.to_vec(), file),
        (format!("usr/bin/{slug}"), std::fs::read(folder.join(slug)).context("reading the game executable")?, exe),
    ];
    for rel in walk(&folder.join(DATA_DIR))? {
        let bytes = std::fs::read(folder.join(DATA_DIR).join(&rel))?;
        files.push((format!("usr/bin/{DATA_DIR}/{rel}"), bytes, file));
    }
    let mut dirs: Vec<String> = files.iter().flat_map(|(p, _, _)| parents(p)).collect();
    dirs.sort();
    dirs.dedup();
    for d in &dirs {
        fs.push_dir(d, dir)?;
    }
    for (path, bytes, header) in files {
        fs.push_file(Cursor::new(bytes), path, header)?;
    }
    fs.push_symlink(format!("usr/bin/{slug}"), "AppRun", exe)?;
    fs.push_symlink(format!("{slug}.png"), ".DirIcon", file)?;
    let mut out = Cursor::new(Vec::new());
    fs.write(&mut out).context("writing the squashfs image")?;
    Ok(out.into_inner())
}

/// The cached AppImage runtime, downloaded on first use (curl or wget) and checked against
/// its pinned hash.
fn appimage_runtime() -> Result<Vec<u8>> {
    let cache = cache_dir().join(format!("appimage-runtime-{RUNTIME_VERSION}-x86_64"));
    if let Ok(b) = std::fs::read(&cache) {
        if hex(&sha256(&b)) == RUNTIME_SHA256 {
            return Ok(b);
        }
    }
    std::fs::create_dir_all(cache.parent().unwrap())?;
    let part = cache.with_extension("part");
    let ok = |c: &mut Command| c.status().is_ok_and(|s| s.success());
    let fetched = ok(Command::new("curl").args(["-fsSL", "--retry", "2", "-o"]).arg(&part).arg(RUNTIME_URL))
        || ok(Command::new("wget").args(["-q", "-O"]).arg(&part).arg(RUNTIME_URL));
    if !fetched {
        let _ = std::fs::remove_file(&part);
        bail!(
            "could not download the AppImage runtime (needs curl or wget and an internet connection). You can also download {RUNTIME_URL} yourself and save it as {}",
            cache.display()
        );
    }
    let b = std::fs::read(&part)?;
    if hex(&sha256(&b)) != RUNTIME_SHA256 {
        let _ = std::fs::remove_file(&part);
        bail!("the downloaded AppImage runtime has an unexpected checksum; nothing was used");
    }
    std::fs::rename(&part, &cache)?;
    Ok(b)
}

// ------------------------------------------------------------------ Flatpak

/// Whether the `flatpak` command is available (needed for Flatpak bundles).
pub fn flatpak_available() -> bool {
    Command::new("flatpak").arg("--version").output().is_ok_and(|o| o.status.success())
}

/// A Flatpak application id for a game: `org.rust2d.<slug with underscores>`.
pub fn default_app_id(slug: &str) -> String {
    let mut s: String = slug.chars().map(|c| if c == '-' { '_' } else { c }).collect();
    if s.starts_with(|c: char| c.is_ascii_digit()) {
        s.insert_str(0, "game_");
    }
    format!("org.rust2d.{s}")
}

/// Checks a Flatpak application id: at least three dot-separated parts of letters, digits,
/// `_` and `-`, none starting with a digit.
pub fn check_app_id(id: &str) -> Result<(), String> {
    let parts: Vec<&str> = id.split('.').collect();
    if parts.len() < 3 {
        return Err("needs at least three parts, like org.example.MyGame".into());
    }
    for p in &parts {
        if p.is_empty() || p.starts_with(|c: char| c.is_ascii_digit()) || !p.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-') {
            return Err(format!("'{p}' isn't allowed: letters, digits, _ and - only, not starting with a digit"));
        }
    }
    if id.len() > 255 {
        return Err("is too long".into());
    }
    Ok(())
}

/// Builds `out_dir/<slug>.flatpak` from the export `folder`.
pub fn flatpak(folder: &Path, slug: &str, name: &str, app_id: &str, runtime: &str, icon: &[u8], out_dir: &Path) -> Result<PathBuf> {
    check_app_id(app_id).map_err(|e| anyhow::anyhow!("Flatpak app id: {e}"))?;
    let work = out_dir.join(format!(".{slug}-flatpak-build"));
    let _ = std::fs::remove_dir_all(&work);
    let res = flatpak_in(&work, folder, slug, name, app_id, runtime, icon, out_dir);
    let _ = std::fs::remove_dir_all(&work);
    res
}

#[allow(clippy::too_many_arguments)]
fn flatpak_in(work: &Path, folder: &Path, slug: &str, name: &str, app_id: &str, runtime: &str, icon: &[u8], out_dir: &Path) -> Result<PathBuf> {
    let build = work.join("build");
    let files = build.join("files");
    // The game in /app/lib/<slug>/ (executable next to data/), started via /app/bin/<slug>.
    let lib = files.join("lib").join(slug);
    copy_dir(folder, &lib, &|rel| rel == slug || rel.starts_with(&format!("{DATA_DIR}/")))?;
    set_executable(&lib.join(slug))?;
    std::fs::create_dir_all(files.join("bin"))?;
    std::os::unix::fs::symlink(format!("../lib/{slug}/{slug}"), files.join("bin").join(slug))?;
    let share = files.join("share");
    write(&share.join(format!("applications/{app_id}.desktop")), desktop_entry(name, slug, app_id).as_bytes())?;
    write(&share.join(format!("icons/hicolor/{ICON}x{ICON}/apps/{app_id}.png")), icon)?;
    write(&share.join(format!("metainfo/{app_id}.metainfo.xml")), metainfo(app_id, name).as_bytes())?;
    std::fs::create_dir_all(build.join("var"))?;
    let metadata = format!("[Application]\nname={app_id}\nruntime=org.freedesktop.Platform/x86_64/{runtime}\nsdk=org.freedesktop.Sdk/x86_64/{runtime}\n");
    std::fs::write(build.join("metadata"), metadata)?;
    // Window (X11; Wayland desktops run it through XWayland), GPU, sound.
    run(Command::new("flatpak")
        .args(["build-finish", "--command", slug, "--share=ipc", "--socket=x11", "--socket=pulseaudio", "--device=dri"])
        .arg(&build))?;
    let repo = work.join("repo");
    run(Command::new("flatpak").args(["build-export", "--no-update-summary"]).arg(&repo).arg(&build).arg("stable"))?;
    let path = out_dir.join(format!("{slug}.flatpak"));
    run(Command::new("flatpak").arg("build-bundle").arg(format!("--runtime-repo={FLATHUB_REPO}")).arg(&repo).arg(&path).arg(app_id).arg("stable"))?;
    Ok(path)
}

/// AppStream metadata (shown by software centers when the bundle is opened).
fn metainfo(app_id: &str, name: &str) -> String {
    let name = xml_escape(name);
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<component type="desktop-application">
  <id>{app_id}</id>
  <name>{name}</name>
  <summary>A role-playing game made with rust2d</summary>
  <metadata_license>CC0-1.0</metadata_license>
  <description><p>{name} is a 2D role-playing game made with rust2d.</p></description>
  <launchable type="desktop-id">{app_id}.desktop</launchable>
  <categories><category>Game</category><category>RolePlaying</category></categories>
  <content_rating type="oars-1.1"/>
</component>
"#
    )
}

fn run(cmd: &mut Command) -> Result<()> {
    let out = cmd.output().with_context(|| format!("running {:?}", cmd.get_program()))?;
    if !out.status.success() {
        let args: Vec<String> = cmd.get_args().take(1).map(|a| a.to_string_lossy().into_owned()).collect();
        bail!("flatpak {} failed: {}", args.join(" "), String::from_utf8_lossy(&out.stderr).trim());
    }
    Ok(())
}

// ------------------------------------------------------------------ shared

/// A desktop entry for the game (`exec` = command, `icon` = icon name).
fn desktop_entry(name: &str, exec: &str, icon: &str) -> String {
    let name: String = name.chars().filter(|c| !c.is_control()).collect();
    format!("[Desktop Entry]\nType=Application\nName={name}\nComment=A role-playing game made with rust2d\nExec={exec}\nIcon={icon}\nTerminal=false\nCategories=Game;RolePlaying;\n")
}

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

/// The game's icon: a PNG chosen by the creator (any size) or the player character.
pub fn icon(project: &Project, custom: &str) -> Result<Vec<u8>> {
    if !custom.trim().is_empty() {
        let p = PathBuf::from(custom.trim());
        let p = if p.is_absolute() { p } else { project.root.join(p) };
        let bytes = std::fs::read(&p).with_context(|| format!("reading the icon {}", p.display()))?;
        Rgba::decode(&bytes).with_context(|| format!("the icon {} isn't a PNG image", p.display()))?;
        return Ok(bytes);
    }
    hero_icon(project).unwrap_or_else(emblem).encode()
}

/// The player character facing the viewer (layers composed like in the game), enlarged with
/// sharp pixels onto a 256 x 256 transparent square.
fn hero_icon(project: &Project) -> Option<Rgba> {
    let db = &project.db;
    let d = db.character(&project.config.player)?;
    let mut canvas: Option<Rgba> = None;
    for l in character::layers(db, &d.appearance, &d.equipment, Facing::Down, 0, false) {
        let Some(sheet) = db.sheet(&l.sheet) else { continue };
        let Ok(img) = std::fs::read(project.path(&sheet.image)).map_err(anyhow::Error::from).and_then(|b| Rgba::decode(&b)) else { continue };
        let (fw, fh) = (sheet.frame_w.max(1), sheet.frame_h.max(1));
        let cols = (img.w / fw).max(1);
        let (fx, fy) = ((l.frame % cols * fw) as i32, (l.frame / cols * fh) as i32);
        let mut frame = Rgba::new(fw, fh);
        for y in 0..fh as i32 {
            for x in 0..fw as i32 {
                frame.set(x, y, img.get(fx + x, fy + y));
            }
        }
        recolor::recolor(&mut frame.px, l.skin, l.hair);
        let c = canvas.get_or_insert_with(|| Rgba::new(fw, fh));
        // Layers of another frame size stand on the same ground, centered.
        let (ox, oy) = ((c.w as i32 - fw as i32) / 2, c.h as i32 - fh as i32);
        for y in 0..fh as i32 {
            for x in 0..fw as i32 {
                over(c, ox + x, oy + y, frame.get(x, y));
            }
        }
    }
    let c = canvas?;
    // Trim to the drawn pixels.
    let opaque: Vec<(i32, i32)> = (0..c.h as i32).flat_map(|y| (0..c.w as i32).map(move |x| (x, y))).filter(|&(x, y)| c.get(x, y)[3] > 0).collect();
    let (x0, x1) = (opaque.iter().map(|p| p.0).min()?, opaque.iter().map(|p| p.0).max()?);
    let (y0, y1) = (opaque.iter().map(|p| p.1).min()?, opaque.iter().map(|p| p.1).max()?);
    let (w, h) = ((x1 - x0 + 1) as u32, (y1 - y0 + 1) as u32);
    let scale = ((ICON - 16) / w.max(h)).max(1);
    let mut out = Rgba::new(ICON, ICON);
    let (ox, oy) = ((ICON - w * scale) as i32 / 2, (ICON - h * scale) as i32 / 2);
    for y in 0..(h * scale) as i32 {
        for x in 0..(w * scale) as i32 {
            out.set(ox + x, oy + y, c.get(x0 + x / scale as i32, y0 + y / scale as i32));
        }
    }
    Some(out)
}

/// Fallback icon (no player character): a sword on a dark shield.
fn emblem() -> Rgba {
    let mut img = Rgba::new(ICON, ICON);
    let c = ICON as f32 / 2.0;
    for y in 0..ICON as i32 {
        for x in 0..ICON as i32 {
            let (dx, dy) = ((x as f32 - c) / c, (y as f32 - c) / c);
            // A shield: flat top, round bottom.
            let inside = dx.abs() < 0.8 && dy > -0.85 && (dy < 0.1 || dx * dx + (dy - 0.1) * (dy - 0.1) * 1.6 < 0.64);
            let edge = dx.abs() < 0.86 && dy > -0.92 && (dy < 0.1 || dx * dx + (dy - 0.1) * (dy - 0.1) * 1.6 < 0.74);
            if inside {
                img.set(x, y, [0x1b, 0x2a, 0x55, 255]);
            } else if edge {
                img.set(x, y, [0xe8, 0xc0, 0x5a, 255]);
            }
        }
    }
    let rect = |img: &mut Rgba, x: i32, y: i32, w: i32, h: i32, col: [u8; 4]| {
        for yy in y..y + h {
            for xx in x..x + w {
                img.set(xx, yy, col);
            }
        }
    };
    rect(&mut img, 120, 52, 16, 120, [0xd8, 0xdc, 0xe8, 255]); // blade
    rect(&mut img, 126, 52, 4, 120, [0xff, 0xff, 0xff, 255]);
    rect(&mut img, 92, 168, 72, 12, [0xe8, 0xc0, 0x5a, 255]); // guard
    rect(&mut img, 122, 180, 12, 28, [0x7a, 0x54, 0x10, 255]); // grip
    rect(&mut img, 116, 206, 24, 12, [0xe8, 0xc0, 0x5a, 255]); // pommel
    img
}

/// Alpha-composites `c` over the pixel at x, y.
fn over(img: &mut Rgba, x: i32, y: i32, c: [u8; 4]) {
    if c[3] == 0 {
        return;
    }
    let d = img.get(x, y);
    let a = c[3] as f32 / 255.0;
    let da = d[3] as f32 / 255.0 * (1.0 - a);
    let oa = a + da;
    let mix = |s: u8, t: u8| ((s as f32 * a + t as f32 * da) / oa).round() as u8;
    img.set(x, y, [mix(c[0], d[0]), mix(c[1], d[1]), mix(c[2], d[2]), (oa * 255.0).round() as u8]);
}

/// The highest glibc version a Linux executable needs (from its `GLIBC_2.x` version names),
/// e.g. (2, 34). None if it names none (not a glibc binary).
pub fn glibc_needed(binary: &[u8]) -> Option<(u32, u32)> {
    const PAT: &[u8] = b"GLIBC_2.";
    let mut best: Option<(u32, u32)> = None;
    let mut i = 0;
    while let Some(k) = binary[i..].windows(PAT.len()).position(|w| w == PAT) {
        let start = i + k + PAT.len();
        let digits: String = binary[start..].iter().take_while(|b| b.is_ascii_digit()).map(|&b| b as char).collect();
        if let Ok(minor) = digits.parse::<u32>() {
            best = best.max(Some((2, minor)));
        }
        i = start;
    }
    best
}

fn cache_dir() -> PathBuf {
    std::env::var_os("XDG_CACHE_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".cache")))
        .unwrap_or_else(std::env::temp_dir)
        .join("rust2d")
}

fn now() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs())
}

fn set_executable(p: &Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(p, std::fs::Permissions::from_mode(0o755)).with_context(|| format!("making {} executable", p.display()))
}

fn write(p: &Path, bytes: &[u8]) -> Result<()> {
    std::fs::create_dir_all(p.parent().unwrap())?;
    std::fs::write(p, bytes).with_context(|| format!("writing {}", p.display()))
}

/// Files below `dir` as relative paths with `/`, sorted.
fn walk(dir: &Path) -> Result<Vec<String>> {
    let mut out = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        for e in std::fs::read_dir(&d).with_context(|| format!("reading {}", d.display()))?.flatten() {
            let p = e.path();
            if p.is_dir() {
                stack.push(p);
            } else {
                out.push(p.strip_prefix(dir)?.components().map(|c| c.as_os_str().to_string_lossy()).collect::<Vec<_>>().join("/"));
            }
        }
    }
    out.sort();
    Ok(out)
}

/// The parent folders of a relative path ("a/b/c" -> "a", "a/b").
fn parents(p: &str) -> Vec<String> {
    let parts: Vec<&str> = p.split('/').collect();
    (1..parts.len()).map(|n| parts[..n].join("/")).collect()
}

/// Copies the files of `from` that `keep` accepts (relative paths) into `to`.
fn copy_dir(from: &Path, to: &Path, keep: &dyn Fn(&str) -> bool) -> Result<()> {
    for rel in walk(from)?.into_iter().filter(|r| keep(r)) {
        let dst = to.join(&rel);
        std::fs::create_dir_all(dst.parent().unwrap())?;
        std::fs::copy(from.join(&rel), &dst).with_context(|| format!("copying {rel}"))?;
    }
    Ok(())
}

// ------------------------------------------------------------------ SHA-256

fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

/// SHA-256 (FIPS 180-4), to check the downloaded AppImage runtime.
fn sha256(data: &[u8]) -> [u8; 32] {
    const K: [u32; 64] = [
        0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4, 0xab1c5ed5, 0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe,
        0x9bdc06a7, 0xc19bf174, 0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f, 0x4a7484aa, 0x5cb0a9dc, 0x76f988da, 0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7,
        0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967, 0x27b70a85, 0x2e1b2138, 0x4d2c6dfc, 0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85, 0xa2bfe8a1, 0xa81a664b,
        0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070, 0x19a4c116, 0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
        0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7, 0xc67178f2,
    ];
    let mut h: [u32; 8] = [0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab, 0x5be0cd19];
    let mut msg = data.to_vec();
    msg.push(0x80);
    while msg.len() % 64 != 56 {
        msg.push(0);
    }
    msg.extend_from_slice(&(data.len() as u64 * 8).to_be_bytes());
    for block in msg.as_chunks::<64>().0 {
        let mut w = [0u32; 64];
        for t in 0..16 {
            w[t] = u32::from_be_bytes(block[t * 4..t * 4 + 4].try_into().unwrap());
        }
        for t in 16..64 {
            let s0 = w[t - 15].rotate_right(7) ^ w[t - 15].rotate_right(18) ^ (w[t - 15] >> 3);
            let s1 = w[t - 2].rotate_right(17) ^ w[t - 2].rotate_right(19) ^ (w[t - 2] >> 10);
            w[t] = w[t - 16].wrapping_add(s0).wrapping_add(w[t - 7]).wrapping_add(s1);
        }
        let [mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut hh] = h;
        for t in 0..64 {
            let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let ch = (e & f) ^ (!e & g);
            let t1 = hh.wrapping_add(s1).wrapping_add(ch).wrapping_add(K[t]).wrapping_add(w[t]);
            let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let maj = (a & b) ^ (a & c) ^ (b & c);
            let t2 = s0.wrapping_add(maj);
            (hh, g, f, e, d, c, b, a) = (g, f, e, d.wrapping_add(t1), c, b, a, t1.wrapping_add(t2));
        }
        for (x, v) in h.iter_mut().zip([a, b, c, d, e, f, g, hh]) {
            *x = x.wrapping_add(v);
        }
    }
    let mut out = [0u8; 32];
    for (k, v) in h.iter().enumerate() {
        out[k * 4..k * 4 + 4].copy_from_slice(&v.to_be_bytes());
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sha256_vectors() {
        assert_eq!(hex(&sha256(b"")), "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855");
        assert_eq!(hex(&sha256(b"abc")), "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad");
        let long = b"abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq";
        assert_eq!(hex(&sha256(long)), "248d6a61d20638b8e5c026930c3e6039a33ce45964ff2167f6ecedd419db06c1");
    }

    #[test]
    fn app_ids() {
        assert_eq!(default_app_id("knight-tales"), "org.rust2d.knight_tales");
        assert_eq!(default_app_id("2d-quest"), "org.rust2d.game_2d_quest");
        assert!(check_app_id(&default_app_id("knight-tales")).is_ok());
        assert!(check_app_id("org.example").is_err());
        assert!(check_app_id("org.example.1game").is_err());
        assert!(check_app_id("org.exa mple.game").is_err());
        assert!(check_app_id("io.github.someone.My-Game").is_ok());
    }

    #[test]
    fn glibc_scan() {
        assert_eq!(glibc_needed(b"\0GLIBC_2.2.5\0GLIBC_2.34\0GLIBC_2.17\0"), Some((2, 34)));
        assert_eq!(glibc_needed(b"no versions here"), None);
    }

    #[test]
    fn icons() {
        use rust2d_engine::defs::Database;
        use rust2d_engine::project::ProjectConfig;
        let tmp = std::env::temp_dir().join(format!("rust2d_icon_test_{}", std::process::id()));
        std::fs::create_dir_all(&tmp).unwrap();
        let p = Project { root: tmp.clone(), config: ProjectConfig::new("Test"), db: Database::default() };
        // No player character: the emblem.
        let png = icon(&p, "").unwrap();
        let img = Rgba::decode(&png).unwrap();
        assert_eq!((img.w, img.h), (ICON, ICON));
        // A picture of the creator's choice is used as it is (relative to the project).
        let own = Rgba::new(64, 64).encode().unwrap();
        std::fs::write(tmp.join("icon.png"), &own).unwrap();
        assert_eq!(icon(&p, "icon.png").unwrap(), own);
        std::fs::write(tmp.join("bad.png"), b"not a png").unwrap();
        assert!(icon(&p, "bad.png").is_err());
        assert!(icon(&p, "missing.png").is_err());
        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn appimage_contents() {
        use backhand::{FilesystemReader, InnerNode};
        let tmp = std::env::temp_dir().join(format!("rust2d_appimage_test_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&tmp);
        let folder = tmp.join("knight-tales");
        std::fs::create_dir_all(folder.join("data/maps")).unwrap();
        std::fs::write(folder.join("knight-tales"), b"\x7fELF game").unwrap();
        std::fs::write(folder.join("data/project.ron"), b"()").unwrap();
        std::fs::write(folder.join("data/maps/start.map"), b"map").unwrap();
        std::fs::write(folder.join("README.txt"), b"readme").unwrap();
        let icon = emblem().encode().unwrap();
        let img = appimage_squashfs(&folder, "knight-tales", "Knight Tales", &icon).unwrap();
        let fs = FilesystemReader::from_reader(Cursor::new(img)).unwrap();
        let mut seen = std::collections::BTreeMap::new();
        for node in fs.files() {
            let path = node.fullpath.to_string_lossy().trim_start_matches('/').to_string();
            let what = match &node.inner {
                InnerNode::File(f) => {
                    let mut buf = Vec::new();
                    std::io::Read::read_to_end(&mut fs.file(f).reader(), &mut buf).unwrap();
                    format!("file {} {:o}", String::from_utf8_lossy(&buf[..buf.len().min(9)]), node.header.permissions)
                }
                InnerNode::Symlink(s) => format!("-> {}", s.link.display()),
                InnerNode::Dir(_) => "dir".into(),
                _ => "other".into(),
            };
            seen.insert(path, what);
        }
        assert_eq!(seen["AppRun"], "-> usr/bin/knight-tales");
        assert_eq!(seen[".DirIcon"], "-> knight-tales.png");
        assert_eq!(seen["usr/bin/knight-tales"], "file \x7fELF game 755");
        assert_eq!(seen["usr/bin/data/maps/start.map"], "file map 644");
        assert!(seen["knight-tales.desktop"].starts_with("file [Desktop"));
        assert!(seen.contains_key("usr/share/icons/hicolor/256x256/apps/knight-tales.png"));
        assert!(!seen.keys().any(|k| k.contains("README")), "only the game and its data");
        let _ = std::fs::remove_dir_all(&tmp);
    }
}
