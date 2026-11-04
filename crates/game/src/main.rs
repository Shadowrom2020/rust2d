//! rust2d game runner.
//!
//! Usage: rust2d-game [PROJECT_DIR] [--map NAME] [--spawn NAME] [--pos X,Y] [--no-title]
//!
//! Without PROJECT_DIR a released game runs the project in `data/` next to the executable
//! (saves in `~/.local/share/<game>/saves`); otherwise `example_project` is used.

use std::path::PathBuf;

use anyhow::{bail, Context, Result};
use macroquad::prelude::*;
use rust2d_engine::project::{Project, ProjectConfig};
use rust2d_engine::release;
use rust2d_engine::runtime::{Game, StartOptions};

/// Project folder and whether it is the bundle next to the executable.
fn default_root() -> (PathBuf, bool) {
    match release::bundled_root() {
        Some(r) => (r, true),
        None => (PathBuf::from("example_project"), false),
    }
}

/// The project folder given on the command line, if any.
fn root_arg() -> Option<PathBuf> {
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        match a.as_str() {
            "--map" | "--spawn" | "--pos" => {
                args.next();
            }
            s if s.starts_with('-') => {}
            s => return Some(PathBuf::from(s)),
        }
    }
    None
}

fn parse_args() -> Result<(PathBuf, StartOptions)> {
    let mut root = default_root().0;
    let mut start = StartOptions::default();
    let mut no_title = false;
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        let mut value = || args.next().with_context(|| format!("{a} needs a value"));
        match a.as_str() {
            "--map" => start.map = Some(value()?),
            "--no-title" => no_title = true,
            "--engine-build" => {}
            "--spawn" => start.spawn = Some(value()?),
            "--pos" => {
                let v = value()?;
                let (x, y) = v.split_once(',').context("--pos expects X,Y")?;
                start.pos = Some(vec2(x.trim().parse()?, y.trim().parse()?));
            }
            "-h" | "--help" => {
                println!("usage: rust2d-game [PROJECT_DIR] [--map NAME] [--spawn NAME] [--pos X,Y] [--no-title]");
                std::process::exit(0);
            }
            s if s.starts_with('-') => bail!("unknown option {s}"),
            s => root = PathBuf::from(s),
        }
    }
    // The title screen is skipped when starting somewhere specific (editor play-tests).
    start.title = !no_title && start.map.is_none() && start.spawn.is_none() && start.pos.is_none();
    Ok((root, start))
}

fn window_conf() -> Conf {
    if std::env::args().any(|a| a == "--engine-build") {
        println!("{}", rust2d_engine::ENGINE_BUILD);
        std::process::exit(0);
    }
    // The window is titled with the game's name.
    let root = root_arg().unwrap_or_else(|| default_root().0);
    let title = std::fs::read_to_string(root.join("project.ron"))
        .ok()
        .and_then(|s| ron::from_str::<ProjectConfig>(&s).ok())
        .map(|c| c.name)
        .filter(|n| !n.trim().is_empty())
        .unwrap_or_else(|| "rust2d".to_owned());
    Conf {
        window_title: title,
        window_width: 1280,
        window_height: 720,
        high_dpi: true,
        ..Default::default()
    }
}

#[macroquad::main(window_conf)]
async fn main() {
    let result = (|| -> Result<Game> {
        let (root, start) = parse_args()?;
        let project = Project::load(&root).with_context(|| format!("loading project {}", root.display()))?;
        // A released game may live in a read-only folder: keep saves in the user's data folder.
        if root_arg().is_none() && default_root().1 {
            if let Some(dir) = release::user_saves_dir(&project.config.name) {
                rust2d_engine::project::set_saves_dir(dir);
            }
        }
        Game::new(project, start)
    })();
    let mut game = match result {
        Ok(g) => g,
        Err(e) => {
            eprintln!("error: {e:#}");
            std::process::exit(1);
        }
    };
    // Debug aids:
    //   RUST2D_SHOT=out.png        screenshot after ~1s, then exit
    //   RUST2D_AUTOPLAY="hold right 2; press e; wait 1; shot a.png; status; quit"
    let mut auto = match (std::env::var("RUST2D_AUTOPLAY"), std::env::var("RUST2D_SHOT")) {
        (Ok(script), _) => Some(Autoplay::parse(&script)),
        (_, Ok(path)) => Some(Autoplay::parse(&format!("wait 1; shot {path}; quit"))),
        _ => None,
    };
    // The window's close button asks whether to save first (see Game::request_close).
    prevent_quit();
    loop {
        if let Some(a) = &mut auto {
            if !a.step(&game) {
                break;
            }
            if std::mem::take(&mut a.close) {
                game.request_close();
            }
        }
        if is_quit_requested() {
            game.request_close();
        }
        game.frame();
        if game.wants_quit() {
            break;
        }
        if let Some(path) = auto.as_mut().and_then(|a| a.shot.take()) {
            get_screen_data().export_png(&path);
        }
        next_frame().await;
    }
}

/// Scripted input for automated play-throughs.
struct Autoplay {
    cmds: std::collections::VecDeque<Vec<String>>,
    timer: f32,
    held: Vec<KeyCode>,
    /// Screenshot to take after the next frame is drawn.
    shot: Option<String>,
    mouse: Option<Vec2>,
    mouse_frames: std::collections::VecDeque<rust2d_engine::runtime::input::MouseInject>,
    /// Simulates the window's close button.
    close: bool,
}

