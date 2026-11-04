//! Procedural pixel art for the example project (32x32 tiles).
//!
//! Everything is drawn with a few helpers: shaded shapes lit from the top-left, an automatic
//! dark outline around sprites, and soft drop shadows underneath.

use std::path::Path;

use macroquad::prelude::{Color, Image};
use rust2d_engine::entity::Facing;

/// Tile size in pixels.
pub const TS: u32 = rust2d_engine::project::TILE_SIZE;
/// Scale of the art relative to the classic 16px grid (hitboxes and speeds are given in
/// 16px units and multiplied by this).
pub const R: f32 = TS as f32 / 16.0;

// ------------------------------------------------------------------ colors

pub fn hex(v: u32) -> Color {
    Color::from_rgba((v >> 16) as u8, (v >> 8) as u8, v as u8, 255)
}
pub const CLEAR: Color = Color::new(0.0, 0.0, 0.0, 0.0);

/// `v` with its brightness scaled by `f`.
fn mul(v: u32, f: f32) -> u32 {
    let ch = |s: u32| (((v >> s) & 0xff) as f32 * f).round().clamp(0.0, 255.0) as u32;
    (ch(16) << 16) | (ch(8) << 8) | ch(0)
}
/// Dark, mid-dark, base and light shade of a color.
fn ramp(v: u32) -> [u32; 4] {
    [mul(v, 0.55), mul(v, 0.78), v, mul(v, 1.22)]
}

const OUTLINE: u32 = 0x1a1622;
const WOOD: [u32; 4] = [0x3e2410, 0x6b4423, 0x8a5a2e, 0xb07e4a];
const STONE: [u32; 4] = [0x45454e, 0x6c6c76, 0x8f8f9a, 0xbcbcc8];
const IRON: [u32; 4] = [0x1c1c22, 0x32323a, 0x4c4c56, 0x777784];
const LEAF: [u32; 4] = [0x1d5124, 0x2e7d32, 0x43a047, 0x7cc56f];
const PINE: [u32; 4] = [0x0d3a22, 0x1b5e3a, 0x2e7d4f, 0x5aa872];
const GOLD: [u32; 4] = [0x7a5410, 0xc0901e, 0xe8c05a, 0xfff2a8];

// ------------------------------------------------------------------ random numbers

pub struct Rng(pub u64);
impl Rng {
    pub fn next(&mut self) -> u32 {
        self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        (self.0 >> 33) as u32
    }
    pub fn range(&mut self, n: u32) -> u32 {
        self.next() % n
    }
    pub fn chance(&mut self, p: f32) -> bool {
        (self.next() % 10000) as f32 / 10000.0 < p
    }
    fn f(&mut self) -> f32 {
        (self.next() % 10000) as f32 / 10000.0
    }
}

/// 4x4 ordered dither threshold in 0..1.
fn bayer(x: i32, y: i32) -> f32 {
    const M: [[u8; 4]; 4] = [[0, 8, 2, 10], [12, 4, 14, 6], [3, 11, 1, 9], [15, 7, 13, 5]];
    (M[y.rem_euclid(4) as usize][x.rem_euclid(4) as usize] as f32 + 0.5) / 16.0
}

// ------------------------------------------------------------------ canvas

pub struct Canvas {
    img: Image,
    ox: u32,
    oy: u32,
}

impl Canvas {
    pub fn new(w: u32, h: u32) -> Self {
        Canvas { img: Image::gen_image_color(w as u16, h as u16, CLEAR), ox: 0, oy: 0 }
    }
    pub fn at(&mut self, ox: u32, oy: u32) -> &mut Self {
        self.ox = ox;
        self.oy = oy;
        self
    }
    fn abs(&self, x: i32, y: i32) -> Option<(u32, u32)> {
        let (x, y) = (self.ox as i32 + x, self.oy as i32 + y);
        (x >= 0 && y >= 0 && x < self.img.width as i32 && y < self.img.height as i32).then_some((x as u32, y as u32))
    }
    fn get(&self, x: i32, y: i32) -> Color {
        self.abs(x, y).map_or(CLEAR, |(x, y)| self.img.get_pixel(x, y))
    }
    /// Sets a pixel; translucent colors are blended over what is there.
    pub fn px(&mut self, x: i32, y: i32, c: Color) {
        // Never spill into the cell above / to the left of the current origin.
        if x < 0 || y < 0 {
            return;
        }
        let Some((ax, ay)) = self.abs(x, y) else { return };
        // Fully transparent (CLEAR) erases: cut-outs like holes, flat bottoms, neck openings.
        if c.a <= 0.0 {
            self.img.set_pixel(ax, ay, c);
            return;
        }
        if c.a >= 0.999 {
            self.img.set_pixel(ax, ay, c);
            return;
        }
        let d = self.img.get_pixel(ax, ay);
        let a = c.a + d.a * (1.0 - c.a);
        if a <= 0.0 {
            return;
        }
        let mix = |s: f32, t: f32| (s * c.a + t * d.a * (1.0 - c.a)) / a;
        self.img.set_pixel(ax, ay, Color::new(mix(c.r, d.r), mix(c.g, d.g), mix(c.b, d.b), a));
    }
    /// Sets a pixel only where nothing opaque is drawn yet (shadows go under sprites).
    fn under(&mut self, x: i32, y: i32, c: Color) {
        if self.get(x, y).a < 0.5 {
            self.px(x, y, c);
        }
    }
    /// Pixel on a seamlessly tiling TS x TS tile.
    fn wrap(&mut self, x: i32, y: i32, c: Color) {
        self.px(x.rem_euclid(TS as i32), y.rem_euclid(TS as i32), c);
    }
    pub fn rect(&mut self, x: i32, y: i32, w: i32, h: i32, c: Color) {
        for yy in y..y + h {
            for xx in x..x + w {
                self.px(xx, yy, c);
            }
        }
    }
    fn hline(&mut self, x: i32, y: i32, w: i32, c: Color) {
        self.rect(x, y, w, 1, c);
    }
    fn vline(&mut self, x: i32, y: i32, h: i32, c: Color) {
        self.rect(x, y, 1, h, c);
    }
    /// Rectangle with rounded corners of radius `r`.
    fn rrect(&mut self, x: i32, y: i32, w: i32, h: i32, r: i32, c: Color) {
        for yy in 0..h {
            for xx in 0..w {
                let dx = (r - xx).max(xx - (w - 1 - r)).max(0);
                let dy = (r - yy).max(yy - (h - 1 - r)).max(0);
                if dx * dx + dy * dy <= r * r + r / 2 {
                    self.px(x + xx, y + yy, c);
                }
            }
        }
    }
    pub fn disc(&mut self, cx: f32, cy: f32, r: f32, c: Color) {
        self.ellipse(cx, cy, r, r, c);
    }
    fn ellipse(&mut self, cx: f32, cy: f32, rx: f32, ry: f32, c: Color) {
        for y in (cy - ry).floor() as i32..=(cy + ry).ceil() as i32 {
            for x in (cx - rx).floor() as i32..=(cx + rx).ceil() as i32 {
                let (dx, dy) = ((x as f32 + 0.5 - cx) / rx, (y as f32 + 0.5 - cy) / ry);
                if dx * dx + dy * dy <= 1.0 {
                    self.px(x, y, c);
                }
            }
        }
    }
    /// Ellipse shaded like a lit sphere (light from the top-left), dithered between 4 tones.
    fn shade_ellipse(&mut self, cx: f32, cy: f32, rx: f32, ry: f32, pal: [u32; 4]) {
        for y in (cy - ry).floor() as i32..=(cy + ry).ceil() as i32 {
            for x in (cx - rx).floor() as i32..=(cx + rx).ceil() as i32 {
                let (nx, ny) = ((x as f32 + 0.5 - cx) / rx, (y as f32 + 0.5 - cy) / ry);
                let d = nx * nx + ny * ny;
                if d > 1.0 {
                    continue;
                }
                let l = 0.55 - 0.42 * nx - 0.55 * ny - 0.3 * d + (bayer(x, y) - 0.5) * 0.18;
                let k = if l < 0.12 {
                    0
                } else if l < 0.42 {
                    1
                } else if l < 0.78 {
                    2
                } else {
                    3
                };
                self.px(x, y, hex(pal[k]));
            }
        }
    }
    fn shade_disc(&mut self, cx: f32, cy: f32, r: f32, pal: [u32; 4]) {
        self.shade_ellipse(cx, cy, r, r, pal);
    }
    /// Box lit from the top-left: light top/left edges, dark bottom/right edges.
    fn shade_box(&mut self, x: i32, y: i32, w: i32, h: i32, pal: [u32; 4]) {
        self.rect(x, y, w, h, hex(pal[2]));
        self.hline(x, y, w, hex(pal[3]));
        self.vline(x, y, h, hex(pal[3]));
        self.hline(x, y + h - 1, w, hex(pal[1]));
        self.vline(x + w - 1, y, h, hex(pal[1]));
    }
    /// Vertical cylinder shading (left lit, right in shadow), e.g. barrels and trunks.
    fn shade_cyl(&mut self, x: i32, y: i32, w: i32, h: i32, pal: [u32; 4]) {
        for xx in 0..w {
            let t = (xx as f32 + 0.5) / w as f32;
            for yy in 0..h {
                let l = 0.95 - 1.1 * (t - 0.3).abs() + (bayer(x + xx, y + yy) - 0.5) * 0.25;
                let k = if l < 0.3 {
                    0
                } else if l < 0.6 {
                    1
                } else if l < 0.88 {
                    2
                } else {
                    3
                };
                self.px(x + xx, y + yy, hex(pal[k]));
            }
        }
    }
    fn line(&mut self, x0: i32, y0: i32, x1: i32, y1: i32, c: Color) {
        let (dx, dy) = ((x1 - x0).abs(), -(y1 - y0).abs());
        let (sx, sy) = (if x0 < x1 { 1 } else { -1 }, if y0 < y1 { 1 } else { -1 });
        let (mut x, mut y, mut err) = (x0, y0, dx + dy);
        loop {
            self.px(x, y, c);
            if x == x1 && y == y1 {
                break;
            }
            let e2 = 2 * err;
            if e2 >= dy {
                err += dy;
                x += sx;
            }
            if e2 <= dx {
                err += dx;
                y += sy;
            }
        }
    }
    /// Thick line made of discs.
    fn stroke(&mut self, x0: f32, y0: f32, x1: f32, y1: f32, r: f32, c: Color) {
        let n = ((x1 - x0).hypot(y1 - y0) * 2.0).ceil().max(1.0) as i32;
        for k in 0..=n {
            let t = k as f32 / n as f32;
            self.disc(x0 + (x1 - x0) * t, y0 + (y1 - y0) * t, r, c);
        }
    }
    /// Dark outline around everything opaque in the `w` x `h` cell at the origin.
    fn auto_outline(&mut self, w: i32, h: i32, c: u32) {
        let mut edge = Vec::new();
        for y in 0..h {
            for x in 0..w {
                if self.get(x, y).a >= 0.5 {
                    continue;
                }
                let solid = |dx: i32, dy: i32| {
                    let (nx, ny) = (x + dx, y + dy);
                    nx >= 0 && ny >= 0 && nx < w && ny < h && self.get(nx, ny).a >= 0.5
                };
                if solid(1, 0) || solid(-1, 0) || solid(0, 1) || solid(0, -1) {
                    edge.push((x, y));
                }
            }
        }
        for (x, y) in edge {
            self.px(x, y, hex(c));
        }
    }
    /// Soft elliptical shadow under what is already drawn.
    fn shadow(&mut self, cx: f32, cy: f32, rx: f32, ry: f32) {
        for y in (cy - ry).floor() as i32..=(cy + ry).ceil() as i32 {
            for x in (cx - rx).floor() as i32..=(cx + rx).ceil() as i32 {
                let (dx, dy) = ((x as f32 + 0.5 - cx) / rx, (y as f32 + 0.5 - cy) / ry);
                let d = dx * dx + dy * dy;
                if d <= 1.0 {
                    self.under(x, y, Color::new(0.0, 0.0, 0.0, if d < 0.55 { 0.3 } else { 0.18 }));
                }
            }
        }
    }
    /// Copies the TS x TS tile at (0, `sy`) of another canvas to the current origin.
    fn blit_tile(&mut self, src: &Canvas, sy: u32) {
        for y in 0..TS {
            for x in 0..TS {
                let c = src.img.get_pixel(x, sy + y);
                self.img.set_pixel(self.ox + x, self.oy + y, c);
            }
        }
    }
    /// Base color with speckles and small blobs of a lighter / darker shade (tiles seamlessly).
    fn ground(&mut self, base: u32, light: u32, dark: u32, rng: &mut Rng, density: f32) {
        let n = TS as i32;
        self.rect(0, 0, n, n, hex(base));
        for y in 0..n {
            for x in 0..n {
                if rng.chance(density) {
                    let c = hex(if rng.chance(0.5) { light } else { dark });
                    self.px(x, y, c);
                    if rng.chance(0.3) {
                        self.wrap(x + 1, y, c);
                    }
                }
            }
        }
    }
    pub fn save(&self, path: &Path) {
        // `export_png` flips vertically (it's meant for GPU readback), so pre-flip.
        let mut img = self.img.clone();
        let row = img.width as usize * 4;
        let h = img.height as usize;
        for y in 0..h / 2 {
            let (a, b) = img.bytes.split_at_mut((h - 1 - y) * row);
            a[y * row..y * row + row].swap_with_slice(&mut b[..row]);
        }
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        img.export_png(path.to_str().unwrap());
    }
}

