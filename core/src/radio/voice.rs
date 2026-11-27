//! Saying a line: the clip, through a team radio, out of the speakers.
//!
//! **Next to nothing while the car is on track.** The voice is a clip already
//! spoken; saying it is decoding a second or two of Ogg, a few filters over
//! it, and handing a WAV to the system. All of it happens on a thread of its
//! own that sleeps on a channel between lines — no audio stream is kept open,
//! nothing runs in the drawing loop, and a minute without a line costs
//! exactly nothing.
//!
//! **No audio library of our own.** Windows plays a WAV from memory with
//! `PlaySound`, which every Windows has. Linux hands a file to whatever plays
//! sound on that desktop — `pw-play` (PipeWire, the Steam Deck), `paplay`
//! (PulseAudio) or `aplay` (ALSA), the first one found. A machine with none of
//! them still gets the subtitles; it just does not hear them.
//!
//! **The radio is put on here, not in the clips.** The voice bank's clips are
//! clean, so the effect can be tuned — or a voice heard plainly — without
//! making the voice again. The chain is what a team radio does to a voice: a
//! narrow band, the microphone driven into soft saturation, a compressor
//! squashing it level, the channel's hiss under it and the button's click at
//! either end.

use std::io::Cursor;
use std::sync::mpsc::{Receiver, Sender, channel};
use std::thread;

use anyhow::{Context, Result};
use lewton::inside_ogg::OggStreamReader;

use super::pack::{Pack, Voice};

/// What a driver chose about the radio.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Sound {
    pub voice: Voice,
    /// 0..1.
    pub volume: f32,
    /// The team-radio effect. Off is the voice as the bank made it.
    pub radio: bool,
}

impl Default for Sound {
    fn default() -> Self {
        Self {
            voice: Voice::Male,
            volume: 0.8,
            radio: true,
        }
    }
}

/// A clip as samples, -1..1, and its rate.
pub fn decode(ogg: &[u8]) -> Result<(Vec<f32>, u32)> {
    let mut reader = OggStreamReader::new(Cursor::new(ogg)).context("not an Ogg Vorbis clip")?;
    let rate = reader.ident_hdr.audio_sample_rate;
    let channels = usize::from(reader.ident_hdr.audio_channels.max(1));
    let mut samples = Vec::new();
    while let Some(packet) = reader.read_dec_packet_itl().context("a damaged clip")? {
        // Mono by design; a stereo clip is folded rather than refused.
        for frame in packet.chunks(channels) {
            let sum: i32 = frame.iter().map(|s| i32::from(*s)).sum();
            samples.push(sum as f32 / (channels as f32 * 32768.0));
        }
    }
    Ok((samples, rate))
}

/// A second-order filter, the RBJ cookbook's.
struct Biquad {
    b: [f32; 3],
    a: [f32; 2],
    x: [f32; 2],
    y: [f32; 2],
}

impl Biquad {
    fn new(kind: Kind, hz: f32, rate: u32) -> Self {
        let w = std::f32::consts::TAU * hz / rate as f32;
        let (sin, cos) = w.sin_cos();
        let alpha = sin / (2.0 * std::f32::consts::FRAC_1_SQRT_2);
        let (b0, b1, b2) = match kind {
            Kind::Low => ((1.0 - cos) / 2.0, 1.0 - cos, (1.0 - cos) / 2.0),
            Kind::High => ((1.0 + cos) / 2.0, -(1.0 + cos), (1.0 + cos) / 2.0),
        };
        let a0 = 1.0 + alpha;
        Self {
            b: [b0 / a0, b1 / a0, b2 / a0],
            a: [-2.0 * cos / a0, (1.0 - alpha) / a0],
            x: [0.0; 2],
            y: [0.0; 2],
        }
    }

    fn run(&mut self, input: f32) -> f32 {
        let [b0, b1, b2] = self.b;
        let [a1, a2] = self.a;
        let out = b0 * input + b1 * self.x[0] + b2 * self.x[1] - a1 * self.y[0] - a2 * self.y[1];
        self.x = [input, self.x[0]];
        self.y = [out, self.y[0]];
        out
    }
}

#[derive(Clone, Copy)]
enum Kind {
    Low,
    High,
}

/// The same hiss on every run, so a line sounds the same every time.
struct Noise(u32);

impl Noise {
    fn next(&mut self) -> f32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 17;
        self.0 ^= self.0 << 5;
        (self.0 as f32 / u32::MAX as f32) * 2.0 - 1.0
    }
}

