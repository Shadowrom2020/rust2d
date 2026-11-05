//! Input: keyboard, mouse and gamepads, with optional injected keys, buttons and mouse state
//! (used for automated play-throughs). Gameplay and menus ask for named actions ([`Act`]:
//! `act_pressed(Act::Confirm)`), which the keyboard and a gamepad both drive; text fields read
//! raw keys and typed characters. Gamepads come through gilrs, polled once per frame
//! ([`poll_pad`]); the left stick and the d-pad both count as the [`Pad`] directions.

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
    /// Mouse wheel (vertical) this frame.
    pub wheel: f32,
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

/// Vertical mouse wheel movement this frame (positive = up).
pub fn wheel() -> f32 {
    INJECTED.with(|i| i.borrow().mouse.as_ref().map(|m| m.wheel)).unwrap_or_else(|| macroquad::input::mouse_wheel().1)
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
        "h" => KeyCode::H,
        "k" => KeyCode::K,
        "l" => KeyCode::L,
        "m" => KeyCode::M,
        "f" => KeyCode::F,
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
        "pageup" | "pgup" => KeyCode::PageUp,
        "pagedown" | "pgdn" => KeyCode::PageDown,
        "home" => KeyCode::Home,
        "end" => KeyCode::End,
        _ => return None,
    })
}

// ------------------------------------------------------------------ gamepads

/// Gamepad buttons (Xbox names; the left stick counts as the d-pad).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Pad {
    /// A (Xbox) / Cross (PlayStation).
    South,
    /// B / Circle.
    East,
    /// X / Square.
    West,
    /// Y / Triangle.
    North,
    Lb,
    Rb,
    Lt,
    Rt,
    Select,
    Start,
    /// Stick clicks.
    LStick,
    RStick,
    Up,
    Down,
    Left,
    Right,
}

impl Pad {
    pub const ALL: [Pad; 16] = [
        Pad::South,
        Pad::East,
        Pad::West,
        Pad::North,
        Pad::Lb,
        Pad::Rb,
        Pad::Lt,
        Pad::Rt,
        Pad::Select,
        Pad::Start,
        Pad::LStick,
        Pad::RStick,
        Pad::Up,
        Pad::Down,
        Pad::Left,
        Pad::Right,
    ];

    /// The name used by autoplay scripts (`pad a`) and shown to players.
    pub fn name(self) -> &'static str {
        match self {
            Pad::South => "a",
            Pad::East => "b",
            Pad::West => "x",
            Pad::North => "y",
            Pad::Lb => "lb",
            Pad::Rb => "rb",
            Pad::Lt => "lt",
            Pad::Rt => "rt",
            Pad::Select => "select",
            Pad::Start => "start",
            Pad::LStick => "ls",
            Pad::RStick => "rs",
            Pad::Up => "up",
            Pad::Down => "down",
            Pad::Left => "left",
            Pad::Right => "right",
        }
    }

    pub fn from_name(name: &str) -> Option<Pad> {
        let n = name.trim().to_ascii_lowercase();
        Pad::ALL.into_iter().find(|p| p.name() == n)
    }

    fn bit(self) -> u32 {
        1 << Pad::ALL.iter().position(|p| *p == self).unwrap()
    }
}

/// Stick deflection that counts as a direction (and the lower one that keeps it held).
const STICK_ON: f32 = 0.5;
const STICK_OFF: f32 = 0.35;

#[derive(Default)]
struct PadState {
    gilrs: Option<gilrs::Gilrs>,
    started: bool,
    /// Buttons held now / last frame (bits of [`Pad::bit`]).
    down: u32,
    prev: u32,
    /// The connected gamepad's name ("" = none).
    name: String,
    inj_down: Vec<Pad>,
    inj_pressed: Vec<Pad>,
}

thread_local! {
    static PAD: RefCell<PadState> = RefCell::new(PadState::default());
}

