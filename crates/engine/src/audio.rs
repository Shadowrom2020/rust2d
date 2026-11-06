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
    /// The built-in rain loop (made on first use) and its volume (0 = stopped).
    rain: Option<Sound>,
    rain_vol: f32,
    /// Player option (0..1): environmental sounds (rain), master applied.
    ambience: f32,
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
        Audio { sounds, music: None, preview: None, errors, enabled: std::env::var_os("RUST2D_MUTE").is_none(), effects: 1.0, music_vol: 1.0, rain: None, rain_vol: 0.0, ambience: 1.0 }
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

    /// The player's ambience volume (Options › Ambience volume), also for the playing rain.
    pub fn set_ambience_volume(&mut self, v: f32) {
        self.ambience = v.clamp(0.0, 1.0);
        if let (Some(s), true) = (&self.rain, self.rain_vol > 0.0) {
            set_sound_volume(s, self.rain_vol * self.ambience);
        }
    }

    /// The built-in rain loop at `volume` (0..1, before the ambience volume; 0 stops it).
    pub fn set_rain(&mut self, volume: f32) {
        let v = if self.enabled { volume.clamp(0.0, 1.0) } else { 0.0 };
        if (v - self.rain_vol).abs() < 0.01 {
            return;
        }
        if v <= 0.0 {
            if let Some(s) = &self.rain {
                stop_sound(s);
            }
            self.rain_vol = 0.0;
            return;
        }
        if self.rain.is_none() {
            let bytes = wav_bytes(&rain_samples(), MIX_RATE);
            self.rain = poll_now(load_sound_from_bytes(&bytes)).and_then(|r| r.ok());
        }
        let Some(s) = &self.rain else { return };
        if self.rain_vol <= 0.0 {
            play_sound(s, PlaySoundParams { looped: true, volume: v * self.ambience });
        } else {
            set_sound_volume(s, v * self.ambience);
        }
        self.rain_vol = v;
    }

    /// Volume of the rain loop (0 = not playing).
    pub fn rain_volume(&self) -> f32 {
        self.rain_vol
    }
}

/// Sample rate of macroquad's mixer (sounds at other rates are resampled when loaded).
pub const MIX_RATE: u32 = 44_100;
/// Frames macroquad's mixer (quad-snd) fills per pass on Linux.
pub const MIX_BUFFER_FRAMES: usize = 4096;