// ------------------------------------------------------------------ terrain tileset

pub mod extra;

pub mod t {
    pub const GRASS: u32 = 0;
    pub const FLOWERS: u32 = 1;
    pub const PATH: u32 = 2;
    pub const WATER: u32 = 3;
    pub const SAND: u32 = 4;
    pub const STONE_FLOOR: u32 = 5;
    pub const WOOD_FLOOR: u32 = 6;
    pub const VOID: u32 = 7;
    pub const STONE_WALL: u32 = 8;
    pub const HOUSE_WALL: u32 = 9;
    pub const ROOF: u32 = 10;
    pub const DOOR: u32 = 11;
    pub const TRUNK: u32 = 12;
    pub const CANOPY: u32 = 13;
    pub const BUSH: u32 = 14;
    pub const ROCK: u32 = 15;
    pub const FENCE: u32 = 16;
    pub const TALL_GRASS: u32 = 17;
    pub const BRIDGE: u32 = 18;
    pub const WINDOW: u32 = 19;
    pub const RUG: u32 = 20;
    pub const TABLE: u32 = 21;
    pub const BED: u32 = 22;
    pub const SHELF: u32 = 23;
    pub const FOREST_FLOOR: u32 = 24;
    pub const MUSHROOM: u32 = 25;
    pub const CAVE: u32 = 26;
    pub const CLIFF: u32 = 27;
    pub const ROOF_EDGE: u32 = 28;
    pub const PINE_TRUNK: u32 = 29;
    pub const PINE_TOP: u32 = 30;
    pub const SNOW: u32 = 31;
    // Animation frames for water (row 4).
    pub const WATER_2: u32 = 32;
    pub const WATER_3: u32 = 33;
    /// Lower half of the bed (blanket, no pillow).
    pub const BED_FOOT: u32 = 34;
}

pub const SOLID: [u32; 18] = [
    t::WATER, t::VOID, t::STONE_WALL, t::HOUSE_WALL, t::ROOF, t::TRUNK, t::BUSH, t::ROCK, t::FENCE,
    t::WINDOW, t::TABLE, t::BED, t::SHELF, t::CLIFF, t::ROOF_EDGE, t::PINE_TRUNK, t::MUSHROOM, t::BED_FOOT,
];

const GRASS: (u32, u32, u32) = (0x4f9a43, 0x5fae52, 0x438a3a);

fn grass(c: &mut Canvas, rng: &mut Rng) {
    c.ground(GRASS.0, GRASS.1, GRASS.2, rng, 0.16);
    // Little tufts of blades.
    for _ in 0..9 {
        let (x, y) = (rng.range(TS) as i32, rng.range(TS) as i32);
        for (dx, h) in [(0, 3), (1, 4), (2, 2)] {
            for k in 0..h {
                c.wrap(x + dx, y - k, hex(if k == h - 1 { 0x7cc56f } else { 0x3d8034 }));
            }
        }
    }
}

/// A tree drawn into a 32x64 canvas: the top half becomes the canopy tile (overhead layer),
/// the bottom half the trunk tile.
fn tree_pair(pine: bool) -> Canvas {
    let mut c = Canvas::new(TS, 2 * TS);
    if pine {
        c.shade_cyl(13, 40, 6, 20, WOOD);
        for (k, y) in [(5.0, 4.0), (8.0, 12.0), (11.0, 21.0), (13.5, 30.0), (15.0, 38.0)] {
            pine_tier(&mut c, 16.0, y, k, 10.0);
        }
    } else {
        c.shade_cyl(12, 30, 8, 30, WOOD);
        c.rect(10, 56, 12, 3, hex(WOOD[1])); // roots
        c.px(9, 58, hex(WOOD[1]));
        c.px(22, 58, hex(WOOD[1]));
        for (x, y, r) in [(16.0, 22.0, 14.0), (8.0, 24.0, 8.0), (24.0, 24.0, 8.0), (13.0, 14.0, 10.0), (21.0, 15.0, 9.0), (16.0, 9.0, 8.0)] {
            c.shade_disc(x, y, r, LEAF);
        }
    }
    c.auto_outline(TS as i32, 2 * TS as i32, 0x10301a);
    c.shadow(16.0, 59.5, 12.0, 3.5);
    c
}

/// One triangular tier of a pine, apex at (`cx`, `y`), half-width `k`, height `h`.
fn pine_tier(c: &mut Canvas, cx: f32, y: f32, k: f32, h: f32) {
    for row in 0..h as i32 {
        let half = k * (row as f32 + 1.0) / h;
        for x in (cx - half).floor() as i32..=(cx + half).ceil() as i32 {
            let nx = (x as f32 + 0.5 - cx) / half.max(1.0);
            let l = 0.6 - 0.5 * nx - 0.04 * row as f32 + (bayer(x, row) - 0.5) * 0.25;
            let k = if l < 0.15 {
                0
            } else if l < 0.45 {
                1
            } else if l < 0.8 {
                2
            } else {
                3
            };
            c.px(x, y as i32 + row, hex(PINE[k]));
        }
    }
}

