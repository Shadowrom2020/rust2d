//! Sound effects and looping music (WAV / OGG via macroquad).

use std::collections::HashMap;
use std::future::Future;
use std::pin::pin;
use std::task::{Context, Poll, Waker};

use macroquad::audio::{load_sound_from_bytes, play_sound, stop_sound, PlaySoundParams, Sound};

use crate::project::Project;

/// Polls a future once. macroquad's native sound loading completes immediately.
fn poll_now<F: Future>(f: F) -> Option<F::Output> {
    let mut cx = Context::from_waker(Waker::noop());
    match pin!(f).poll(&mut cx) {
        Poll::Ready(v) => Some(v),
        Poll::Pending => None,
    }
}

/// Whether the bytes look like a format the mixer can decode.
fn supported(bytes: &[u8]) -> bool {
    bytes.len() > 12 && (&bytes[..4] == b"RIFF" && &bytes[8..12] == b"WAVE" || &bytes[..4] == b"OggS")
}

pub struct Audio {
    sounds: HashMap<String, (Sound, f32)>,
    music: Option<String>,
    pub errors: Vec<String>,
    pub enabled: bool,
}

impl Audio {
    /// Loads all sounds of the project. Needs a macroquad window.
    pub fn load(project: &Project) -> Audio {
        let mut sounds = HashMap::new();
        let mut errors = Vec::new();
        for def in &project.db.sounds {
            let path = project.path(&def.file);
            let res = std::fs::read(&path).map_err(|e| e.to_string()).and_then(|bytes| {
                if !supported(&bytes) {
                    return Err("not a WAV or OGG file".to_string());
                }
                // The decoder panics on some malformed files; don't take the game down.
                std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| poll_now(load_sound_from_bytes(&bytes))))
                    .map_err(|_| "could not decode".to_string())?
                    .ok_or("loading did not complete".to_string())?
                    .map_err(|e| e.to_string())
            });
            match res {
                Ok(s) => {
                    sounds.insert(def.id.clone(), (s, def.volume.clamp(0.0, 1.0)));
                }
                Err(e) => errors.push(format!("{}: {e}", path.display())),
            }
        }
        for e in &errors {
            eprintln!("[audio] {e}");
        }
        Audio { sounds, music: None, errors, enabled: std::env::var_os("RUST2D_MUTE").is_none() }
    }

    pub fn has(&self, id: &str) -> bool {
        self.sounds.contains_key(id)
    }

    /// Plays a sound effect once (unknown or empty ids are ignored).
    pub fn play(&self, id: &str) {
        if !self.enabled {
            return;
        }
        if let Some((s, vol)) = self.sounds.get(id) {
            play_sound(s, PlaySoundParams { looped: false, volume: *vol });
        }
    }

    /// Switches the looping music; `""` stops it. Re-requesting the current track keeps it playing.
    pub fn play_music(&mut self, id: &str) {
        if self.music.as_deref() == Some(id) {
            return;
        }
        self.stop_music();
        if id.is_empty() || !self.enabled {
            return;
        }
        if let Some((s, vol)) = self.sounds.get(id) {
            play_sound(s, PlaySoundParams { looped: true, volume: *vol });
            self.music = Some(id.to_string());
        }
    }

    pub fn stop_music(&mut self) {
        if let Some(id) = self.music.take() {
            if let Some((s, _)) = self.sounds.get(&id) {
                stop_sound(s);
            }
        }
    }

    pub fn current_music(&self) -> Option<&str> {
        self.music.as_deref()
    }
}

/// Encodes mono 16-bit PCM samples (-1..1) as a WAV file.
pub fn wav_bytes(samples: &[f32], sample_rate: u32) -> Vec<u8> {
    let data_len = samples.len() as u32 * 2;
    let mut out = Vec::with_capacity(44 + data_len as usize);
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&(36 + data_len).to_le_bytes());
    out.extend_from_slice(b"WAVEfmt ");
    out.extend_from_slice(&16u32.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes()); // PCM
    out.extend_from_slice(&1u16.to_le_bytes()); // mono
    out.extend_from_slice(&sample_rate.to_le_bytes());
    out.extend_from_slice(&(sample_rate * 2).to_le_bytes());
    out.extend_from_slice(&2u16.to_le_bytes());
    out.extend_from_slice(&16u16.to_le_bytes());
    out.extend_from_slice(b"data");
    out.extend_from_slice(&data_len.to_le_bytes());
    for s in samples {
        out.extend_from_slice(&((s.clamp(-1.0, 1.0) * i16::MAX as f32) as i16).to_le_bytes());
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wav_header() {
        let w = wav_bytes(&[0.0, 1.0, -1.0], 22050);
        assert!(supported(&w));
        assert_eq!(w.len(), 44 + 6);
        assert_eq!(&w[46..48], &i16::MAX.to_le_bytes());
        assert!(!supported(b"definitely not audio"));
    }
}
