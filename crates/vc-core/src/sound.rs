//! Alert sounds: the generated beep (exact v1 formula), sound files
//! (WAV/MP3/OGG/FLAC), the per-person sound bank, and one audio worker that
//! plays everything strictly in order so two alerts never overlap.

use crate::config::{Beep, Config, SoundSpec};
use std::collections::HashMap;
use std::io::Cursor;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, RecvTimeoutError};
use std::sync::{Arc, Mutex};
use std::time::Duration;

pub const RATE: u32 = 44_100;
/// Largest sound file we accept (alerts are short).
pub const MAX_FILE_BYTES: u64 = 10 * 1024 * 1024;
pub const NOT_AUDIO: &str =
    "That file doesn't look like a sound VC Alert can play. Use a WAV, MP3, OGG or FLAC file.";

/// v1 `make_tone_wav` samples: mono, 16-bit, 44.1 kHz, 5 ms linear fades.
pub fn tone_samples(freq: f64, dur_ms: i64, repeats: i64, gap_ms: i64, volume: f64) -> Vec<i16> {
    let rate = RATE as f64;
    let n = (rate * dur_ms as f64 / 1000.0) as i64;
    let fade = ((rate * 0.005) as i64).max(1) as f64;
    let amp = (32767.0 * volume.clamp(0.0, 1.0)) as i64 as f64;
    let repeats = repeats.max(1);
    let gap = (rate * gap_ms as f64 / 1000.0) as usize;
    let mut out = Vec::with_capacity((n.max(0) as usize + gap) * repeats as usize);
    for r in 0..repeats {
        for i in 0..n {
            let fi = i as f64;
            let env = 1.0f64.min(fi / fade).min((n - 1 - i) as f64 / fade);
            let s = amp * env * (2.0 * std::f64::consts::PI * freq * fi / rate).sin();
            out.push(s as i16);
        }
        if r < repeats - 1 {
            out.extend(std::iter::repeat_n(0i16, gap));
        }
    }
    out
}

/// The pause scales with the beep length (v1: `round(duration_ms * gap_ratio)`).
pub fn gap_ms(b: &Beep) -> i64 {
    (b.duration_ms as f64 * b.gap_ratio).round_ties_even() as i64
}

pub fn beep_samples(b: &Beep) -> Vec<i16> {
    tone_samples(b.frequency_hz as f64, b.duration_ms, b.repeats, gap_ms(b), b.volume)
}

pub fn wav_bytes(samples: &[i16]) -> Vec<u8> {
    let data_len = (samples.len() * 2) as u32;
    let mut v = Vec::with_capacity(44 + data_len as usize);
    v.extend_from_slice(b"RIFF");
    v.extend_from_slice(&(36 + data_len).to_le_bytes());
    v.extend_from_slice(b"WAVEfmt ");
    v.extend_from_slice(&16u32.to_le_bytes());
    v.extend_from_slice(&1u16.to_le_bytes()); // PCM
    v.extend_from_slice(&1u16.to_le_bytes()); // mono
    v.extend_from_slice(&RATE.to_le_bytes());
    v.extend_from_slice(&(RATE * 2).to_le_bytes());
    v.extend_from_slice(&2u16.to_le_bytes());
    v.extend_from_slice(&16u16.to_le_bytes());
    v.extend_from_slice(b"data");
    v.extend_from_slice(&data_len.to_le_bytes());
    for s in samples {
        v.extend_from_slice(&s.to_le_bytes());
    }
    v
}

/// Decode a sound and return its length in seconds, or an error if it isn't audio.
pub fn probe(bytes: &[u8]) -> Result<f64, String> {
    use rodio::Source;
    let dec = rodio::Decoder::new(Cursor::new(bytes.to_vec())).map_err(|_| NOT_AUDIO.to_string())?;
    let channels = dec.channels().get() as f64;
    let rate = dec.sample_rate().get() as f64;
    let mut count: u64 = 0;
    for _ in dec {
        count += 1;
        if count > (rate * channels * 120.0) as u64 {
            break; // anything over two minutes is long enough to know it's audio
        }
    }
    if count == 0 {
        return Err(NOT_AUDIO.into());
    }
    Ok(count as f64 / (rate * channels))
}