pub fn terrain(out: &Path) {
    let cols = 8;
    let mut c = Canvas::new(cols * TS, 5 * TS);
    let mut rng = Rng(7);
    let n = TS as i32;
    let oak = tree_pair(false);
    let pine = tree_pair(true);
    for i in 0..35u32 {
        c.at(i % cols * TS, i / cols * TS);
        match i {
            t::GRASS => grass(&mut c, &mut rng),
            t::FLOWERS => {
                grass(&mut c, &mut rng);
                for _ in 0..5 {
                    let (x, y) = (rng.range(26) as i32 + 3, rng.range(26) as i32 + 3);
                    let col = [0xf4e04d, 0xf06292, 0xffffff, 0x9fa8ff, 0xff9f43][rng.range(5) as usize];
                    for (dx, dy) in [(0, -1), (-1, 0), (1, 0), (0, 1)] {
                        c.px(x + dx, y + dy, hex(col));
                    }
                    c.px(x, y, hex(0xfff3b0));
                    c.px(x + 1, y + 2, hex(0x2f6e2a));
                }
            }
            t::PATH => {
                c.ground(0xc2a46e, 0xd4b886, 0xab8d5a, &mut rng, 0.22);
                for _ in 0..5 {
                    let (x, y) = (rng.range(TS) as i32, rng.range(TS) as i32);
                    c.wrap(x, y, hex(0x9c8158));
                    c.wrap(x + 1, y, hex(0x9c8158));
                    c.wrap(x, y - 1, hex(0xe6d2a6));
                }
            }
            t::WATER | t::WATER_2 | t::WATER_3 => {
                // Same speckles each frame; the wave crests drift right.
                let mut wrng = Rng(77);
                c.ground(0x3a72c8, 0x4682d6, 0x3268ba, &mut wrng, 0.12);
                let shift = match i {
                    t::WATER => 0,
                    t::WATER_2 => 4,
                    _ => 8,
                };
                for (k, x) in [(0, 3), (1, 18), (2, 9), (3, 25)] {
                    let y = 4 + k * 8;
                    for d in 0..7 {
                        let lift = if d == 0 || d == 6 { 1 } else { 0 };
                        c.wrap(x + shift + d, y + lift, hex(0x9ccbf5));
                        c.wrap(x + shift + d, y + 1 + lift, hex(0x5a92dc));
                    }
                }
                c.wrap(12 + shift * 2, 15, hex(0xffffff));
            }
            t::SAND => {
                c.ground(0xe2cf92, 0xf0e0aa, 0xcdb878, &mut rng, 0.18);
                for y in [7, 22] {
                    for x in 0..n {
                        if (x / 4) % 3 != 0 {
                            c.px(x, y + ((x / 4) % 2), hex(0xd2bd80));
                        }
                    }
                }
            }
            t::STONE_FLOOR => {
                for (x, y) in [(0, 0), (16, 0), (0, 16), (16, 16)] {
                    let tone = if (x + y) % 32 == 0 { 0x8d8d96 } else { 0x85858e };
                    c.shade_box(x, y, 16, 16, ramp(tone));
                    for _ in 0..6 {
                        c.px(x + 2 + rng.range(12) as i32, y + 2 + rng.range(12) as i32, hex(mul(tone, 0.88)));
                    }
                }
            }
            t::WOOD_FLOOR => {
                for (row, y) in (0..n).step_by(8).enumerate() {
                    let tone = [0xa0703f, 0x98693a, 0xa87644, 0x9c6c3c][row % 4];
                    c.rect(0, y, n, 8, hex(tone));
                    c.hline(0, y, n, hex(mul(tone, 1.15)));
                    c.hline(0, y + 7, n, hex(0x5e3d1e));
                    let seam = [5, 21, 13, 27][row % 4];
                    c.vline(seam, y, 8, hex(0x5e3d1e));
                    c.px(seam - 2, y + 3, hex(0x4a2e14));
                    c.px(seam + 2, y + 3, hex(0x4a2e14));
                    for _ in 0..3 {
                        let (x, gy) = (rng.range(TS) as i32, y + 2 + rng.range(4) as i32);
                        c.hline(x, gy, 4, hex(mul(tone, 0.88)));
                    }
                }
            }
            t::VOID => c.rect(0, 0, n, n, hex(0x0d0d14)),
            t::STONE_WALL | t::CLIFF => {
                let base = if i == t::CLIFF { 0x6b5d52 } else { 0x8a8a94 };
                let mortar = if i == t::CLIFF { 0x3e342d } else { 0x4c4c55 };
                c.rect(0, 0, n, n, hex(mortar));
                for (row, y) in (0..n).step_by(8).enumerate() {
                    let off = if row % 2 == 0 { 0 } else { 8 };
                    for x in (-16 + off..n).step_by(16) {
                        let tone = mul(base, 0.92 + rng.f() * 0.16);
                        let pal = ramp(tone);
                        // Bricks wrap around the tile edge so walls tile seamlessly.
                        for yy in 0..7 {
                            for xx in 0..15 {
                                let col = if yy == 0 || xx == 0 {
                                    pal[3]
                                } else if yy == 6 || xx == 14 || rng.chance(0.08) {
                                    pal[1]
                                } else {
                                    pal[2]
                                };
                                c.wrap(x + xx, y + yy, hex(col));
                            }
                        }
                    }
                }
                if i == t::CLIFF {
                    for _ in 0..3 {
                        let (x, y) = (rng.range(28) as i32 + 2, rng.range(28) as i32 + 2);
                        c.line(x, y, x + 2, y + 3, hex(0x3e342d));
                    }
                }
            }
            t::HOUSE_WALL | t::WINDOW | t::DOOR => {
                c.ground(0xe0caa4, 0xeadab8, 0xd2b88f, &mut rng, 0.1);
                // Timber frame: a beam along the top and the left edge (walls stack and tile
                // seamlessly), plaster slightly shaded towards the bottom.
                c.rect(0, 0, n, 3, hex(WOOD[1]));
                c.hline(0, 0, n, hex(WOOD[2]));
                c.rect(0, 0, 3, n, hex(WOOD[1]));
                c.vline(0, 0, n, hex(WOOD[2]));
                for x in 3..n {
                    c.px(x, 3, hex(0xb89c74));
                    if (x + 31) % 4 < 2 {
                        c.px(x, 30, hex(0xd2b88f));
                    }
                    c.px(x, 31, hex(0xc8ac82));
                }
                if i == t::WINDOW {
                    c.rect(7, 6, 18, 17, hex(WOOD[0]));
                    c.rect(9, 8, 14, 13, hex(0x8ec9ef));
                    c.rect(9, 8, 14, 4, hex(0xb8e2fa));
                    c.line(11, 18, 15, 10, hex(0xe8f6ff));
                    c.vline(15, 8, 13, hex(WOOD[1]));
                    c.hline(9, 14, 14, hex(WOOD[1]));
                    c.rect(6, 22, 20, 2, hex(WOOD[2])); // sill
                    c.px(9, 21, hex(0xf06292)); // flowers on the sill
                    c.px(12, 21, hex(0xf4e04d));
                    c.px(20, 21, hex(0xf06292));
                }
                if i == t::DOOR {
                    c.rrect(6, 3, 20, 29, 6, hex(WOOD[0]));
                    c.rrect(8, 5, 16, 27, 5, hex(WOOD[2]));
                    for x in [11, 15, 19] {
                        c.vline(x, 6, 26, hex(WOOD[1]));
                    }
                    c.rect(8, 10, 16, 2, hex(IRON[1])); // hinges
                    c.rect(8, 23, 16, 2, hex(IRON[1]));
                    c.rect(20, 17, 2, 2, hex(GOLD[2]));
                    c.px(20, 17, hex(GOLD[3]));
                }
            }
            t::ROOF | t::ROOF_EDGE => {
                for (row, y) in (0..n).step_by(8).enumerate() {
                    let off = if row % 2 == 0 { 0 } else { 4 };
                    for x in (-8 + off..n).step_by(8) {
                        let tone = mul(0xa8433a, 0.92 + rng.f() * 0.14);
                        for yy in 0..8 {
                            for xx in 0..8 {
                                // Rounded shingle bottoms.
                                let bottom = yy == 7 || (yy == 6 && (xx == 0 || xx == 7));
                                let col = if bottom {
                                    0x5e231e
                                } else if xx == 0 {
                                    mul(tone, 0.8)
                                } else if yy < 2 {
                                    mul(tone, 1.12)
                                } else {
                                    tone
                                };
                                c.wrap(x + xx, y + yy, hex(col));
                            }
                        }
                    }
                }
                if i == t::ROOF_EDGE {
                    c.rect(0, 24, n, 6, hex(0x6e2a24));
                    c.hline(0, 24, n, hex(0x8e3730));
                    c.rect(0, 30, n, 2, hex(0x3a1512));
                }
            }
            t::TRUNK => c.blit_tile(&oak, TS),
            t::CANOPY => c.blit_tile(&oak, 0),
            t::PINE_TRUNK => c.blit_tile(&pine, TS),
            t::PINE_TOP => c.blit_tile(&pine, 0),
            t::BUSH => {
                for (x, y, r) in [(16.0, 18.0, 12.0), (9.0, 21.0, 7.0), (23.0, 21.0, 7.0), (14.0, 13.0, 7.0)] {
                    c.shade_disc(x, y, r, [0x1f5a22, 0x2c6e2a, 0x3d8a38, 0x63ab55]);
                }
                for (x, y) in [(10, 14), (20, 20), (14, 22), (22, 12)] {
                    c.rect(x, y, 2, 2, hex(0xd84343));
                    c.px(x, y, hex(0xff9a9a));
                }
                c.auto_outline(n, n, 0x10301a);
                c.shadow(16.0, 28.0, 13.0, 3.5);
            }
            t::ROCK => {
                c.shade_ellipse(16.0, 19.0, 12.0, 10.0, STONE);
                c.line(12, 14, 15, 19, hex(STONE[1]));
                c.line(15, 19, 14, 23, hex(STONE[1]));
                c.rect(19, 12, 3, 2, hex(0x5f8f4a)); // moss
                c.auto_outline(n, n, OUTLINE);
                c.shadow(16.0, 28.0, 13.0, 3.5);
            }
            t::FENCE => {
                for y in [10, 20] {
                    c.rect(0, y, n, 4, hex(WOOD[2]));
                    c.hline(0, y, n, hex(WOOD[3]));
                    c.hline(0, y + 3, n, hex(WOOD[1]));
                }
                for x in [5, 23] {
                    c.shade_box(x, 4, 5, 25, WOOD);
                    c.px(x + 2, 3, hex(WOOD[2]));
                    c.rect(x, 28, 5, 2, Color::new(0.0, 0.0, 0.0, 0.25));
                }
            }
            t::TALL_GRASS => {
                grass(&mut c, &mut rng);
                for _ in 0..16 {
                    let x = rng.range(30) as i32 + 1;
                    let h = 7 + rng.range(10) as i32;
                    let lean = rng.range(3) as i32 - 1;
                    for k in 0..h {
                        let xx = x + if k > h / 2 { lean } else { 0 };
                        c.px(xx, 31 - k, hex(if k > h - 3 { 0x8fd17c } else if k < 3 { 0x2a6a25 } else { 0x3b8a33 }));
                    }
                }
            }
            t::BRIDGE => {
                for x in (0..n).step_by(8) {
                    let tone = mul(0x9a6b3c, 0.92 + rng.f() * 0.14);
                    c.shade_box(x, 0, 8, n, ramp(tone));
                    c.px(x + 3, 6, hex(WOOD[0]));
                    c.px(x + 3, 25, hex(WOOD[0]));
                }
                c.rect(0, 0, n, 3, hex(WOOD[0]));
                c.rect(0, 29, n, 3, hex(WOOD[0]));
            }
            t::RUG => {
                // Seamless woven pattern: a gold diamond lattice with blue centres.
                c.rect(0, 0, n, n, hex(0xa83a3a));
                for y in 0..n {
                    for x in 0..n {
                        let (dx, dy) = ((x - 16).abs(), (y - 16).abs());
                        let d = dx + dy;
                        if d == 15 || d == 16 {
                            c.px(x, y, hex(0xe8c05a));
                        } else if d == 13 || d == 18 {
                            c.px(x, y, hex(0x7e2626));
                        } else if d < 4 {
                            c.px(x, y, hex(if d < 2 { 0x6f97d6 } else { 0x3f6fb5 }));
                        } else if (x + y) % 4 == 0 && d > 6 && d < 11 {
                            c.px(x, y, hex(0xc04848));
                        }
                    }
                }
            }
            t::TABLE => {
                c.shadow(16.0, 28.0, 14.0, 3.0);
                c.shade_box(2, 6, 28, 16, WOOD);
                c.rect(4, 22, 3, 8, hex(WOOD[0]));
                c.rect(25, 22, 3, 8, hex(WOOD[0]));
                c.rect(8, 8, 16, 11, hex(0xf2efe6)); // cloth
                c.hline(8, 18, 16, hex(0xcfc9b8));
                c.rect(11, 10, 4, 5, hex(0xd9d9e0)); // cup
                c.px(11, 10, hex(0xffffff));
                c.disc(20.0, 13.0, 3.0, hex(0xd84343)); // apple
                c.px(19, 11, hex(0xff9a9a));
                c.auto_outline(n, n, OUTLINE);
            }
            t::BED => {
                // Headboard, pillow and the top of the blanket; continues in BED_FOOT.
                c.rect(2, 0, 28, n, hex(WOOD[2]));
                c.vline(2, 0, n, hex(WOOD[3]));
                c.vline(29, 0, n, hex(WOOD[1]));
                c.shade_box(2, 0, 28, 4, WOOD);
                c.rrect(5, 5, 22, 9, 3, hex(0xf5f5f5));
                c.hline(7, 13, 18, hex(0xcfcfd8));
                c.rect(4, 16, 24, 16, hex(0x3f6fb5));
                c.rect(4, 16, 24, 4, hex(0xe8e8f0)); // folded sheet
                c.hline(4, 20, 24, hex(0x2f5590));
            }
            t::BED_FOOT => {
                c.rect(2, 0, 28, n, hex(WOOD[2]));
                c.vline(2, 0, n, hex(WOOD[3]));
                c.vline(29, 0, n, hex(WOOD[1]));
                c.rect(4, 0, 24, 26, hex(0x3f6fb5));
                for y in [8, 17] {
                    c.hline(4, y, 24, hex(0x2f5590));
                }
                c.hline(4, 25, 24, hex(0x2f5590));
                c.shade_box(2, 26, 28, 6, WOOD); // footboard
            }
            t::SHELF => {
                c.shade_box(0, 0, n, n, WOOD);
                c.rect(2, 2, 28, 28, hex(0x3d2712));
                for (row, y) in [(0u32, 3), (1, 17)] {
                    let mut x = 3;
                    let mut k = row;
                    while x < 28 {
                        let w = 2 + (k * 7 % 3) as i32;
                        let h = 9 + (k * 5 % 4) as i32;
                        let col = [0xc0392b, 0x2980b9, 0x27ae60, 0xf1c40f, 0x8e44ad, 0xd35400][(k % 6) as usize];
                        c.rect(x, y + 12 - h, w, h, hex(col));
                        c.vline(x, y + 12 - h, h, hex(mul(col, 1.25)));
                        x += w + 1;
                        k += 1;
                    }
                    c.rect(1, y + 12, 30, 2, hex(WOOD[2]));
                }
            }
            t::FOREST_FLOOR => {
                c.ground(0x356b33, 0x427d3d, 0x2b5a29, &mut rng, 0.25);
                for _ in 0..4 {
                    let (x, y) = (rng.range(TS) as i32, rng.range(TS) as i32);
                    let col = [0x8a6d2e, 0x6b8a2e, 0xa0522d][rng.range(3) as usize];
                    c.wrap(x, y, hex(col));
                    c.wrap(x + 1, y, hex(col));
                    c.wrap(x + 1, y + 1, hex(mul(col, 0.8)));
                }
                for _ in 0..2 {
                    let (x, y) = (rng.range(TS) as i32, rng.range(TS) as i32);
                    for k in 0..4 {
                        c.wrap(x + k, y + k / 2, hex(0x5a4020));
                    }
                }
            }
            t::MUSHROOM => {
                for (cx, cy, r, stem) in [(12.0f32, 14.0f32, 9.0f32, 7), (24.0, 22.0, 5.0, 4)] {
                    let sx = cx as i32;
                    c.shade_box(sx - stem / 2, cy as i32, stem, (30.0 - cy) as i32, [0xb8b0a0, 0xd8d0c0, 0xeeeeee, 0xffffff]);
                    for y in (cy - r) as i32..=cy as i32 {
                        for x in (cx - r) as i32..=(cx + r) as i32 {
                            let (dx, dy) = ((x as f32 + 0.5 - cx) / r, (y as f32 + 0.5 - cy) / (r * 0.8));
                            if dx * dx + dy * dy <= 1.0 {
                                let l = 0.6 - 0.5 * dx - 0.5 * dy + (bayer(x, y) - 0.5) * 0.2;
                                c.px(x, y, hex(if l < 0.3 { 0x8e1c1c } else if l < 0.8 { 0xc62828 } else { 0xef5350 }));
                            }
                        }
                    }
                }
                for (x, y) in [(8, 9), (13, 7), (16, 11), (23, 18)] {
                    c.rect(x, y, 2, 2, hex(0xffffff));
                }
                c.auto_outline(n, n, 0x2a0e0e);
                c.shadow(16.0, 30.0, 13.0, 2.5);
            }
            t::CAVE => {
                c.ground(0x6b5d52, 0x7a6b5f, 0x5a4d43, &mut rng, 0.2);
                c.rrect(3, 6, 26, 26, 11, hex(0x3e342d));
                c.rrect(5, 8, 22, 24, 10, hex(0x120d0a));
                c.rect(8, 26, 16, 6, hex(0x1e1712));
            }
            t::SNOW => {
                c.ground(0xeef2f7, 0xffffff, 0xd8e0ec, &mut rng, 0.14);
                for _ in 0..4 {
                    c.px(rng.range(TS) as i32, rng.range(TS) as i32, hex(0xb8c8e0));
                }
            }
            _ => {}
        }
    }
    c.save(&out.join("tilesets/terrain.png"));
}

