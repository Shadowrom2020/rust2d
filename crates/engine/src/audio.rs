//! Sound effects and looping music: WAV or Ogg Vorbis files, decoded by macroquad (quad-snd)
//! and mixed at 44.1 kHz stereo - other rates are resampled roughly, so 44.1 kHz plays best.
//! A file is checked before it reaches the decoder, and a file in another format is reported
//! by name (Opus, FLAC, MP3) so the creator knows what to convert.

use std::collections::HashMap;
use std::future::Future;
use std::pin::pin;
use std::task::{Context, Poll, Waker};

use macroquad::audio::{load_sound_from_bytes, play_sound, set_sound_volume, stop_sound, PlaySoundParams, Sound};

use crate::project::Project;

/// Polls a future once. macroquad's native sound loading completes immediately.
fn poll_now<F: Future>(f: F) -> Option<F::Output> {
    let mut cx = Context::from_waker(Waker::noop());
    match pin!(f).poll(&mut cx) {
        Poll::Ready(v) => Some(v),
        Poll::Pending => None,
    }
}

/// Checks that the bytes are a format the mixer decodes (WAV or Ogg Vorbis); the error names
/// a format it recognizes but can't play.
fn check_format(bytes: &[u8]) -> Result<(), String> {
    let has = |pat: &[u8]| bytes[..bytes.len().min(512)].windows(pat.len()).any(|w| w == pat);
    if bytes.len() < 12 {
        return Err("not a WAV or Ogg Vorbis file".into());
    }
    if &bytes[..4] == b"RIFF" && &bytes[8..12] == b"WAVE" {
        return Ok(());
    }
    if &bytes[..4] == b"OggS" {
        return if has(b"\x01vorbis") {
            Ok(())
        } else if has(b"OpusHead") {
            Err("Ogg Opus isn't supported: convert it to Ogg Vorbis".into())
        } else if has(b"\x7fFLAC") {
            Err("Ogg FLAC isn't supported: convert it to Ogg Vorbis or WAV".into())
        } else {
            Err("an Ogg file without a Vorbis stream".into())
        };
    }
    if &bytes[..3] == b"ID3" || (bytes[0] == 0xFF && bytes[1] & 0xE0 == 0xE0) {
        return Err("MP3 isn't supported: convert it to Ogg Vorbis or WAV".into());
    }
    if &bytes[..4] == b"fLaC" {
        return Err("FLAC isn't supported: convert it to Ogg Vorbis or WAV".into());
    }
    Err("not a WAV or Ogg Vorbis file".into())
}

pub struct Audio {
    sounds: HashMap<String, (Sound, f32)>,
    music: Option<String>,
    /// The sound last started by [`Audio::play_preview`] (the editor's Play button).
    preview: Option<String>,
    pub errors: Vec<String>,
    pub enabled: bool,
    /// Player options (0..1): effects and music volume, master already applied.
    effects: f32,
    music_vol: f32,
}

impl Audio {
    /// Loads all sounds of the project. Needs a macroquad window.
    pub fn load(project: &Project) -> Audio {
        let mut sounds = HashMap::new();
        let mut errors = Vec::new();
        for def in &project.db.sounds {
            let path = project.path(&def.file);
            let res = std::fs::read(&path).map_err(|e| e.to_string()).and_then(|bytes| {
                check_format(&bytes)?;
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
        Audio { sounds, music: None, preview: None, errors, enabled: std::env::var_os("RUST2D_MUTE").is_none(), effects: 1.0, music_vol: 1.0 }
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
            play_sound(s, PlaySoundParams { looped: false, volume: *vol * self.effects });
        }
    }

    /// Plays a sound once as a preview (the editor): the previous preview stops first.
    pub fn play_preview(&mut self, id: &str) {
        self.stop_preview();
        if self.sounds.contains_key(id) {
            self.play(id);
            self.preview = Some(id.to_string());
        }
    }

    /// Stops the preview (and the music, if it is the same sound).
    pub fn stop_preview(&mut self) {
        if let Some(id) = self.preview.take() {
            self.stop(&id);
        }
    }

    /// Stops every playback of a sound, as preview, effect or music.
    pub fn stop(&mut self, id: &str) {
        if let Some((s, _)) = self.sounds.get(id) {
            stop_sound(s);
        }
        if self.music.as_deref() == Some(id) {
            self.music = None;
        }
        if self.preview.as_deref() == Some(id) {
            self.preview = None;
        }
    }

    /// The sound started as a preview, if one was (a short effect may have ended by itself).
    pub fn previewing(&self) -> Option<&str> {
        self.preview.as_deref()
    }

    /// Applies the player's volume options (also to the music that is playing).
    pub fn set_volumes(&mut self, effects: f32, music: f32) {
        self.effects = effects.clamp(0.0, 1.0);
        self.music_vol = music.clamp(0.0, 1.0);
        if let Some((s, vol)) = self.music.as_ref().and_then(|id| self.sounds.get(id)) {
            set_sound_volume(s, *vol * self.music_vol);
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
            play_sound(s, PlaySoundParams { looped: true, volume: *vol * self.music_vol });
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
        assert_eq!(check_format(&w), Ok(()));
        assert_eq!(w.len(), 44 + 6);
        assert_eq!(&w[46..48], &i16::MAX.to_le_bytes());
    }

    #[test]
    fn formats_are_told_apart() {
        assert!(check_format(b"definitely not audio").is_err());
        let ogg = |body: &[u8]| [b"OggS\0\x02", &[0u8; 22][..], b"\x01\x1e", &[0u8; 30][..], body].concat();
        assert_eq!(check_format(&ogg(b"\x01vorbis\0\0\0\0")), Ok(()));
        assert!(check_format(&ogg(b"OpusHead\x01")).unwrap_err().contains("Opus"));
        assert!(check_format(&ogg(b"\x7fFLAC\x01")).unwrap_err().contains("FLAC"));
        assert!(check_format(&ogg(b"\x80theora")).unwrap_err().contains("without a Vorbis"));
        assert!(check_format(b"ID3\x04\0\0\0\0\0\0\0\0").unwrap_err().contains("MP3"));
        assert!(check_format(&[&[0xFF, 0xFB, 0x90, 0x00][..], &[0u8; 20][..]].concat()).unwrap_err().contains("MP3"));
        assert!(check_format(b"fLaC\0\0\0\x22\0\0\0\0\0").unwrap_err().contains("FLAC"));
    }
}
