//! The in-game overlay: a frame, a way to publish it, and the panel that reads
//! it.
//!
//! There is one overlay, and it is the CSP Lua panel under `apps/lua/`. This
//! module is the desktop half of it — [`frame`] declares the 712 bytes both
//! sides agree on, [`shared_writer`] publishes them, [`bridge`] and
//! [`bridge_update`] get the mapping into the Wine prefix on Linux, and
//! [`install`] writes the panel into the game folder.
//!
//! It used to also carry a second overlay: a layered Win32 window drawn by the
//! desktop application, toggled with F10, with a control centre in the
//! terminal. It never worked on Linux — where the provider was simply `None`,
//! so F10 logged a line and did nothing — and on Windows it duplicated, worse,
//! what the panel already draws. Removed, along with its key bindings.

pub mod bridge;
pub mod bridge_update;
pub mod diagnosis;
pub mod frame;
pub mod install;
pub mod shared_writer;
