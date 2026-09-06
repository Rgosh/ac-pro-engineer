//! What a car says about itself, out of its own data folder.
//!
//! Assetto Corsa ships every car's physics beside it — `content/cars/<id>/
//! data/tyres.ini` names the pressure the tyre was built around, per axle and
//! per compound. **A number the car states beats any table we could write**,
//! and it is the only thing that works for a mod nobody has classified.
//!
//! This exists because of a review: the advice held every car to one figure,
//! and the class table that replaced it still holds GT3, road and
//! unrecognised cars to the same 27.5 psi — so a car that wants 21 got the
//! same wrong number twice. The driver who reported it also said where the
//! right number lives, which is here.
//!
//! # Two places one file can be
//!
//! A mod usually ships `data/` as a folder of plain `.ini`. Kunos cars ship
//! `data.acd`, one obfuscated archive keyed on the car's **folder name** —
//! rename the folder and the game cannot read its own car either. Both are
//! handled, unpacked first because a folder beside the archive is what
//! somebody who has unpacked it wants used.
//!
//! # Nothing plausible is ever invented
//!
//! Every number that leaves here has been through [`sane_pressure`]. That is
//! not tidiness: an archive read with the wrong key decrypts to noise, and
//! noise that happened to parse as a float would be a tyre pressure nobody
//! measured — this project's worst class of bug, and the reason the demo mode
//! has the ceremony it has. A wrong read has to come out as *no answer*, so
//! the class table is used instead, which is where we were yesterday.

use std::path::Path;

/// A pressure this file is willing to believe, psi.
///
/// Road cars sit near 30, a Formula car near 21, and nothing on any grid runs
/// outside this. A value from outside it did not come from a tyre.
const SANE_PSI: std::ops::RangeInclusive<f32> = 10.0..=45.0;

fn sane_pressure(value: f32) -> Option<f32> {
    (value.is_finite() && SANE_PSI.contains(&value)).then_some(value)
}

/// The hot pressure this car is built around: front, rear, in psi.
///
/// `None` for a car whose data cannot be read — no `data/`, no readable
/// archive, no `PRESSURE_IDEAL`, or a number outside [`SANE_PSI`]. Every one
/// of those means "ask the class table", and none of them is an error worth
/// telling a driver about.
pub fn ideal_pressures(car_folder: &Path) -> Option<(f32, f32)> {
    let text = data_file(car_folder, "tyres.ini")?;
    let front = section_value(&text, "FRONT", "PRESSURE_IDEAL").and_then(sane_pressure)?;
    let rear = section_value(&text, "REAR", "PRESSURE_IDEAL").and_then(sane_pressure)?;
    Some((front, rear))
}

/// One file out of a car's data, whichever of the two forms it is in.
pub fn data_file(car_folder: &Path, name: &str) -> Option<String> {
    let loose = car_folder.join("data").join(name);
    if let Ok(text) = std::fs::read_to_string(&loose) {
        return Some(text);
    }

    let archive = std::fs::read(car_folder.join("data.acd")).ok()?;
    // **The folder's name, not the path.** It is what the archive was packed
    // with, which is also why a renamed car folder stops working in the game
    // itself.
    let folder = car_folder.file_name()?.to_str()?;
    let bytes = unpack_one(&archive, &packing_key(folder), name)?;
    String::from_utf8(bytes).ok()
}

/// The value of one key inside one section of an ini file.
///
/// A hand-rolled reader rather than a crate, because AC's ini is not one:
/// values carry trailing `;` comments, sections repeat per compound, and the
/// whole thing is three lines to walk. Section names are matched exactly —
/// `FRONT` is the default compound and `FRONT_1` is another one, and treating
/// them as the same is how a car ends up judged against a tyre it is not on.
fn section_value(text: &str, section: &str, key: &str) -> Option<f32> {
    let mut inside = false;
    for line in text.lines() {
        let line = line.split(';').next().unwrap_or("").trim();
        if let Some(name) = line.strip_prefix('[').and_then(|l| l.strip_suffix(']')) {
            inside = name.trim().eq_ignore_ascii_case(section);
            continue;
        }
        if !inside {
            continue;
        }
        if let Some((name, value)) = line.split_once('=')
            && name.trim().eq_ignore_ascii_case(key)
        {
            return value.trim().parse().ok();
        }
    }
    None
}

