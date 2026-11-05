//! Skin and hair colors of layered character models. Body sheets draw skin in a magenta
//! reference ramp, hair and beard sheets draw hair in a cyan one; [`recolor`] replaces them
//! with the shades of the chosen colors (the same shading the generated art uses).

/// Reference shades (dark, mid-dark, base, light) of skin pixels in body sheets.
pub const SKIN_REF: [[u8; 3]; 4] = [[0x6e, 0x00, 0x6e], [0x9c, 0x00, 0x9c], [0xc8, 0x00, 0xc8], [0xf0, 0x00, 0xf0]];
/// Reference shades of hair pixels in hair / beard sheets.
pub const HAIR_REF: [[u8; 3]; 4] = [[0x00, 0x6e, 0x6e], [0x00, 0x9c, 0x9c], [0x00, 0xc8, 0xc8], [0x00, 0xf0, 0xf0]];

/// Dark, mid-dark, base and light shade of a color.
pub fn ramp(c: [u8; 3]) -> [[u8; 3]; 4] {
    let mul = |f: f32| c.map(|v| (v as f32 * f).round().clamp(0.0, 255.0) as u8);
    [mul(0.55), mul(0.78), c, mul(1.22)]
}

/// Brightness 0..1 along the reference ramp -> color along the target ramp (linear between
/// the four shades, so blended pixels keep their in-between shade).
fn along(level: f32, refs: &[[u8; 3]; 4], key: usize, target: &[[u8; 3]; 4]) -> [u8; 3] {
    let keys = refs.map(|r| r[key] as f32);
    if level <= keys[0] {
        let t = (level / keys[0]).clamp(0.0, 1.0);
        return target[0].map(|v| (v as f32 * t).round() as u8);
    }
    for k in 0..3 {
        if level <= keys[k + 1] {
            let t = (level - keys[k]) / (keys[k + 1] - keys[k]);
            return [0, 1, 2].map(|i| (target[k][i] as f32 + (target[k + 1][i] as f32 - target[k][i] as f32) * t).round() as u8);
        }
    }
    target[3]
}

/// Recolors RGBA pixels in place: magenta-ish pixels to the `skin` ramp, cyan-ish ones to the
/// `hair` ramp (`None` leaves them).
pub fn recolor(rgba: &mut [u8], skin: Option<[u8; 3]>, hair: Option<[u8; 3]>) {
    let (skin, hair) = (skin.map(ramp), hair.map(ramp));
    for px in rgba.as_chunks_mut::<4>().0 {
        if px[3] == 0 {
            continue;
        }
        let (r, g, b) = (px[0] as i32, px[1] as i32, px[2] as i32);
        if let Some(t) = &skin {
            if r > 24 && (r - b).abs() <= 10 && g * 4 < r {
                px[..3].copy_from_slice(&along(r as f32, &SKIN_REF, 0, t));
                continue;
            }
        }
        if let Some(t) = &hair {
            if g > 24 && (g - b).abs() <= 10 && r * 4 < g {
                px[..3].copy_from_slice(&along(g as f32, &HAIR_REF, 1, t));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reference_shades_map_to_the_ramp() {
        let skin = [232, 184, 138];
        let hair = [92, 60, 34];
        let mut px: Vec<u8> = SKIN_REF.iter().chain(HAIR_REF.iter()).flat_map(|c| [c[0], c[1], c[2], 255]).collect();
        px.extend([10, 20, 30, 255, 0x6e, 0x00, 0x6e, 0]); // other colors and transparent pixels stay
        recolor(&mut px, Some(skin), Some(hair));
        let got: Vec<[u8; 3]> = px.chunks(4).map(|p| [p[0], p[1], p[2]]).collect();
        assert_eq!(&got[0..4], &ramp(skin));
        assert_eq!(&got[4..8], &ramp(hair));
        assert_eq!(got[8], [10, 20, 30]);
        assert_eq!(got[9], [0x6e, 0x00, 0x6e]);
        // A shade between two reference shades lands between their targets.
        let mut mid = vec![0xb2, 0x00, 0xb2, 255];
        recolor(&mut mid, Some(skin), None);
        let r = ramp(skin);
        assert!(mid[0] > r[1][0] && mid[0] < r[2][0], "{mid:?}");
    }
}
