//! Findings that hold still long enough to be read.
//!
//! **A verdict is recomputed sixty times a second and a threshold has two
//! sides.** A front-left at 99.4 °C against a window that closes at 100 crosses
//! back and forth several times a second, and the engineer's list is honest
//! every single time — it says what is true at that instant. What the driver
//! sees is a line appearing and vanishing, and a panel that flickers is a panel
//! nobody trusts, however correct each frame of it was.
//!
//! This is the one place that decides when a finding is *settled* enough to
//! show. Two thresholds, and they are deliberately different:
//!
//! * **It has to hold before it appears.** A reading that touches a limit for a
//!   quarter of a second is noise; one that holds for a couple of seconds is
//!   the car telling you something.
//! * **It lingers after it stops.** Symmetric timings would still blink: a
//!   finding sitting exactly on its threshold would appear, vanish, appear.
//!   Lingering longer than it takes to appear is what turns an oscillation into
//!   one steady line.
//!
//! # Why this is in the core
//!
//! Because "how long before a driver is told" is a rule, and this project has
//! three front ends over one core precisely so that a rule cannot be
//! implemented twice and disagree. The terminal, the window and the in-game
//! panel all draw the same list; if each smoothed it its own way they would
//! show different advice about the same lap, which is the failure the whole
//! layering exists to prevent.
//!
//! # What it is not
//!
//! It does not change a verdict, reorder one, or invent one. Every finding that
//! comes out of here went in, with its own words and its own numbers — the
//! latest of them, so a temperature that is still climbing keeps counting up
//! while the line holds still. The only thing decided here is *when*.

use crate::engineer::Recommendation;
use std::collections::HashMap;

/// How long a finding must hold before it is shown, in seconds.
///
/// Two and a half: long enough that a threshold brushed in a single corner
/// does not put a line on the screen, short enough that a real problem is named
/// while the driver is still in the situation that caused it.
pub const APPEAR_AFTER: f32 = 2.5;

/// How long it stays after it stops being reported, in seconds.
///
/// **Longer than [`APPEAR_AFTER`], and that asymmetry is the whole trick.**
/// With both the same, a reading resting on its threshold would appear, drop
/// off, and appear again — the flicker moved rather than removed.
pub const LINGER_FOR: f32 = 4.0;

/// One finding, and how long it has been saying so.
#[derive(Debug, Clone)]
struct Held {
    /// When this finding was first reported in its current run.
    first_seen: f32,
    /// The last moment the engineer reported it.
    last_seen: f32,
    /// Where it sat in the engineer's own ordering when last reported, so
    /// nothing jumps up the list merely because it is being held.
    rank: usize,
    /// Whether it has been on the screen. Once it has, it leaves by lingering
    /// rather than by falling back below [`APPEAR_AFTER`].
    shown: bool,
    /// The most recent wording, so a number that is still moving keeps moving.
    what: Recommendation,
}

/// The findings that have earned their place on a screen.
#[derive(Debug, Clone)]
pub struct Steady {
    seen: HashMap<String, Held>,
    /// When [`Steady::settle`] was last called. **This is what makes "held
    /// continuously" mean anything** without knowing how often the caller
    /// asks: a finding is continuous if it was in the previous batch, whether
    /// the batches are a sixtieth of a second apart or one a second.
    last_call: Option<f32>,
    appear_after: f32,
    linger_for: f32,
}

impl Default for Steady {
    fn default() -> Self {
        Self::new(APPEAR_AFTER, LINGER_FOR)
    }
}

impl Steady {
    pub fn new(appear_after: f32, linger_for: f32) -> Self {
        Self {
            seen: HashMap::new(),
            last_call: None,
            appear_after,
            linger_for,
        }
    }

    /// What identifies a finding across frames.
    ///
    /// **Not the message.** A message carries the reading that produced it and
    /// changes every frame; keying on it would make every frame a new finding
    /// and nothing would ever settle. The component and the category are what
    /// stay the same while a problem persists.
    fn key(of: &Recommendation) -> String {
        format!("{}\u{1}{}", of.component, of.category)
    }

