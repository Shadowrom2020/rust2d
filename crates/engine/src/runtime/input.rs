//! Keyboard input with optional injected keys (used for automated play-throughs).

use std::cell::RefCell;

use macroquad::prelude::{
    is_key_down, is_key_pressed, is_mouse_button_down, is_mouse_button_pressed, is_mouse_button_released, mouse_position, KeyCode,
    MouseButton, Vec2,
};

/// Injected mouse state for one frame.
#[derive(Clone, Debug, Default)]
pub struct MouseInject {
    pub pos: Vec2,
    pub pressed: Vec<MouseButton>,
    pub down: Vec<MouseButton>,
    pub released: Vec<MouseButton>,
}

#[derive(Default)]
struct Injected {
    down: Vec<KeyCode>,
    pressed: Vec<KeyCode>,
    mouse: Option<MouseInject>,
    /// Typed characters waiting to be read.
    chars: std::collections::VecDeque<char>,
}

thread_local! {
    static INJECTED: RefCell<Injected> = RefCell::new(Injected::default());
}

/// Sets the keys held and pressed (this frame) in addition to the real keyboard.
pub fn inject(down: &[KeyCode], pressed: &[KeyCode]) {
    INJECTED.with(|i| {
        let mut i = i.borrow_mut();
        i.down = down.to_vec();
        i.pressed = pressed.to_vec();
    });
}

/// Injects the mouse state for this frame; `None` returns to the real mouse.
pub fn inject_mouse(m: Option<MouseInject>) {
    INJECTED.with(|i| i.borrow_mut().mouse = m);
}

/// Mouse position in screen pixels.
pub fn mouse_pos() -> Vec2 {
    INJECTED.with(|i| i.borrow().mouse.as_ref().map(|m| m.pos)).unwrap_or_else(|| Vec2::from(mouse_position()))
}

fn injected(b: MouseButton, f: fn(&MouseInject) -> &Vec<MouseButton>) -> Option<bool> {
    INJECTED.with(|i| i.borrow().mouse.as_ref().map(|m| f(m).contains(&b)))
}

pub fn mouse_pressed(b: MouseButton) -> bool {
    injected(b, |m| &m.pressed).unwrap_or_else(|| is_mouse_button_pressed(b))
}

pub fn mouse_down(b: MouseButton) -> bool {
    injected(b, |m| &m.down).unwrap_or_else(|| is_mouse_button_down(b))
}

pub fn mouse_released(b: MouseButton) -> bool {
    injected(b, |m| &m.released).unwrap_or_else(|| is_mouse_button_released(b))
}

/// Queues typed text (for automated play-throughs).
pub fn inject_text(text: &str) {
    INJECTED.with(|i| i.borrow_mut().chars.extend(text.chars()));
}

/// Next typed character (injected text first, then the keyboard).
pub fn next_char() -> Option<char> {
    INJECTED.with(|i| i.borrow_mut().chars.pop_front()).or_else(macroquad::input::get_char_pressed)
}

pub fn down(k: KeyCode) -> bool {
    is_key_down(k) || INJECTED.with(|i| i.borrow().down.contains(&k))
}

pub fn pressed(k: KeyCode) -> bool {
    is_key_pressed(k) || INJECTED.with(|i| i.borrow().pressed.contains(&k))
}

pub fn any_pressed(keys: &[KeyCode]) -> bool {
    keys.iter().any(|k| pressed(*k))
}

/// Parses key names used by autoplay scripts.
pub fn key_from_name(name: &str) -> Option<KeyCode> {
    Some(match name.to_ascii_lowercase().as_str() {
        "left" => KeyCode::Left,
        "right" => KeyCode::Right,
        "up" => KeyCode::Up,
        "down" => KeyCode::Down,
        "e" => KeyCode::E,
        "i" => KeyCode::I,
        "u" => KeyCode::U,
        "j" => KeyCode::J,
        "x" => KeyCode::X,
        "q" => KeyCode::Q,
        "t" => KeyCode::T,
        "g" => KeyCode::G,
        "c" => KeyCode::C,
        "1" => KeyCode::Key1,
        "2" => KeyCode::Key2,
        "3" => KeyCode::Key3,
        "4" => KeyCode::Key4,
        "5" => KeyCode::Key5,
        "6" => KeyCode::Key6,
        "7" => KeyCode::Key7,
        "8" => KeyCode::Key8,
        "tab" => KeyCode::Tab,
        "space" => KeyCode::Space,
        "enter" => KeyCode::Enter,
        "escape" | "esc" => KeyCode::Escape,
        "shift" => KeyCode::LeftShift,
        "f3" => KeyCode::F3,
        "f5" => KeyCode::F5,
        "f9" => KeyCode::F9,
        "delete" | "del" => KeyCode::Delete,
        "backspace" => KeyCode::Backspace,
        _ => return None,
    })
}