// ------------------------------------------------------------------ characters

#[derive(Default)]
pub struct Look {
    pub skin: u32,
    pub hair: u32,
    pub shirt: u32,
    pub pants: u32,
    pub beard: bool,
    /// Pointy ears (goblins).
    pub ears: bool,
    pub hat: Hat,
    /// A long robe or dress (covers the legs).
    pub robe: Option<u32>,
    pub apron: Option<u32>,
    /// Cloth over the lower face (nomads, bandits).
    pub veil: Option<u32>,
    /// Bones instead of skin and clothes.
    pub skeleton: bool,
    /// Female look: longer hair, eyelashes, lips.
    pub female: bool,
}

#[derive(Clone, Copy, Default)]
pub enum Hat {
    #[default]
    None,
    Straw(u32),
    Wizard(u32),
    /// Head wrap / turban.
    Wrap(u32),
    Helmet,
    Cap(u32),
    Tiara,
    Hood(u32),
}

#[derive(Clone, Copy)]
pub enum Gear {
    Cap,
    Helmet,
    Leather,
    Chain,
}

/// Character frames are 32x48; feet at the bottom.
const FW: u32 = 32;
const FH: u32 = 48;

/// Head bob and leg lift of walk frame `frame`.
fn walk(frame: u32) -> (i32, i32, i32) {
    let bob = if frame % 2 == 1 { 1 } else { 0 };
    let (l, r) = match frame {
        1 => (2, 0),
        3 => (0, 2),
        _ => (0, 0),
    };
    (bob, l, r)
}

/// Top of the head and of the torso for a walk frame.
fn body_y(frame: u32) -> (i32, i32) {
    let (bob, ..) = walk(frame);
    (5 + bob, 22 + bob)
}

