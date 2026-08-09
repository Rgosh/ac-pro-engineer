//! The struct shared with the in-game Lua overlay.
//!
//! This is a wire format between two programs written in different languages
//! and built by different toolchains, so nothing catches a mismatch at compile
//! time. Three rules keep it honest:
//!
//! 1. `#[repr(C)]`, and the Lua side declares `ac.StructItem.explicitOrder`.
//!    CSP reorders structure fields for optimal packing by default, and its
//!    order will not match ours.
//! 2. Fixed-size scalars only. No pointers, no `String`, no `Vec` — text
//!    travels as fixed byte arrays.
//! 3. Fields grouped by alignment, largest first, so padding cannot differ
//!    between the two declarations.
//!
//! [`layout`] pins the size and every offset in a test, and
//! `tools/gen_lua_layout` emits the Lua declaration from this file so the two
//! cannot drift apart by hand.

use crate::engineer::{Recommendation, Severity};
use crate::session_info::SessionInfo;

/// Bumped whenever the layout changes. The overlay refuses to draw a version
/// it does not recognise rather than misreading a struct from another release.
pub const OVERLAY_VERSION: u32 = 6;

/// Shared memory name. The `AcTools.CSP.Limited.` prefix matters: CSP allows
/// scripts without IO permission to open shared memory only when the name
/// starts with it, so this works whether or not the app is granted IO.
pub const OVERLAY_MMF_NAME: &str = "AcTools.CSP.Limited.ACPE.v1";

/// Longest advice line carried to the overlay, in bytes. UTF-8, NUL-padded.
pub const MESSAGE_BYTES: usize = 64;

/// How many advice lines fit.
///
/// Four for eleven releases, because four is what a panel in the corner of a
/// windscreen has room for. That was the wrong place to make the decision: a
/// driver on a triple-screen setup, or one who keeps the advice window open on
/// a second monitor, has room for more, and the panel already has a control for
/// how many to draw — it was simply pinned to the number of slots in the wire
/// format. Eight is the cap now; how many of them are actually published is a
/// setting on the application's side and how many are drawn is one on the
/// panel's.
pub const MESSAGE_SLOTS: usize = 8;

/// How many finished laps travel with their debrief.
///
/// The panel switches between them on its own. It has to: the frame goes one
/// way, the panel has no channel back to ask for a different lap, and giving it
/// one would mean a second mapping and a protocol to go with it. Publishing the
/// last few laps costs a kilobyte and no protocol at all.
///
/// Three, because a debrief is read in the pits about the stint you have just
/// done. The terminal keeps every lap of the session and always will; this is
/// the recent end of it, at a glance, without taking the helmet off.
pub const DEBRIEF_LAPS: usize = 3;

/// Lines of advice carried per finished lap.
///
/// Eight, the same as the live engineer, and for a reason that only showed up
/// once this was driven: a lap can go wrong in more than four ways at once, and
/// four slots meant the engineer quietly dropped whatever came fifth. The live
/// advice has the same cap for the same reason. How many are *drawn* is the
/// driver's setting; how many exist is not something the wire format should be
/// deciding.
pub const DEBRIEF_LINES: usize = 8;

/// Total debrief slots, laps times lines. Flat rather than nested: the panel
/// needs one named field per string, so the generator writes out
/// `debrief_0_0` .. `debrief_2_3` and the index arithmetic lives in one place.
pub const DEBRIEF_SLOTS: usize = DEBRIEF_LAPS * DEBRIEF_LINES;

/// Sectors per lap. AC's own tracks are almost all three, and the shared
/// memory carries three regardless of what the track actually has.
pub const SECTORS: usize = 3;

/// Room for the application's version string, e.g. `0.3.4`. NUL-padded.
///
/// Sixteen rather than eight: a prerelease tag like `0.4.0-beta.10` is
/// thirteen characters, and a version that arrives truncated is worse than no
/// version at all — the panel would report an update that does not exist.
pub const VERSION_BYTES: usize = 16;