/// v1 `sound_config`: the universal sound with a person's override on top.
/// A person's beep only overrides the keys it has.
pub fn merged_sound(cfg: &Config, spec: Option<&SoundSpec>) -> (String, Beep) {
    let mut file = cfg.sound_file.clone();
    let mut beep = cfg.beep.clone();
    if let Some(s) = spec {
        if let Some(p) = &s.beep {
            beep = p.apply_to(&beep);
        }
        if let Some(f) = &s.file {
            file = f.clone();
        }
    }
    (file, beep)
}

/// Resolve a stored sound path: `~` expanded, relative paths against `base`.
pub fn resolve_path(file: &str, base: &Path) -> PathBuf {
    let p = if let Some(rest) = file.strip_prefix("~/").or_else(|| file.strip_prefix("~\\")) {
        home_dir().map(|h| h.join(rest)).unwrap_or_else(|| PathBuf::from(file))
    } else {
        PathBuf::from(file)
    };
    if p.is_absolute() { p } else { base.join(p) }
}

fn home_dir() -> Option<PathBuf> {
    std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE")).map(PathBuf::from)
}

/// A ready-to-play sound.
#[derive(Debug)]
pub struct Clip {
    pub bytes: Vec<u8>,
    pub duration: f64,
}

/// Hands out the right clip for each person, cached by its settings.
pub struct SoundBank {
    base: PathBuf,
    cache: Mutex<HashMap<String, Arc<Clip>>>,
}

impl SoundBank {
    /// `base` is the folder relative sound paths are resolved against.
    pub fn new(base: &Path) -> SoundBank {
        SoundBank { base: base.to_path_buf(), cache: Mutex::new(HashMap::new()) }
    }

    pub fn base(&self) -> &Path {
        &self.base
    }

    fn build(&self, file: &str, beep: &Beep) -> Result<Arc<Clip>, String> {
        let sig = if file.is_empty() {
            format!("beep:{}", serde_json::to_string(beep).unwrap_or_default())
        } else {
            let p = resolve_path(file, &self.base);
            let stamp = std::fs::metadata(&p)
                .and_then(|m| m.modified())
                .ok()
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| d.as_millis())
                .unwrap_or(0);
            format!("file:{}:{stamp}", p.display())
        };
        if let Some(c) = self.cache.lock().unwrap().get(&sig) {
            return Ok(c.clone());
        }
        let clip = if file.is_empty() {
            let samples = beep_samples(beep);
            let duration = samples.len() as f64 / RATE as f64;
            Clip { bytes: wav_bytes(&samples), duration }
        } else {
            let p = resolve_path(file, &self.base);
            if !p.exists() {
                return Err(format!("sound file not found: {}", p.display()));
            }
            let bytes = read_capped(&p)?;
            let duration = probe(&bytes)?;
            Clip { bytes, duration }
        };
        let clip = Arc::new(clip);
        self.cache.lock().unwrap().insert(sig, clip.clone());
        Ok(clip)
    }

    /// The universal sound. Errors if its file is missing.
    pub fn default_clip(&self, cfg: &Config) -> Result<Arc<Clip>, String> {
        let (file, beep) = merged_sound(cfg, None);
        self.build(&file, &beep)
    }

    /// A sound that isn't saved yet (Test buttons). May error.
    pub fn for_spec(&self, cfg: &Config, spec: Option<&SoundSpec>) -> Result<Arc<Clip>, String> {
        let (file, beep) = merged_sound(cfg, spec);
        self.build(&file, &beep)
    }

    /// A person's saved sound, falling back to the universal one if theirs is
    /// unusable (e.g. a deleted file).
    pub fn clip_for(&self, cfg: &Config, key: &str) -> Result<Arc<Clip>, String> {
        match cfg.find(key).and_then(|t| t.sound.as_ref()) {
            None => self.default_clip(cfg),
            Some(spec) => self.for_spec(cfg, Some(spec)).or_else(|_| self.default_clip(cfg)),
        }
    }
}

pub fn read_capped(p: &Path) -> Result<Vec<u8>, String> {
    let meta = std::fs::metadata(p).map_err(|e| format!("{}: {e}", p.display()))?;
    if meta.len() > MAX_FILE_BYTES {
        return Err("That sound file is too big. Alert sounds should be a few seconds long.".into());
    }
    std::fs::read(p).map_err(|e| format!("{}: {e}", p.display()))
}

/// One worker thread plays every sound in order. The audio device is opened on
/// demand and released after a few idle seconds, so the app doesn't keep the
/// sound card busy and picks up a newly plugged-in headset.
pub struct Player {
    tx: Mutex<mpsc::Sender<Arc<Clip>>>,
}

