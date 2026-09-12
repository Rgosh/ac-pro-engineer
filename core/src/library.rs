//! Laps other people drove, and the ones you chose to share.
//!
//! **The thing a beginner is missing is not a feature, it is a fast lap.**
//! Every comparison this program makes — where the time went, what the brake
//! pedal did wrong, which corner to work on — is a difference from a reference
//! lap. A driver on their first evening at a circuit has no reference but
//! themselves, so the whole analysis answers "you drove like you" and the
//! BRAKING view is blank. One good lap in the same car at the same track turns
//! all of it on at once.
//!
//! That is the entire purpose of this. It is not a leaderboard, and the
//! difference matters: **nobody cheats to upload a lap somebody else learns
//! from.** A ranking creates a reason to lie and then needs an anti-cheat arms
//! race that one author loses; a library has no prize, so it has no liars.
//!
//! # No accounts, and it stays that way
//!
//! The program has never asked anybody for an email address and this does not
//! start. [`Identity`] is two random numbers made on the machine the first time
//! anything is shared: a public one that goes beside a lap, and a secret one
//! that proves a lap is yours when you come back to change or remove it.
//!
//! That is not a promise about spam — it is an architecture. **There is no
//! address to send anything to.** It also means there is no password to lose,
//! nothing to log into, and nothing that stops working when a server goes
//! away.
//!
//! What it buys the other way round is the thing an open upload channel needs:
//! the public id is what a block is applied to. It is a machine rather than a
//! person, so somebody determined can start again in a minute — that is the
//! honest limit, and closing it costs accounts, which costs everything above.
//!
//! # What the person uploading decides
//!
//! [`Sharing`] is theirs, per lap. The setup is **off unless they say so**: a
//! setup is the thing people in this hobby guard most, and a program that sent
//! one because it happened to be in memory would deserve everything it got.

use crate::analyzer::TelemetryPoint;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// How many points a shared trace carries.
///
/// **Decimated before it leaves the machine.** A lap log is thousands of
/// points; a few hundred is invisible at screen resolution and is the
/// difference between a library that costs a server nothing and one that
/// cannot be afforded. Everything this program does with a reference lap
/// interpolates by distance, so the resolution that matters is the distance
/// step and not the sample count.
pub const POINTS: usize = 600;

/// Who you are here, without an account.
///
/// Made on this machine, kept in one small file, and removable: the cabinet
/// shows the public half and offers to forget it, because a permanent
/// identifier nobody can see or delete is the thing this program's users
/// dislike most — and being unable to point at it would make the design look
/// like the one it is deliberately not.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Identity {
    /// Public. It goes beside every lap shared, the cabinet prints it, and it
    /// is what a block is applied to.
    pub id: String,
    /// Private, and never sent anywhere but the upload itself. Proves a lap is
    /// yours when you come back to change what is shared about it or to take
    /// it down.
    pub secret: String,
}

impl Identity {
    /// This machine's identity, made on first use.
    ///
    /// A stand-in, for a harness that must not touch the real one.
    ///
    /// **The guard has to live in the front end.** It was written here first,
    /// as `if cfg!(test)`, and did nothing: `cfg!(test)` in a library is true
    /// only while that library's own tests run, and the core is compiled as a
    /// dependency of the window — so the portraits made an identity in the
    /// author's data directory and put it in a committed picture anyway. The
    /// caller knows whether it is a harness; this cannot.
    pub fn example() -> Self {
        Self {
            id: "0000example00000".to_string(),
            secret: String::new(),
        }
    }

    /// This machine's identity, made on first use.
    pub fn mine() -> std::io::Result<Self> {
        Self::in_dir(crate::config::app_dir())
    }

    /// The same, somewhere else — for the tests, which must not touch a
    /// driver's own identity.
    pub fn in_dir(data_dir: PathBuf) -> std::io::Result<Self> {
        let at = data_dir.join("identity.json");
        if let Ok(bytes) = std::fs::read(&at)
            && let Ok(kept) = serde_json::from_slice::<Identity>(&bytes)
            && !kept.id.is_empty()
            && !kept.secret.is_empty()
        {
            return Ok(kept);
        }
        let fresh = Self {
            id: random_hex(8),
            secret: random_hex(24),
        };
        std::fs::create_dir_all(&data_dir)?;
        let written = serde_json::to_vec_pretty(&fresh)
            .map_err(|why| std::io::Error::other(why.to_string()))?;
        crate::atomic_file::write_atomic(&at, &written)?;
        Ok(fresh)
    }