/// One frame of everything the overlay draws.
///
/// Written by the application once per tick, read by the Lua app once per
/// rendered frame. Around half a kilobyte, so a write is a single memcpy and
/// the cost against the ~9 MB/s already read out of AC's own pages is
/// immaterial.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct OverlayFrame {
    // --- 4-byte header ---
    /// [`OVERLAY_VERSION`]. First field so a reader can check it before
    /// trusting anything else.
    pub version: u32,

    /// Sequence lock. Odd while a write is in progress, even when settled, and
    /// incremented by two per update.
    ///
    /// Doubles as the liveness signal: the overlay hides itself when this
    /// stops advancing, which is how "only visible while the application is
    /// running" is implemented without any extra channel.
    pub sequence: u32,

    // --- floats ---
    pub speed_kmh: f32,
    pub fuel_litres: f32,
    pub fuel_laps_remaining: f32,
    pub fuel_per_lap: f32,
    pub delta_seconds: f32,
    pub air_temp_c: f32,
    pub road_temp_c: f32,
    pub surface_grip: f32,

    pub tyre_pressure_psi: [f32; 4],
    pub tyre_temp_c: [f32; 4],
    pub tyre_wear_percent: [f32; 4],
    pub brake_temp_c: [f32; 4],

    // --- integers ---
    pub rpm: i32,
    pub max_rpm: i32,
    /// Already translated out of AC's convention: -1 reverse, 0 neutral,
    /// 1.. forward. The overlay should not have to know AC encodes reverse
    /// as 0.
    pub gear: i32,
    pub lap_count: i32,
    pub last_lap_ms: i32,
    pub best_lap_ms: i32,
    pub current_lap_ms: i32,
    pub position: i32,

    /// Bit flags, see [`flags`].
    pub flags: u32,

    /// How many of [`Self::messages`] are populated.
    pub message_count: u32,

    /// Hot pressure the driver is aiming for, front and rear, in psi.
    ///
    /// The panel showed what the pressure is; this is what it should be. The
    /// question in the car is the difference between them, and only the
    /// application knows the target — it is a setting, not a measurement.
    pub target_pressure_front: f32,
    pub target_pressure_rear: f32,

    /// Engineer advice, most severe first. UTF-8, NUL-padded, truncated on a
    /// character boundary.
    pub messages: [[u8; MESSAGE_BYTES]; MESSAGE_SLOTS],

    /// How serious each message is: 0 info, 1 warning, 2 critical.
    ///
    /// The text alone cannot carry this. The application colours advice by
    /// severity and the overlay has to be able to do the same, or the same
    /// sentence means one thing on the desktop and another in the car.
    pub message_severity: [u32; MESSAGE_SLOTS],

    /// The application's release, e.g. `0.3.4`. UTF-8, NUL-padded.
    ///
    /// So the panel can tell whether it is the one this application ships.
    /// The application installs the panel at startup, but a game that was
    /// already running has the previous copy loaded and will keep drawing it
    /// until it restarts — which is invisible from both sides otherwise, and
    /// is exactly when someone reports a bug against a version that is no
    /// longer installed.
    ///
    /// A field rather than a flag, unlike everything else that could have been
    /// one: a flag could say "you are old" but not *what to expect*, and the
    /// application cannot compute the comparison itself — it does not know
    /// which copy of the panel the game happens to have in memory. Only the
    /// panel knows that, and only if it is told what the current version is.
    ///
    /// Was last in the struct, and everything added since has gone after it
    /// for the same reason: a field appended to the end moves no offset before
    /// it.
    pub app_version: [u8; VERSION_BYTES],

    // --- the debrief ------------------------------------------------------
    //
    // Everything below is about laps that are over, and all of it is appended
    // rather than woven in, so every offset above is exactly where v5 left it.
    /// How many of the debrief laps carry anything, newest at index 0.
    pub debrief_lap_count: u32,

    /// Which lap each debrief is about, as the driver counts them.
    pub debrief_lap_number: [u32; DEBRIEF_LAPS],

    /// The lap time, in milliseconds. Zero when it is not known — an out lap
    /// that AC never timed, mostly.
    pub debrief_lap_time_ms: [u32; DEBRIEF_LAPS],

    /// How many lines of that lap's debrief are filled in.
    pub debrief_line_count: [u32; DEBRIEF_LAPS],

    /// Severity per line, laid out the same way as [`Self::debrief`].
    pub debrief_severity: [u32; DEBRIEF_SLOTS],

    /// The debrief itself: lap `l` line `n` is at `l * DEBRIEF_LINES + n`.
    /// UTF-8, NUL-padded, truncated on a character boundary.
    pub debrief: [[u8; MESSAGE_BYTES]; DEBRIEF_SLOTS],

    // --- everything a debrief is read *alongside* --------------------------
    //
    // Appended in one go rather than a field at a time across releases: each
    // change to this struct costs every Linux driver a bridge update, and
    // three of those in a row for three small features is three chances to
    // end up with a panel that waits forever.
    /// Sector times for each published lap, `lap * SECTORS + sector`, in
    /// milliseconds. Zero for a sector the game never timed.
    pub debrief_sector_ms: [u32; DEBRIEF_LAPS * SECTORS],

    /// The best each sector has been this session, for the debrief to measure
    /// against. Theoretical rather than achieved — it is the comparison a
    /// driver makes anyway, and "you lost 0.4 in sector three" is worth more
    /// than the same four tenths spread across a lap.
    pub best_sector_ms: [u32; SECTORS],

    /// Inner and outer tyre surface temperatures. The middle one has always
    /// been [`Self::tyre_temp_c`]; without the other two the panel could show
    /// how hot a tyre is and not whether it is leaning the right way, which is
    /// the reading the camber advice is made of.
    pub tyre_temp_inner_c: [f32; 4],
    pub tyre_temp_outer_c: [f32; 4],

    /// Laps of life left in each tyre at the current rate. Negative means not
    /// measured yet — a stint has to be under way before a rate exists, and
    /// zero is a real answer meaning the tyre is finished.
    pub tyre_laps_remaining: [f32; 4],

    /// Laps completed on this set of tyres, since the last time the car left
    /// the pits. Not the same as the lap count: a driver wants to know how old
    /// the tyres are, not how far into the race it is.
    pub stint_laps: u32,
}

/// One finished lap and what the engineer made of it, ready for the frame.
///
/// A small owned struct rather than a borrow of `LapData`: what the panel needs
/// is the number, the time and the sentences, and carrying the whole lap —
/// telemetry trace included — through the publisher would mean the frame writer
/// held a reference into the analyser for as long as it took to copy 768 bytes.
#[derive(Debug, Clone)]
pub struct DebriefLap {
    pub lap_number: u32,
    pub lap_time_ms: u32,
    /// The lap's sectors, in milliseconds. Zero for one the game never timed.
    pub sectors: [u32; SECTORS],
    pub advice: Vec<Recommendation>,
}

/// Severity as it travels in [`OverlayFrame::message_severity`].
pub mod severity {
    pub const INFO: u32 = 0;
    pub const WARNING: u32 = 1;
    pub const CRITICAL: u32 = 2;
}

/// Bit positions in [`OverlayFrame::flags`].
pub mod flags {
    /// The pit limiter is engaged.
    pub const PIT_LIMITER: u32 = 1 << 0;
    /// Telemetry is being read from the game, as opposed to the last known
    /// values being held.
    pub const CONNECTED: u32 = 1 << 1;
    /// The user asked for the tyre panel.
    pub const SHOW_TELEMETRY: u32 = 1 << 2;
    /// The user asked for the engineer panel.
    pub const SHOW_ENGINEER: u32 = 1 << 3;
    /// Fuel is below the configured warning threshold.
    pub const FUEL_WARNING: u32 = 1 << 4;
    /// The user asked for the session block: position, lap, conditions.
    pub const SHOW_SESSION: u32 = 1 << 5;
    /// The user asked for the lap timing block.
    pub const SHOW_TIMING: u32 = 1 << 6;
    /// The user asked for the fuel block.
    pub const SHOW_FUEL: u32 = 1 << 7;
    /// The application is running in Russian, so the panel should be too.
    ///
    /// A flag rather than a field: the application has two languages, and the
    /// panel's own words should follow the ones it already receives translated.
    pub const RUSSIAN: u32 = 1 << 8;
}

impl Default for OverlayFrame {
    fn default() -> Self {
        Self::empty()
    }
}

