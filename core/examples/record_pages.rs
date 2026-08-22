//! Watch a game's pages for a whole session and write down what moved.
//!
//! `capture_pages` takes one snapshot, and a snapshot has the weakness this
//! project already paid for: a field that happens to be zero at that instant
//! pins nothing, because every wrong offset also reads zero. Assetto Corsa's
//! own graphics capture cannot speak for anything past offset 300 for exactly
//! that reason, and it took a year to notice.
//!
//! A recording does not have that problem. Over a lap, a speed sweeps from 0
//! to 250, a pressure sits between 26 and 28, a flag flips between 0 and 1,
//! and padding never changes at all. **What a four-byte word did over ten
//! minutes identifies it far better than what it held for one frame.**
//!
//! ```text
//! cargo run -p ac_core --example record_pages
//! cargo run -p ac_core --example record_pages -- my-session.txt
//! ```
//!
//! Start it, drive, then close the window or press Ctrl-C. **The report is
//! rewritten every few seconds**, so whatever is on disk is always complete
//! and there is nothing to remember to do at the end — no signal handling,
//! and a hard kill loses at most the last few seconds.
//!
//! # Driving for a good recording
//!
//! Every field only gives itself away when it moves, so in one session:
//!
//! * complete **at least two laps** — several fields stay zero until a lap has
//!   been finished, fuel-per-lap among them;
//! * use the whole speed range, brake hard, and take some kerb;
//! * change **TC and ABS** during the run, turn the lights on, use the pit
//!   limiter, and go through the pit lane once;
//! * finish in the pits with the engine running, so the "in pit" flags move.
//!
//! Anything never touched stays a row of constants in the report, which is
//! itself useful: it says the field is padding, a setting, or something this
//! session never reached.

use std::collections::BTreeMap;
use std::time::{Duration, Instant};

/// Pages Assetto Corsa and Competizione both publish, under the same names.
const DEFAULT_PAGES: &[&str] = &["acpmf_physics", "acpmf_graphics", "acpmf_static"];

/// Fast enough to catch a gear change, slow enough to be free.
const SAMPLE_EVERY: Duration = Duration::from_millis(40);

/// How often the report is rewritten. Short enough that closing the window
/// costs nothing worth having.
const FLUSH_EVERY: Duration = Duration::from_secs(5);

/// Distinct values worth remembering per word before it is simply "many".
const DISTINCT_CAP: usize = 32;

#[cfg(target_os = "windows")]
const WINDOWS_MAP_BYTES: usize = 4096;

/// What one four-byte word did over the whole recording.
struct Word {
    first: u32,
    last: u32,
    min_f: f32,
    max_f: f32,
    min_i: i32,
    max_i: i32,
    distinct: Vec<u32>,
    overflowed: bool,
}

impl Word {
    fn new(raw: u32) -> Self {
        let float = f32::from_le_bytes(raw.to_le_bytes());
        let int = i32::from_le_bytes(raw.to_le_bytes());
        Self {
            first: raw,
            last: raw,
            min_f: float,
            max_f: float,
            min_i: int,
            max_i: int,
            distinct: vec![raw],
            overflowed: false,
        }
    }

    fn see(&mut self, raw: u32) {
        self.last = raw;
        let float = f32::from_le_bytes(raw.to_le_bytes());
        let int = i32::from_le_bytes(raw.to_le_bytes());
        if float.is_finite() {
            self.min_f = self.min_f.min(float);
            self.max_f = self.max_f.max(float);
        }
        self.min_i = self.min_i.min(int);
        self.max_i = self.max_i.max(int);

        if !self.overflowed && !self.distinct.contains(&raw) {
            if self.distinct.len() >= DISTINCT_CAP {
                self.overflowed = true;
            } else {
                self.distinct.push(raw);
            }
        }
    }

    fn moved(&self) -> bool {
        self.overflowed || self.distinct.len() > 1
    }

    /// How many values it took, or "many" once the cap was passed.
    fn spread(&self) -> String {
        if self.overflowed {
            format!("{}+", DISTINCT_CAP)
        } else {
            self.distinct.len().to_string()
        }
    }
}