    /// Hand in what the engineer says now; get back what should be on screen.
    ///
    /// `now` is seconds on any clock that only moves forwards — the caller's
    /// own uptime is the obvious one.
    pub fn settle(&mut self, now: f32, fresh: Vec<Recommendation>) -> Vec<Recommendation> {
        let previous = self.last_call;
        self.last_call = Some(now);

        for (rank, finding) in fresh.into_iter().enumerate() {
            let key = Self::key(&finding);
            match self.seen.get_mut(&key) {
                Some(held) => {
                    // **A gap starts the wait again.** Without this, a reading
                    // sitting on its threshold and reported every other frame
                    // still reaches two and a half seconds of *age* and is
                    // promoted — the flicker survives the thing written to
                    // stop it. Only a finding that was also in the previous
                    // batch is holding; one that missed a batch is starting
                    // over. Already-shown findings are exempt: they leave by
                    // lingering, which is what stops them blinking.
                    let unbroken = previous.is_none_or(|before| held.last_seen >= before);
                    if !held.shown && !unbroken {
                        held.first_seen = now;
                    }
                    held.last_seen = now;
                    held.rank = rank;
                    // The latest wording, always: the line holds still, the
                    // number inside it does not.
                    held.what = finding;
                }
                None => {
                    self.seen.insert(
                        key,
                        Held {
                            first_seen: now,
                            last_seen: now,
                            rank,
                            shown: false,
                            what: finding,
                        },
                    );
                }
            }
        }

        // Gone long enough to forget. A finding that comes back after this
        // starts its wait again, which is right: it stopped being true.
        let linger_for = self.linger_for;
        self.seen
            .retain(|_, held| now - held.last_seen <= linger_for);

        let appear_after = self.appear_after;
        let mut showing: Vec<(usize, Recommendation)> = self
            .seen
            .values_mut()
            .filter_map(|held| {
                if !held.shown {
                    // It has to be reported *now* and to have held since:
                    // a finding that flickered on and off for five seconds has
                    // never held for two and a half, and is not promoted by
                    // the clock alone.
                    if held.last_seen != now || now - held.first_seen < appear_after {
                        return None;
                    }
                    held.shown = true;
                }
                Some((held.rank, held.what.clone()))
            })
            .collect();

        // The engineer's own ordering, kept. `sort_by_key` is stable, so two
        // findings that were once at the same rank stay in the order the map
        // yielded them rather than swapping about between frames.
        showing.sort_by_key(|(rank, _)| *rank);
        showing.into_iter().map(|(_, what)| what).collect()
    }

