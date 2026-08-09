//! One folder per game, and the trait they all sit behind.
//!
//! Everything that knows how a *particular* simulator publishes telemetry lives
//! under here: the shape of its shared memory or its UDP packets, where its
//! files are installed, how its setups are stored. Nothing above this module
//! should have to know which game is running.
//!
//! Today there is exactly one, and that is deliberate — see
//! `docs/ARCHITECTURE.md`. An abstraction with one implementation is a guess;
//! the second game will find the wrong assumptions in this trait, and it is far
//! cheaper to fix them while there is one consumer than five. So [`Source`] is
//! kept to what Assetto Corsa actually needs and no wider.
//!
//! What it does buy today, which is the point of doing it now:
//!
//! * the reading of the game moved **into the core**. It used to live in
//!   `tui/src/lib.rs`, so the terminal owned the connection to the simulator
//!   and anything else that wanted telemetry had to go through a user
//!   interface to get it.
//! * a second game is a new folder beside `assetto_corsa/` rather than a set of
//!   `if` statements through the middle of the engineer.

pub mod assetto_corsa;

/// Which simulator a source speaks for.
///
/// A string rather than an enum with one variant: this is what goes into a
/// broadcast message and out to whatever is listening, and a receiver should be
/// able to recognise a game this build has never heard of rather than fail to
/// parse the message.
pub type GameId = &'static str;

/// What a game can actually tell us.
///
/// The reason this exists at all is that "not measured" and "measured as zero"
/// are different answers and every wrong verdict this project has shipped came
/// from confusing them: four tyres reading zero wear were reported as four
/// destroyed tyres, and a lap with no published temperatures produced a camber
/// verdict about a car nobody had driven. A game that cannot report a thing
/// must be able to say so.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Capabilities {
    /// Inner and outer tyre surface temperatures, not just the middle. The
    /// camber advice is built entirely on the difference between them.
    pub tyre_edge_temps: bool,
    /// Per-sector timing.
    pub sectors: bool,
    /// The car's setup can be read from disk.
    pub setups: bool,
    /// Tyre wear is published.
    pub tyre_wear: bool,
}

/// A running simulator, as far as the rest of the program is concerned.
///
/// Deliberately thin. It says whether the game is there and hands over the most
/// recent reading; everything about *interpreting* that reading belongs above,
/// in the analyser and the engineer, which is what makes them reusable when a
/// second game arrives.
pub trait Source {
    /// Which game this is, for broadcast messages and logs.
    fn id(&self) -> GameId;

    /// What it can and cannot report.
    fn capabilities(&self) -> Capabilities;

    /// Take a fresh reading. `false` means nothing could be read this tick —
    /// the game is not running, or has not published yet — which is a normal
    /// state and not an error.
    fn poll(&mut self) -> bool;
}