/// A clean clip, as a driver hears it on the radio.
pub fn on_the_radio(clean: &[f32], rate: u32, volume: f32) -> Vec<f32> {
    let lead = (0.05 * rate as f32) as usize;
    let click = (0.03 * rate as f32) as usize;
    let mut voice = vec![0.0; lead];
    voice.extend_from_slice(clean);
    voice.extend(std::iter::repeat_n(0.0, lead));

    // The microphone and the channel: narrow, driven, squashed.
    let mut low_cut = Biquad::new(Kind::High, 380.0, rate);
    let mut high_cut = Biquad::new(Kind::Low, 3000.0, rate);
    let (attack, release) = (
        (-1.0 / (0.002 * rate as f32)).exp(),
        (-1.0 / (0.040 * rate as f32)).exp(),
    );
    let threshold = 10f32.powf(-22.0 / 20.0);
    let mut level = 0.0f32;
    let mut low_cut_2 = Biquad::new(Kind::High, 380.0, rate);
    let mut high_cut_2 = Biquad::new(Kind::Low, 3200.0, rate);
    for s in &mut voice {
        let mut x = high_cut.run(low_cut.run(*s));
        x = (x * 3.5).tanh();
        let a = x.abs();
        level = if a > level {
            attack * level + (1.0 - attack) * a
        } else {
            release * level + (1.0 - release) * a
        };
        if level > threshold {
            // Ratio 8: above the threshold, one eighth of the rise survives.
            let over = level / threshold;
            x *= over.powf(1.0 / 8.0 - 1.0);
        }
        *s = high_cut_2.run(low_cut_2.run(x));
    }

    // The channel's hiss, the whole length, and the button either side.
    let mut noise = Noise(0x9E37_79B9);
    let mut hiss_low = Biquad::new(Kind::High, 500.0, rate);
    let mut hiss_high = Biquad::new(Kind::Low, 3500.0, rate);
    let peak = voice.iter().fold(0.0f32, |m, s| m.max(s.abs())).max(1e-6);
    let mut out: Vec<f32> = Vec::with_capacity(voice.len() + 2 * click);
    out.extend((0..click).map(|_| noise.next() * 0.25));
    for s in &voice {
        let hiss = hiss_high.run(hiss_low.run(noise.next())) * 0.03;
        out.push(s / peak * 0.85 + hiss);
    }
    out.extend((0..click).map(|_| noise.next() * 0.25));
    let peak = out.iter().fold(0.0f32, |m, s| m.max(s.abs())).max(1e-6);
    let gain = 0.89 / peak * volume.clamp(0.0, 1.0);
    out.iter_mut().for_each(|s| *s *= gain);
    out
}

/// Samples as a 16-bit mono WAV.
pub fn wav(samples: &[f32], rate: u32) -> Vec<u8> {
    let data = (samples.len() * 2) as u32;
    let mut out = Vec::with_capacity(44 + data as usize);
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&(36 + data).to_le_bytes());
    out.extend_from_slice(b"WAVEfmt ");
    out.extend_from_slice(&16u32.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes()); // PCM
    out.extend_from_slice(&1u16.to_le_bytes()); // mono
    out.extend_from_slice(&rate.to_le_bytes());
    out.extend_from_slice(&(rate * 2).to_le_bytes());
    out.extend_from_slice(&2u16.to_le_bytes());
    out.extend_from_slice(&16u16.to_le_bytes());
    out.extend_from_slice(b"data");
    out.extend_from_slice(&data.to_le_bytes());
    for s in samples {
        let v = (s.clamp(-1.0, 1.0) * 32767.0) as i16;
        out.extend_from_slice(&v.to_le_bytes());
    }
    out
}

/// A line, ready for the speakers: decoded, on the radio, as a WAV.
pub fn render(pack: &Pack, key: &str, sound: Sound) -> Option<Vec<u8>> {
    let (clean, rate) = decode(pack.clip(key)?).ok()?;
    let samples = if sound.radio {
        on_the_radio(&clean, rate, sound.volume)
    } else {
        clean.iter().map(|s| s * sound.volume).collect()
    };
    Some(wav(&samples, rate))
}

enum Order {
    Say(String),
    Sound(Sound),
}

/// The thread that speaks. Dropping it ends the thread.
pub struct Player {
    orders: Sender<Order>,
}

impl Player {
    /// Start the speaking thread with a voice built into the program.
    pub fn start(sound: Sound) -> Result<Player> {
        let (orders, inbox) = channel();
        thread::Builder::new()
            .name("radio".to_string())
            .spawn(move || speak(inbox, sound))
            .context("could not start the radio's thread")?;
        Ok(Player { orders })
    }

    /// Say a line. Returns at once; the thread does the rest.
    pub fn say(&self, key: &str) {
        let _ = self.orders.send(Order::Say(key.to_string()));
    }

    /// A new voice, volume or effect, from the next line on.
    pub fn set(&self, sound: Sound) {
        let _ = self.orders.send(Order::Sound(sound));
    }
}