fn draw_person(c: &mut Canvas, look: &Look, f: Facing, frame: u32) {
    let (_, l_up, r_up) = walk(frame);
    let (hy, by) = body_y(frame);
    let skin = ramp(look.skin);
    let hair = ramp(look.hair);
    let shirt = ramp(look.shirt);
    let pants = ramp(look.pants);
    let boots = [0x24180e, 0x3a2a1e, 0x4e3a28, 0x6a5038];
    let side = matches!(f, Facing::Left | Facing::Right);
    let dir = if f == Facing::Left { -1 } else { 1 };
    let swing = match frame {
        1 => 2,
        3 => -2,
        _ => 0,
    };

    // Legs (or a robe reaching the ground).
    let leg = |c: &mut Canvas, x: i32, lift: i32, pal: [u32; 4]| {
        c.shade_box(x, 34 - lift, 5, 9, pal);
        c.shade_box(x, 41 - lift, 5, 4, boots);
    };
    if let Some(col) = look.robe {
        let pal = ramp(col);
        for (k, y) in (by + 9..45).enumerate() {
            let half = 7 + (k as i32) / 4;
            c.hline(16 - half, y, 2 * half + 1, hex(pal[2]));
            c.px(16 - half, y, hex(pal[3]));
            c.px(16 + half, y, hex(pal[1]));
        }
        c.hline(9, 44, 15, hex(pal[0]));
        // Shoes peeking out while walking.
        if l_up > 0 || r_up > 0 {
            c.rect(if l_up > 0 { 11 } else { 18 }, 44, 4, 2, hex(boots[1]));
        }
    } else if side {
        leg(c, 14 - swing * dir, r_up, [pants[0], pants[0], pants[1], pants[2]]);
        leg(c, 14 + swing * dir, l_up, pants);
    } else {
        leg(c, 11, l_up, pants);
        leg(c, 17, r_up, pants);
    }

    // Hood mass behind the head.
    if let Hat::Hood(col) = look.hat {
        c.rrect(6, hy - 3, 21, 22, 6, hex(ramp(col)[1]));
    }
    // Back arm (side view) goes behind the torso.
    if side {
        c.shade_box(14 + swing * dir, by + 2, 4, 9, [shirt[0], shirt[0], shirt[1], shirt[1]]);
    }

    // Torso.
    c.rrect(9, by, 15, 13, 2, hex(shirt[2]));
    c.vline(10, by + 1, 11, hex(shirt[3]));
    c.vline(22, by + 1, 11, hex(shirt[1]));
    c.hline(10, by + 12, 13, hex(shirt[1]));
    c.rect(9, by + 10, 15, 2, hex(0x4a2c14)); // belt
    if look.skeleton {
        // Rib cage over a dark torso.
        let bone = ramp(look.skin);
        c.rrect(9, by, 15, 13, 2, hex(0x2b2620));
        for y in [by + 2, by + 5, by + 8] {
            c.hline(11, y, 11, hex(bone[2]));
        }
        c.vline(16, by + 1, 11, hex(bone[3]));
    }
    if let Some(col) = look.apron {
        let pal = ramp(col);
        match f {
            Facing::Down => {
                c.shade_box(11, by + 3, 11, 17, pal);
                c.hline(11, by + 3, 11, hex(pal[3]));
            }
            Facing::Left => c.shade_box(9, by + 4, 6, 15, pal),
            Facing::Right => c.shade_box(18, by + 4, 6, 15, pal),
            Facing::Up => c.hline(9, by + 4, 15, hex(pal[1])),
        }
    }
    if f == Facing::Down && !look.skeleton {
        c.rect(15, by + 10, 3, 2, hex(GOLD[2]));
        c.px(16, by, hex(skin[2]));
        c.px(15, by, hex(skin[2]));
        c.px(17, by, hex(skin[2]));
        c.px(16, by + 1, hex(skin[2]));
    }

    // Arms.
    if side {
        let x = 14 - swing * dir;
        c.shade_box(x, by + 2, 4, 7, shirt);
        c.shade_box(x, by + 9, 4, 3, skin);
    } else {
        for (x, s) in [(5, swing), (24, -swing)] {
            c.shade_box(x, by + 1 + s / 2, 4, 7, shirt);
            c.shade_box(x, by + 8 + s / 2, 4, 3, skin);
        }
    }

    // Head.
    let (hx, hw, hh) = (8, 17, 16);
    c.rrect(hx, hy, hw, hh, 4, hex(skin[2]));
    c.hline(hx + 3, hy + hh - 1, hw - 6, hex(skin[1]));
    c.vline(hx + hw - 1, hy + 4, hh - 8, hex(skin[1]));
    if look.ears {
        let ear = |c: &mut Canvas, x: i32, d: i32| {
            for k in 0..4 {
                c.vline(x + d * k, hy + 6 + k / 2, 4 - k / 2, hex(skin[if k == 0 { 2 } else { 1 }]));
            }
        };
        match f {
            Facing::Down | Facing::Up => {
                ear(c, hx - 1, -1);
                ear(c, hx + hw, 1);
            }
            Facing::Left => ear(c, hx + hw, 1),
            Facing::Right => ear(c, hx - 1, -1),
        }
    }
    let skeleton = look.skeleton;
    let eye = |c: &mut Canvas, x: i32| {
        if skeleton {
            c.rect(x - 1, hy + 7, 4, 4, hex(0x1a1612)); // dark sockets
            c.px(x, hy + 8, hex(0xd84343));
        } else {
            c.rect(x, hy + 8, 2, 3, hex(0x1a1a22));
            c.px(x, hy + 8, hex(0xffffff));
        }
    };
    // Hair cap (lit from the top-left).
    let cap = |c: &mut Canvas, x: i32, w: i32, h: i32| {
        c.rrect(x, hy - 1, w, h, 4, hex(hair[2]));
        c.hline(x + 4, hy, w - 8, hex(hair[3]));
        c.px(x + 3, hy + 1, hex(hair[3]));
    };
    match f {
        Facing::Down => {
            cap(c, hx, hw, 7);
            for x in (hx + 1..hx + hw - 1).step_by(3) {
                c.px(x + 1, hy + 6, hex(hair[1])); // fringe
            }
            c.rect(hx, hy + 4, 2, 7, hex(hair[1]));
            c.rect(hx + hw - 2, hy + 4, 2, 7, hex(hair[1]));
            eye(c, 11);
            eye(c, 20);
            if !skeleton {
                c.px(10, hy + 12, hex(0xe8907a)); // cheeks
                c.px(22, hy + 12, hex(0xe8907a));
            }
            if skeleton {
                for x in (12..21).step_by(2) {
                    c.px(x, hy + 13, hex(0x1a1612)); // teeth
                }
            } else if look.beard {
                c.rrect(10, hy + 10, 13, 9, 3, hex(hair[2]));
                c.hline(12, hy + 10, 9, hex(hair[3]));
                c.hline(14, hy + 13, 5, hex(hair[0]));
            } else if look.female {
                c.hline(15, hy + 13, 3, hex(0xc0505a)); // lips
                c.px(16, hy + 14, hex(0xa03a46));
            } else {
                c.hline(15, hy + 13, 3, hex(skin[0]));
            }
            if look.female {
                // Long hair falling past the face, eyelashes.
                c.rect(hx - 1, hy + 4, 3, 15, hex(hair[2]));
                c.vline(hx - 1, hy + 5, 13, hex(hair[3]));
                c.rect(hx + hw - 2, hy + 4, 3, 15, hex(hair[1]));
                for x in [11, 20] {
                    c.hline(x - 1, hy + 7, 3, hex(0x1a1a22));
                }
            }
        }
        Facing::Up => {
            cap(c, hx, hw, hh);
            c.vline(hx + hw - 2, hy + 3, hh - 6, hex(hair[1]));
            c.hline(hx + 3, hy + hh - 2, hw - 6, hex(hair[1]));
            if look.female {
                c.rrect(hx, hy + 8, hw, 14, 4, hex(hair[2]));
                c.vline(hx + hw - 2, hy + 9, 12, hex(hair[1]));
                c.vline(hx + 2, hy + 9, 11, hex(hair[3]));
            }
        }
        Facing::Left | Facing::Right => {
            let mirror = |x: i32| if f == Facing::Left { x } else { 2 * 16 - x };
            cap(c, hx, hw, 7);
            // Hair at the back of the head.
            let (bx, bw) = if f == Facing::Left { (hx + 8, 9) } else { (hx, 9) };
            c.rrect(bx, hy + 2, bw, if look.female { 19 } else { 12 }, 3, hex(hair[2]));
            c.vline(if f == Facing::Left { hx + hw - 2 } else { hx + 1 }, hy + 4, 9, hex(hair[1]));
            let ex = if f == Facing::Left { 10 } else { 21 };
            eye(c, ex);
            if look.female && !skeleton {
                c.hline(ex - 1, hy + 7, 3, hex(0x1a1a22));
                c.px(if f == Facing::Left { 9 } else { 23 }, hy + 13, hex(0xc0505a));
            }
            c.px(mirror(8) - if f == Facing::Left { 1 } else { 0 }, hy + 11, hex(skin[2])); // nose
            c.px(mirror(8) - if f == Facing::Left { 1 } else { 0 }, hy + 12, hex(skin[1]));
            if !look.ears {
                let ear_x = if f == Facing::Left { hx + 6 } else { hx + 9 };
                c.rect(ear_x, hy + 8, 2, 3, hex(skin[1]));
            }
            if look.beard {
                let x = if f == Facing::Left { hx } else { hx + 7 };
                c.rrect(x, hy + 10, 10, 9, 3, hex(hair[2]));
            }
        }
    }
    draw_headwear(c, look, f, hy);
}

/// Hats, hoods and veils (drawn over the finished head).
fn draw_headwear(c: &mut Canvas, look: &Look, f: Facing, hy: i32) {
    let side = matches!(f, Facing::Left | Facing::Right);
    let back = if f == Facing::Left { 1 } else { -1 }; // direction of the back of the head
    match look.hat {
        Hat::None => {}
        Hat::Straw(col) => {
            let p = ramp(col);
            c.ellipse(16.0, hy as f32 + 2.5, 15.0, 3.5, hex(p[1]));
            c.rrect(10, hy - 5, 13, 8, 3, hex(p[2]));
            c.hline(12, hy - 4, 8, hex(p[3]));
            c.hline(10, hy + 1, 13, hex(0xb03a2e)); // band
        }
        Hat::Wizard(col) => {
            let p = ramp(col);
            c.ellipse(16.0, hy as f32 + 1.5, 13.0, 3.0, hex(p[1]));
            // A cone whose tip flops over to the side (there is only ~5 px above the head).
            let tip = if side { -back } else { 1 };
            for row in 0..6 {
                let half = 8.0 - row as f32 * 1.2;
                let cx = 16.0 + (row * row) as f32 / 8.0 * tip as f32;
                let y = hy + 1 - row;
                for x in (cx - half).round() as i32..=(cx + half).round() as i32 {
                    let shade = if (x as f32) < cx - half / 3.0 { p[3] } else if (x as f32) > cx + half / 2.0 { p[1] } else { p[2] };
                    c.px(x, y, hex(shade));
                }
            }
            for k in 0..6 {
                c.px(16 + tip * (5 + k), hy - 4 + k / 2, hex(p[1])); // drooping tip
            }
            c.px(16 + tip * 11, hy - 1, hex(GOLD[3]));
            c.px(13, hy - 1, hex(GOLD[3]));
            c.px(18, hy - 3, hex(GOLD[3]));
        }
        Hat::Wrap(col) => {
            let p = ramp(col);
            c.rrect(7, hy - 3, 19, 9, 4, hex(p[2]));
            c.hline(9, hy, 15, hex(p[1]));
            c.hline(10, hy + 3, 13, hex(p[1]));
            c.hline(10, hy - 2, 10, hex(p[3]));
            if side {
                let x = if back > 0 { 23 } else { 7 };
                c.rect(x, hy + 3, 3, 10, hex(p[1])); // loose end
            }
        }
        Hat::Helmet => {
            let p = ramp(0x9ea7b0);
            c.rrect(7, hy - 3, 19, 11, 5, hex(p[2]));
            c.hline(11, hy - 2, 10, hex(p[3]));
            c.hline(7, hy + 6, 19, hex(p[1]));
            match f {
                Facing::Down => {
                    c.shade_box(7, hy + 6, 4, 8, p);
                    c.shade_box(22, hy + 6, 4, 8, p);
                    c.rect(16, hy + 6, 2, 5, hex(p[1]));
                }
                Facing::Up => c.rrect(7, hy - 3, 19, 18, 5, hex(p[2])),
                _ => {
                    let x = if back > 0 { 16 } else { 8 };
                    c.shade_box(x, hy + 6, 9, 9, p);
                }
            }
            c.rect(14, hy - 6, 5, 4, hex(0xb03a2e)); // plume
        }
        Hat::Cap(col) => {
            let p = ramp(col);
            c.rrect(8, hy - 2, 17, 7, 3, hex(p[2]));
            c.hline(11, hy - 1, 10, hex(p[3]));
            match f {
                Facing::Down => c.rect(7, hy + 4, 19, 2, hex(p[0])),
                Facing::Left => c.rect(4, hy + 4, 12, 2, hex(p[0])),
                Facing::Right => c.rect(17, hy + 4, 12, 2, hex(p[0])),
                Facing::Up => {}
            }
        }
        Hat::Tiara => {
            if f != Facing::Up {
                c.hline(10, hy + 1, 13, hex(GOLD[2]));
                c.px(12, hy, hex(GOLD[3]));
                c.px(20, hy, hex(GOLD[3]));
                if f == Facing::Down {
                    c.rect(15, hy - 1, 3, 2, hex(0xd84343));
                }
            } else {
                c.hline(10, hy + 1, 13, hex(GOLD[1]));
            }
        }
        Hat::Hood(col) => {
            let p = ramp(col);
            match f {
                Facing::Down => {
                    c.rrect(7, hy - 3, 19, 8, 4, hex(p[2]));
                    c.rect(7, hy + 3, 3, 12, hex(p[2]));
                    c.rect(23, hy + 3, 3, 12, hex(p[1]));
                    c.hline(11, hy - 2, 10, hex(p[3]));
                }
                Facing::Up => c.rrect(7, hy - 3, 19, 20, 6, hex(p[2])),
                _ => {
                    let x = if back > 0 { 15 } else { 7 };
                    c.rrect(x, hy - 3, 11, 19, 5, hex(p[2]));
                    c.rrect(7, hy - 3, 19, 7, 4, hex(p[2]));
                }
            }
        }
    }
    if let Some(col) = look.veil {
        let p = ramp(col);
        match f {
            Facing::Down => {
                c.rrect(9, hy + 10, 15, 6, 2, hex(p[2]));
                c.hline(10, hy + 12, 13, hex(p[1]));
            }
            Facing::Left => c.rrect(7, hy + 10, 10, 6, 2, hex(p[2])),
            Facing::Right => c.rrect(16, hy + 10, 10, 6, 2, hex(p[2])),
            Facing::Up => {}
        }
    }
}

pub fn character(out: &Path, name: &str, look: &Look) {
    let mut c = Canvas::new(FW * 4, FH * 4);
    for f in Facing::ALL {
        for frame in 0..4u32 {
            c.at(frame * FW, f as u32 * FH);
            draw_person(&mut c, look, f, frame);
            c.auto_outline(FW as i32, FH as i32, OUTLINE);
            c.shadow(16.0, 45.5, 10.0, 2.5);
        }
    }
    c.save(&out.join(format!("sprites/{name}.png")));
}

