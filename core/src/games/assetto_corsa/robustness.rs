//! **Nothing a stranger can hand this program makes it fall over.**
//!
//! Links come from chats, presets and lists from forums and other
//! programs, results and answers from the game and from servers anybody
//! runs. Every reader of them is fed here with thousands of pieces of
//! nonsense — random bytes, random runs of the characters each format is
//! made of, truncations of real examples — and has to come back with an
//! answer or a refusal, never a panic. A launcher that crashes on one
//! server's answer is a launcher that crashes on a Friday night.

#![cfg(test)]

use super::*;

/// A small, seeded generator: the same nonsense every run, so a failure is
/// one that can be found again.
struct Noise(u64);

impl Noise {
    fn next(&mut self) -> u64 {
        // xorshift64*
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    /// A string of up to `most` characters drawn from `alphabet`, with an
    /// occasional character from anywhere in Unicode.
    fn text(&mut self, alphabet: &[char], most: usize) -> String {
        let len = (self.next() % (most as u64 + 1)) as usize;
        (0..len)
            .map(|_| {
                let roll = self.next();
                match roll % 17 {
                    0 => char::from_u32((roll >> 8) as u32 % 0x10_FFFF).unwrap_or('\u{fffd}'),
                    _ => alphabet
                        .get((roll >> 8) as usize % alphabet.len().max(1))
                        .copied()
                        .unwrap_or('x'),
                }
            })
            .collect()
    }
}

/// The characters the formats here are made of.
const MARKS: &[char] = &[
    '[', ']', '{', '}', '"', ':', ';', '=', ',', '.', '/', '\\', '%', '#', '?', '&', '\n', ' ',
    '-', '_', '0', '1', '7', '9', 'a', 'Z', 'x', '\u{2139}', '\u{201c}', '\u{201d}',
];

/// Real examples, cut at every length — what a download stopped half way
/// through, or a message cut by a chat's limit, looks like.
const EXAMPLES: &[&str] = &[
    "acmanager://race/online/join?ip=203.0.113.7&httpPort=8081&password=x%20y",
    "https://acstuff.club/s/q:race/online/join?ip=1.2.3.4&httpPort=9616",
    "[CONFIGURATION]\nAFFECTED_SECTIONS=WEATHER_FX\n[WEATHER_FX:BASIC]\nENABLED=1\n",
    "#version=1\n203.0.113.7:8081;Friday Night League\n",
    r#"{"track":"x","players":[{"name":"A","car":"c"}],"sessions":[{"name":"R","type":3,"laps":[{"lap":0,"car":0,"time":1}],"raceResult":[0]}]}"#,
    r#"{"current":{"time":"2026-11-11T14:15","temperature_2m":8.4,"weather_code":61}}"#,
    r#"{"Cars":[{"Model":"x","IsConnected":true,"DriverName":"A"}]}"#,
    r#"{"name":"Spa","geotags":["50.43° N","5.97° E"]}"#,
    "[SUN] ; The sun\nBRIGHTNESS=1.2 ; Sun brightness (how strong), from 0 to 3, round to 0.05\n",
];

/// Every reader of outside text, on one input.
fn read_everything(text: &str) {
    let _ = join_link::parse(text);
    let _ = csp_presets::text(&csp_presets::parse(text));
    let _ = content_manager::parse_list(text);
    if let Some(outcome) = results::parse(text) {
        for session in &outcome.sessions {
            let _ = session.table(&outcome.players);
        }
        let _ = outcome.main();
    }
    let _ = real_weather::parse(text).map(|now| (now.sky(), now.said()));
    let _ = real_weather::place_in(text);
    let _ = server_slots::parse(text)
        .map(|places| (server_slots::rooms(&places), server_slots::drivers(&places)));
    let _ = csp_config::sections(text, text);
    let _ = lobby::parse(text);
    let _ = server_details::parse(text);
    let _ = championship::parse(text).map(|out| out.finishing_order());
}

#[test]
fn no_reader_falls_over_on_nonsense() {
    let mut noise = Noise(0x5eed_cafe_f00d_d00d);
    for _ in 0..4000 {
        read_everything(&noise.text(MARKS, 160));
    }
    // And on bytes that are not text at all, read as the lossy text a
    // file of them would become.
    for _ in 0..1000 {
        let bytes: Vec<u8> = (0..(noise.next() % 200))
            .map(|_| noise.next() as u8)
            .collect();
        read_everything(&String::from_utf8_lossy(&bytes));
    }
}

#[test]
fn no_reader_falls_over_on_a_real_example_cut_short() {
    for example in EXAMPLES {
        for (at, _) in example.char_indices() {
            read_everything(example.get(..at).unwrap_or(""));
        }
        read_everything(example);
    }
}