    /// Forget everything.
    ///
    /// For a new session, a new car, or a feed that went away: holding a
    /// finding about the last car across into this one is the same class of
    /// mistake as a stale shared-memory page.
    pub fn forget(&mut self) {
        self.seen.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engineer::Severity;

    fn finding(component: &str, message: &str) -> Recommendation {
        Recommendation {
            component: component.to_string(),
            category: "tyres".to_string(),
            severity: Severity::Warning,
            message: message.to_string(),
            action: "do something".to_string(),
            parameters: Vec::new(),
            confidence: 0.8,
            chain: None,
        }
    }

    /// A finding that only touches its threshold never reaches the screen.
    ///
    /// **The fault this exists for.** A front-left at 99.4 °C against a window
    /// closing at 100 crosses back and forth several times a second; every
    /// frame is honest and the panel still flickers.
    #[test]
    fn a_reading_that_brushes_its_limit_is_not_shown() {
        let mut steady = Steady::default();
        let mut now = 0.0;
        for _ in 0..30 {
            // On for a fifth of a second, off for a fifth, over and over.
            now += 0.2;
            assert!(steady.settle(now, vec![finding("FL", "97 °C")]).is_empty());
            now += 0.2;
            assert!(steady.settle(now, Vec::new()).is_empty());
        }
    }

    /// One that holds is shown, and not before it has.
    #[test]
    fn a_finding_that_holds_appears_once_it_has_held() {
        let mut steady = Steady::default();
        let mut now = 0.0;
        while now < APPEAR_AFTER - 0.1 {
            now += 0.5;
            assert!(
                steady.settle(now, vec![finding("FL", "104 °C")]).is_empty(),
                "shown after only {now} s"
            );
        }
        now += 0.5;
        let shown = steady.settle(now, vec![finding("FL", "104 °C")]);
        assert_eq!(shown.len(), 1);
        assert_eq!(shown[0].component, "FL");
    }

    /// The wording keeps up while the line holds still.
    #[test]
    fn the_number_inside_a_settled_finding_still_moves() {
        let mut steady = Steady::default();
        let mut now = 0.0;
        for step in 0..8 {
            now += 0.5;
            let _ = steady.settle(now, vec![finding("FL", &format!("{} °C", 100 + step))]);
        }
        let shown = steady.settle(now, vec![finding("FL", "108 °C")]);
        assert_eq!(
            shown[0].message, "108 °C",
            "the line held, the reading did not"
        );
    }

    /// It lingers rather than blinking off, and then it goes.
    #[test]
    fn a_finding_that_stops_lingers_and_then_leaves() {
        let mut steady = Steady::default();
        let mut now = 0.0;
        for _ in 0..8 {
            now += 0.5;
            let _ = steady.settle(now, vec![finding("FL", "104 °C")]);
        }
        assert_eq!(steady.settle(now, vec![finding("FL", "104 °C")]).len(), 1);

        // Gone from the engineer's list, still on the screen.
        now += LINGER_FOR - 0.5;
        assert_eq!(
            steady.settle(now, Vec::new()).len(),
            1,
            "it blinked off the moment the engineer stopped saying it"
        );

        // And then away.
        now += 1.0;
        assert!(steady.settle(now, Vec::new()).is_empty());
    }

    /// Once it has been shown, it does not have to earn its place again on
    /// every frame — which is what lingering is for.
    #[test]
    fn a_settled_finding_survives_a_frame_that_does_not_report_it() {
        let mut steady = Steady::default();
        let mut now = 0.0;
        for _ in 0..8 {
            now += 0.5;
            let _ = steady.settle(now, vec![finding("FL", "104 °C")]);
        }
        now += 0.02;
        assert_eq!(steady.settle(now, Vec::new()).len(), 1);
        now += 0.02;
        assert_eq!(steady.settle(now, vec![finding("FL", "104 °C")]).len(), 1);
    }

    /// The engineer's ordering is the ordering.
    ///
    /// Nothing here may promote a finding: this decides *when* a line appears
    /// and never which line matters more.
    #[test]
    fn the_engineers_own_order_is_kept() {
        let mut steady = Steady::default();
        let list = || {
            vec![
                finding("brakes", "hot"),
                finding("FL", "hot"),
                finding("fuel", "short"),
            ]
        };
        let mut now = 0.0;
        for _ in 0..8 {
            now += 0.5;
            let _ = steady.settle(now, list());
        }
        let shown = steady.settle(now, list());
        let order: Vec<&str> = shown.iter().map(|one| one.component.as_str()).collect();
        assert_eq!(order, vec!["brakes", "FL", "fuel"]);
    }

    /// A new car starts with nothing held over from the last one.
    #[test]
    fn forgetting_leaves_nothing_behind() {
        let mut steady = Steady::default();
        let mut now = 0.0;
        for _ in 0..8 {
            now += 0.5;
            let _ = steady.settle(now, vec![finding("FL", "104 °C")]);
        }
        assert_eq!(steady.settle(now, vec![finding("FL", "104 °C")]).len(), 1);
        steady.forget();
        now += 0.5;
        assert!(
            steady.settle(now, vec![finding("FL", "104 °C")]).is_empty(),
            "a finding about the last car survived into this one"
        );
    }

    /// Findings are told apart by what they are about, not by their wording.
    #[test]
    fn two_findings_about_different_corners_settle_separately() {
        let mut steady = Steady::default();
        let mut now = 0.0;
        for _ in 0..8 {
            now += 0.5;
            let _ = steady.settle(now, vec![finding("FL", "104 °C")]);
        }
        // The rear arrives late and has to hold on its own account.
        let both = vec![finding("FL", "104 °C"), finding("RL", "101 °C")];
        let shown = steady.settle(now + 0.5, both.clone());
        assert_eq!(shown.len(), 1, "the new one was shown immediately");
        now += 0.5;
        while now < 8.0 {
            now += 0.5;
            let _ = steady.settle(now, both.clone());
        }
        assert_eq!(steady.settle(now, both).len(), 2);
    }
}