/// Armor drawn over the character sheet layout (same frames, same head bob).
pub fn overlay(out: &Path, name: &str, gear: Gear) {
    let mut c = Canvas::new(FW * 4, FH * 4);
    for f in Facing::ALL {
        for frame in 0..4u32 {
            c.at(frame * FW, f as u32 * FH);
            let (hy, by) = body_y(frame);
            let side = matches!(f, Facing::Left | Facing::Right);
            let outline = match gear {
                Gear::Cap => {
                    let pal = ramp(0x8d5a2b);
                    c.rrect(7, hy - 2, 19, 8, 4, hex(pal[2]));
                    c.hline(11, hy - 1, 10, hex(pal[3]));
                    c.hline(8, hy + 4, 17, hex(pal[1]));
                    match f {
                        Facing::Down => c.rect(6, hy + 5, 21, 2, hex(pal[0])),
                        Facing::Left => c.rect(4, hy + 5, 13, 2, hex(pal[0])),
                        Facing::Right => c.rect(16, hy + 5, 13, 2, hex(pal[0])),
                        Facing::Up => c.rrect(7, hy - 2, 19, 11, 4, hex(pal[2])),
                    }
                    0x3a2410
                }
                Gear::Helmet => {
                    let pal = ramp(0x9ea7b0);
                    c.rrect(7, hy - 3, 19, 11, 5, hex(pal[2]));
                    c.hline(11, hy - 2, 10, hex(pal[3]));
                    c.rect(10, hy - 1, 3, 2, hex(0xffffff));
                    c.hline(7, hy + 6, 19, hex(pal[1]));
                    match f {
                        Facing::Down => {
                            c.shade_box(7, hy + 6, 4, 9, pal); // cheek guards
                            c.shade_box(22, hy + 6, 4, 9, pal);
                            c.rect(16, hy + 6, 2, 6, hex(pal[1])); // nose guard
                        }
                        Facing::Left => c.shade_box(16, hy + 6, 9, 9, pal),
                        Facing::Right => c.shade_box(8, hy + 6, 9, 9, pal),
                        Facing::Up => c.rrect(7, hy - 3, 19, 19, 5, hex(pal[2])),
                    }
                    0x2f353b
                }
                Gear::Leather | Gear::Chain => {
                    let pal = match gear {
                        Gear::Leather => ramp(0x8d5a2b),
                        _ => ramp(0xa7b1b9),
                    };
                    c.rrect(9, by, 15, 11, 2, hex(pal[2]));
                    c.vline(10, by + 1, 9, hex(pal[3]));
                    c.vline(22, by + 1, 9, hex(pal[1]));
                    c.rect(9, by + 9, 15, 2, hex(pal[0])); // belt
                    if !side {
                        c.shade_box(4, by, 6, 4, pal); // shoulder pads
                        c.shade_box(23, by, 6, 4, pal);
                    }
                    if matches!(gear, Gear::Chain) {
                        for y in (by + 2..by + 9).step_by(2) {
                            for x in (11 + (y % 2)..22).step_by(2) {
                                c.px(x, y, hex(pal[1]));
                            }
                        }
                    } else {
                        c.vline(16, by + 1, 8, hex(pal[1])); // lacing
                        c.px(15, by + 3, hex(pal[1]));
                        c.px(17, by + 5, hex(pal[1]));
                    }
                    mul(pal[0], 0.6)
                }
            };
            c.auto_outline(FW as i32, FH as i32, outline);
        }
    }
    c.save(&out.join(format!("sprites/{name}.png")));
}

pub fn slime(out: &Path) {
    let (fw, fh) = (32u32, 32u32);
    let mut c = Canvas::new(fw * 4, fh * 4);
    let pal = [0x1f7a63, 0x2fa784, 0x4fc3a1, 0x9ff0d8];
    for f in Facing::ALL {
        for frame in 0..4u32 {
            c.at(frame * fw, f as u32 * fh);
            let squish = [0, 2, 0, -2][frame as usize] as f32;
            let (w, h) = (24.0 + squish, 18.0 - squish);
            let cy = 29.0 - h / 2.0;
            c.shade_ellipse(16.0, cy + 2.0, w / 2.0, h / 2.0 + 2.0, pal);
            // Flat bottom: cut the body at row 30 — also what the ellipse drew below the frame
            // (rows 32-33 belong to the next facing row, which is drawn after this one).
            for x in 0..32 {
                for y in 30..36 {
                    c.px(x, y, CLEAR);
                }
            }
            c.rect(9, (cy - h / 2.0 + 5.0) as i32, 3, 2, hex(0xe0fff6)); // shine
            c.px(8, (cy - h / 2.0 + 7.0) as i32, hex(0xe0fff6));
            let eye = |c: &mut Canvas, x: i32| {
                let y = (cy + 1.0) as i32;
                c.rect(x, y, 3, 4, hex(0x10302a));
                c.px(x, y, hex(0xffffff));
            };
            match f {
                Facing::Down => {
                    eye(&mut c, 10);
                    eye(&mut c, 19);
                    c.hline(14, (cy + 6.0) as i32, 4, hex(0x10302a));
                }
                Facing::Left => eye(&mut c, 7),
                Facing::Right => eye(&mut c, 22),
                Facing::Up => {}
            }
            c.auto_outline(fw as i32, fh as i32, 0x0d3a30);
            c.shadow(16.0, 30.5, 13.0, 2.0);
        }
    }
    c.save(&out.join("sprites/slime.png"));
}

// ------------------------------------------------------------------ props

pub mod p {
    pub const CHEST: u32 = 0; // +1 open
    pub const SIGN: u32 = 2;
    pub const GATE: u32 = 3;
    /// The small gate, opened (a free cell; open frame offset = GATE_OPEN - GATE).
    pub const GATE_OPEN: u32 = 15;
    /// The small gate, destroyed (bent bars, rubble).
    pub const GATE_BROKEN: u32 = 23;
    pub const BARREL: u32 = 4;
    pub const SHRINE: u32 = 5; // +1 glowing
    pub const WELL: u32 = 7;
    // Multi-cell props (sheet is 8 cells wide).
    pub const DOUBLE_GATE: u32 = 8; // 2x2, open state at +2
    pub const BOULDER: u32 = 12; // 2x2
    pub const STATUE: u32 = 14; // 1x2
    pub const BIG_OAK: u32 = 24; // 3x4
    pub const BIG_PINE: u32 = 27; // 2x4
    pub const VGATE: u32 = 29; // 1x4 vertical double gate, open state at +1
}

/// Origin of sheet cell `cell` (8 cells per row).
fn cell(cell: u32) -> (u32, u32) {
    (cell % 8 * TS, cell / 8 * TS)
}

/// Iron bars with spear tips between `x0` and `x1`, from `top` to `bottom`.
fn bars(c: &mut Canvas, x0: i32, x1: i32, top: i32, bottom: i32, step: usize) {
    for x in (x0..x1).step_by(step) {
        c.shade_box(x, top, 3, bottom - top, IRON);
        c.px(x + 1, top - 1, hex(IRON[3]));
        c.px(x + 1, top - 2, hex(IRON[2]));
    }
}

fn big_props(c: &mut Canvas) {
    // Double gate, closed (2x2) and open (+2).
    for (k, open) in [(0u32, false), (2, true)] {
        let (x, y) = cell(p::DOUBLE_GATE + k);
        c.at(x, y);
        for px in [0, 53] {
            c.shade_box(px, 6, 11, 58, STONE);
            c.shade_box(px - 1, 2, 13, 6, STONE);
            for yy in (14..60).step_by(10) {
                c.hline(px + 1, yy, 9, hex(STONE[1]));
            }
        }
        if open {
            // Leaves swung inward, seen edge-on next to the posts.
            for px in [11, 47] {
                c.shade_box(px, 16, 6, 46, IRON);
            }
        } else {
            bars(c, 12, 52, 12, 62, 5);
            for yy in [18, 44] {
                c.shade_box(11, yy, 42, 4, IRON);
            }
            c.shade_box(29, 27, 6, 8, GOLD); // latch
            c.px(31, 30, hex(GOLD[0]));
        }
        c.auto_outline(64, 64, OUTLINE);
        c.shadow(32.0, 62.0, 30.0, 3.0);
    }
    // Boulder (2x2).
    let (x, y) = cell(p::BOULDER);
    c.at(x, y);
    c.shade_ellipse(32.0, 42.0, 27.0, 20.0, STONE);
    c.line(20, 34, 26, 44, hex(STONE[1]));
    c.line(26, 44, 24, 52, hex(STONE[1]));
    c.line(40, 30, 44, 38, hex(STONE[1]));
    for (mx, my) in [(30, 23), (34, 24), (38, 26), (27, 25)] {
        c.rect(mx, my, 4, 2, hex(0x5f8f4a));
        c.px(mx + 1, my, hex(0x86b86a));
    }
    c.auto_outline(64, 64, OUTLINE);
    c.shadow(32.0, 61.0, 28.0, 4.0);
    // Statue (1x2): a knight on a pedestal.
    let (x, y) = cell(p::STATUE);
    c.at(x, y);
    c.shade_box(3, 48, 26, 14, STONE);
    c.shade_box(5, 44, 22, 5, STONE);
    c.shade_box(10, 20, 12, 24, STONE); // body
    c.shade_disc(16.0, 14.0, 7.0, STONE); // head
    c.shade_box(6, 22, 4, 14, STONE); // arms
    c.shade_box(22, 22, 4, 14, STONE);
    c.shade_box(24, 6, 3, 30, [STONE[1], STONE[2], STONE[3], 0xe0e0ea]); // sword
    c.rect(21, 30, 9, 2, hex(STONE[1]));
    c.auto_outline(32, 64, OUTLINE);
    c.shadow(16.0, 62.0, 15.0, 2.5);
    // Big oak (3x4).
    let (x, y) = cell(p::BIG_OAK);
    c.at(x, y);
    c.shade_cyl(40, 66, 16, 58, WOOD);
    for (rx, w) in [(34, 6), (54, 6)] {
        c.shade_box(rx, 118, w, 6, WOOD);
    }
    for (bx, by, ex, ey) in [(44, 80, 26, 62), (52, 78, 70, 60)] {
        c.stroke(bx as f32, by as f32, ex as f32, ey as f32, 2.5, hex(WOOD[1]));
    }
    for (cx, cy, r) in [
        (48.0, 44.0, 34.0),
        (22.0, 56.0, 20.0),
        (74.0, 56.0, 20.0),
        (36.0, 30.0, 22.0),
        (62.0, 32.0, 20.0),
        (48.0, 20.0, 18.0),
        (30.0, 62.0, 13.0),
        (64.0, 64.0, 13.0),
    ] {
        c.shade_disc(cx, cy, r, LEAF);
    }
    for (ax, ay) in [(30, 40), (60, 50), (45, 26), (70, 38), (24, 58)] {
        c.rect(ax, ay, 3, 3, hex(0xd84343)); // apples
        c.px(ax, ay, hex(0xff9a9a));
    }
    c.auto_outline(96, 128, 0x10301a);
    c.shadow(48.0, 123.0, 30.0, 5.0);
    // Big pine (2x4).
    let (x, y) = cell(p::BIG_PINE);
    c.at(x, y);
    c.shade_cyl(28, 96, 9, 28, WOOD);
    for (k, ty) in [(9.0, 4.0), (14.0, 18.0), (19.0, 34.0), (24.0, 50.0), (28.0, 66.0), (30.0, 80.0)] {
        pine_tier(c, 32.0, ty, k, 20.0);
    }
    c.auto_outline(64, 128, 0x082a18);
    c.shadow(32.0, 123.0, 22.0, 4.0);
    // Vertical double gate (1x4, open at +1): seen from the side, a post at the top and the
    // bottom, the two leaves edge-on between them.
    for (k, open) in [(0u32, false), (1, true)] {
        let (x, y) = cell(p::VGATE + k);
        c.at(x, y);
        for py in [6, 102] {
            c.shade_box(7, py, 18, 24, STONE);
            c.shade_box(6, py - 3, 20, 5, STONE);
            c.hline(8, py + 12, 16, hex(STONE[1]));
        }
        if open {
            // Leaves folded back against the posts.
            c.shade_box(12, 30, 8, 12, IRON);
            c.shade_box(12, 90, 8, 12, IRON);
        } else {
            c.shade_box(10, 30, 12, 72, IRON);
            for yy in (34..100).step_by(6) {
                c.hline(12, yy, 8, hex(IRON[3]));
            }
            c.shade_box(8, 44, 16, 4, IRON);
            c.shade_box(8, 86, 16, 4, IRON);
            c.shade_box(18, 63, 6, 6, GOLD); // latch
        }
        c.auto_outline(32, 128, OUTLINE);
        for yy in 30..124 {
            c.under(4, yy, Color::new(0.0, 0.0, 0.0, 0.18));
            c.under(27, yy, Color::new(0.0, 0.0, 0.0, 0.12));
        }
    }
}

