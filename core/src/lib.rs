pub mod games;

// Where these two used to live. Kept as re-exports so the move into `games/`
// changed no call sites: every `ac_core::ac_structs::AcPhysics` in the tree,
// the tests included, still resolves. New code should reach for
// `games::assetto_corsa::{paths, structs}` and say which game it means.
pub use games::assetto_corsa::paths as ac_paths;
pub use games::assetto_corsa::structs as ac_structs;
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
pub mod memory;
pub mod net;
pub mod overlay;
pub mod process;
pub mod records;
pub mod ring_buffer;
pub mod session_info;
pub mod setup_history;
pub mod setup_manager;
pub mod updater;

pub use ring_buffer::RingBuffer;
