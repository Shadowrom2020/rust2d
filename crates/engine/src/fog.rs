//! Fog of war: which tiles of a map the player has explored (saved in the game per map). Only
//! maps with `Map::fog` use it; the map screen covers everything else.

use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// Tiles around the player that count as explored (radius in tiles).
pub const REVEAL_RADIUS: f32 = 6.5;

/// One bit per tile. Saved as a short string (`"WxH:" + base64 of the lz4-compressed bits`), so
/// even huge maps take little room in the RON save game.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Explored {
    width: u32,
    height: u32,
    bits: Vec<u8>,
}

impl Explored {
    pub fn new(width: u32, height: u32) -> Self {
        Explored { width, height, bits: vec![0; (width as usize * height as usize).div_ceil(8)] }
    }

    pub fn size(&self) -> (u32, u32) {
        (self.width, self.height)
    }

    pub fn get(&self, x: i32, y: i32) -> bool {
        if x < 0 || y < 0 || x as u32 >= self.width || y as u32 >= self.height {
            return false;
        }
        let i = y as usize * self.width as usize + x as usize;
        self.bits[i / 8] & (1 << (i % 8)) != 0
    }

    fn set(&mut self, x: u32, y: u32) {
        let i = y as usize * self.width as usize + x as usize;
        self.bits[i / 8] |= 1 << (i % 8);
    }

    /// Number of explored tiles.
    pub fn count(&self) -> usize {
        self.bits.iter().map(|b| b.count_ones() as usize).sum()
    }

    /// Marks the tiles within `radius` of tile (`cx`, `cy`) as explored.
    pub fn reveal(&mut self, cx: f32, cy: f32, radius: f32) {
        let r = radius.ceil() as i32;
        let (tx, ty) = (cx.floor() as i32, cy.floor() as i32);
        for y in (ty - r).max(0)..=(ty + r).min(self.height as i32 - 1) {
            for x in (tx - r).max(0)..=(tx + r).min(self.width as i32 - 1) {
                let (dx, dy) = (x as f32 + 0.5 - cx, y as f32 + 0.5 - cy);
                if dx * dx + dy * dy <= radius * radius {
                    self.set(x as u32, y as u32);
                }
            }
        }
    }

    /// Reveals the whole map.
    pub fn reveal_all(&mut self) {
        for y in 0..self.height {
            for x in 0..self.width {
                self.set(x, y);
            }
        }
    }

    /// The same explored tiles on a map of another size (the map was resized in the editor).
    pub fn resized(&self, width: u32, height: u32) -> Explored {
        let mut out = Explored::new(width, height);
        for y in 0..height.min(self.height) {
            for x in 0..width.min(self.width) {
                if self.get(x as i32, y as i32) {
                    out.set(x, y);
                }
            }
        }
        out
    }

    fn encode(&self) -> String {
        format!("{}x{}:{}", self.width, self.height, base64_encode(&lz4_flex::compress_prepend_size(&self.bits)))
    }

    fn decode(s: &str) -> Option<Explored> {
        let (size, data) = s.split_once(':')?;
        let (w, h) = size.split_once('x')?;
        let (width, height) = (w.parse().ok()?, h.parse().ok()?);
        let bits = lz4_flex::decompress_size_prepended(&base64_decode(data)?).ok()?;
        let e = Explored { width, height, bits };
        (e.bits.len() == Explored::new(width, height).bits.len()).then_some(e)
    }
}

impl Serialize for Explored {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.encode())
    }
}

impl<'de> Deserialize<'de> for Explored {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let s = String::deserialize(d)?;
        Explored::decode(&s).ok_or_else(|| serde::de::Error::custom("invalid explored-tiles data"))
    }
}

const B64: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

fn base64_encode(data: &[u8]) -> String {
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for chunk in data.chunks(3) {
        let n = (chunk[0] as u32) << 16 | (*chunk.get(1).unwrap_or(&0) as u32) << 8 | *chunk.get(2).unwrap_or(&0) as u32;
        for k in 0..4 {
            if k <= chunk.len() {
                out.push(B64[(n >> (18 - 6 * k) & 63) as usize] as char);
            } else {
                out.push('=');
            }
        }
    }
    out
}

fn base64_decode(s: &str) -> Option<Vec<u8>> {
    let mut out = Vec::with_capacity(s.len() / 4 * 3);
    let (mut acc, mut n) = (0u32, 0);
    for c in s.bytes().filter(|c| *c != b'=') {
        acc = acc << 6 | B64.iter().position(|b| *b == c)? as u32;
        n += 6;
        if n >= 8 {
            n -= 8;
            out.push((acc >> n) as u8);
        }
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reveal_and_round_trip() {
        let mut e = Explored::new(40, 30);
        assert_eq!(e.count(), 0);
        e.reveal(10.5, 10.5, REVEAL_RADIUS);
        assert!(e.get(10, 10) && e.get(16, 10) && !e.get(18, 10) && !e.get(16, 16));
        let n = e.count();
        assert!(n > 100 && n < 160, "{n}");
        let s = ron::to_string(&e).unwrap();
        let back: Explored = ron::from_str(&s).unwrap();
        assert_eq!(back, e);
        // Resizing keeps what fits.
        let r = e.resized(12, 12);
        assert!(r.get(10, 10) && !r.get(12, 10));
        // Base64 of every length.
        for len in 0..10 {
            let v: Vec<u8> = (0..len).map(|i| (i * 37 + 5) as u8).collect();
            assert_eq!(base64_decode(&base64_encode(&v)).unwrap(), v);
        }
        // Huge maps stay small.
        let mut big = Explored::new(1024, 1024);
        big.reveal(512.0, 512.0, REVEAL_RADIUS);
        assert!(big.encode().len() < 2000);
    }
}