/// About six seconds of rain that loop seamlessly: filtered noise (the hiss) with soft drop
/// ticks, at the mixer's own rate.
///
/// - The length is a whole number of mixer buffers: quad-snd rewinds a looped sound that ends
///   inside a buffer but writes the rest at the buffer's start again, leaving the end of that
///   buffer silent — a short gap at every loop. Sounds start at a buffer boundary, so a length
///   of whole buffers always ends on one (and smaller power-of-two buffers divide it too).
/// - The extra tail fades into the start with an equal-power crossfade: the two noises are
///   unrelated, so a linear fade would dip in loudness at every loop.
pub fn rain_samples() -> Vec<f32> {
    let rate = MIX_RATE;
    let n = MIX_BUFFER_FRAMES * 65;
    let fade = rate as usize;
    let mut seed = 0x1234_5678u32;
    let mut rnd = move || {
        seed ^= seed << 13;
        seed ^= seed >> 17;
        seed ^= seed << 5;
        (seed >> 8) as f32 / (1u32 << 24) as f32 * 2.0 - 1.0
    };
    let (mut low, mut high, mut drop, mut drop_env) = (0.0f32, 0.0f32, 0.0f32, 0.0f32);
    let mut raw = Vec::with_capacity(n + fade);
    for _ in 0..n + fade {
        let w = rnd();
        low += (w - low) * 0.18; // body of the hiss
        high = 0.8 * (high + w - low) * 0.5; // a little brightness
        if rnd() > 0.9975 {
            drop_env = 0.6 + 0.4 * rnd().abs();
        }
        drop = drop * 0.6 + rnd() * drop_env;
        drop_env *= 0.993;
        raw.push(low * 0.55 + high * 0.25 + drop * 0.12);
    }
    // Crossfade the extra tail into the beginning so the loop has no seam.
    let mut out: Vec<f32> = raw[..n].to_vec();
    for i in 0..fade {
        let t = i as f32 / fade as f32 * std::f32::consts::FRAC_PI_2;
        out[i] = out[i] * t.sin() + raw[n + i] * t.cos();
    }
    let peak = out.iter().fold(0.0f32, |m, v| m.max(v.abs())).max(1e-6);
    out.iter().map(|v| v / peak * 0.6).collect()
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

    /// quad-snd's mixing of one looped sound, as it is (0.2.8): buffers of `MIX_BUFFER_FRAMES`
    /// stereo frames; after a rewind the rest is written from the buffer's start again.
    fn quad_snd_mix(data: &[f32], buffers: usize) -> Vec<f32> {
        let mut out = Vec::new();
        let mut pos = 0usize;
        for _ in 0..buffers {
            let mut buffer = vec![0.0f32; MIX_BUFFER_FRAMES * 2];
            let mut remainder = buffer.len();
            loop {
                let chunk = &data[pos.min(data.len())..];
                let chunk = &chunk[..remainder.min(chunk.len())];
                pos += remainder;
                for (b, s) in buffer.iter_mut().zip(chunk) {
                    *b += s;
                }
                remainder -= chunk.len();
                if remainder > 0 {
                    pos = 0;
                    continue;
                }
                break;
            }
            out.extend(buffer);
        }
        out
    }

    #[test]
    fn rain_loop_survives_quad_snds_mixer() {
        let stereo = |m: &[f32]| m.iter().flat_map(|&v| [v, v]).collect::<Vec<f32>>();
        let rain = stereo(&rain_samples());
        let n = rain.len() / (MIX_BUFFER_FRAMES * 2);
        let mixed = quad_snd_mix(&rain, 2 * n + 3);
        let ideal: Vec<f32> = rain.iter().cycle().take(mixed.len()).copied().collect();
        assert_eq!(mixed, ideal, "the rain loops exactly through the mixer");
        // A loop that ends inside a buffer (like the old 6 s at 22 050 Hz, resampled) gets a gap.
        let odd = &rain[..rain.len() - 3000];
        let mixed = quad_snd_mix(odd, n + 2);
        let silent = mixed.chunks(2).skip(odd.len() / 2).take(MIX_BUFFER_FRAMES).filter(|f| f[0] == 0.0).count();
        assert!(silent > 1000, "the mixer bug this avoids: {silent} silent frames after the loop point");
    }

    #[test]
    fn rain_loops_without_a_click() {
        let s = rain_samples();
        assert_eq!(s.len() % MIX_BUFFER_FRAMES, 0, "whole mixer buffers (quad-snd's loop gap)");
        assert!((5.9..6.2).contains(&(s.len() as f32 / MIX_RATE as f32)));
        // At the mixer's rate: loading doesn't resample (which could change the length).
        let wav = wav_bytes(&s, MIX_RATE);
        assert_eq!(u32::from_le_bytes(wav[24..28].try_into().unwrap()), 44_100);
        let peak = s.iter().fold(0.0f32, |m, v| m.max(v.abs()));
        assert!((0.59..=0.61).contains(&peak), "normalized: {peak}");
        // Loudness stays even across the loop point (no dip after it): RMS of 50 ms windows over
        // two passes.
        let two: Vec<f32> = s.iter().chain(s.iter()).copied().collect();
        let rms: Vec<f32> = two.chunks(1102).map(|c| (c.iter().map(|v| v * v).sum::<f32>() / c.len() as f32).sqrt()).collect();
        let avg = rms.iter().sum::<f32>() / rms.len() as f32;
        let low = rms.iter().cloned().fold(f32::MAX, f32::min);
        assert!(low > avg * 0.8, "no loudness dip: lowest {low} vs average {avg}");
        // The end flows into the start like any two neighbouring samples do.
        let step = |a: f32, b: f32| (a - b).abs();
        let typical = s.windows(2).map(|w| step(w[0], w[1])).sum::<f32>() / s.len() as f32;
        assert!(step(s[s.len() - 1], s[0]) < typical * 6.0, "seam {} vs typical {typical}", step(s[s.len() - 1], s[0]));
    }

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