fn speak(inbox: Receiver<Order>, mut sound: Sound) {
    let mut pack = Pack::embedded(sound.voice).ok();
    let out = Speakers::find();
    // Asleep on the channel between lines: no polling, no open stream.
    while let Ok(order) = inbox.recv() {
        match order {
            Order::Sound(next) => {
                if next.voice != sound.voice {
                    pack = Pack::embedded(next.voice).ok();
                }
                sound = next;
            }
            Order::Say(key) => {
                let Some(wav) = pack.as_ref().and_then(|p| render(p, &key, sound)) else {
                    continue;
                };
                out.play(&wav);
            }
        }
    }
}

/// Whatever plays sound on this machine.
struct Speakers {
    #[cfg(not(target_os = "windows"))]
    command: Option<(&'static str, &'static [&'static str])>,
}

impl Speakers {
    #[cfg(not(target_os = "windows"))]
    fn find() -> Self {
        const PLAYERS: [(&str, &[&str]); 3] =
            [("pw-play", &[]), ("paplay", &[]), ("aplay", &["-q"])];
        let found = PLAYERS.into_iter().find(|(name, _)| on_path(name));
        if found.is_none() {
            tracing::warn!("no pw-play, paplay or aplay: the radio shows subtitles only");
        }
        Self { command: found }
    }

    #[cfg(target_os = "windows")]
    fn find() -> Self {
        Self {}
    }

    #[cfg(not(target_os = "windows"))]
    fn play(&self, wav: &[u8]) {
        let Some((name, args)) = self.command else {
            return;
        };
        let path = std::env::temp_dir().join(format!("pe-radio-{}.wav", std::process::id()));
        if std::fs::write(&path, wav).is_err() {
            return;
        }
        // Waited for here, on the radio's own thread: the next line starts
        // when this one has finished, never on top of it.
        let _ = std::process::Command::new(name)
            .args(args)
            .arg(&path)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status();
        let _ = std::fs::remove_file(&path);
    }

    #[cfg(target_os = "windows")]
    fn play(&self, wav: &[u8]) {
        use windows::Win32::Media::Audio::{PlaySoundW, SND_MEMORY, SND_NODEFAULT, SND_SYNC};
        use windows::core::PCWSTR;
        // SAFETY: with SND_MEMORY the "name" is a pointer to a whole WAV in
        // memory, which `wav` is, and SND_SYNC returns only once it has been
        // played — so the buffer outlives every read of it.
        unsafe {
            let _ = PlaySoundW(
                PCWSTR(wav.as_ptr().cast()),
                None,
                SND_MEMORY | SND_SYNC | SND_NODEFAULT,
            );
        }
    }
}

#[cfg(not(target_os = "windows"))]
fn on_path(name: &str) -> bool {
    std::env::var_os("PATH")
        .is_some_and(|paths| std::env::split_paths(&paths).any(|dir| dir.join(name).is_file()))
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;

    #[test]
    fn a_clip_decodes_to_its_length() {
        let pack = Pack::embedded(Voice::Male).unwrap();
        let (samples, rate) = decode(pack.clip("radio.check").unwrap()).unwrap();
        let seconds = samples.len() as f32 / rate as f32;
        let said = pack.lengths()["radio.check"];
        assert!(
            (seconds - said).abs() < 0.3,
            "{seconds} s decoded, {said} s in the pack"
        );
    }

    #[test]
    fn the_radio_is_a_band_and_never_clips() {
        let pack = Pack::embedded(Voice::Female).unwrap();
        let (clean, rate) = decode(pack.clip("tyres.hot").unwrap()).unwrap();
        let radio = on_the_radio(&clean, rate, 1.0);
        let peak = radio.iter().fold(0.0f32, |m, s| m.max(s.abs()));
        assert!(peak <= 0.9, "peaks at {peak}");
        assert!(peak > 0.5, "too quiet: {peak}");
        // The clicks and the lead make it a little longer, never shorter.
        assert!(radio.len() > clean.len());
    }

    #[test]
    fn a_line_renders_as_a_wav() {
        let pack = Pack::embedded(Voice::Male).unwrap();
        let wav = render(&pack, "best.lap", Sound::default()).unwrap();
        assert_eq!(wav.get(..4), Some(b"RIFF".as_slice()));
        assert_eq!(wav.get(8..12), Some(b"WAVE".as_slice()));
        assert!(render(&pack, "no.such.line", Sound::default()).is_none());
    }

    /// The whole of saying a line — decode, the radio, the WAV — is a few
    /// milliseconds of one core, once per line. Held under a generous bound
    /// so a slow CI machine passes and a real regression does not.
    #[test]
    fn saying_a_line_costs_almost_nothing() {
        let pack = Pack::embedded(Voice::Male).unwrap();
        let started = std::time::Instant::now();
        for _ in 0..10 {
            render(&pack, "bias.front", Sound::default()).unwrap();
        }
        let each = started.elapsed() / 10;
        assert!(each.as_millis() < 150, "{each:?} a line");
    }
}