struct Page {
    name: String,
    bytes: usize,
    words: BTreeMap<usize, Word>,
    last_raw: Vec<u8>,
    samples: u64,
}

impl Page {
    fn see(&mut self, bytes: &[u8]) {
        self.samples += 1;
        self.bytes = self.bytes.max(bytes.len());
        self.last_raw = bytes.to_vec();
        // `as_chunks` rather than `chunks_exact(4)`: the chunk is a fixed-size
        // array, so the four indices below cannot be out of bounds and the
        // compiler knows it. Clippy started asking for this in 1.98.
        for (index, chunk) in bytes.as_chunks::<4>().0.iter().enumerate() {
            let raw = u32::from_le_bytes(*chunk);
            self.words
                .entry(index * 4)
                .and_modify(|word| word.see(raw))
                .or_insert_with(|| Word::new(raw));
        }
    }
}

fn main() {
    let output = std::env::args().nth(1).unwrap_or_else(|| {
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or_default();
        format!("page-recording-{stamp}.txt")
    });

    let mut pages: Vec<Page> = DEFAULT_PAGES
        .iter()
        .map(|name| Page {
            name: (*name).to_string(),
            bytes: 0,
            words: BTreeMap::new(),
            last_raw: Vec::new(),
            samples: 0,
        })
        .collect();

    println!("Recording into {output}");
    println!("Drive. Close this window or press Ctrl-C when you are done —");
    println!(
        "the file is rewritten every {} seconds and is always",
        FLUSH_EVERY.as_secs()
    );
    println!("complete, so there is nothing to do at the end.\n");

    let started = Instant::now();
    let mut last_flush = Instant::now();
    let mut announced = false;

    loop {
        let mut saw_anything = false;
        for page in &mut pages {
            if let Some(bytes) = read_page(&page.name) {
                page.see(&bytes);
                saw_anything = true;
            }
        }

        if saw_anything && !announced {
            println!("Reading. Keep driving.");
            announced = true;
        }

        if last_flush.elapsed() >= FLUSH_EVERY {
            let report = report(&pages, started.elapsed());
            match ac_core::atomic_file::write_atomic(
                std::path::Path::new(&output),
                report.as_bytes(),
            ) {
                Ok(()) => {
                    let samples: u64 = pages.iter().map(|p| p.samples).sum();
                    println!(
                        "  {:>4} s, {samples} samples written to {output}",
                        started.elapsed().as_secs()
                    );
                }
                Err(error) => eprintln!("cannot write {output}: {error}"),
            }
            last_flush = Instant::now();
        }

        std::thread::sleep(SAMPLE_EVERY);
    }
}

#[cfg(not(target_os = "windows"))]
fn read_page(name: &str) -> Option<Vec<u8>> {
    std::fs::read(format!("/dev/shm/{name}")).ok()
}

#[cfg(target_os = "windows")]
fn read_page(name: &str) -> Option<Vec<u8>> {
    use windows::Win32::Foundation::CloseHandle;
    use windows::Win32::System::Memory::{FILE_MAP_READ, MapViewOfFile, OpenFileMappingW};
    use windows::core::HSTRING;

    let full = format!("Local\\{name}");
    unsafe {
        let handle = OpenFileMappingW(FILE_MAP_READ.0, false, &HSTRING::from(full)).ok()?;
        let view = MapViewOfFile(handle, FILE_MAP_READ, 0, 0, WINDOWS_MAP_BYTES);
        let ptr = view.Value as *const u8;
        if ptr.is_null() {
            let _ = CloseHandle(handle);
            return None;
        }
        let bytes = std::slice::from_raw_parts(ptr, WINDOWS_MAP_BYTES).to_vec();
        let _ = CloseHandle(handle);
        Some(bytes)
    }
}

fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write as _;
    let mut out = String::new();
    for chunk in bytes.chunks(32) {
        let mut line = String::with_capacity(64);
        for byte in chunk {
            let _ = write!(line, "{byte:02x}");
        }
        let _ = writeln!(out, "    \"{line}\",");
    }
    out
}