impl OverlayFrame {
    /// A frame with nothing in it but the versions.
    ///
    /// `sequence` starts at zero, which reads as "never written" to the
    /// overlay — so a mapping that exists but has never been filled does not
    /// draw stale zeroes.
    ///
    /// `app_version` is filled here rather than by the caller: every frame
    /// carries it, and a publisher that had to remember to set it would
    /// eventually publish one that did not — which the panel would read as a
    /// version mismatch and refuse to draw.
    pub const fn empty() -> Self {
        Self {
            version: OVERLAY_VERSION,
            app_version: version_bytes(),
            sequence: 0,
            speed_kmh: 0.0,
            fuel_litres: 0.0,
            fuel_laps_remaining: 0.0,
            fuel_per_lap: 0.0,
            delta_seconds: 0.0,
            air_temp_c: 0.0,
            road_temp_c: 0.0,
            surface_grip: 0.0,
            tyre_pressure_psi: [0.0; 4],
            tyre_temp_c: [0.0; 4],
            tyre_wear_percent: [0.0; 4],
            brake_temp_c: [0.0; 4],
            rpm: 0,
            max_rpm: 0,
            gear: 0,
            lap_count: 0,
            last_lap_ms: 0,
            best_lap_ms: 0,
            current_lap_ms: 0,
            position: 0,
            flags: 0,
            message_count: 0,
            messages: [[0; MESSAGE_BYTES]; MESSAGE_SLOTS],
            target_pressure_front: 0.0,
            target_pressure_rear: 0.0,
            message_severity: [0; MESSAGE_SLOTS],
            debrief_lap_count: 0,
            debrief_lap_number: [0; DEBRIEF_LAPS],
            debrief_lap_time_ms: [0; DEBRIEF_LAPS],
            debrief_line_count: [0; DEBRIEF_LAPS],
            debrief_severity: [0; DEBRIEF_SLOTS],
            debrief: [[0; MESSAGE_BYTES]; DEBRIEF_SLOTS],
            debrief_sector_ms: [0; DEBRIEF_LAPS * SECTORS],
            best_sector_ms: [0; SECTORS],
            tyre_temp_inner_c: [0.0; 4],
            tyre_temp_outer_c: [0.0; 4],
            tyre_laps_remaining: [-1.0; 4],
            stint_laps: 0,
        }
    }

    /// Set or clear a flag.
    pub fn set_flag(&mut self, flag: u32, on: bool) {
        if on {
            self.flags |= flag;
        } else {
            self.flags &= !flag;
        }
    }

    pub fn has_flag(&self, flag: u32) -> bool {
        self.flags & flag != 0
    }

    /// Copy the engineer's advice in, truncating to what the overlay can show.
    ///
    /// Truncation is on a character boundary: cutting a multi-byte character
    /// in half would leave the Lua side with invalid UTF-8, and Russian advice
    /// is two bytes per character throughout.
    pub fn set_messages(&mut self, recommendations: &[Recommendation]) {
        self.set_messages_capped(recommendations, MESSAGE_SLOTS);
    }

    /// As [`Self::set_messages`], but publishing at most `limit` of them.
    ///
    /// The cap is a setting rather than a property of the data: eight lines is
    /// a paragraph in a headset and a glance on a monitor, and the overlay
    /// cannot make that call — it does not know how the panel is being read.
    pub fn set_messages_capped(&mut self, recommendations: &[Recommendation], limit: usize) {
        self.messages = [[0; MESSAGE_BYTES]; MESSAGE_SLOTS];
        self.message_severity = [severity::INFO; MESSAGE_SLOTS];
        let limit = limit.min(MESSAGE_SLOTS);
        let taken = recommendations.len().min(limit);

        for (index, rec) in recommendations.iter().take(limit).enumerate() {
            let text = truncate_on_boundary(&rec.message, MESSAGE_BYTES);
            self.messages[index][..text.len()].copy_from_slice(text.as_bytes());
            self.message_severity[index] = match rec.severity {
                Severity::Critical => severity::CRITICAL,
                Severity::Warning => severity::WARNING,
                Severity::Info => severity::INFO,
            };
        }

        self.message_count = taken as u32;
    }

    /// Copy in the debrief for the most recent finished laps, newest first.
    ///
    /// Each entry is a lap number, its time, and that lap's advice. The panel
    /// switches between them itself — there is no way for it to ask for a
    /// different one, so everything it might show has to already be here.
    ///
    /// `lines_per_lap` caps how many lines of each lap travel, the same way the
    /// live advice is capped: how much a panel can hold is the panel's
    /// business, and the application cannot see it.
    pub fn set_debrief(&mut self, laps: &[DebriefLap], lines_per_lap: usize) {
        self.debrief = [[0; MESSAGE_BYTES]; DEBRIEF_SLOTS];
        self.debrief_severity = [severity::INFO; DEBRIEF_SLOTS];
        self.debrief_lap_number = [0; DEBRIEF_LAPS];
        self.debrief_lap_time_ms = [0; DEBRIEF_LAPS];
        self.debrief_line_count = [0; DEBRIEF_LAPS];

        let lines_per_lap = lines_per_lap.min(DEBRIEF_LINES);
        let taken = laps.len().min(DEBRIEF_LAPS);

        for (lap_index, lap) in laps.iter().take(DEBRIEF_LAPS).enumerate() {
            self.debrief_lap_number[lap_index] = lap.lap_number;
            self.debrief_lap_time_ms[lap_index] = lap.lap_time_ms;

            let mut written = 0;
            for rec in lap.advice.iter().take(lines_per_lap) {
                let slot = lap_index * DEBRIEF_LINES + written;
                let text = truncate_on_boundary(&rec.message, MESSAGE_BYTES);
                self.debrief[slot][..text.len()].copy_from_slice(text.as_bytes());
                self.debrief_severity[slot] = match rec.severity {
                    Severity::Critical => severity::CRITICAL,
                    Severity::Warning => severity::WARNING,
                    Severity::Info => severity::INFO,
                };
                written += 1;
            }
            self.debrief_line_count[lap_index] = written as u32;
        }

        self.debrief_lap_count = taken as u32;
    }

    /// Sector times for the published laps, and the best each has been.
    ///
    /// Separate from `set_debrief` because they come from a different place:
    /// the advice is the engineer's, these are the analyser's, and a caller
    /// that has one does not always have the other.
    pub fn set_sectors(&mut self, laps: &[DebriefLap], best: [u32; SECTORS]) {
        self.debrief_sector_ms = [0; DEBRIEF_LAPS * SECTORS];
        for (lap_index, lap) in laps.iter().take(DEBRIEF_LAPS).enumerate() {
            for sector in 0..SECTORS {
                self.debrief_sector_ms[lap_index * SECTORS + sector] = lap.sectors[sector];
            }
        }
        self.best_sector_ms = best;
    }

    /// Fill in the parts that come from the session rather than from physics.
    pub fn apply_session(&mut self, session: &SessionInfo) {
        self.max_rpm = session.max_rpm;
        self.lap_count = session.lap_count;
    }
}

