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

use crate::engineer::Recommendation;
use crate::session_info::SessionInfo;

/// Bumped whenever the layout changes. The overlay refuses to draw a version
/// it does not recognise rather than misreading a struct from another release.
pub const OVERLAY_VERSION: u32 = 1;

/// Shared memory name. The `AcTools.CSP.Limited.` prefix matters: CSP allows
/// scripts without IO permission to open shared memory only when the name
/// starts with it, so this works whether or not the app is granted IO.
pub const OVERLAY_MMF_NAME: &str = "AcTools.CSP.Limited.ACPE.v1";

/// Longest advice line carried to the overlay, in bytes. UTF-8, NUL-padded.
pub const MESSAGE_BYTES: usize = 64;

/// How many advice lines fit. Four is what the overlay has room to draw
/// without covering the track.
pub const MESSAGE_SLOTS: usize = 4;

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

    /// Engineer advice, most severe first. UTF-8, NUL-padded, truncated on a
    /// character boundary.
    pub messages: [[u8; MESSAGE_BYTES]; MESSAGE_SLOTS],
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
}

impl Default for OverlayFrame {
    fn default() -> Self {
        Self::empty()
    }
}

impl OverlayFrame {
    /// A frame with nothing in it but a version.
    ///
    /// `sequence` starts at zero, which reads as "never written" to the
    /// overlay — so a mapping that exists but has never been filled does not
    /// draw stale zeroes.
    pub const fn empty() -> Self {
        Self {
            version: OVERLAY_VERSION,
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
        self.messages = [[0; MESSAGE_BYTES]; MESSAGE_SLOTS];
        let taken = recommendations.len().min(MESSAGE_SLOTS);

        for (slot, rec) in self.messages.iter_mut().zip(recommendations) {
            let text = truncate_on_boundary(&rec.message, MESSAGE_BYTES);
            slot[..text.len()].copy_from_slice(text.as_bytes());
        }

        self.message_count = taken as u32;
    }

    /// Fill in the parts that come from the session rather than from physics.
    pub fn apply_session(&mut self, session: &SessionInfo) {
        self.max_rpm = session.max_rpm;
        self.lap_count = session.lap_count;
    }
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
    (
        "messages",
        "ac.StructItem.array(ac.StructItem.string(64), 4)",
    ),
];

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
        // deliberately: 20 scalars + 16 array floats + the message block.
        // 2 header + 8 floats + 8 integers + 2 trailing counters = 20.
        let scalars = 20 * 4;
        let arrays = 16 * 4;
        let messages = MESSAGE_SLOTS * MESSAGE_BYTES;
        assert_eq!(size_of::<OverlayFrame>(), scalars + arrays + messages);
    }

    /// Every field must be listed for the generator, or the Lua side silently
    /// omits one and every field after it reads from the wrong offset.
    #[test]
    fn the_generator_lists_every_field() {
        // 20 scalars + 4 arrays + the message block = 25 entries.
        assert_eq!(FIELDS.len(), 25);

        // The declared types have to add up to the struct's actual size, which
        // is what catches a field added to the struct but not to FIELDS.
        let bytes: usize = FIELDS
            .iter()
            .map(|(_, kind)| {
                if kind.contains("string(64), 4") {
                    MESSAGE_BYTES * MESSAGE_SLOTS
                } else if kind.contains(", 4)") {
                    4 * 4
                } else {
                    4
                }
            })
            .sum();
        assert_eq!(bytes, size_of::<OverlayFrame>());
    }

    /// Without explicitOrder, CSP packs fields in its own order and every
    /// read is misaligned. It is the one line that cannot be omitted.
    #[test]
    fn the_generated_lua_pins_the_field_order() {
        let lua = lua_struct_declaration();
        assert!(lua.contains("ac.StructItem.explicit(4, 4)"));
        assert!(lua.contains("version = ac.StructItem.uint32()"));
        assert!(lua.contains("messages = ac.StructItem.array(ac.StructItem.string(64), 4)"));
        // Order matters as much as presence.
        let version_at = lua.find("version =").expect("version");
        let sequence_at = lua.find("sequence =").expect("sequence");
        let messages_at = lua.find("messages =").expect("messages");
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
