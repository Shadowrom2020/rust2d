//! File → Export game: writes a standalone bundle (game executable + project data).

use std::path::PathBuf;

use egui_macroquad::egui::{self, Color32};
use rust2d_engine::release;

use crate::app::Editor;
use crate::files::Purpose;

pub struct ExportDialog {
    pub name: String,
    pub out: String,
    pub zip: bool,
    pub result: Option<Result<String, String>>,
}

/// The rust2d-game executable to ship: a release build if there is one, else the one next to
/// the editor. Returns (path, is a debug build).
pub fn game_binary() -> Option<(PathBuf, bool)> {
    let exe = std::env::current_exe().ok()?;
    let dir = exe.parent()?;
    let next_to = dir.join("rust2d-game");
    // Inside a cargo target folder, prefer target/release over target/debug.
    if dir.file_name().is_some_and(|n| n == "debug") {
        let rel = dir.parent()?.join("release/rust2d-game");
        if rel.is_file() {
            return Some((rel, false));
        }
        return next_to.is_file().then_some((next_to, true));
    }
    next_to.is_file().then_some((next_to, false))
}

impl Editor {
    pub fn open_export(&mut self) {
        let out = self.project.root.canonicalize().ok().and_then(|r| r.parent().map(|p| p.join("releases"))).unwrap_or_else(|| PathBuf::from("releases"));
        self.export = Some(ExportDialog { name: self.project.config.name.clone(), out: out.display().to_string(), zip: true, result: None });
    }

    /// Saves everything, then exports. Returns a summary.
    pub fn run_export(&mut self, name: &str, out: &str, zip: bool) -> Result<String, String> {
        self.save_all();
        if self.doc.as_ref().is_some_and(|d| d.is_dirty()) || self.defs_dirty {
            return Err("Unsaved changes could not be saved; nothing was exported.".into());
        }
        let Some((bin, debug)) = game_binary() else {
            return Err("rust2d-game was not found next to the editor. Build it with `cargo build --release -p rust2d-game`.".into());
        };
        let r = release::export(&self.project, name, &bin, std::path::Path::new(out), zip).map_err(|e| format!("{e:#}"))?;
        let mut msg = format!("Exported to {} ({} files, {:.1} MB)", r.folder.display(), r.files, r.bytes as f64 / 1_048_576.0);
        if let Some(a) = &r.archive {
            msg += &format!("\nArchive: {}", a.display());
        }
        if debug {
            msg += "\nNote: this used a debug build of the game. For full speed run `cargo build --release -p rust2d-game` and export again.";
        }
        for w in r.warnings {
            msg += &format!("\nWarning: {w}");
        }
        Ok(msg)
    }

    pub fn export_window(&mut self, ctx: &egui::Context) {
        let Some(mut d) = self.export.take() else { return };
        let mut open = true;
        let mut go = false;
        let (_, check) = release::check(&self.project);
        egui::Window::new("Export game").open(&mut open).collapsible(false).resizable(false).anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0]).show(ctx, |ui| {
            ui.label("Creates a standalone copy of the game that runs without the editor: the game executable plus the project's data (maps, definitions, scripts and the images and sounds in use; never save games).");
            egui::Grid::new("export").num_columns(2).show(ui, |ui| {
                ui.label("Game name");
                ui.add(egui::TextEdit::singleline(&mut d.name).desired_width(260.0));
                ui.end_row();
                ui.label("Output folder");
                ui.horizontal(|ui| {
                    ui.add(egui::TextEdit::singleline(&mut d.out).desired_width(260.0));
                    if ui.button("Browse…").clicked() {
                        self.browse_request = Some(Purpose::ExportGameFolder);
                    }
                });
                ui.end_row();
                ui.label("");
                ui.checkbox(&mut d.zip, "Also create a .zip to share");
                ui.end_row();
            });
            let slug = release::slug(&d.name);
            ui.weak(format!("Result: {}/{slug}/{slug} (run it to play), saves go to ~/.local/share/{slug}/saves", d.out.trim_end_matches('/')));
            match game_binary() {
                Some((p, false)) => ui.weak(format!("Game executable: {}", p.display())),
                Some((p, true)) => ui.colored_label(Color32::YELLOW, format!("Only a debug build of the game was found ({}). It works but runs slower; `cargo build --release -p rust2d-game` gives a fast one.", p.display())),
                None => ui.colored_label(Color32::LIGHT_RED, "rust2d-game not found next to the editor; build it with `cargo build --release -p rust2d-game`."),
            };
            for e in &check.errors {
                ui.colored_label(Color32::LIGHT_RED, format!("✖ {e}"));
            }
            for w in &check.warnings {
                ui.colored_label(Color32::YELLOW, format!("⚠ {w}"));
            }
            if let Some(r) = &d.result {
                match r {
                    Ok(m) => ui.colored_label(Color32::LIGHT_GREEN, m),
                    Err(e) => ui.colored_label(Color32::LIGHT_RED, e),
                };
            }
            let ok = check.errors.is_empty() && !d.name.trim().is_empty() && !d.out.trim().is_empty() && game_binary().is_some();
            if ui.add_enabled(ok, egui::Button::new("📦 Export")).clicked() {
                go = true;
            }
        });
        if go {
            let r = self.run_export(&d.name, &d.out, d.zip);
            self.set_status(match &r {
                Ok(m) => m.lines().next().unwrap_or_default().to_string(),
                Err(e) => format!("Export failed: {e}"),
            });
            d.result = Some(r);
        }
        if open {
            self.export = Some(d);
        }
    }
}