    /// Throw this identity away. The next share makes a new one, and nothing
    /// already uploaded can be changed or removed any more.
    ///
    /// **Offered rather than hidden.** Somebody who wants to stop being
    /// recognised should not have to find a JSON file to do it.
    pub fn forget(data_dir: PathBuf) -> std::io::Result<()> {
        match std::fs::remove_file(data_dir.join("identity.json")) {
            Err(why) if why.kind() == std::io::ErrorKind::NotFound => Ok(()),
            other => other,
        }
    }
}

/// Random hex, from the operating system.
///
/// `getrandom` rather than anything home-made: a secret somebody can guess is
/// a secret that lets them take down another driver's laps.
fn random_hex(bytes: usize) -> String {
    let mut raw = vec![0u8; bytes];
    // A machine that cannot produce randomness is a machine this should refuse
    // to invent it for. The clock is *not* a fallback — it is guessable to the
    // millisecond by anybody who saw when the upload happened.
    if getrandom::getrandom(&mut raw).is_err() {
        return String::new();
    }
    raw.iter().map(|byte| format!("{byte:02x}")).collect()
}

/// What the person uploading decides about their own lap.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Sharing {
    /// The name shown beside the lap. Empty is "somebody", which is a
    /// perfectly good answer and the default.
    #[serde(default)]
    pub name: String,
    /// Whether the car setup goes with it.
    ///
    /// **Off unless they say so.** A setup is the thing people in this hobby
    /// guard most, and a program that sent one because it happened to be in
    /// memory would deserve everything it got.
    #[serde(default)]
    pub setup: bool,
    /// Whether it appears in the lists at all, or is only reachable by its own
    /// link.
    ///
    /// On by default: an unlisted lap helps nobody, and somebody who wants to
    /// send one lap to one friend can say so.
    #[serde(default = "yes")]
    pub listed: bool,
}

fn yes() -> bool {
    true
}

impl Default for Sharing {
    fn default() -> Self {
        Self {
            name: String::new(),
            setup: false,
            listed: true,
        }
    }
}

/// Whether the program shares a lap on its own, and what it waits for.
///
/// **Off until somebody says otherwise, and asked once rather than assumed.**
/// The same shape as the crash reports: a program that started sending laps
/// because it seemed helpful would have broken the one promise this whole
/// project is built on, and "there is a setting" is not consent.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Automatically {
    /// Nobody has been asked yet.
    #[default]
    NotAsked,
    /// Asked, and told no. Never asked again.
    Never,
    /// A lap that beats everything this machine has done in that car at that
    /// circuit goes up by itself.
    WhenItIsYourBest,
}

impl Automatically {
    pub fn on(self) -> bool {
        self == Automatically::WhenItIsYourBest
    }

    /// Whether this lap is one to send without being told.
    ///
    /// **Only a personal best, and only a real one.** Every lap would be a
    /// firehose and most of them are worse than the one already up; an invalid
    /// lap is not a lap; and a lap that ties the record is not an improvement,
    /// it is the same lap driven again.
    pub fn would_send(self, lap_time_ms: i32, best_before_ms: i32) -> bool {
        self.on() && lap_time_ms > 0 && (best_before_ms <= 0 || lap_time_ms < best_before_ms)
    }
}

impl Sharing {
    /// The name a list shows.
    pub fn shown_as(&self) -> &str {
        match self.name.trim() {
            "" => "somebody",
            named => named,
        }
    }
}

/// One lap in the library, without the trace.
///
/// **What a list is made of.** A listing of fifty laps is fifty of these, and
/// none of them carries the thousand points that make a lap worth downloading
/// — so browsing costs a few kilobytes and only what somebody chose costs the
/// rest.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Listed {
    /// The lap's own id, given by the server.
    pub id: String,
    /// Whose it is — the public half of an [`Identity`].
    pub by: String,
    pub name: String,
    pub game: String,
    pub car: String,
    pub track: String,
    pub lap_time_ms: i32,
    /// `YYYY-MM-DD`, as the lap itself was stamped.
    pub when: String,
    /// Which release drove it, so an old trace can be read as an old trace.
    pub version: String,
    /// Whether a setup came with it.
    #[serde(default)]
    pub has_setup: bool,
}

