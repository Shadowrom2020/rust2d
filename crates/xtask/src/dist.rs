//! `cargo xtask-dist`: the rust2d toolkit as AppImage and Flatpak (feature `dist`).

use std::path::PathBuf;

use anyhow::{bail, Context, Result};
use rust2d_packaging::{AppSpec, Entry, Source};

use crate::{read, root, workspace_version};

/// The oldest glibc the packaged binaries may need (Debian 12, Ubuntu 22.04, RHEL 9).
const MAX_GLIBC: (u32, u32) = (2, 35);
const APP_ID: &str = "io.github.Shadowrom2020.rust2d";

pub fn dist(args: &[String]) -> Result<()> {
    let root = root();
    let opt = |name: &str| args.iter().position(|a| a == name).and_then(|i| args.get(i + 1)).map(PathBuf::from);
    let out = opt("--out").unwrap_or_else(|| root.join("dist"));
    let bins = opt("--bin-dir").unwrap_or_else(|| root.join("target/release"));
    let version = workspace_version(&read(&root.join("Cargo.toml"))?)?;
    let (editor, game) = (bins.join("rust2d-editor"), bins.join("rust2d-game"));
    for b in [&editor, &game] {
        let bytes = std::fs::read(b).with_context(|| format!("{} not found: run `cargo build --release` first", b.display()))?;
        match rust2d_packaging::glibc_needed(&bytes) {
            Some(g) if g > MAX_GLIBC => bail!("{} needs glibc {}.{} (at most {}.{} allowed): build on an older system", b.display(), g.0, g.1, MAX_GLIBC.0, MAX_GLIBC.1),
            Some(g) => println!("{}: needs glibc {}.{}", b.display(), g.0, g.1),
            None => println!("{}: no glibc version information", b.display()),
        }
    }
    let example = root.join("example_project");
    let project = rust2d_engine::project::Project::load(&example).context("loading example_project")?;
    let icon = rust2d_packaging::hero_icon(&project).unwrap_or_else(rust2d_packaging::emblem).encode()?;
    let mut spec = AppSpec {
        command: "rust2d-editor".into(),
        name: "rust2d".into(),
        comment: "Editor and engine for 2D top-down role-playing games".into(),
        categories: "Development;Game;".into(),
        app_id: APP_ID.into(),
        runtime: rust2d_packaging::FLATPAK_RUNTIMES[0].into(),
        // Window, sound, devices (GPU, gamepads for play-tests); projects anywhere in home; network for the AppImage runtime of
        // game exports; the host's flatpak for Flatpak game exports.
        finish_args: ["--share=ipc", "--socket=x11", "--socket=pulseaudio", "--device=all", "--share=network", "--filesystem=home", "--talk-name=org.freedesktop.Flatpak"]
            .map(String::from)
            .to_vec(),
        files: vec![
            Entry { dest: "lib/rust2d/rust2d-editor".into(), src: Source::Path(editor), exec: true },
            Entry { dest: "lib/rust2d/rust2d-game".into(), src: Source::Path(game), exec: true },
        ],
        bin: vec![("rust2d-editor".into(), "lib/rust2d/rust2d-editor".into()), ("rust2d-game".into(), "lib/rust2d/rust2d-game".into())],
        icon,
        license: "MIT".into(),
    };
    let license = root.join("LICENSE");
    if !license.is_file() {
        bail!("LICENSE is missing: the packages must carry rust2d's license");
    }
    spec.files.push(Entry { dest: format!("share/licenses/{APP_ID}/LICENSE"), src: Source::Path(license), exec: false });
    // The example project without the developer's save games.
    let before = spec.files.len();
    spec.add_dir(&example, "share/rust2d/example_project", &|_| false)?;
    let mut i = before;
    while i < spec.files.len() {
        if spec.files[i].dest.starts_with("share/rust2d/example_project/saves/") {
            spec.files.remove(i);
        } else {
            i += 1;
        }
    }
    std::fs::create_dir_all(&out)?;
    let appimage = out.join(format!("rust2d-{version}-x86_64.AppImage"));
    let flatpak = out.join(format!("rust2d-{version}.flatpak"));
    rust2d_packaging::appimage(&spec, &appimage)?;
    println!("{}", appimage.display());
    rust2d_packaging::flatpak_bundle(&spec, &flatpak)?;
    println!("{}", flatpak.display());
    let mut sums = String::new();
    for p in [&appimage, &flatpak] {
        sums += &format!("{}  {}\n", rust2d_packaging::sha256_hex(&std::fs::read(p)?), p.file_name().unwrap().to_string_lossy());
    }
    std::fs::write(out.join("SHA256SUMS"), sums)?;
    Ok(())
}

