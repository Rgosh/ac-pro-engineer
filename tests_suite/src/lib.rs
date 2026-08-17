//! The workspace's cross-crate tests, split the way the code is.
//!
//! **One file per game, named after the game's folder**, plus the ones that
//! belong to nobody: the neutral core, the boundary rules and the
//! translations. A third simulator is `<its id>_tests.rs` beside the other
//! two and a line here — the same shape as `core/src/games/<its id>/`, so
//! there is nothing to decide about where a test goes.
//!
//! That split is what `tools/test-game.sh` runs against: working on one game,
//! its file and the core are the two things worth re-running, and the other
//! game is noise.
//!
//! A game's file holds what can only be checked against *that game's bytes* —
//! a captured page, the values it decodes to, its own conventions. Anything
//! that would still be true with the game removed belongs in `core_tests`.

#[cfg(test)]
pub mod assetto_corsa_competizione_tests;

#[cfg(test)]
pub mod assetto_corsa_tests;

#[cfg(test)]
pub mod boundary_tests;

#[cfg(test)]
pub mod core_tests;

#[cfg(test)]
pub mod fixtures;

#[cfg(test)]
pub mod i18n_tests;
