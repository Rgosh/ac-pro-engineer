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
pub mod assetto_corsa_competizione;
pub mod catalogue;
pub mod reading;
pub mod registry;

pub use catalogue::CarSpecs;
pub use reading::{Car, Fixed, Reading, Session, SessionKind, Status};
pub use registry::{Backend, Game, Support};

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
///
/// **Default is nothing measured**, and deliberately: a consumer that is never
/// told what the game reports withholds everything rather than inventing it.
/// That failure is loud — the advice goes silent, and the screenshots show it —
/// where the permissive default fails silently, one wrong verdict at a time.
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
    /// How much grip the track has, as a fraction.
    ///
    /// Assetto Corsa publishes it and Competizione does not — ACC says how the
    /// track *is* instead, as one of a handful of named states, and turning
    /// that into a number would be inventing a measurement. Without this flag
    /// the missing value reads as a green track: the cold-pressure calculator
    /// clamps it to 0.80 and adds 0.3 psi to the target, on every lap, for a
    /// condition nobody measured.
    pub track_grip: bool,
    /// Brake pad and disc thickness are published.
    ///
    /// Competizione's answer to tyre wear, which it does not publish: over a
    /// GT3 stint the pads are the consumable that decides the race.
    pub brake_wear: bool,
    /// The game says whether the lap being driven still counts.
    ///
    /// Assetto Corsa never does, so every lap is treated as valid there —
    /// which is not a claim that it *is* valid, only that nothing said
    /// otherwise. ACC says.
    pub lap_validity: bool,
}

impl Capabilities {
    /// Everything measured.
    ///
    /// For tests, and for a game that genuinely reports all of it. Assetto
    /// Corsa spells its own out by hand instead, beside the capture that
    /// proves each one — a game claiming this without having checked is the
    /// mistake the whole type exists to prevent.
    pub fn all() -> Self {
        Self {
            tyre_edge_temps: true,
            sectors: true,
            setups: true,
            tyre_wear: true,
            track_grip: true,
            brake_wear: true,
            lap_validity: true,
        }
    }
}

/// A running simulator, as far as the rest of the program is concerned.
///
/// Deliberately thin. It says whether the game is there and hands over the most
/// recent reading; everything about *interpreting* that reading belongs above,
/// in the analyser and the engineer, which is what makes them reusable when a
/// second game arrives.
///
/// The reading comes out of [`poll`](Self::poll) and nowhere else. It used to
/// come out beside it, through accessors returning Assetto Corsa's own structs,
/// which meant the trait could be — and was — bypassed entirely: five screens,
/// the engineer and the analyser read AC's memory layout directly while this
/// file described a boundary that carried no data.
pub trait Source {
    /// Which game this is, for broadcast messages and logs.
    fn id(&self) -> GameId;

    /// What it can and cannot report.
    fn capabilities(&self) -> Capabilities;

    /// Take a fresh reading. `None` means nothing could be read this tick —
    /// the game is not running, or has not published yet — which is a normal
    /// state and not an error.
    fn poll(&mut self) -> Option<Reading>;
}