/// Reads the gamepads; call once per frame before the game looks at input. Without gamepad
/// support (no udev, no permission) it quietly does nothing.
pub fn poll_pad() {
    use gilrs::{Axis, Button};
    PAD.with(|p| {
        let mut p = p.borrow_mut();
        if !p.started {
            p.started = true;
            p.gilrs = match gilrs::Gilrs::new() {
                Ok(g) => Some(g),
                Err(e) => {
                    eprintln!("[input] gamepads unavailable: {e}");
                    None
                }
            };
        }
        let prev = p.down;
        let mut down = 0;
        let mut name = String::new();
        if let Some(g) = &mut p.gilrs {
            while g.next_event().is_some() {}
            for (_, gp) in g.gamepads().filter(|(_, gp)| gp.is_connected()) {
                if name.is_empty() {
                    name = gp.name().to_string();
                }
                let buttons = [
                    (Button::South, Pad::South),
                    (Button::East, Pad::East),
                    (Button::West, Pad::West),
                    (Button::North, Pad::North),
                    (Button::LeftTrigger, Pad::Lb),
                    (Button::RightTrigger, Pad::Rb),
                    (Button::LeftTrigger2, Pad::Lt),
                    (Button::RightTrigger2, Pad::Rt),
                    (Button::Select, Pad::Select),
                    (Button::Start, Pad::Start),
                    (Button::LeftThumb, Pad::LStick),
                    (Button::RightThumb, Pad::RStick),
                    (Button::DPadUp, Pad::Up),
                    (Button::DPadDown, Pad::Down),
                    (Button::DPadLeft, Pad::Left),
                    (Button::DPadRight, Pad::Right),
                ];
                for (b, pad) in buttons {
                    if gp.is_pressed(b) {
                        down |= pad.bit();
                    }
                }
                // The left stick as a d-pad, with a little hysteresis.
                let (x, y) = (gp.value(Axis::LeftStickX), gp.value(Axis::LeftStickY));
                for (v, pad) in [(-y, Pad::Down), (y, Pad::Up), (-x, Pad::Left), (x, Pad::Right)] {
                    let held = prev & pad.bit() != 0;
                    if v > STICK_ON || (held && v > STICK_OFF) {
                        down |= pad.bit();
                    }
                }
            }
        }
        p.prev = prev;
        p.down = down;
        p.name = name;
    });
}

/// Sets gamepad buttons held and pressed (this frame) in addition to a real gamepad.
pub fn inject_pad(down: &[Pad], pressed: &[Pad]) {
    PAD.with(|p| {
        let mut p = p.borrow_mut();
        p.inj_down = down.to_vec();
        p.inj_pressed = pressed.to_vec();
    });
}

pub fn pad_down(b: Pad) -> bool {
    PAD.with(|p| {
        let p = p.borrow();
        p.down & b.bit() != 0 || p.inj_down.contains(&b)
    })
}

pub fn pad_pressed(b: Pad) -> bool {
    PAD.with(|p| {
        let p = p.borrow();
        (p.down & !p.prev) & b.bit() != 0 || p.inj_pressed.contains(&b)
    })
}

/// The connected gamepad's name, or "none".
pub fn pad_name() -> String {
    PAD.with(|p| {
        let n = p.borrow().name.clone();
        if n.is_empty() {
            "none".into()
        } else {
            n
        }
    })
}

// ------------------------------------------------------------------ actions

/// What the player wants to do; each action has keyboard keys and gamepad buttons.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Act {
    /// Movement and menu navigation.
    Up,
    Down,
    Left,
    Right,
    /// Hold to run (and to buy / sell ten, to scroll the map faster).
    Run,
    /// Talk, interact, advance dialogs, choose in menus, use the selected item.
    Confirm,
    /// Close a window, go back in a menu.
    Cancel,
    /// Open the pause menu.
    Pause,
    Attack,
    SwitchWeapon,
    /// The character window (character sheet and inventory).
    Character,
    Skills,
    Spells,
    /// The next / previous tab of a window.
    NextTab,
    PrevTab,
    Map,
    Journal,
    HideUi,
    /// Inventory: drop the selected stack; use the selected item; loot: take everything.
    Drop,
    Use,
    TakeAll,
    /// Map screen.
    ZoomIn,
    ZoomOut,
    MapCenter,
    Quick1,
    Quick2,
    Quick3,
    Quick4,
    Quick5,
    Quick6,
    Quick7,
    Quick8,
    QuickSave,
    QuickLoad,
    Debug,
}

impl Act {
    pub const QUICK: [Act; 8] = [Act::Quick1, Act::Quick2, Act::Quick3, Act::Quick4, Act::Quick5, Act::Quick6, Act::Quick7, Act::Quick8];