pub fn props(out: &Path) {
    let mut c = Canvas::new(TS * 8, TS * 7);
    let n = TS as i32;
    for i in 0..8u32 {
        let (x, y) = cell(i);
        c.at(x, y);
        match i {
            0 | 1 => {
                c.shade_box(4, 14, 24, 16, WOOD);
                for bx in [7, 22] {
                    c.shade_box(bx, 14, 3, 16, IRON);
                }
                if i == 0 {
                    c.rrect(4, 6, 24, 10, 3, hex(WOOD[2]));
                    c.hline(6, 7, 20, hex(WOOD[3]));
                    c.hline(4, 15, 24, hex(WOOD[0]));
                    for bx in [7, 22] {
                        c.rect(bx, 6, 3, 10, hex(IRON[2]));
                    }
                    c.shade_box(13, 12, 6, 7, GOLD);
                    c.rect(15, 15, 2, 3, hex(GOLD[0]));
                } else {
                    c.rrect(4, 1, 24, 9, 3, hex(WOOD[1])); // lid, swung back
                    c.hline(6, 2, 20, hex(WOOD[2]));
                    c.rect(5, 10, 22, 5, hex(0x1a1208));
                    for (gx, gy) in [(9, 12), (14, 11), (19, 12), (23, 11)] {
                        c.rect(gx, gy, 3, 2, hex(GOLD[2]));
                        c.px(gx, gy, hex(GOLD[3]));
                    }
                }
                c.auto_outline(n, n, OUTLINE);
                c.shadow(16.0, 30.0, 14.0, 2.5);
            }
            2 => {
                c.shade_box(14, 18, 4, 13, WOOD);
                c.shade_box(3, 4, 26, 16, WOOD);
                c.hline(4, 11, 24, hex(WOOD[1]));
                for (ly, lw) in [(7, 18), (14, 14)] {
                    c.hline(7, ly, lw, hex(WOOD[0]));
                }
                for nx in [5, 26] {
                    c.px(nx, 6, hex(IRON[3]));
                    c.px(nx, 17, hex(IRON[3]));
                }
                c.auto_outline(n, n, OUTLINE);
                c.shadow(16.0, 30.5, 8.0, 2.0);
            }
            3 => {
                for px in [0, 26] {
                    c.shade_box(px, 2, 6, 29, STONE);
                }
                bars(&mut c, 7, 26, 5, 31, 4);
                c.shade_box(6, 10, 20, 3, IRON);
                c.shade_box(6, 22, 20, 3, IRON);
                c.shade_box(14, 15, 4, 6, GOLD);
                c.auto_outline(n, n, OUTLINE);
            }
            4 => {
                c.shade_cyl(7, 7, 18, 23, WOOD);
                for hy in [10, 24] {
                    c.shade_box(7, hy, 18, 3, IRON);
                }
                c.ellipse(16.0, 7.0, 9.0, 3.5, hex(WOOD[2]));
                c.ellipse(16.0, 7.0, 7.0, 2.0, hex(WOOD[1]));
                c.auto_outline(n, n, OUTLINE);
                c.shadow(16.0, 30.0, 11.0, 2.5);
            }
            5 | 6 => {
                c.shade_box(5, 26, 22, 5, STONE);
                c.shade_box(9, 7, 14, 20, STONE);
                c.shade_box(7, 4, 18, 4, STONE);
                let glow = i == 6;
                c.rect(14, 11, 4, 11, hex(if glow { 0x7df9ff } else { STONE[0] }));
                if glow {
                    c.vline(15, 12, 9, hex(0xe0ffff));
                }
                c.auto_outline(n, n, OUTLINE);
                if glow {
                    for (gx, gy) in [(4, 8), (27, 13), (11, 1), (24, 3), (3, 20)] {
                        c.px(gx, gy, hex(0x7df9ff));
                        c.px(gx + 1, gy, Color::new(0.5, 0.97, 1.0, 0.5));
                    }
                    c.ellipse(16.0, 16.0, 15.0, 15.0, Color::new(0.5, 0.97, 1.0, 0.08));
                }
                c.shadow(16.0, 30.5, 12.0, 2.0);
            }
            7 => {
                c.shade_ellipse(16.0, 22.0, 13.0, 8.0, STONE);
                c.ellipse(16.0, 20.0, 9.0, 4.5, hex(0x1d3557));
                c.hline(12, 19, 5, hex(0x4a7ab0));
                for px in [4, 25] {
                    c.shade_box(px, 4, 3, 19, WOOD);
                }
                for (k, w) in [(0, 30), (1, 26), (2, 22)] {
                    c.rect(16 - w / 2, k * 2, w, 2, hex(if k == 0 { 0xc65a50 } else { 0xa8433a }));
                }
                c.vline(16, 6, 9, hex(0xd8c8a0)); // rope
                c.shade_box(14, 14, 5, 4, WOOD); // bucket
                c.auto_outline(n, n, OUTLINE);
                c.shadow(16.0, 30.0, 14.0, 2.5);
            }
            _ => {}
        }
    }
    // The small iron gate, opened: both leaves swung back against the posts.
    let (x, y) = cell(p::GATE_OPEN);
    c.at(x, y);
    for px in [0, 26] {
        c.shade_box(px, 2, 6, 29, STONE);
    }
    for lx in [6, 22] {
        c.shade_box(lx, 6, 4, 25, IRON);
        c.px(lx + 1, 5, hex(IRON[3]));
        c.px(lx + 2, 4, hex(IRON[2]));
    }
    c.auto_outline(n, n, OUTLINE);
    for yy in 26..31 {
        c.under(10, yy, Color::new(0.0, 0.0, 0.0, 0.15));
        c.under(21, yy, Color::new(0.0, 0.0, 0.0, 0.15));
    }
    // The small iron gate, destroyed: a cracked post, bent and broken bars, rubble.
    let (x, y) = cell(p::GATE_BROKEN);
    c.at(x, y);
    c.shade_box(0, 2, 6, 29, STONE);
    c.shade_box(26, 12, 6, 19, STONE); // the right post broke off
    c.line(27, 14, 30, 18, hex(STONE[0]));
    for (x0, y0, x1, y1) in [(8, 31, 9, 8), (12, 31, 14, 14), (17, 31, 21, 20), (22, 31, 25, 25)] {
        c.stroke(x0 as f32, y0 as f32, x1 as f32, y1 as f32, 1.0, hex(IRON[1]));
        c.px(x1, y1, hex(IRON[3]));
    }
    c.stroke(6.0, 22.0, 15.0, 26.0, 1.0, hex(IRON[1])); // fallen crossbar
    for (rx, ry, r) in [(19.0f32, 29.0f32, 2.5f32), (25.0, 29.5, 2.0), (13.0, 30.0, 1.5), (29.0, 30.0, 1.5)] {
        c.shade_disc(rx, ry, r, STONE);
    }
    c.auto_outline(n, n, OUTLINE);
    c.shadow(16.0, 30.5, 14.0, 1.5);
    big_props(&mut c);
    c.save(&out.join("sprites/props.png"));
}

// ------------------------------------------------------------------ items

pub mod it {
    pub const KEY: u32 = 0;
    pub const POTION: u32 = 1;
    pub const COIN: u32 = 2;
    pub const APPLE: u32 = 3;
    pub const LETTER: u32 = 4;
    pub const BOW: u32 = 5;
    pub const CROSSBOW: u32 = 6;
    pub const ARROWS: u32 = 7;
    pub const BOLTS: u32 = 8;
    /// Projectiles (drawn pointing right).
    pub const ARROW_SHOT: u32 = 9;
    pub const BOLT_SHOT: u32 = 10;
    pub const SWORD: u32 = 11;
    pub const SPEAR: u32 = 12;
    pub const AXE: u32 = 13;
    pub const CAP: u32 = 14;
    pub const HELMET: u32 = 15;
    pub const LEATHER: u32 = 16;
    pub const CHAIN: u32 = 17;
    pub const BRONZE_KEY: u32 = 18;
    /// Left-hand items: lights and a shield.
    pub const LANTERN: u32 = 19;
    pub const TORCH: u32 = 20;
    pub const SHIELD: u32 = 21;
}

/// An arrow pointing right along row `y`, from `x0`, `len` pixels long.
fn arrow(c: &mut Canvas, x0: i32, y: i32, len: i32, feathers: bool) {
    c.hline(x0, y, len, hex(WOOD[2]));
    c.hline(x0, y + 1, len, hex(WOOD[1]));
    let tip = x0 + len;
    for k in 0..4 {
        c.vline(tip + k, y - 2 + k / 2, 6 - k, hex(if k == 0 { 0x8a96a0 } else { 0xcfd8dc }));
    }
    if feathers {
        for k in 0..4 {
            c.px(x0 + k, y - 1 - (k % 2), hex(0xe53935));
            c.px(x0 + k, y + 2 + (k % 2), hex(0xb71c1c));
        }
    }
}

