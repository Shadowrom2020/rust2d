//! LPC character sheets (the Universal LPC Spritesheet layout: 64×64 frames, one animation
//! per row, 13 frames wide and 21+ rows high; or a plain 4-row walk cycle, 9 frames wide). The
//! four walk rows (LPC order up, left, down, right) become a sheet in the engine's character
//! layout: rows Down, Left, Right, Up (`Facing`), 4 frames each (`LAYOUT_FRAMES`): LPC frames
//! 0 (standing, also the idle frame), 2, 4 and 6 of the walk. Other animations are dropped.

use anyhow::{bail, Result};
use rust2d_engine::imageio::Rgba;
use rust2d_engine::render::character::LAYOUT_FRAMES;

/// LPC frame size.
pub const FRAME: u32 = 64;
/// Which LPC walk frames become the engine's 4 (frame 0 stands: the idle pose).
const PICK: [u32; LAYOUT_FRAMES as usize] = [0, 2, 4, 6];

/// The walk rows of an LPC sheet, top to bottom (up, left, down, right), and a note on
/// what else the sheet had.
fn walk_rows(img: &Rgba) -> Result<(u32, &'static str)> {
    let (cols, rows) = (img.w / FRAME, img.h / FRAME);
    if cols < 9 || !img.w.is_multiple_of(FRAME) || !img.h.is_multiple_of(FRAME) {
        bail!("not an LPC sheet: expected 64×64 frames and at least 9 columns (got {}×{} px)", img.w, img.h);
    }
    match rows {
        4 => Ok((0, "")),
        r if r >= 12 => Ok((8, "spellcast, thrust, slash, shoot, hurt and other animations were left out")),
        _ => bail!("not an LPC sheet: expected the 4 rows of a walk cycle or the full sheet (21 rows), got {rows} rows"),
    }
}

/// Converts an LPC sheet into the engine's character layout (64×64 frames, 4×4). Returns the
/// image and a note for the report.
pub fn convert(img: &Rgba) -> Result<(Rgba, &'static str)> {
    let (base, note) = walk_rows(img)?;
    // Engine row (Down, Left, Right, Up) ← LPC row offset (up 0, left 1, down 2, right 3).
    let from = [2, 1, 3, 0];
    let mut out = Rgba::new(FRAME * LAYOUT_FRAMES, FRAME * 4);
    for (row, lpc_row) in from.iter().enumerate() {
        for (col, &f) in PICK.iter().enumerate() {
            let (sx, sy) = (f * FRAME, (base + lpc_row) * FRAME);
            let (dx, dy) = (col as u32 * FRAME, row as u32 * FRAME);
            for y in 0..FRAME {
                for x in 0..FRAME {
                    out.set((dx + x) as i32, (dy + y) as i32, img.get((sx + x) as i32, (sy + y) as i32));
                }
            }
        }
    }
    Ok((out, note))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The fixture colors each frame (row r, column c) with (r * 60 + 20, c * 25, 100).
    fn color(r: u32, c: u32) -> [u8; 4] {
        [(r * 60 + 20) as u8, (c * 25) as u8, 100, 255]
    }

    #[test]
    fn walk_cycle_rows_are_reordered() {
        let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/lpc_walk.png");
        let img = Rgba::decode(&std::fs::read(path).unwrap()).unwrap();
        let (out, note) = convert(&img).unwrap();
        assert_eq!((out.w, out.h, note), (256, 256, ""));
        // Engine rows Down, Left, Right, Up ← LPC rows 2, 1, 3, 0; columns ← LPC frames 0, 2, 4, 6.
        for (row, lpc) in [2, 1, 3, 0].into_iter().enumerate() {
            for (col, f) in [0, 2, 4, 6].into_iter().enumerate() {
                assert_eq!(out.get(col as i32 * 64 + 10, row as i32 * 64 + 10), color(lpc, f), "row {row} col {col}");
            }
        }
    }

    #[test]
    fn full_sheet_uses_rows_8_to_11() {
        let mut img = Rgba::new(13 * 64, 21 * 64);
        img.set(2 * 64 + 5, 10 * 64 + 5, [9, 9, 9, 255]); // walk down (row 10), frame 2
        let (out, note) = convert(&img).unwrap();
        assert_eq!(out.get(64 + 5, 5), [9, 9, 9, 255], "Down row, second frame");
        assert!(note.contains("left out"));
        assert!(convert(&Rgba::new(64, 64)).is_err());
        assert!(convert(&Rgba::new(9 * 64, 7 * 64)).is_err());
    }
}