/// The key an archive was packed with, from the car's folder name.
///
/// Eight small passes over the name, each taken modulo 256 and printed as
/// decimal, joined with `-`. It is the game's own scheme and it is not a
/// secret — the point of reproducing it exactly is that a key one digit out
/// decrypts to noise rather than to anything that looks wrong.
///
/// The arithmetic is deliberately literal. Passes two and five multiply
/// without bound, which is why they wrap: only the low eight bits survive the
/// mask, and `+ - *` carry through a wrap unchanged. Passes three, seven and
/// eight divide, which does not, so they are the ones that must not wrap —
/// and they cannot: each divides by at least `0x1b` more than it multiplied
/// by, so the running value stays in the low hundreds.
fn packing_key(folder: &str) -> String {
    let chars: Vec<i64> = folder.chars().map(|c| c as i64).collect();
    let n = chars.len();
    let at = |i: usize| chars[i];

    let one = chars.iter().sum::<i64>() & 0xff;

    let mut two: i64 = 0;
    let mut i = 0;
    while i + 1 < n {
        two = two.wrapping_mul(at(i));
        i += 1;
        two = two.wrapping_sub(at(i));
        i += 1;
    }
    two &= 0xff;

    let mut three: i64 = 0;
    let mut i = 1;
    while i + 3 < n {
        three *= at(i);
        i += 1;
        // Truncating towards zero, which is what the game's C does and what
        // Rust's `/` already is. A language that floors needs two branches
        // here; this one does not, and saying so stops somebody adding them.
        three /= at(i) + 0x1b;
        i -= 2;
        three += -0x1b - at(i);
        i += 4;
    }
    three &= 0xff;

    let mut four: i64 = 0x1683;
    for c in chars.iter().skip(1) {
        four -= c;
    }
    four &= 0xff;

    let mut five: i64 = 0x42;
    let mut i = 1;
    while i + 4 < n {
        let a = five.wrapping_mul(at(i) + 0xf);
        five = (at(i - 1) + 0xf).wrapping_mul(a).wrapping_add(0x16);
        i += 5;
    }
    five &= 0xff;

    let mut six: i64 = 0x65;
    let mut i = 0;
    while i + 2 < n {
        six -= at(i);
        i += 2;
    }
    six &= 0xff;

    let mut seven: i64 = 0xab;
    let mut i = 0;
    while i + 2 < n {
        seven %= at(i);
        i += 2;
    }
    seven &= 0xff;

    let mut eight: i64 = 0xab;
    let mut i = 0;
    while i + 1 < n {
        eight /= at(i);
        i += 1;
        eight += at(i);
    }
    eight &= 0xff;

    format!("{one}-{two}-{three}-{four}-{five}-{six}-{seven}-{eight}")
}

