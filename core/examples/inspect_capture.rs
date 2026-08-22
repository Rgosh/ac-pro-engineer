//! Read a captured page and say what is plausibly at each offset.
//!
//! `capture_pages` takes the bytes; this is what turns them into a layout. The
//! second simulator's pages are a few hundred fields with no names attached,
//! and finding where one field ends and the next begins by staring at hex is
//! how a struct ends up 964 bytes out — which is the mistake this project
//! actually made, with Competizione's layout on Assetto Corsa's pages.
//!
//! So it does the reading. At every four-byte offset it decodes a float and an
//! integer and says whether either is *plausible* — a tyre temperature, a
//! pressure in psi, a lap time in milliseconds — and it finds the runs of
//! four identical-looking floats that are almost always the four wheels.
//!
//! ```text
//! cargo run -p ac_core --example capture_pages > capture.txt
//! cargo run -p ac_core --example inspect_capture -- capture.txt
//! cat capture.txt | cargo run -p ac_core --example inspect_capture
//! ```
//!
//! **It suggests; it does not conclude.** Every offset it prints is a
//! candidate to be confirmed against a value you can see on screen in the
//! game — a speed you were doing, a pressure the setup screen showed. A field
//! this agrees with and the game does not is a field in the wrong place.

use std::io::Read;

/// A run of four floats that all look like the same kind of measurement is
/// almost always the four wheels — pressures, temperatures, wear, loads.
const WHEELS: usize = 4;

fn main() {
    let source = std::env::args().nth(1);
    let text = match &source {
        Some(path) => match std::fs::read_to_string(path) {
            Ok(text) => text,
            Err(error) => {
                eprintln!("cannot read {path}: {error}");
                std::process::exit(1);
            }
        },
        None => {
            let mut buffer = String::new();
            if std::io::stdin().read_to_string(&mut buffer).is_err() {
                eprintln!("nothing on stdin, and no file named");
                std::process::exit(1);
            }
            buffer
        }
    };

    let bytes = hex_bytes(&text);
    if bytes.len() < 8 {
        eprintln!(
            "found {} bytes of hex — is this the output of capture_pages?",
            bytes.len()
        );
        std::process::exit(1);
    }

    println!("{} bytes\n", bytes.len());
    report(&bytes);
}

/// The shortest run of hex a payload line can be.
///
/// `capture_pages` writes thirty-two bytes to a line; the last line of a page
/// is shorter. Sixteen digits is eight bytes, which no English word reaches.
const MIN_HEX_LINE: usize = 16;

/// Pull the hex out of whatever it is pasted into.
///
/// **Line by line, not character by character**, and that distinction is the
/// whole correctness of this tool. `capture_pages` prints its hex inside a
/// Rust constant with a paragraph of prose above it — and `acc`, `added` and
/// `face` are all valid hex. Sieving the whole text for hex digits swallowed
/// those too and shifted every byte after them, which turned a 596-byte page
/// into 639 bytes of nonsense that still looked like a capture. It reported
/// almost no candidates, which is exactly how it was caught.
///
/// So a line counts only if, once quotes and commas are gone, there is
/// nothing in it but an even run of hex digits.
fn hex_bytes(text: &str) -> Vec<u8> {
    let mut out = Vec::new();
    for line in text.lines() {
        let cleaned: String = line
            .chars()
            .filter(|c| !matches!(c, '"' | ',' | ' ' | '\t' | '\\'))
            .collect();
        if cleaned.len() < MIN_HEX_LINE
            || !cleaned.len().is_multiple_of(2)
            || !cleaned.bytes().all(|b| b.is_ascii_hexdigit())
        {
            continue;
        }
        // `as_chunks` rather than `chunks_exact(2)`, for the reason clippy
        // started giving in 1.98: the pair is a fixed-size array, so neither
        // index below can be out of bounds.
        for pair in cleaned.as_bytes().as_chunks::<2>().0 {
            let value = |b: u8| match b {
                b'0'..=b'9' => b - b'0',
                b'a'..=b'f' => b - b'a' + 10,
                _ => b - b'A' + 10,
            };
            out.push((value(pair[0]) << 4) | value(pair[1]));
        }
    }
    out
}

