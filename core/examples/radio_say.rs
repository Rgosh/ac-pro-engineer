//! One of the engineer's lines, through the radio exactly as the programs
//! say it, written to a WAV — for listening without starting a game.
//!
//!     cargo run -p ac_core --example radio_say -- tyres.hot female out.wav
//!     cargo run -p ac_core --example radio_say -- all male demo.wav
//!
//! `all` puts every line without a value in a row, a second apart.

use ac_core::radio::lines::LINES;
use ac_core::radio::pack::{Pack, Voice};
use ac_core::radio::voice::{Sound, decode, on_the_radio, wav};

fn main() -> anyhow::Result<()> {
    let mut args = std::env::args().skip(1);
    let key = args.next().unwrap_or_else(|| "radio.check".to_string());
    let voice = match args.next().as_deref() {
        Some("female") => Voice::Female,
        _ => Voice::Male,
    };
    let out = args.next().unwrap_or_else(|| "radio.wav".to_string());
    let pack = Pack::embedded(voice)?;
    let sound = Sound {
        voice,
        ..Sound::default()
    };
    let keys: Vec<String> = if key == "all" {
        LINES
            .iter()
            .filter(|l| l.values.is_none())
            .map(|l| l.id.to_string())
            .collect()
    } else {
        vec![key]
    };
    let mut samples = Vec::new();
    let mut rate = pack.rate();
    for key in &keys {
        let clip = pack
            .clip(key)
            .ok_or_else(|| anyhow::anyhow!("no line called {key}"))?;
        let (clean, r) = decode(clip)?;
        rate = r;
        samples.extend(on_the_radio(&clean, r, sound.volume));
        samples.extend(std::iter::repeat_n(0.0, r as usize));
    }
    std::fs::write(&out, wav(&samples, rate))?;
    println!(
        "{} line(s), {:.1} s, {out}",
        keys.len(),
        samples.len() as f32 / rate as f32
    );
    Ok(())
}