impl Listed {
    /// The lap time as a driver reads it.
    pub fn lap_time(&self) -> String {
        if self.lap_time_ms <= 0 {
            return "—".to_string();
        }
        let seconds = self.lap_time_ms as f32 / 1000.0;
        format!(
            "{}:{:06.3}",
            (seconds / 60.0).floor() as i32,
            seconds % 60.0
        )
    }
}

/// A whole lap, as it is uploaded and as it comes back.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Shared {
    #[serde(flatten)]
    pub about: Listed,
    /// The lap itself, decimated to [`POINTS`].
    pub trace: Vec<TelemetryPoint>,
    /// The setup, when the uploader said to send it. Whatever the game's own
    /// file says, as text — the two games disagree about the format and this
    /// is not the place to reconcile them.
    #[serde(default)]
    pub setup: Option<String>,
}

/// What a listing was asked for.
///
/// **Every field optional, because the useful question changes.** "Every lap
/// at Spa" is what somebody browsing asks; "this car at this track" is what
/// the program asks on its own behalf when it wants a reference.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Ask {
    pub game: Option<String>,
    pub car: Option<String>,
    pub track: Option<String>,
    /// Only laps from this identity — which is what the cabinet asks.
    pub by: Option<String>,
    /// Only laps that came with a setup.
    #[serde(default)]
    pub with_setup: bool,
}

impl Ask {
    /// The query string a request carries. Empty when nothing was asked.
    pub fn query(&self) -> String {
        let mut parts: Vec<String> = Vec::new();
        for (key, value) in [
            ("game", &self.game),
            ("car", &self.car),
            ("track", &self.track),
            ("by", &self.by),
        ] {
            if let Some(value) = value.as_ref().filter(|value| !value.trim().is_empty()) {
                parts.push(format!("{key}={}", escape(value.trim())));
            }
        }
        if self.with_setup {
            parts.push("setup=1".to_string());
        }
        parts.join("&")
    }
}

/// Percent-encoding for a query value.
///
/// **Written out rather than pulled in.** A car model is whatever the game
/// calls it and a mod author may have put a space, a slash or a plus in it;
/// none of those may reach a URL unescaped, and the alternative to eight lines
/// here is a dependency for eight lines.
fn escape(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for byte in value.as_bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(*byte as char)
            }
            other => out.push_str(&format!("%{other:02X}")),
        }
    }
    out
}

