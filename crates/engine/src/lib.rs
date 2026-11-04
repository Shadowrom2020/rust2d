//! rust2d engine: tile maps, entities, events, scripting, rendering and the game runtime.

pub mod audio;
pub mod defs;
pub mod entity;
pub mod events;
pub mod imageio;
pub mod inventory;
pub mod library;
pub mod map;
pub mod migrate;
pub mod path;
pub mod project;
pub mod quest;
pub mod refactor;
pub mod release;
pub mod render;
pub mod runtime;
pub mod script;
pub mod script_refs;
pub mod state;

pub use macroquad;

/// Fingerprint of the engine sources this binary was built from. The editor compares it
/// with `rust2d-game --engine-build` before a play-test to catch a stale game binary.
pub const ENGINE_BUILD: &str = env!("RUST2D_ENGINE_BUILD");