/// What a four-byte word might be.
#[derive(Debug, PartialEq)]
enum Reading {
    /// A float in a range something on a car actually occupies.
    Float(f32, &'static str),
    /// An integer small enough to be a count, a flag or a millisecond time.
    Int(i32, &'static str),
    Nothing,
}

/// Judge one four-byte word.
///
/// The ranges are deliberately wide: this is a sieve for "worth looking at",
/// not a classifier. A number that passes still has to be checked against
/// something seen in the game.
fn judge(word: [u8; 4]) -> Reading {
    let float = f32::from_le_bytes(word);
    let int = i32::from_le_bytes(word);

    if float.is_finite() && float != 0.0 {
        let magnitude = float.abs();
        let what = match magnitude {
            m if (0.05..=1.0).contains(&m) => Some("0..1 — a pedal, a ratio, grip"),
            m if (10.0..=45.0).contains(&m) => Some("psi, or a temperature in °C"),
            m if (45.0..=160.0).contains(&m) => Some("°C — tyre or air"),
            m if (160.0..=1200.0).contains(&m) => Some("°C brake, km/h, or litres×"),
            m if (1200.0..=20000.0).contains(&m) => Some("rpm, or metres"),
            _ => None,
        };
        if let Some(what) = what {
            return Reading::Float(float, what);
        }
    }

    // A plausible integer is a small count or a time. Anything huge is far more
    // likely to be a float or two packed shorts.
    match int {
        0 => Reading::Nothing,
        1..=64 => Reading::Int(int, "a count, an index or a flag"),
        1_000..=3_600_000 => Reading::Int(int, "milliseconds — a lap or a sector"),
        _ => Reading::Nothing,
    }
}

fn word_at(bytes: &[u8], offset: usize) -> Option<[u8; 4]> {
    bytes.get(offset..offset + 4)?.try_into().ok()
}

/// Four consecutive floats that all fall in the same band.
fn wheels_at(bytes: &[u8], offset: usize) -> Option<[f32; WHEELS]> {
    let mut out = [0.0f32; WHEELS];
    let mut band = None;
    for (i, slot) in out.iter_mut().enumerate() {
        let word = word_at(bytes, offset + i * 4)?;
        match judge(word) {
            Reading::Float(value, what) => {
                if *band.get_or_insert(what) != what {
                    return None;
                }
                *slot = value;
            }
            _ => return None,
        }
    }
    Some(out)
}

fn report(bytes: &[u8]) {
    println!("── likely four-wheel groups ────────────────────────────");
    println!("Four floats of the same kind in a row. Pressures, temperatures,");
    println!("wear and loads all look like this, and they are the easiest");
    println!("fields to confirm against the game's own screens.\n");

    let mut offset = 0;
    let mut groups = 0;
    while offset + WHEELS * 4 <= bytes.len() {
        if let Some(values) = wheels_at(bytes, offset) {
            println!(
                "  {offset:>4}  [{:.2}, {:.2}, {:.2}, {:.2}]",
                values[0], values[1], values[2], values[3]
            );
            groups += 1;
            offset += WHEELS * 4;
        } else {
            offset += 4;
        }
    }
    if groups == 0 {
        println!("  none — a page captured in the pits often has nothing in it");
    }

    println!("\n── every word that decodes to something ────────────────");
    for offset in (0..bytes.len().saturating_sub(3)).step_by(4) {
        let Some(word) = word_at(bytes, offset) else {
            continue;
        };
        match judge(word) {
            Reading::Float(value, what) => println!("  {offset:>4}  {value:>12.3}   {what}"),
            Reading::Int(value, what) => println!("  {offset:>4}  {value:>12}   {what}"),
            Reading::Nothing => {}
        }
    }

    println!("\nEvery line above is a candidate, not a conclusion. Confirm each");
    println!("against a number you could see in the game when the capture was");
    println!("taken — a field this agrees with and the game does not is a field");
    println!("in the wrong place.");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hex_is_pulled_out_of_whatever_surrounds_it() {
        let payload = "0a0b0c0d".repeat(2);
        let pasted = format!("const PHYSICS_PAGE_HEX: &str = concat!(\n    \"{payload}\",\n);");
        assert_eq!(hex_bytes(&pasted).len(), 8);
        assert_eq!(hex_bytes(&pasted)[..4], [0x0a, 0x0b, 0x0c, 0x0d]);
    }

    /// The bug this tool shipped with for one run: prose is full of hex.
    ///
    /// `acc`, `added` and `face` are all valid hex digits, and swallowing them
    /// shifts every byte after them — a page that is 43 bytes too long and
    /// decodes to nothing. A line of prose contributes nothing now.
    #[test]
    fn prose_is_not_mistaken_for_hex() {
        let payload = "ab".repeat(16);
        let text = format!(
            "Paste into tests_suite, the added face of the capture:\n\n    \"{payload}\",\n"
        );
        assert_eq!(hex_bytes(&text), vec![0xab; 16]);
    }

    /// Short runs and odd runs are not payload either.
    #[test]
    fn a_short_or_odd_line_is_not_payload() {
        assert!(hex_bytes("abc").is_empty());
        assert!(hex_bytes(&"a".repeat(17)).is_empty());
    }

    #[test]
    fn a_tyre_temperature_reads_as_one() {
        let word = 88.5f32.to_le_bytes();
        assert_eq!(judge(word), Reading::Float(88.5, "°C — tyre or air"));
    }

    #[test]
    fn a_lap_time_reads_as_milliseconds() {
        let word = 81_452i32.to_le_bytes();
        assert_eq!(
            judge(word),
            Reading::Int(81_452, "milliseconds — a lap or a sector")
        );
    }

    /// Zero is the one value a capture cannot speak for, so it is never a
    /// candidate: every wrong offset also reads zero.
    #[test]
    fn zero_is_never_a_candidate() {
        assert_eq!(judge([0, 0, 0, 0]), Reading::Nothing);
    }

    /// The four wheels are found together, and a run that changes kind halfway
    /// is not a wheel group.
    #[test]
    fn four_matching_floats_are_found_and_a_mixed_run_is_not() {
        let mut bytes = Vec::new();
        for value in [26.8f32, 27.0, 26.1, 26.3] {
            bytes.extend_from_slice(&value.to_le_bytes());
        }
        assert_eq!(wheels_at(&bytes, 0), Some([26.8, 27.0, 26.1, 26.3]));

        let mut mixed = Vec::new();
        for value in [26.8f32, 27.0, 26.1, 8000.0] {
            mixed.extend_from_slice(&value.to_le_bytes());
        }
        assert_eq!(wheels_at(&mixed, 0), None);
    }
}
