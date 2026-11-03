//! Minimal RGBA images: PNG decode / encode, nearest-neighbour resize and pixel access.
//! Used by the asset library (rescaling between tile sizes) and the editor's pixel editor.

use std::io::Cursor;

use anyhow::{bail, Context, Result};

/// 8-bit RGBA pixels, row-major, top row first.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Rgba {
    pub w: u32,
    pub h: u32,
    pub px: Vec<u8>,
}

impl Rgba {
    /// A fully transparent image.
    pub fn new(w: u32, h: u32) -> Self {
        Rgba { w, h, px: vec![0; (w * h * 4) as usize] }
    }

    pub fn decode(bytes: &[u8]) -> Result<Rgba> {
        let mut dec = png::Decoder::new(Cursor::new(bytes));
        dec.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
        let mut reader = dec.read_info().context("not a PNG image")?;
        let mut buf = vec![0; reader.output_buffer_size()];
        let info = reader.next_frame(&mut buf).context("reading PNG")?;
        let data = &buf[..info.buffer_size()];
        let px: Vec<u8> = match info.color_type {
            png::ColorType::Rgba => data.to_vec(),
            png::ColorType::Rgb => data.as_chunks::<3>().0.iter().flat_map(|c| [c[0], c[1], c[2], 255]).collect(),
            png::ColorType::GrayscaleAlpha => data.as_chunks::<2>().0.iter().flat_map(|c| [c[0], c[0], c[0], c[1]]).collect(),
            png::ColorType::Grayscale => data.iter().flat_map(|&g| [g, g, g, 255]).collect(),
            png::ColorType::Indexed => bail!("unsupported indexed PNG"),
        };
        Ok(Rgba { w: info.width, h: info.height, px })
    }

    pub fn encode(&self) -> Result<Vec<u8>> {
        let mut out = Vec::new();
        {
            let mut enc = png::Encoder::new(&mut out, self.w, self.h);
            enc.set_color(png::ColorType::Rgba);
            enc.set_depth(png::BitDepth::Eight);
            let mut wr = enc.write_header()?;
            wr.write_image_data(&self.px)?;
        }
        Ok(out)
    }

    pub fn in_bounds(&self, x: i32, y: i32) -> bool {
        x >= 0 && y >= 0 && (x as u32) < self.w && (y as u32) < self.h
    }

    pub fn get(&self, x: i32, y: i32) -> [u8; 4] {
        if !self.in_bounds(x, y) {
            return [0; 4];
        }
        let i = ((y as u32 * self.w + x as u32) * 4) as usize;
        [self.px[i], self.px[i + 1], self.px[i + 2], self.px[i + 3]]
    }

    pub fn set(&mut self, x: i32, y: i32, c: [u8; 4]) {
        if self.in_bounds(x, y) {
            let i = ((y as u32 * self.w + x as u32) * 4) as usize;
            self.px[i..i + 4].copy_from_slice(&c);
        }
    }

    /// Nearest-neighbour resize.
    pub fn resized(&self, w: u32, h: u32) -> Rgba {
        let mut out = Rgba::new(w, h);
        for y in 0..h {
            for x in 0..w {
                let sx = (x as u64 * self.w as u64 / w.max(1) as u64) as i32;
                let sy = (y as u64 * self.h as u64 / h.max(1) as u64) as i32;
                out.set(x as i32, y as i32, self.get(sx, sy));
            }
        }
        out
    }

    /// Copy with a different canvas size; content stays in the top-left corner.
    pub fn with_canvas(&self, w: u32, h: u32) -> Rgba {
        let mut out = Rgba::new(w, h);
        for y in 0..h.min(self.h) as i32 {
            for x in 0..w.min(self.w) as i32 {
                out.set(x, y, self.get(x, y));
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn png_roundtrip_and_resize() {
        let mut img = Rgba::new(3, 2);
        img.set(0, 0, [255, 0, 0, 255]);
        img.set(2, 1, [0, 0, 255, 128]);
        let back = Rgba::decode(&img.encode().unwrap()).unwrap();
        assert_eq!(back, img);
        let big = img.resized(6, 4);
        assert_eq!(big.get(1, 1), [255, 0, 0, 255]);
        assert_eq!(big.get(5, 3), [0, 0, 255, 128]);
        assert_eq!(big.resized(3, 2), img);
        assert_eq!(img.with_canvas(4, 1).get(0, 0), [255, 0, 0, 255]);
    }
}