impl Autoplay {
    fn parse(script: &str) -> Self {
        let cmds = script
            .split(';')
            .map(|c| c.split_whitespace().map(str::to_string).collect::<Vec<_>>())
            .filter(|c| !c.is_empty())
            .collect();
        Autoplay { cmds, timer: 0.0, held: Vec::new(), shot: None, mouse: None, mouse_frames: Default::default(), close: false }
    }

    /// Advances the script; returns false to quit.
    fn step(&mut self, game: &Game) -> bool {
        use rust2d_engine::runtime::input::{inject, inject_mouse, key_from_name, MouseInject};
        // Queued mouse frames (clicks, drags) play one per frame; afterwards the cursor rests.
        if let Some(f) = self.mouse_frames.pop_front() {
            self.mouse = Some(f.pos);
            inject_mouse(Some(f));
            return true;
        }
        if let Some(p) = self.mouse {
            inject_mouse(Some(MouseInject { pos: p, ..Default::default() }));
        }
        let click = |frames: &mut std::collections::VecDeque<MouseInject>, p: Vec2, b: MouseButton| {
            frames.push_back(MouseInject { pos: p, pressed: vec![b], down: vec![b], released: vec![] });
            frames.push_back(MouseInject { pos: p, pressed: vec![], down: vec![], released: vec![b] });
            frames.push_back(MouseInject { pos: p, ..Default::default() });
        };
        if self.timer > 0.0 {
            self.timer -= get_frame_time();
            inject(&self.held, &[]);
            return true;
        }
        self.held.clear();
        inject(&[], &[]);
        while let Some(cmd) = self.cmds.pop_front() {
            let arg = |i: usize| cmd.get(i).map(String::as_str).unwrap_or("");
            let secs = |i: usize| arg(i).parse::<f32>().unwrap_or(0.5);
            match arg(0) {
                "hold" => {
                    self.held = arg(1).split('+').filter_map(key_from_name).collect();
                    self.timer = secs(2);
                    inject(&self.held, &[]);
                    return true;
                }
                "press" => {
                    let keys: Vec<_> = arg(1).split('+').filter_map(key_from_name).collect();
                    inject(&keys, &keys); // held too, so "shift+g" works
                    // Let one frame consume the press, then idle a little.
                    self.timer = 0.1;
                    return true;
                }
                // click X Y [right]: mouse click on a tile (tile coordinates)
                "click" | "rclick" | "clickenemy" => {
                    let p = if arg(0) == "clickenemy" {
                        let Some((x, y)) = game.nearest_enemy_tile() else { continue };
                        game.tile_to_screen(x, y)
                    } else {
                        game.tile_to_screen(secs(1), secs(2))
                    };
                    let b = if arg(0) == "rclick" { MouseButton::Right } else { MouseButton::Left };
                    click(&mut self.mouse_frames, p, b);
                    return true;
                }
                // slot player|chest|equip IDX [right|double]: click a slot of the open inventory / loot window
                "slot" => {
                    let Some(p) = game.slot_screen_pos(arg(1), secs(2) as usize) else { continue };
                    let b = if arg(3) == "right" { MouseButton::Right } else { MouseButton::Left };
                    click(&mut self.mouse_frames, p, b);
                    if arg(3) == "double" {
                        click(&mut self.mouse_frames, p, b);
                    }
                    return true;
                }
                // shop buy|sell ROW [shift]: click the trade button of a row in the open shop
                "shop" => {
                    let Some(p) = game.shop_button_pos(arg(1), secs(2) as usize) else {
                        println!("[autoplay] shop: no {} row {}", arg(1), arg(2));
                        continue;
                    };
                    click(&mut self.mouse_frames, p, MouseButton::Left);
                    return true;
                }
                // slotdrag player|chest IDX player|chest IDX
                "slotdrag" => {
                    // "slotdrag player 3 out" drags the stack out of the window (drops it).
                    let to = if arg(3) == "out" { Some(vec2(8.0, screen_height() / 2.0)) } else { game.slot_screen_pos(arg(3), secs(4) as usize) };
                    let (Some(a), Some(b)) = (game.slot_screen_pos(arg(1), secs(2) as usize), to) else { continue };
                    let l = MouseButton::Left;
                    self.mouse_frames.push_back(MouseInject { pos: a, pressed: vec![l], down: vec![l], released: vec![] });
                    for k in 1..=8 {
                        self.mouse_frames.push_back(MouseInject { pos: a.lerp(b, k as f32 / 8.0), down: vec![l], ..Default::default() });
                    }
                    self.mouse_frames.push_back(MouseInject { pos: b, released: vec![l], ..Default::default() });
                    self.mouse_frames.push_back(MouseInject { pos: b, ..Default::default() });
                    return true;
                }
                // type TEXT: typed characters (rest of the command, spaces kept)
                "type" => {
                    rust2d_engine::runtime::input::inject_text(&cmd[1..].join(" "));
                }
                "wait" => {
                    self.timer = secs(1);
                    return true;
                }
                "shot" => {
                    self.shot = Some(arg(1).to_string());
                    return true;
                }
                "status" => println!("[autoplay] {} order={}", game.status(), game.order_debug()),
                "spawners" => println!("[autoplay] spawners: {}", game.spawner_status()),
                "quit" => return false,
                // close: as if the window's close button was clicked
                "close" => {
                    self.close = true;
                    return true;
                }
                other => eprintln!("[autoplay] unknown command '{other}'"),
            }
        }
        true
    }
}