/// This crate's version as a NUL-padded fixed array.
///
/// `const` so [`OverlayFrame::empty`] stays `const` and every frame carries it
/// without a runtime copy. Written as a byte loop because `copy_from_slice` is
/// not available in a const context.
const fn version_bytes() -> [u8; VERSION_BYTES] {
    let source = env!("CARGO_PKG_VERSION").as_bytes();
    let mut out = [0u8; VERSION_BYTES];
    let mut index = 0;
    // Truncating rather than failing to build: a version longer than this is a
    // release-naming problem, and a panel that shows a clipped version is a
    // smaller one than a workspace that will not compile.
    while index < source.len() && index < VERSION_BYTES {
        out[index] = source[index];
        index += 1;
    }
    out
}

/// Read a NUL-padded fixed array back as a string.
pub fn read_fixed_string(bytes: &[u8]) -> &str {
    let end = bytes.iter().position(|b| *b == 0).unwrap_or(bytes.len());
    std::str::from_utf8(&bytes[..end]).unwrap_or("")
}

/// Longest prefix of `text` that fits in `limit` bytes without splitting a
/// character.
fn truncate_on_boundary(text: &str, limit: usize) -> &str {
    if text.len() <= limit {
        return text;
    }
    let mut end = limit;
    while end > 0 && !text.is_char_boundary(end) {
        end -= 1;
    }
    &text[..end]
}

/// Emit the `ac.StructItem` declaration matching [`OverlayFrame`].
///
/// The Lua side has to describe this layout independently, and two
/// hand-maintained copies of one structure drift — the only question is when.
/// This generates the Lua from the Rust, and a test compares the result
/// against the checked-in app so a field added on one side cannot ship without
/// the other.
pub fn lua_struct_declaration() -> String {
    let mut out = String::new();
    out.push_str("-- GENERATED by ac_core::overlay::frame::lua_struct_declaration.\n");
    out.push_str("-- Do not edit by hand: `cargo test -p ac_core lua_layout` checks it.\n");
    out.push_str("local FRAME_LAYOUT = {\n");
    // Without this CSP reorders fields for its own packing and nothing lines
    // up. It is the single most important line in the file.
    //
    // Named `explicit`, not `explicitOrder` — checked against the SDK shipped
    // with CSP in extension/internal/lua-sdk/ac_apps/lib.lua. Calling the
    // wrong name is a nil call at load time, which takes the whole app down.
    out.push_str("  ac.StructItem.explicit(4, 4),\n\n");

    for (name, kind) in FIELDS {
        out.push_str(&format!("  {name} = {kind},\n"));
    }

    out.push_str("}\n\n");
    // `require` takes the returned value, so without this the app gets nil
    // and every field read is an index-a-nil error at load time.
    out.push_str("return FRAME_LAYOUT\n");
    out
}

/// Every field of [`OverlayFrame`], in declaration order, with its
/// `ac.StructItem` type.
///
/// Kept next to the struct so adding a field without adding it here fails the
/// size check in the tests below.
const FIELDS: &[(&str, &str)] = &[
    ("version", "ac.StructItem.uint32()"),
    ("sequence", "ac.StructItem.uint32()"),
    ("speed_kmh", "ac.StructItem.float()"),
    ("fuel_litres", "ac.StructItem.float()"),
    ("fuel_laps_remaining", "ac.StructItem.float()"),
    ("fuel_per_lap", "ac.StructItem.float()"),
    ("delta_seconds", "ac.StructItem.float()"),
    ("air_temp_c", "ac.StructItem.float()"),
    ("road_temp_c", "ac.StructItem.float()"),
    ("surface_grip", "ac.StructItem.float()"),
    (
        "tyre_pressure_psi",
        "ac.StructItem.array(ac.StructItem.float(), 4)",
    ),
    (
        "tyre_temp_c",
        "ac.StructItem.array(ac.StructItem.float(), 4)",
    ),
    (
        "tyre_wear_percent",
        "ac.StructItem.array(ac.StructItem.float(), 4)",
    ),
    (
        "brake_temp_c",
        "ac.StructItem.array(ac.StructItem.float(), 4)",
    ),
    ("rpm", "ac.StructItem.int32()"),
    ("max_rpm", "ac.StructItem.int32()"),
    ("gear", "ac.StructItem.int32()"),
    ("lap_count", "ac.StructItem.int32()"),
    ("last_lap_ms", "ac.StructItem.int32()"),
    ("best_lap_ms", "ac.StructItem.int32()"),
    ("current_lap_ms", "ac.StructItem.int32()"),
    ("position", "ac.StructItem.int32()"),
    ("flags", "ac.StructItem.uint32()"),
    ("message_count", "ac.StructItem.uint32()"),
    // Named fields rather than an array of them: CSP hands back raw cdata for
    // an array of strings, which reaches the panel as `cdata<char (&)[64]>`
    // where the engineer's advice should be. Same bytes, same offsets, and each
    // one reads as a Lua string. One line per slot, so `MESSAGE_SLOTS` and this
    // list have to be changed together — `the_generator_lists_every_field`
    // fails if they are not.
    ("target_pressure_front", "ac.StructItem.float()"),
    ("target_pressure_rear", "ac.StructItem.float()"),
    ("message_0", "ac.StructItem.string(64)"),
    ("message_1", "ac.StructItem.string(64)"),
    ("message_2", "ac.StructItem.string(64)"),
    ("message_3", "ac.StructItem.string(64)"),
    ("message_4", "ac.StructItem.string(64)"),
    ("message_5", "ac.StructItem.string(64)"),
    ("message_6", "ac.StructItem.string(64)"),
    ("message_7", "ac.StructItem.string(64)"),
    (
        "message_severity",
        "ac.StructItem.array(ac.StructItem.uint32(), 8)",
    ),
    ("app_version", "ac.StructItem.string(16)"),
    ("debrief_lap_count", "ac.StructItem.uint32()"),
    (
        "debrief_lap_number",
        "ac.StructItem.array(ac.StructItem.uint32(), 3)",
    ),
    (
        "debrief_lap_time_ms",
        "ac.StructItem.array(ac.StructItem.uint32(), 3)",
    ),
    (
        "debrief_line_count",
        "ac.StructItem.array(ac.StructItem.uint32(), 3)",
    ),
    (
        "debrief_severity",
        "ac.StructItem.array(ac.StructItem.uint32(), 24)",
    ),
    // One named field per line, for the reason the messages above are named:
    // CSP hands back raw cdata for an array of strings. `debrief_<lap>_<line>`,
    // so `DEBRIEF_LAPS` and `DEBRIEF_LINES` and this list change together.
    ("debrief_0_0", "ac.StructItem.string(64)"),
    ("debrief_0_1", "ac.StructItem.string(64)"),
    ("debrief_0_2", "ac.StructItem.string(64)"),
    ("debrief_0_3", "ac.StructItem.string(64)"),
    ("debrief_0_4", "ac.StructItem.string(64)"),
    ("debrief_0_5", "ac.StructItem.string(64)"),
    ("debrief_0_6", "ac.StructItem.string(64)"),
    ("debrief_0_7", "ac.StructItem.string(64)"),
    ("debrief_1_0", "ac.StructItem.string(64)"),
    ("debrief_1_1", "ac.StructItem.string(64)"),
    ("debrief_1_2", "ac.StructItem.string(64)"),
    ("debrief_1_3", "ac.StructItem.string(64)"),
    ("debrief_1_4", "ac.StructItem.string(64)"),
    ("debrief_1_5", "ac.StructItem.string(64)"),
    ("debrief_1_6", "ac.StructItem.string(64)"),
    ("debrief_1_7", "ac.StructItem.string(64)"),
    ("debrief_2_0", "ac.StructItem.string(64)"),
    ("debrief_2_1", "ac.StructItem.string(64)"),
    ("debrief_2_2", "ac.StructItem.string(64)"),
    ("debrief_2_3", "ac.StructItem.string(64)"),
    ("debrief_2_4", "ac.StructItem.string(64)"),
    ("debrief_2_5", "ac.StructItem.string(64)"),
    ("debrief_2_6", "ac.StructItem.string(64)"),
    ("debrief_2_7", "ac.StructItem.string(64)"),
    (
        "debrief_sector_ms",
        "ac.StructItem.array(ac.StructItem.uint32(), 9)",
    ),
    (
        "best_sector_ms",
        "ac.StructItem.array(ac.StructItem.uint32(), 3)",
    ),
    (
        "tyre_temp_inner_c",
        "ac.StructItem.array(ac.StructItem.float(), 4)",
    ),
    (
        "tyre_temp_outer_c",
        "ac.StructItem.array(ac.StructItem.float(), 4)",
    ),
    (
        "tyre_laps_remaining",
        "ac.StructItem.array(ac.StructItem.float(), 4)",
    ),
    ("stint_laps", "ac.StructItem.uint32()"),
];