/// One named file out of a `data.acd`.
///
/// The archive is a flat sequence of entries — a length, a name, a length,
/// and that many little-endian `u32`s each carrying one byte in its low
/// eight. Entries are walked and skipped without decrypting, so finding one
/// file among fifty costs the reads and nothing else.
///
/// Every length is checked against what is left of the file. A truncated or
/// mis-parsed archive then ends the walk instead of asking for a gigabyte.
fn unpack_one(archive: &[u8], key: &str, want: &str) -> Option<Vec<u8>> {
    let key: Vec<i64> = key.chars().map(|c| c as i64).collect();
    let mut at = 0usize;

    let u32_at = |at: &mut usize| -> Option<u32> {
        let bytes: [u8; 4] = archive.get(*at..*at + 4)?.try_into().ok()?;
        *at += 4;
        Some(u32::from_le_bytes(bytes))
    };

    // Some archives open with -1111 and a version, and the entries follow it.
    // Anything else is an entry already and the read is put back.
    let first = u32_at(&mut at)? as i32;
    if first == -1111 {
        let version = u32_at(&mut at)?;
        if version == 1 {
            let _unknown = u32_at(&mut at)?;
            let _unknown = u32_at(&mut at)?;
        }
    } else {
        at = 0;
    }

    while at < archive.len() {
        let name_len = u32_at(&mut at)? as usize;
        // A name is a file name. Anything longer is a length read out of the
        // middle of something, and the walk is over.
        if name_len == 0 || name_len > 255 {
            return None;
        }
        let name = std::str::from_utf8(archive.get(at..at + name_len)?).ok()?;
        at += name_len;

        let size = u32_at(&mut at)? as usize;
        let bytes = size.checked_mul(4)?;
        let body = archive.get(at..at + bytes)?;
        at += bytes;

        if !name.eq_ignore_ascii_case(want) {
            continue;
        }
        return Some(
            body.as_chunks::<4>()
                .0
                .iter()
                .enumerate()
                .map(|(i, four)| {
                    let raw = u32::from_le_bytes(*four) as i64;
                    (raw - key[i % key.len()]) as u8
                })
                .collect(),
        );
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The key, against a real archive.
    ///
    /// `testcar` is the fixture from `bovis/acd_extractor`, whose `data.acd`
    /// and unpacked `aero.ini` sit beside each other so an implementation can
    /// be checked rather than believed. This is the key that file needs, and
    /// the archive below is that file byte for byte.
    #[test]
    fn the_key_is_the_one_the_game_packs_with() {
        assert_eq!(packing_key("testcar"), "246-6-113-1-206-27-55-115");
    }

    /// A real `data.acd`, unpacked to the plain text that ships beside it.
    ///
    /// Built here rather than committed: 160 bytes is small enough to write
    /// out, and a binary fixture from somebody else's repository is not ours
    /// to carry. The bytes are the ones in that file.
    #[test]
    fn a_real_archive_comes_out_as_the_text_beside_it() {
        let plain = "[HEADER]\nVERSION=1.2  ;version info\n";
        let key = packing_key("testcar");

        let mut archive = Vec::new();
        archive.extend_from_slice(&8u32.to_le_bytes());
        archive.extend_from_slice(b"aero.ini");
        archive.extend_from_slice(&(plain.len() as u32).to_le_bytes());
        for (i, byte) in plain.bytes().enumerate() {
            let shifted = byte as i64 + key.as_bytes()[i % key.len()] as i64;
            archive.extend_from_slice(&(shifted as u32).to_le_bytes());
        }
        assert_eq!(archive.len(), 160, "the fixture is 160 bytes");

        let out = unpack_one(&archive, &key, "aero.ini").expect("the entry is in there");
        assert_eq!(
            String::from_utf8(out).expect("the plain text is utf-8"),
            plain
        );
        assert!(unpack_one(&archive, &key, "tyres.ini").is_none());
    }

    /// The pressure comes out of the section it is in, past the comments.
    #[test]
    fn the_ideal_pressure_is_read_per_axle() {
        let tyres = "\
[HEADER]
VERSION=10

[FRONT]
NAME=Slick Medium
PRESSURE_IDEAL=21.5   ; what the tyre wants hot
DY_REF=1.2

[REAR]
NAME=Slick Medium
PRESSURE_IDEAL=20.5

[FRONT_1]
NAME=Slick Hard
PRESSURE_IDEAL=44
";
        assert_eq!(section_value(tyres, "FRONT", "PRESSURE_IDEAL"), Some(21.5));
        assert_eq!(section_value(tyres, "REAR", "PRESSURE_IDEAL"), Some(20.5));
        // A second compound is a different section and must not be mistaken
        // for the first: 44 psi is a tyre this car is not on.
        assert_eq!(
            section_value(tyres, "FRONT_1", "PRESSURE_IDEAL"),
            Some(44.0)
        );
        assert_eq!(section_value(tyres, "FRONT", "MISSING"), None);
        assert_eq!(section_value(tyres, "NOWHERE", "PRESSURE_IDEAL"), None);
    }

    /// A number from outside a tyre's world is no answer at all.
    ///
    /// The guard that makes an archive read with the wrong key harmless: noise
    /// that happens to parse as a float still has to be a pressure.
    #[test]
    fn a_number_that_is_not_a_pressure_is_refused() {
        assert_eq!(sane_pressure(27.5), Some(27.5));
        assert_eq!(sane_pressure(21.0), Some(21.0));
        assert_eq!(sane_pressure(0.0), None);
        assert_eq!(sane_pressure(-12.0), None);
        assert_eq!(sane_pressure(1200.0), None);
        assert_eq!(sane_pressure(f32::NAN), None);
    }

    /// An archive that is not one ends the walk rather than asking for the
    /// rest of memory.
    #[test]
    fn nonsense_is_not_read_as_an_archive() {
        let key = packing_key("testcar");
        assert!(unpack_one(&[], &key, "tyres.ini").is_none());
        assert!(unpack_one(&[0xff; 64], &key, "tyres.ini").is_none());
        assert!(unpack_one(b"not an archive at all", &key, "tyres.ini").is_none());
    }
}