/// Take every `step`th point, so a lap is [`POINTS`] long or shorter.
///
/// The same rule the plots follow, and for the same reason: egui rebuilds its
/// geometry every frame, a server pays for every byte, and neither can tell a
/// six-hundred-point lap from a seven-thousand-point one.
pub fn decimate(trace: &[TelemetryPoint]) -> Vec<TelemetryPoint> {
    if trace.len() <= POINTS {
        return trace.to_vec();
    }
    let step = trace.len().div_ceil(POINTS);
    trace.iter().step_by(step).cloned().collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn point(at: f32) -> TelemetryPoint {
        TelemetryPoint {
            distance: at,
            time_ms: (at * 90_000.0) as i32,
            speed: 180.0,
            gas: 1.0,
            brake: 0.0,
            gear: 5,
            steer: 0.0,
            lat_g: 0.0,
            lon_g: 0.0,
            slip_avg: 0.0,
            x: at,
            y: at,
            rpms: 7_000,
            detail: Default::default(),
        }
    }

    /// An identity made once is the same identity next time, or every share
    /// comes from a stranger and nobody can ever take their own lap down.
    #[test]
    fn an_identity_is_made_once_and_kept() {
        let root = std::env::temp_dir().join("rg-identity-kept");
        let _ = std::fs::remove_dir_all(&root);

        let first = Identity::in_dir(root.clone()).expect("an identity is made");
        let again = Identity::in_dir(root.clone()).expect("and read back");
        assert_eq!(first, again);
        assert_eq!(first.id.len(), 16, "eight bytes of it");
        assert_eq!(first.secret.len(), 48);

        Identity::forget(root.clone()).expect("it can be thrown away");
        let fresh = Identity::in_dir(root.clone()).expect("and a new one made");
        assert_ne!(fresh.id, first.id, "forgetting means being somebody else");

        let _ = std::fs::remove_dir_all(&root);
    }

    /// Forgetting an identity that was never made is not a failure — it is
    /// somebody pressing the button twice.
    #[test]
    fn forgetting_nothing_is_not_an_error() {
        let root = std::env::temp_dir().join("rg-identity-absent");
        let _ = std::fs::remove_dir_all(&root);
        Identity::forget(root).expect("nothing to forget is fine");
    }

    /// **The setup is off unless somebody says so.** A program that sent one
    /// because it happened to be in memory would deserve everything it got,
    /// and a default is the one place that decision is actually made.
    #[test]
    fn a_setup_is_never_shared_by_accident() {
        assert!(!Sharing::default().setup);
        // And the lap itself is listed, because an unlisted one helps nobody.
        assert!(Sharing::default().listed);
        assert_eq!(Sharing::default().shown_as(), "somebody");
    }

    /// A car model is whatever the game calls it, and a mod author may have
    /// put a space or a slash in it. None of those may reach a URL as itself.
    #[test]
    fn a_car_named_awkwardly_survives_a_query() {
        let ask = Ask {
            car: Some("Some Mod / GT3 [2024]".to_string()),
            track: Some("spa".to_string()),
            ..Default::default()
        };
        let query = ask.query();
        assert!(
            query.contains("car=Some%20Mod%20%2F%20GT3%20%5B2024%5D"),
            "{query}"
        );
        assert!(query.contains("track=spa"));
        assert!(!query.contains(' '), "{query}");
    }

    /// Nothing asked for is no query at all, rather than a `?` with nothing
    /// after it.
    #[test]
    fn asking_for_everything_asks_for_nothing() {
        assert_eq!(Ask::default().query(), "");
        // Blank is not a filter either: a車 empty text box is not a car named "".
        let blank = Ask {
            car: Some("   ".to_string()),
            ..Default::default()
        };
        assert_eq!(blank.query(), "");
    }

    /// A lap leaves the machine at a size a server can afford, and a lap that
    /// is already small is not padded.
    #[test]
    fn a_trace_is_decimated_before_it_leaves() {
        let long: Vec<TelemetryPoint> = (0..7_200)
            .map(|step| point(step as f32 / 7_200.0))
            .collect();
        let sent = decimate(&long);
        assert!(sent.len() <= POINTS, "{}", sent.len());
        assert!(sent.len() > POINTS / 2, "and not needlessly coarse");
        // The shape survives: first and last are still the ends of the lap.
        assert_eq!(sent[0].distance, 0.0);
        assert!(sent.last().expect("a last point").distance > 0.98);

        let short: Vec<TelemetryPoint> = (0..120).map(|s| point(s as f32 / 120.0)).collect();
        assert_eq!(decimate(&short).len(), 120);
    }

    /// A lap time is read by a driver, and a lap with none says so rather than
    /// reading as `0:00.000`.
    #[test]
    fn a_lap_with_no_time_shows_no_time() {
        let mut listed = Listed {
            lap_time_ms: 131_402,
            ..Default::default()
        };
        assert_eq!(listed.lap_time(), "2:11.402");
        listed.lap_time_ms = 0;
        assert_eq!(listed.lap_time(), "—");
    }
}

#[cfg(test)]
mod automatic {
    use super::*;

    /// **Nothing goes anywhere until somebody has said so.** The default is
    /// the one place that decision is actually made, and a default that
    /// changed by accident would be the program breaking its own promise
    /// quietly.
    #[test]
    fn nothing_is_sent_before_anybody_is_asked() {
        assert_eq!(Automatically::default(), Automatically::NotAsked);
        assert!(!Automatically::default().on());
        assert!(!Automatically::default().would_send(90_000, 0));
        assert!(!Automatically::Never.would_send(90_000, 0));
    }

    /// A personal best, and only a real one.
    #[test]
    fn only_a_lap_that_beat_the_last_one_goes_by_itself() {
        let on = Automatically::WhenItIsYourBest;
        assert!(on.would_send(90_000, 91_500), "quicker than before");
        assert!(on.would_send(90_000, 0), "and the first one there is");
        assert!(!on.would_send(91_500, 90_000), "slower");
        assert!(
            !on.would_send(90_000, 90_000),
            "the same time is the same lap driven again, not an improvement"
        );
        assert!(!on.would_send(0, 91_500), "a lap with no time is not a lap");
    }
}