    /// The keyboard keys that trigger it.
    pub fn keys(self) -> &'static [KeyCode] {
        use KeyCode as K;
        match self {
            Act::Up => &[K::Up, K::W],
            Act::Down => &[K::Down, K::S],
            Act::Left => &[K::Left, K::A],
            Act::Right => &[K::Right, K::D],
            Act::Run => &[K::LeftShift, K::RightShift],
            Act::Confirm => &[K::E, K::Space, K::Enter],
            Act::Cancel | Act::Pause => &[K::Escape],
            Act::Attack => &[K::X],
            Act::SwitchWeapon => &[K::Q],
            Act::Character => &[K::C, K::I],
            Act::Skills => &[K::K],
            Act::Spells => &[K::L],
            Act::NextTab => &[K::Tab],
            Act::PrevTab => &[],
            Act::Map => &[K::M],
            Act::Journal => &[K::J],
            Act::HideUi => &[K::H],
            Act::Drop => &[K::G],
            Act::Use => &[K::U],
            Act::TakeAll => &[K::T],
            Act::ZoomIn => &[K::Equal, K::KpAdd],
            Act::ZoomOut => &[K::Minus, K::KpSubtract],
            Act::MapCenter => &[K::Space, K::Home, K::C],
            Act::Quick1 => &[K::Key1],
            Act::Quick2 => &[K::Key2],
            Act::Quick3 => &[K::Key3],
            Act::Quick4 => &[K::Key4],
            Act::Quick5 => &[K::Key5],
            Act::Quick6 => &[K::Key6],
            Act::Quick7 => &[K::Key7],
            Act::Quick8 => &[K::Key8],
            Act::QuickSave => &[K::F5],
            Act::QuickLoad => &[K::F9],
            Act::Debug => &[K::F3],
        }
    }

    /// The gamepad buttons that trigger it.
    pub fn pad(self) -> &'static [Pad] {
        match self {
            Act::Up => &[Pad::Up],
            Act::Down => &[Pad::Down],
            Act::Left => &[Pad::Left],
            Act::Right => &[Pad::Right],
            Act::Run => &[Pad::Lt],
            Act::Confirm => &[Pad::South],
            Act::Cancel => &[Pad::East, Pad::Start],
            Act::Pause => &[Pad::Start],
            Act::Attack => &[Pad::West],
            Act::SwitchWeapon => &[Pad::Rt],
            Act::Character => &[Pad::North],
            Act::NextTab => &[Pad::Rb],
            Act::PrevTab => &[Pad::Lb],
            Act::Map => &[Pad::Select],
            Act::Journal => &[Pad::RStick],
            Act::HideUi => &[Pad::LStick],
            Act::TakeAll => &[Pad::West],
            Act::ZoomIn => &[Pad::Rb],
            Act::ZoomOut => &[Pad::Lb],
            Act::MapCenter => &[Pad::South],
            _ => &[],
        }
    }
}

/// The action is held (keyboard or gamepad).
pub fn act_down(a: Act) -> bool {
    a.keys().iter().any(|k| down(*k)) || a.pad().iter().any(|b| pad_down(*b))
}

/// The action was triggered this frame.
pub fn act_pressed(a: Act) -> bool {
    a.keys().iter().any(|k| pressed(*k)) || a.pad().iter().any(|b| pad_pressed(*b))
}

pub fn any_act_pressed(acts: &[Act]) -> bool {
    acts.iter().any(|a| act_pressed(*a))
}

/// Menu navigation in text-entry screens: arrow keys or the gamepad (not WASD, which types).
pub fn nav_pressed(a: Act) -> bool {
    let key = match a {
        Act::Up => KeyCode::Up,
        Act::Down => KeyCode::Down,
        Act::Left => KeyCode::Left,
        Act::Right => KeyCode::Right,
        _ => return act_pressed(a),
    };
    pressed(key) || a.pad().iter().any(|b| pad_pressed(*b))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pad_names_round_trip() {
        for p in Pad::ALL {
            assert_eq!(Pad::from_name(p.name()), Some(p));
        }
        assert_eq!(Pad::from_name(" A "), Some(Pad::South));
        assert_eq!(Pad::from_name("z"), None);
        // Every button has its own bit.
        let bits: u32 = Pad::ALL.iter().map(|p| p.bit()).fold(0, |a, b| {
            assert_eq!(a & b, 0);
            a | b
        });
        assert_eq!(bits.count_ones() as usize, Pad::ALL.len());
    }

    #[test]
    fn actions_keep_the_old_keys() {
        assert_eq!(Act::Confirm.keys(), &[KeyCode::E, KeyCode::Space, KeyCode::Enter]);
        assert_eq!(Act::Attack.keys(), &[KeyCode::X]);
        assert_eq!(Act::Character.keys(), &[KeyCode::C, KeyCode::I]);
        assert!(Act::QUICK.iter().enumerate().all(|(k, a)| a.keys().len() == 1 && key_from_name(&(k + 1).to_string()) == Some(a.keys()[0])));
        // The gamepad reaches everything a controller player needs.
        for a in [Act::Up, Act::Down, Act::Left, Act::Right, Act::Confirm, Act::Cancel, Act::Pause, Act::Attack, Act::Character, Act::NextTab, Act::PrevTab, Act::Map, Act::Journal] {
            assert!(!a.pad().is_empty(), "{a:?} has no gamepad button");
        }
    }
}
