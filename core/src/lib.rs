// AC Pro Engineer — telemetry and race engineering for Assetto Corsa.
// Copyright (c) 2026 Rgosh and contributors.
//
// This program is free software under the GNU Affero General Public License,
// version 3, with the additional terms in NOTICE. It comes with NO WARRANTY.
// LICENSE has the text; LICENSING.md says what it means, including how to ask
// for a closed-source exception. Versions up to v0.3.6 were MIT and stay MIT.

pub mod games;

// Where AC's file paths used to live, kept as a re-export so the move into
// `games/` changed no call sites. The matching `ac_structs` alias is gone: the
// three shared-memory structs are no longer anybody's business outside the
// folder that reads them — see `games::reading` — and the two places that do
// speak AC's layout, the fake-telemetry simulator and the layout tests, now
// import it by its full name and say which game they mean.
pub use games::assetto_corsa::paths as ac_paths;
pub mod analyzer;
pub mod atomic_file;
pub mod broadcast;
pub mod confidence;
pub mod config;
pub mod content_manager;
pub mod corners;
pub mod crash_logger;
pub mod debrief;
pub mod discord;
pub mod driver_vs_car;
pub mod engineer;
pub mod i18n;
pub mod memory;
pub mod net;
pub mod overlay;
pub mod process;
pub mod records;
pub mod ring_buffer;
pub mod session_info;
pub mod setup_manager;
pub mod updater;

pub use ring_buffer::RingBuffer;