fn report(pages: &[Page], elapsed: Duration) -> String {
    use std::fmt::Write as _;
    let mut out = String::new();

    let _ = writeln!(out, "# Page recording");
    let _ = writeln!(out);
    let _ = writeln!(out, "Recorded for {} seconds.", elapsed.as_secs());
    let _ = writeln!(
        out,
        "Every four-byte word, and what it did. A word that never changed is\n\
         padding, a setting, or something this session never reached — and a\n\
         word that swept a wide range is a live measurement.\n"
    );

    for page in pages {
        let _ = writeln!(out, "\n═══ {} ═══", page.name);
        if page.samples == 0 {
            let _ = writeln!(
                out,
                "never published — the game was not running for this one\n"
            );
            continue;
        }
        let _ = writeln!(out, "{} bytes, {} samples\n", page.bytes, page.samples);

        let moved: Vec<(&usize, &Word)> = page.words.iter().filter(|(_, w)| w.moved()).collect();
        let _ = writeln!(
            out,
            "── words that changed ({} of {}) ──",
            moved.len(),
            page.words.len()
        );
        let _ = writeln!(
            out,
            "{:>6}  {:>14} {:>14}  {:>12} {:>12}  {:>6}",
            "offset", "float min", "float max", "int min", "int max", "values"
        );
        for (offset, word) in moved {
            let _ = writeln!(
                out,
                "{offset:>6}  {:>14.4} {:>14.4}  {:>12} {:>12}  {:>6}",
                word.min_f,
                word.max_f,
                word.min_i,
                word.max_i,
                word.spread()
            );
        }

        let still: Vec<String> = page
            .words
            .iter()
            .filter(|(_, w)| !w.moved())
            .map(|(offset, word)| format!("{offset}={:#010x}", word.first))
            .collect();
        let _ = writeln!(out, "\n── words that never changed ({}) ──", still.len());
        let _ = writeln!(out, "{}", still.join("  "));

        let _ = writeln!(out, "\n── the last sample, as a layout-test constant ──");
        let _ = writeln!(
            out,
            "const {}_PAGE_HEX: &str = concat!(",
            page.name
                .strip_prefix("acpmf_")
                .unwrap_or(&page.name)
                .to_uppercase()
        );
        let _ = write!(out, "{}", hex(&page.last_raw));
        let _ = writeln!(out, ");");
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn word_of(values: &[f32]) -> Word {
        let mut word = Word::new(values[0].to_bits());
        for value in &values[1..] {
            word.see(value.to_bits());
        }
        word
    }

    /// A word that held one value all session is padding or a setting, and
    /// saying so is half the value of a recording: it narrows the search.
    #[test]
    fn a_word_that_never_moved_says_so() {
        let still = word_of(&[27.5, 27.5, 27.5]);
        assert!(!still.moved());

        let moving = word_of(&[0.0, 120.0, 250.0]);
        assert!(moving.moved());
    }

    /// The range is what identifies a field. A speed sweeping 0..250 and a
    /// pressure sitting at 27 are told apart by nothing else.
    #[test]
    fn the_range_is_kept_across_the_session() {
        let word = word_of(&[60.0, 295.0, 120.0]);
        assert_eq!(word.min_f, 60.0);
        assert_eq!(word.max_f, 295.0);
    }

    /// Past the cap it stops counting rather than growing a set per offset for
    /// a session that may run an hour.
    #[test]
    fn counting_distinct_values_gives_up_gracefully() {
        let values: Vec<f32> = (0..DISTINCT_CAP as u32 + 10).map(|i| i as f32).collect();
        let word = word_of(&values);
        assert!(word.overflowed);
        assert_eq!(word.spread(), format!("{DISTINCT_CAP}+"));
    }

    /// A word that is not a number at all — a fragment of a UTF-16 name —
    /// must not poison the float range with a NaN that swallows everything.
    #[test]
    fn a_non_numeric_word_does_not_poison_the_range() {
        let mut word = Word::new(12.0f32.to_bits());
        word.see(f32::NAN.to_bits());
        word.see(20.0f32.to_bits());
        assert_eq!(word.min_f, 12.0);
        assert_eq!(word.max_f, 20.0);
    }
}