impl Default for Player {
    fn default() -> Self {
        Self::new()
    }
}

impl Player {
    pub fn new() -> Player {
        let (tx, rx) = mpsc::channel::<Arc<Clip>>();
        std::thread::Builder::new()
            .name("vc-alert-audio".into())
            .spawn(move || audio_worker(rx))
            .expect("audio thread");
        Player { tx: Mutex::new(tx) }
    }

    pub fn play(&self, clip: Arc<Clip>) {
        let _ = self.tx.lock().unwrap().send(clip);
    }
}

fn audio_worker(rx: mpsc::Receiver<Arc<Clip>>) {
    let mut device: Option<rodio::MixerDeviceSink> = None;
    loop {
        let clip = match rx.recv_timeout(Duration::from_secs(8)) {
            Ok(c) => c,
            Err(RecvTimeoutError::Timeout) => {
                device = None;
                continue;
            }
            Err(RecvTimeoutError::Disconnected) => return,
        };
        if device.is_none() {
            match rodio::DeviceSinkBuilder::open_default_sink() {
                Ok(mut d) => {
                    d.log_on_drop(false);
                    device = Some(d);
                }
                Err(e) => {
                    eprintln!("Could not open the sound device: {e}");
                    continue;
                }
            }
        }
        let Some(d) = device.as_ref() else { continue };
        match rodio::Decoder::new(Cursor::new(clip.bytes.clone())) {
            Ok(src) => {
                let p = rodio::Player::connect_new(&d.mixer());
                p.append(src);
                p.sleep_until_end();
            }
            Err(e) => eprintln!("Could not play sound: {e}"),
        }
    }
}

/// Play a clip and wait for it to finish (command-line test).
pub fn play_blocking(clip: &Clip) -> Result<(), String> {
    let mut d = rodio::DeviceSinkBuilder::open_default_sink().map_err(|e| e.to_string())?;
    d.log_on_drop(false);
    let src = rodio::Decoder::new(Cursor::new(clip.bytes.clone())).map_err(|e| e.to_string())?;
    let p = rodio::Player::connect_new(&d.mixer());
    p.append(src);
    p.sleep_until_end();
    Ok(())
}

/// "1000 Hz · 180 ms · 3 beeps" (v1 wording, no ♪).
pub fn describe_beep(b: &Beep) -> String {
    format!(
        "{} Hz · {} ms · {} {}",
        b.frequency_hz,
        b.duration_ms,
        b.repeats,
        if b.repeats == 1 { "beep" } else { "beeps" }
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tone_length_matches_v1() {
        let b = Beep::default(); // 180 ms × 3, ratio 0.5 → 0.72 s
        let s = beep_samples(&b);
        assert_eq!(s.len(), 3 * 7938 + 2 * 3969);
        assert!((s.len() as f64 / RATE as f64 - 0.72).abs() < 0.001);
        let long = Beep { duration_ms: 360, ..b.clone() };
        assert!(beep_samples(&long).len() > s.len());
        assert_eq!(s[0], 0, "fade-in starts silent");
    }

    #[test]
    fn wav_decodes() {
        let bytes = wav_bytes(&beep_samples(&Beep::default()));
        let d = probe(&bytes).unwrap();
        assert!((d - 0.72).abs() < 0.001);
        assert!(probe(b"not audio at all").is_err());
    }

    #[test]
    fn person_sound_layers_over_universal() {
        use crate::config::PartialBeep;
        let cfg = Config::default();
        let spec = SoundSpec { file: None, beep: Some(PartialBeep { repeats: Some(2), ..Default::default() }) };
        let (file, b) = merged_sound(&cfg, Some(&spec));
        assert_eq!(file, "");
        assert_eq!(b.repeats, 2);
        assert_eq!(b.frequency_hz, 1000);
    }

    #[test]
    fn missing_file_falls_back() {
        let dir = tempfile::tempdir().unwrap();
        let bank = SoundBank::new(dir.path());
        let mut cfg = Config::default();
        cfg.targets.push(crate::config::Target {
            label: "x".into(),
            id: None,
            username: Some("xx".into()),
            sound: Some(SoundSpec { file: Some("gone.wav".into()), beep: None }),
        });
        assert!(bank.for_spec(&cfg, cfg.targets[0].sound.as_ref()).is_err());
        let c = bank.clip_for(&cfg, "xx").unwrap();
        assert!((c.duration - 0.72).abs() < 0.001);
    }
}