/// How many bytes an `ac.StructItem` declaration occupies.
///
/// Parsed from the declaration rather than matched against known spellings: the
/// version string is `string(16)` where the advice is `string(64)`, and a size
/// table that recognised only the latter counted the new field as four bytes
/// and agreed with itself about a layout that was twelve bytes short.
///
/// Only the layout tests need this — the generator emits the declarations
/// verbatim and never has to know how wide they are.
#[cfg(test)]
fn declared_size(kind: &str) -> usize {
    if let Some(rest) = kind.split("string(").nth(1)
        && let Some(count) = rest.split(')').next()
        && let Ok(count) = count.parse::<usize>()
    {
        return count;
    }
    if let Some(rest) = kind.split("), ").nth(1)
        && let Some(count) = rest.split(')').next()
        && let Ok(count) = count.trim().parse::<usize>()
    {
        return count * 4;
    }
    4
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engineer::{Recommendation, Severity};

    fn advice(message: &str) -> Recommendation {
        Recommendation {
            component: "Tyres".to_string(),
            category: "Pressure".to_string(),
            severity: Severity::Warning,
            message: message.to_string(),
            action: String::new(),
            parameters: Vec::new(),
            confidence: 1.0,
        }
    }

    /// The Lua side declares this layout independently, so its size is part of
    /// the contract. A change here without a matching change there produces a
    /// silently misaligned read, not an error.
    #[test]
    fn the_layout_is_what_the_lua_side_declares() {
        use std::mem::offset_of;

        assert_eq!(offset_of!(OverlayFrame, version), 0);
        assert_eq!(offset_of!(OverlayFrame, sequence), 4);
        assert_eq!(offset_of!(OverlayFrame, speed_kmh), 8);

        // Every field is 4-byte aligned, so the struct must have no padding
        // at all. If this ever fails, a field of a different size was added
        // and the Lua declaration needs the same treatment.
        assert_eq!(
            size_of::<OverlayFrame>() % 4,
            0,
            "no field may introduce padding"
        );
        assert_eq!(align_of::<OverlayFrame>(), 4);

        // Counted rather than hardcoded, so adding a field updates this
        // deliberately: 22 scalars + 16 array floats + the message block.
        // 2 header + 8 floats + 8 integers + 2 counters + 2 targets = 22.
        let scalars = 22 * 4;
        let arrays = 16 * 4;
        let messages = MESSAGE_SLOTS * MESSAGE_BYTES;
        let severities = MESSAGE_SLOTS * 4;
        // The debrief: a lap counter, three arrays of one u32 per lap, a
        // severity per slot, and the sentences themselves.
        let debrief_counters = 4 + DEBRIEF_LAPS * 3 * 4;
        let debrief_severities = DEBRIEF_SLOTS * 4;
        let debrief_text = DEBRIEF_SLOTS * MESSAGE_BYTES;
        // Sector times per lap, the session's best sectors, the two tyre
        // surface temperatures the middle one was always missing, and the wear
        // projection.
        let alongside = (DEBRIEF_LAPS * SECTORS + SECTORS) * 4 + (4 + 4 + 4) * 4 + 4;
        assert_eq!(
            size_of::<OverlayFrame>(),
            scalars
                + arrays
                + messages
                + severities
                + VERSION_BYTES
                + debrief_counters
                + debrief_severities
                + debrief_text
                + alongside
        );

        // Everything new goes after `app_version`, never before it: a field
        // inserted earlier moves every offset behind it, and a panel or bridge
        // one version behind then misreads the lot rather than simply not
        // seeing the new part.
        assert_eq!(
            offset_of!(OverlayFrame, app_version),
            scalars + arrays + messages + severities,
            "app_version has to stay where v5 left it"
        );
        assert_eq!(
            offset_of!(OverlayFrame, debrief_lap_count),
            scalars + arrays + messages + severities + VERSION_BYTES,
            "the debrief is appended, so it starts where the struct used to end"
        );
    }

    /// The struct's order and the generator's list have to be the same order.
    ///
    /// Size and count both matched while the targets sat after the messages in
    /// one and before them in the other, and the panel read eight bytes of a
    /// sentence as two pressures.
    #[test]
    fn the_generator_lists_the_fields_in_the_struct_s_order() {
        use std::mem::offset_of;

        let mut offset = 0usize;
        for (name, kind) in FIELDS {
            let size = declared_size(kind);

            let expected = match *name {
                "version" => Some(offset_of!(OverlayFrame, version)),
                "sequence" => Some(offset_of!(OverlayFrame, sequence)),
                "flags" => Some(offset_of!(OverlayFrame, flags)),
                "message_count" => Some(offset_of!(OverlayFrame, message_count)),
                "target_pressure_front" => Some(offset_of!(OverlayFrame, target_pressure_front)),
                "target_pressure_rear" => Some(offset_of!(OverlayFrame, target_pressure_rear)),
                "messages" | "message_0" => Some(offset_of!(OverlayFrame, messages)),
                "message_severity" => Some(offset_of!(OverlayFrame, message_severity)),
                "app_version" => Some(offset_of!(OverlayFrame, app_version)),
                _ => None,
            };

            if let Some(expected) = expected {
                assert_eq!(offset, expected, "{name} is declared at the wrong offset");
            }
            offset += size;
        }
    }

    /// As `advice`, but the severity matters to the caller.
    fn advice_at(message: &str, severity: Severity) -> Recommendation {
        Recommendation {
            severity,
            ..advice(message)
        }
    }

    /// Laps land in their own block of slots. Lap 1 line 0 must not be able to
    /// read as lap 0 line 4 — the panel indexes by arithmetic, so an off-by-one
    /// here shows one lap's advice under another lap's number.
    #[test]
    fn each_lap_gets_its_own_slots() {
        let mut frame = OverlayFrame::empty();
        frame.set_debrief(
            &[
                DebriefLap {
                    lap_number: 7,
                    lap_time_ms: 91_234,
                    sectors: [0; crate::overlay::frame::SECTORS],
                    advice: vec![advice_at("newest", Severity::Critical)],
                },
                DebriefLap {
                    lap_number: 6,
                    lap_time_ms: 92_000,
                    sectors: [0; crate::overlay::frame::SECTORS],
                    advice: vec![advice_at("older", Severity::Info)],
                },
            ],
            DEBRIEF_LINES,
        );

        assert_eq!(frame.debrief_lap_count, 2);
        assert_eq!(frame.debrief_lap_number[0], 7);
        assert_eq!(frame.debrief_lap_time_ms[0], 91_234);
        assert_eq!(frame.debrief_line_count[0], 1);
        assert!(String::from_utf8_lossy(&frame.debrief[0]).starts_with("newest"));
        assert_eq!(frame.debrief_severity[0], severity::CRITICAL);

        // Lap 1 starts a whole DEBRIEF_LINES further along, not right after
        // whatever lap 0 happened to use.
        assert!(String::from_utf8_lossy(&frame.debrief[DEBRIEF_LINES]).starts_with("older"));
        assert_eq!(frame.debrief_severity[DEBRIEF_LINES], severity::INFO);
        // And the gap between them stays empty.
        assert_eq!(frame.debrief[1], [0; MESSAGE_BYTES]);
    }

    /// The cap is the driver's setting, so it has to actually cap.
    #[test]
    fn a_lap_publishes_only_as_many_lines_as_asked_for() {
        let mut frame = OverlayFrame::empty();
        frame.set_debrief(
            &[DebriefLap {
                lap_number: 1,
                lap_time_ms: 1,
                sectors: [0; crate::overlay::frame::SECTORS],
                advice: vec![advice("one"), advice("two"), advice("three")],
            }],
            2,
        );

        assert_eq!(frame.debrief_line_count[0], 2);
        assert_eq!(
            frame.debrief[2], [0; MESSAGE_BYTES],
            "the third was not written"
        );
    }

    /// A stint that ends leaves the frame holding its last debrief otherwise,
    /// and the panel would draw a lap from the previous session.
    #[test]
    fn a_new_debrief_clears_the_one_before_it() {
        let mut frame = OverlayFrame::empty();
        frame.set_debrief(
            &[DebriefLap {
                lap_number: 4,
                lap_time_ms: 1,
                sectors: [0; crate::overlay::frame::SECTORS],
                advice: vec![advice("stale")],
            }],
            DEBRIEF_LINES,
        );
        frame.set_debrief(&[], DEBRIEF_LINES);

        assert_eq!(frame.debrief_lap_count, 0);
        assert_eq!(frame.debrief_lap_number[0], 0);
        assert_eq!(frame.debrief_line_count[0], 0);
        assert_eq!(frame.debrief[0], [0; MESSAGE_BYTES]);
    }

    /// More laps than the frame carries is a long stint, not an error.
    #[test]
    fn more_laps_than_slots_are_dropped_not_overflowed() {
        let mut frame = OverlayFrame::empty();
        let laps: Vec<DebriefLap> = (0..DEBRIEF_LAPS + 3)
            .map(|index| DebriefLap {
                lap_number: index as u32,
                lap_time_ms: 1,
                sectors: [0; crate::overlay::frame::SECTORS],
                advice: vec![advice("line")],
            })
            .collect();
        frame.set_debrief(&laps, DEBRIEF_LINES);
        assert_eq!(frame.debrief_lap_count as usize, DEBRIEF_LAPS);
    }

    /// Every field must be listed for the generator, or the Lua side silently
    /// omits one and every field after it reads from the wrong offset.
    #[test]
    fn the_generator_lists_every_field() {
        // 22 scalars + 4 arrays + MESSAGE_SLOTS messages + their severities +
        // the application's version, then the debrief: its lap count, four
        // arrays alongside it, and one named string per slot.
        // ... and five more arrays alongside the debrief.
        assert_eq!(
            FIELDS.len(),
            22 + 4 + MESSAGE_SLOTS + 1 + 1 + 1 + 4 + DEBRIEF_SLOTS + 5 + 1
        );

        // The declared types have to add up to the struct's actual size, which
        // is what catches a field added to the struct but not to FIELDS.
        let bytes: usize = FIELDS.iter().map(|(_, kind)| declared_size(kind)).sum();
        assert_eq!(bytes, size_of::<OverlayFrame>());
    }

    /// Without explicitOrder, CSP packs fields in its own order and every
    /// read is misaligned. It is the one line that cannot be omitted.
    #[test]
    fn the_generated_lua_pins_the_field_order() {
        let lua = lua_struct_declaration();
        assert!(lua.contains("ac.StructItem.explicit(4, 4)"));
        assert!(lua.contains("version = ac.StructItem.uint32()"));
        // Named strings rather than an array of them: CSP hands back raw
        // cdata for the array, which the panel cannot print. Every slot, not
        // just the first and the last: a generator that stopped emitting the
        // middle ones would still satisfy those two.
        for slot in 0..MESSAGE_SLOTS {
            assert!(
                lua.contains(&format!("message_{slot} = ac.StructItem.string(64)")),
                "message_{slot} is missing from the generated layout"
            );
        }

        // Order matters as much as presence.
        let version_at = lua.find("version =").expect("version");
        let sequence_at = lua.find("sequence =").expect("sequence");
        let messages_at = lua.find("message_0 =").expect("messages");
        assert!(version_at < sequence_at);
        assert!(sequence_at < messages_at);

        // require() takes the returned value; without this the app loads nil.
        assert!(lua.trim_end().ends_with("return FRAME_LAYOUT"));
    }

    /// shm-bridge hardcodes this size, because it deliberately does not
    /// depend on ac_core — it is a small Windows binary that runs under Wine.
    /// This is the check that keeps the two in step.
    #[test]
    fn the_bridge_knows_the_right_size() {
        let bridge = std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../shm-bridge/src/main.rs"
        ))
        .expect("shm-bridge source");

        let expected = format!("OVERLAY_FILE_SIZE: usize = {};", size_of::<OverlayFrame>());
        assert!(
            bridge.contains(&expected),
            "shm-bridge declares a different size than OverlayFrame ({} bytes); \
             update OVERLAY_FILE_SIZE in shm-bridge/src/main.rs",
            size_of::<OverlayFrame>()
        );

        assert!(
            bridge.contains(OVERLAY_MMF_NAME),
            "shm-bridge must map the same name the writer uses"
        );
    }

    /// Every `ac.StructItem.*` the generator emits must exist in the CSP SDK,
    /// or the app dies with a nil call the moment it loads.
    ///
    /// Skipped when CSP is not installed, because most machines building this
    /// do not have Assetto Corsa on them — but it runs where it can, and it is
    /// the only check that catches an API renamed between CSP versions.
    /// `explicitOrder` vs `explicit` was exactly that, and cost a release.
    #[test]
    fn the_generated_calls_exist_in_the_installed_csp_sdk() {
        let Some(lib) = installed_csp_lib() else {
            eprintln!("CSP not installed; skipping SDK conformance check");
            return;
        };

        let lua = lua_struct_declaration();
        for call in lua
            .lines()
            .filter_map(|line| line.split("ac.StructItem.").nth(1))
            .filter_map(|rest| rest.split('(').next())
        {
            let declaration = format!("function ac.StructItem.{call}(");
            assert!(
                lib.contains(&declaration),
                "the generator emits ac.StructItem.{call}, which this CSP does not define"
            );
        }
    }

    /// The manifest must use keys CSP actually understands, and reference
    /// files that exist.
    ///
    /// Unknown keys are ignored in silence — `SIZE_MIN` instead of `MIN_SIZE`
    /// cost nothing visible — and a missing `ICON` leaves the app without an
    /// entry in the sidebar, which reads as "the script did not load" rather
    /// than as a missing file.
    #[test]
    fn the_app_manifest_matches_what_csp_expects() {
        let app_dir = std::path::Path::new(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../apps/lua/ac_pro_engineer"
        ));
        let manifest =
            std::fs::read_to_string(app_dir.join("manifest.ini")).expect("the app manifest");

        // Keys CSP's own bundled apps use. Anything outside this set is at
        // best ignored.
        const KNOWN: &[&str] = &[
            "NAME",
            "AUTHOR",
            "VERSION",
            "DESCRIPTION",
            "LAZY",
            "ID",
            "ICON",
            "FUNCTION_MAIN",
            "FUNCTION_SETTINGS",
            "FLAGS",
            "MIN_SIZE",
            "SIZE",
            "SCRIPT",
            "PERIOD",
            "EVENT",
        ];

        for line in manifest.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with(';') || line.starts_with('[') {
                continue;
            }
            let Some(key) = line.split('=').next().map(str::trim) else {
                continue;
            };
            assert!(
                KNOWN.contains(&key),
                "manifest key {key} is not one CSP recognises"
            );
        }

        // Every file the manifest points at has to be there.
        for line in manifest.lines() {
            if let Some(icon) = line.trim().strip_prefix("ICON") {
                let name = icon.trim_start_matches([' ', '=']).trim();
                assert!(
                    app_dir.join(name).exists(),
                    "manifest references {name}, which is missing — the app then \
                     has no sidebar entry"
                );
            }
        }

        // CSP finds the entry point by folder name.
        assert!(
            app_dir.join("ac_pro_engineer.lua").exists(),
            "the main script must be named after its folder"
        );
    }

    /// Every `ui.*` the overlay app calls must exist in the installed CSP.
    ///
    /// A missing one is a nil call at draw time, which takes the app's window
    /// down mid-frame — and the only way to find out is to launch the game.
    /// This is that check, run wherever CSP happens to be installed.
    #[test]
    fn the_overlay_app_only_calls_ui_functions_csp_provides() {
        let Some(lib) = installed_csp_lib() else {
            eprintln!("CSP not installed; skipping UI conformance check");
            return;
        };

        // Every file of the panel, not just the entry point. Ten of the eleven
        // `ui.*` calls live in the modules now, and a check pinned to one
        // filename would have gone on passing while checking almost nothing.
        let app_dir = std::path::Path::new(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../apps/lua/ac_pro_engineer"
        ));
        let mut app = String::new();
        let mut stack = vec![app_dir.to_path_buf()];
        while let Some(dir) = stack.pop() {
            let Ok(entries) = std::fs::read_dir(&dir) else {
                continue;
            };
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    stack.push(path);
                } else if path.extension().is_some_and(|e| e == "lua")
                    && let Ok(text) = std::fs::read_to_string(&path)
                {
                    app.push_str(&text);
                    app.push('\n');
                }
            }
        }
        assert!(!app.is_empty(), "the overlay app source");

        let mut checked = 0;
        for line in app.lines() {
            // Skip comments: they name functions in prose.
            if line.trim_start().starts_with("--") {
                continue;
            }
            for call in ui_calls(line) {
                let as_function = format!("function ui.{call}(");
                let as_table = format!("ui.{call} = ");
                assert!(
                    lib.contains(&as_function) || lib.contains(&as_table),
                    "the overlay calls ui.{call}, which this CSP does not define"
                );
                checked += 1;
            }
        }
        // The whole panel makes well over a hundred `ui.*` calls; a threshold
        // that only proves "some" would survive the file list going empty.
        assert!(
            checked > 100,
            "expected to have checked the whole panel's calls, got {checked}"
        );
    }

    /// `ui.<name>` occurrences in a line of Lua.
    fn ui_calls(line: &str) -> Vec<String> {
        let mut found = Vec::new();
        for (index, _) in line.match_indices("ui.") {
            // Only a standalone `ui.`, not the tail of another identifier.
            if index > 0 {
                let prev = line.as_bytes()[index - 1];
                if prev.is_ascii_alphanumeric() || prev == b'_' || prev == b'.' {
                    continue;
                }
            }
            let name: String = line[index + 3..]
                .chars()
                .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
                .collect();
            if !name.is_empty() {
                found.push(name);
            }
        }
        found
    }

    /// `ac_apps/lib.lua` from an installed CSP, if there is one.
    fn installed_csp_lib() -> Option<String> {
        let home = std::env::var_os("HOME")?;
        for steam in [".steam/steam", ".local/share/Steam"] {
            let path = std::path::Path::new(&home)
                .join(steam)
                .join("steamapps/common/assettocorsa/extension/internal/lua-sdk/ac_apps/lib.lua");
            if let Ok(text) = std::fs::read_to_string(&path) {
                return Some(text);
            }
        }
        None
    }

    /// The Lua app ships a copy of this declaration. If the two drift, every
    /// field after the divergence is read from the wrong offset — silently.
    #[test]
    fn the_checked_in_lua_matches_the_generator() {
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../apps/lua/ac_pro_engineer/frame_layout.lua"
        );
        let on_disk = std::fs::read_to_string(path).expect(
            "the Lua app ships frame_layout.lua; regenerate with \
             `cargo run -p ac_core --example gen_lua_layout`",
        );
        assert_eq!(
            on_disk,
            lua_struct_declaration(),
            "frame_layout.lua is stale — regenerate it with \
             `cargo run -p ac_core --example gen_lua_layout > \
             apps/lua/ac_pro_engineer/frame_layout.lua`"
        );
    }

    #[test]
    fn an_empty_frame_reads_as_never_written() {
        let frame = OverlayFrame::empty();
        assert_eq!(frame.version, OVERLAY_VERSION);
        assert_eq!(
            frame.sequence, 0,
            "a mapping that exists but was never filled must not draw"
        );
    }

    /// The cap is a setting, and a setting that only takes effect on the next
    /// launch is a setting people stop trusting.
    #[test]
    fn capped_messages_publish_only_what_was_asked_for() {
        let all = [
            advice("one"),
            advice("two"),
            advice("three"),
            advice("four"),
        ];
        let mut frame = OverlayFrame::empty();

        frame.set_messages_capped(&all, 2);
        assert_eq!(frame.message_count, 2);
        assert!(
            frame.messages[2].iter().all(|b| *b == 0),
            "the third slot stays empty"
        );

        frame.set_messages_capped(&all, 0);
        assert_eq!(frame.message_count, 0);
        assert!(
            frame.messages[0].iter().all(|b| *b == 0),
            "nothing is left over from the previous cap"
        );

        frame.set_messages_capped(&all, 9);
        assert_eq!(
            frame.message_count, 4,
            "a cap past the slot count is clamped"
        );
    }

    #[test]
    fn flags_set_and_clear_independently() {
        let mut frame = OverlayFrame::empty();
        frame.set_flag(flags::PIT_LIMITER, true);
        frame.set_flag(flags::CONNECTED, true);
        assert!(frame.has_flag(flags::PIT_LIMITER));
        assert!(frame.has_flag(flags::CONNECTED));

        frame.set_flag(flags::PIT_LIMITER, false);
        assert!(!frame.has_flag(flags::PIT_LIMITER));
        assert!(frame.has_flag(flags::CONNECTED), "unrelated flag survives");
    }

    #[test]
    fn messages_are_copied_and_counted() {
        let mut frame = OverlayFrame::empty();
        frame.set_messages(&[advice("Tyres cold"), advice("Lockup detected")]);

        assert_eq!(frame.message_count, 2);
        assert!(frame.messages[0].starts_with(b"Tyres cold"));
        assert_eq!(frame.messages[0][10], 0, "NUL-padded, not left dirty");
        assert!(
            frame.messages[2].iter().all(|b| *b == 0),
            "unused slot clear"
        );
    }

    #[test]
    fn more_messages_than_slots_are_dropped_not_overflowed() {
        let mut frame = OverlayFrame::empty();
        let many: Vec<_> = (0..10).map(|i| advice(&format!("advice {i}"))).collect();
        frame.set_messages(&many);

        assert_eq!(frame.message_count as usize, MESSAGE_SLOTS);
        assert!(frame.messages[0].starts_with(b"advice 0"));
        assert!(frame.messages[3].starts_with(b"advice 3"));
    }

    /// Russian advice is two bytes per character, so a naive byte truncation
    /// splits one in half and hands Lua invalid UTF-8.
    #[test]
    fn truncation_never_splits_a_character() {
        let mut frame = OverlayFrame::empty();
        // 40 Cyrillic characters is 80 bytes, past the 64-byte limit.
        let long = "я".repeat(40);
        frame.set_messages(&[advice(&long)]);

        let bytes = &frame.messages[0];
        let end = bytes.iter().position(|b| *b == 0).unwrap_or(MESSAGE_BYTES);
        let text = std::str::from_utf8(&bytes[..end])
            .expect("what lands in the buffer must be valid UTF-8");
        assert_eq!(text.chars().count(), 32, "64 bytes / 2 per char");
    }

    #[test]
    fn a_message_that_exactly_fills_the_slot_is_not_truncated() {
        let exact = "a".repeat(MESSAGE_BYTES);
        assert_eq!(
            truncate_on_boundary(&exact, MESSAGE_BYTES).len(),
            MESSAGE_BYTES
        );
    }

    #[test]
    fn messages_are_cleared_between_updates() {
        let mut frame = OverlayFrame::empty();
        frame.set_messages(&[advice("first"), advice("second")]);
        frame.set_messages(&[advice("only")]);

        assert_eq!(frame.message_count, 1);
        assert!(
            frame.messages[1].iter().all(|b| *b == 0),
            "the previous second message must not linger"
        );
    }
}
