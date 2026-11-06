//! Screen feedback ("juice"): a short camera shake and a red flash when the player is hurt,
//! shakes from explosions (`effect(..., #{ shake: 0.3 })`) and from scripts / the *Shake the
//! screen* action. The player can switch both off (Options › Shake & flash).

use macroquad::prelude::*;

use super::Game;

/// The shake and flash of a hit on the player.
const HURT_SHAKE: f32 = 0.25;
const HURT_FLASH: f32 = 0.15;
/// Shake strength 1 moves the view up to this many art pixels.
const SHAKE_PIXELS: f32 = 3.0;

#[derive(Default)]
pub(super) struct Juice {
    /// Seconds of shaking left, and how long the current shake lasts in total.
    shake: f32,
    shake_total: f32,
    /// Strength of the current shake (1 = a hit).
    strength: f32,
    /// Seconds of the red hurt flash left.
    flash: f32,
}

impl Game {
    /// Shakes the view for `seconds` (`strength` 1 = like a hit; a stronger shake replaces a
    /// weaker one, a longer one extends it).
    pub(super) fn shake_screen(&mut self, seconds: f32, strength: f32) {
        if seconds <= 0.0 || strength <= 0.0 {
            return;
        }
        let j = &mut self.juice;
        if j.shake <= 0.0 || strength >= j.strength {
            j.strength = strength;
        }
        j.shake = j.shake.max(seconds);
        j.shake_total = j.shake;
    }

    /// The player got hurt: a shake and a red flash.
    pub(super) fn hurt_feedback(&mut self) {
        self.shake_screen(HURT_SHAKE, 1.0);
        self.juice.flash = HURT_FLASH;
    }

    pub(super) fn update_juice(&mut self, dt: f32) {
        let j = &mut self.juice;
        j.shake = (j.shake - dt).max(0.0);
        j.flash = (j.flash - dt).max(0.0);
    }

    /// The camera offset of the shake this frame (world pixels; zero when switched off).
    pub(super) fn shake_offset(&self) -> Vec2 {
        let j = &self.juice;
        if j.shake <= 0.0 || !self.settings.screen_effects || j.shake_total <= 0.0 {
            return Vec2::ZERO;
        }
        // Fades out over the shake; a new random direction every frame.
        let amount = SHAKE_PIXELS * self.u() * j.strength * (j.shake / j.shake_total);
        vec2(rand::gen_range(-1.0, 1.0), rand::gen_range(-1.0, 1.0)) * amount
    }

    /// The red hurt flash over the whole screen (screen space).
    pub(super) fn draw_hurt_flash(&self) {
        if self.juice.flash > 0.0 && self.settings.screen_effects {
            let a = 0.32 * self.juice.flash / HURT_FLASH;
            draw_rectangle(0.0, 0.0, screen_width(), screen_height(), Color::new(0.85, 0.05, 0.05, a));
        }
    }

    /// For `status()`: seconds of shake left (`off` when switched off).
    pub(super) fn juice_status(&self) -> String {
        if self.settings.screen_effects {
            format!("{:.2}", self.juice.shake)
        } else {
            "off".into()
        }
    }
}