pub fn items(out: &Path) {
    let mut c = Canvas::new(TS * 22, TS);
    let n = TS as i32;
    let steel = [0x5f6b73, 0x90a4ae, 0xc5d0d6, 0xf0f4f6];
    let string = hex(0xe8e0c8);
    for i in 0..22u32 {
        c.at(i * TS, 0);
        match i {
            it::KEY | it::BRONZE_KEY => {
                let pal = if i == it::KEY { GOLD } else { ramp(0xb5652b) };
                c.shade_disc(10.0, 12.0, 7.0, pal);
                c.disc(10.0, 12.0, 3.0, CLEAR);
                c.shade_box(16, 10, 13, 4, pal);
                c.shade_box(22, 14, 3, 5, pal);
                c.shade_box(26, 14, 3, 7, pal);
            }
            it::POTION => {
                c.shade_box(13, 3, 6, 4, WOOD); // cork
                c.rect(14, 7, 4, 5, hex(0xcfe8ff));
                c.shade_disc(16.0, 20.0, 10.0, [0x7fa8c8, 0xa8cbe8, 0xcfe8ff, 0xffffff]);
                c.shade_disc(16.0, 21.0, 8.0, [0x7a1010, 0xb71c1c, 0xe53935, 0xff8a80]);
                c.hline(9, 17, 14, hex(0xff8a80));
                c.rect(11, 14, 2, 4, hex(0xffffff));
            }
            it::COIN => {
                c.shade_disc(16.0, 16.0, 11.0, GOLD);
                c.disc(16.0, 16.0, 7.5, hex(GOLD[1]));
                c.disc(15.5, 15.5, 6.5, hex(GOLD[2]));
                c.rect(14, 10, 3, 12, hex(GOLD[1]));
                c.px(10, 9, hex(0xffffff));
            }
            it::APPLE => {
                c.shade_disc(16.0, 19.0, 10.0, [0x7a1010, 0xb71c1c, 0xd32f2f, 0xff6e60]);
                c.rect(11, 13, 2, 3, hex(0xffcdd2));
                c.shade_box(15, 4, 3, 7, WOOD);
                c.disc(21.0, 7.0, 3.5, hex(LEAF[2]));
                c.px(20, 6, hex(LEAF[3]));
            }
            it::LETTER => {
                c.shade_box(4, 8, 24, 17, [0x9e9580, 0xd8d0bc, 0xf5f0e1, 0xffffff]);
                for k in 0..10 {
                    c.px(5 + k, 9 + k * 6 / 10, hex(0x9e9580));
                    c.px(26 - k, 9 + k * 6 / 10, hex(0x9e9580));
                }
                c.shade_disc(16.0, 16.0, 3.0, [0x6a0e0e, 0x9a1a1a, 0xc62828, 0xef5350]);
            }
            it::BOW => {
                // Curved limb from the top-right to the bottom-left, string straight.
                let pts: Vec<(f32, f32)> = (0..=20).map(|k| {
                    let t = k as f32 / 20.0;
                    let (x, y) = (27.0 - 22.0 * t, 4.0 + 23.0 * t);
                    let bend = (t * std::f32::consts::PI).sin() * 7.0;
                    (x - bend * 0.72, y - bend * 0.69)
                }).collect();
                c.line(27, 4, 5, 27, string);
                for w in pts.windows(2) {
                    c.stroke(w[0].0, w[0].1, w[1].0, w[1].1, 1.3, hex(WOOD[2]));
                }
                c.shade_box(11, 10, 5, 5, [0x2a1a0a, 0x4a2c14, 0x6b4423, 0x8a5a2e]); // grip
            }
            it::CROSSBOW => {
                c.shade_box(3, 14, 22, 5, WOOD); // stock
                c.shade_box(3, 18, 6, 6, WOOD);
                c.shade_box(21, 5, 4, 23, steel); // bow
                c.line(21, 5, 15, 16, string);
                c.line(21, 27, 15, 17, string);
                c.rect(25, 15, 4, 3, hex(steel[2]));
            }
            it::ARROWS | it::BOLTS => {
                for (k, y) in [(0, 8), (1, 15), (2, 22)] {
                    if i == it::ARROWS {
                        arrow(&mut c, 3 + k * 2, y, 20, true);
                    } else {
                        c.shade_box(6 + k * 2, y, 15, 3, WOOD);
                        c.shade_box(21 + k * 2, y - 1, 5, 5, steel);
                        c.rect(6 + k * 2, y - 1, 2, 5, hex(WOOD[0]));
                    }
                }
            }
            it::ARROW_SHOT => arrow(&mut c, 4, 15, 20, true),
            it::BOLT_SHOT => {
                c.shade_box(7, 14, 15, 4, WOOD);
                c.shade_box(22, 12, 6, 8, steel);
                c.rect(7, 12, 2, 8, hex(WOOD[0]));
            }
            it::SWORD => {
                for k in 0..17 {
                    c.rect(8 + k, 21 - k, 3, 3, hex(steel[1]));
                    c.px(9 + k, 21 - k, hex(steel[3]));
                }
                c.rect(25, 3, 3, 3, hex(steel[2]));
                c.stroke(4.0, 18.0, 13.0, 27.0, 1.5, hex(GOLD[1])); // guard
                c.stroke(4.0, 27.0, 7.0, 24.0, 1.8, hex(WOOD[1])); // grip
                c.disc(3.5, 28.5, 2.0, hex(GOLD[2]));
            }
            it::SPEAR => {
                c.stroke(4.0, 28.0, 22.0, 10.0, 1.2, hex(WOOD[2]));
                for k in 0..7 {
                    c.rect(22 + k, 4 + (6 - k), 3, 3, hex(steel[if k < 3 { 1 } else { 2 }]));
                }
                c.px(29, 3, hex(steel[3]));
                c.rect(20, 10, 3, 3, hex(0xb71c1c)); // tassel
            }
            it::AXE => {
                c.stroke(7.0, 28.0, 19.0, 6.0, 1.4, hex(WOOD[2]));
                // Crescent blade on the right of the haft, a small spike on the left.
                for y in 1..22 {
                    for x in 18..31 {
                        let (dx, dy) = ((x as f32 + 0.5 - 17.0) / 13.0, (y as f32 + 0.5 - 11.0) / 10.0);
                        let inner = ((x as f32 + 0.5 - 14.0) / 8.0).powi(2) + dy * dy;
                        if dx * dx + dy * dy <= 1.0 && (inner > 1.0 || x > 26) && (y as f32 - 11.0).abs() < 9.5 {
                            let edge = dx * dx + dy * dy > 0.72;
                            c.px(x, y, hex(if edge { steel[3] } else if dy < 0.0 { steel[2] } else { steel[1] }));
                        }
                    }
                }
                c.shade_box(16, 7, 5, 9, IRON); // socket
                for k in 0..4 {
                    c.vline(15 - k, 10 + k / 2, 3 - k / 2 * 2, hex(steel[1]));
                }
            }
            it::CAP | it::HELMET => {
                let pal = if i == it::CAP { ramp(0x8d5a2b) } else { ramp(0x9ea7b0) };
                c.shade_ellipse(16.0, 17.0, 11.0, 10.0, pal);
                c.rect(4, 17, 24, 10, CLEAR);
                c.shade_box(3, 17, 26, 4, pal);
                if i == it::HELMET {
                    c.shade_box(5, 20, 5, 8, pal);
                    c.shade_box(22, 20, 5, 8, pal);
                    c.shade_box(15, 7, 3, 18, pal);
                }
            }
            it::LEATHER | it::CHAIN => {
                let pal = if i == it::LEATHER { ramp(0x8d5a2b) } else { ramp(0xa7b1b9) };
                c.shade_box(8, 6, 16, 22, pal);
                c.shade_box(3, 6, 7, 9, pal);
                c.shade_box(22, 6, 7, 9, pal);
                c.rrect(12, 4, 8, 6, 3, CLEAR); // neck
                c.rect(8, 21, 16, 3, hex(pal[0]));
                c.rect(15, 21, 3, 3, hex(GOLD[2]));
                if i == it::CHAIN {
                    for y in (9..20).step_by(2) {
                        for x in (9 + (y % 4) / 2..23).step_by(2) {
                            c.px(x, y, hex(pal[1]));
                        }
                    }
                } else {
                    c.vline(16, 10, 10, hex(pal[1]));
                }
            }
            it::LANTERN => {
                // Iron frame with a glowing glass body and a ring handle.
                c.stroke(12.0, 7.0, 16.0, 3.0, 1.2, hex(IRON[2]));
                c.stroke(16.0, 3.0, 20.0, 7.0, 1.2, hex(IRON[2]));
                c.shade_box(10, 7, 12, 3, IRON);
                c.shade_box(11, 10, 10, 13, [0xb07a20, 0xf0b030, 0xffd860, 0xfff4c0]);
                c.rect(15, 13, 2, 6, hex(0xffffff));
                c.vline(11, 10, 13, hex(IRON[1]));
                c.vline(20, 10, 13, hex(IRON[1]));
                c.vline(15, 10, 3, hex(IRON[1]));
                c.shade_box(9, 23, 14, 4, IRON);
            }
            it::TORCH => {
                // Wooden handle with a wrapped head and a flame.
                c.stroke(9.0, 29.0, 18.0, 13.0, 1.6, hex(WOOD[2]));
                c.stroke(9.5, 29.0, 18.5, 13.0, 0.8, hex(WOOD[1]));
                c.shade_box(16, 10, 6, 5, [0x3a2a1a, 0x5a4028, 0x7a5a38, 0x9a7a50]);
                c.shade_disc(20.0, 8.0, 5.5, [0xc62828, 0xef6c00, 0xffa726, 0xffe082]);
                c.disc(20.5, 9.0, 2.5, hex(0xfff8e1));
                c.px(21, 2, hex(0xffa726));
                c.px(18, 3, hex(0xef6c00));
            }
            it::SHIELD => {
                // Round wooden shield: planks, iron rim and boss.
                c.shade_disc(16.0, 16.0, 13.0, IRON);
                c.shade_disc(16.0, 16.0, 11.0, WOOD);
                for x in [11, 16, 21] {
                    c.vline(x, 6, 21, hex(WOOD[0]));
                }
                c.shade_disc(16.0, 16.0, 3.5, [0x5f6b73, 0x90a4ae, 0xc5d0d6, 0xf0f4f6]);
            }
            _ => {}
        }
        c.auto_outline(n, n, OUTLINE);
    }
    c.save(&out.join("sprites/items.png"));
}
