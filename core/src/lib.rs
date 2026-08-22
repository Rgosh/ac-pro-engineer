// Pro Engineer — telemetry and race engineering for Assetto Corsa.
// Copyright (c) 2026 Rgosh and contributors.
//
// This program is free software under the GNU Affero General Public License,
// version 3, with the additional terms in NOTICE. It comes with NO WARRANTY.
// LICENSE has the text; LICENSING.md says what it means, including how to ask
// for a closed-source exception. Versions up to v0.3.6 were MIT and stay MIT.

pub mod games;

// Both `ac_paths` and `ac_structs` used to be re-exported here so that moving
// them into `games/` changed no call sites. Neither is now: what a game keeps
// where is that game's business, and the places that legitimately speak
// Assetto Corsa — its own folder, the overlay that installs a mod into it, the
// fake-telemetry simulator and the layout tests — say so by importing it under
// its own name.
pub mod analyzer;
pub mod atomic_file;
pub mod broadcast;
pub mod confidence;
pub mod config;
pub mod content_manager;
pub mod corners;
pub mod crash_logger;
pub mod debrief;
pub mod driver_vs_car;
pub mod engineer;
/// The handbook both front ends draw. The words live here; the styling does
/// not — see the module note.
pub mod guide;
pub mod i18n;
pub mod memory;
pub mod net;
pub mod overlay;
pub mod process;
pub mod records;
pub mod ring_buffer;
pub mod session_info;
pub mod setup_manager;
pub mod steam;
pub mod updater;

pub use ring_buffer::RingBuffer;
