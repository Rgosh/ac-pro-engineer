//! A session, written down as something a driver can post.
//!
//! **Every number in this program stayed on the screen it was drawn on.** A
//! driver who wanted to show a league, a coach or a forum thread how an evening
//! went took a screenshot of one tab — which carries one lap, one view, and
//! nothing the person reading it can copy. This writes the whole session as
//! Markdown: the laps with their sectors, the best, the lap of best sectors,
//! the consistency run, what the engineer said and which corners are worth
//! practising. Markdown because it is what Discord, GitHub and every forum
//! that matters render as written, and reads as plain text everywhere else.
//!
//! # What it is not
//!
//! It computes nothing of its own. Every figure in it is one the screens
//! already show, from the same functions — the sectors from
//! [`crate::analyzer::best_sectors_of`], the run from
//! [`crate::consistency`], the corners from [`crate::repeatability`] — so the
//! report and the program cannot disagree about the same lap. And nothing
//! leaves the machine: it is a file in the program's own folder, and what
//! happens to it after that is the driver's choice.

use crate::analyzer::{LapData, best_sectors_of, theoretical_best_of};
use crate::consistency::{self, lap_time};
use crate::engineer::{Recommendation, Severity};
use crate::i18n::{Translate, tr_fmt};
use crate::repeatability::Repeatability;
use std::fmt::Write as _;
use std::path::PathBuf;

/// The most findings the report lists. A report that says everything is read
/// by nobody; the worst first, and these are enough.
const FINDINGS: usize = 8;

/// The most corners it names as worth practising.
const CORNERS: usize = 3;

/// What a report is written from.
pub struct Session<'a> {
    /// The game, as a driver names it.
    pub game: &'a str,
    pub car: &'a str,
    pub track: &'a str,
    /// Who drove, when the game said. Empty is left out, not "unknown".
    pub driver: &'a str,
    /// The session's laps, in the order they were driven. Laps loaded from a
    /// file are not this session's and are left out.
    pub laps: &'a [LapData],
    /// What the engineer is saying now, in whatever order; the worst go first.
    pub findings: &'a [Recommendation],
    /// The corners, when there were enough laps to measure them.
    pub corners: Option<&'a Repeatability>,
    /// How many sectors the track has, so a two-sector track is not shown an
    /// empty third column.
    pub sector_count: usize,
}

/// Write the report.
pub fn markdown(session: &Session<'_>, ru: bool) -> String {
    let laps: Vec<&LapData> = session
        .laps
        .iter()
        .filter(|lap| !lap.from_file && lap.lap_time_ms > 0)
        .collect();
    let own: Vec<LapData> = laps.iter().map(|lap| (*lap).clone()).collect();
    let sectors = session.sector_count.clamp(1, 3);
    let mut out = String::new();

    // The title is what a forum shows as the link, so it carries the three
    // things somebody scanning a thread wants: what, where, and when.
    let when = laps
        .first()
        .map(|lap| {
            format!(
                "{} {}",
                lap.save_date,
                lap.timestamp.chars().take(5).collect::<String>()
            )
        })
        .unwrap_or_default();
    let _ = writeln!(out, "# {} · {}", name(session.car), name(session.track));
    let mut about = vec![session.game.to_string()];
    if !when.trim().is_empty() {
        about.push(when.trim().to_string());
    }
    if !session.driver.trim().is_empty() {
        about.push(session.driver.trim().to_string());
    }
    let _ = writeln!(out, "\n_{}_\n", about.join(" · "));

    if laps.is_empty() {
        let _ = writeln!(out, "{}", "No lap was finished in this session.".tr(ru));
        footer(&mut out, ru);
        return out;
    }

    // ---- The headline numbers.
    let _ = writeln!(out, "## {}\n", "Summary".tr(ru));
    let valid: Vec<&&LapData> = laps.iter().filter(|lap| lap.valid).collect();
    let best = valid.iter().min_by_key(|lap| lap.lap_time_ms);
    let _ = writeln!(
        out,
        "- **{}:** {} ({} {})",
        "Laps".tr(ru),
        laps.len(),
        valid.len(),
        "valid".tr(ru)
    );
    if let Some(best) = best {
        let _ = writeln!(
            out,
            "- **{}:** {} — {} {}",
            "Best lap".tr(ru),
            lap_time(best.lap_time_ms),
            "lap".tr(ru),
            best.lap_number + 1
        );
    }
    if let Some(ideal) = theoretical_best_of(&own, sectors) {
        let _ = writeln!(
            out,
            "- **{}:** {}",
            "Best sectors together".tr(ru),
            lap_time(ideal)
        );
    }
    if let Some(corners) = session
        .corners
        .filter(|corners| corners.own_best_lap_ms > 0)
    {
        let _ = writeln!(
            out,
            "- **{}:** {} ({})",
            "Your best corners together".tr(ru),
            lap_time(corners.own_best_lap_ms),
            tr_fmt(
                "{0} to find",
                ru,
                &[&format!("{:.3} s", corners.to_find_ms() as f32 / 1000.0)]
            )
        );
    }
    if let Some(counted) = consistency::measure(&own) {
        let _ = writeln!(
            out,
            "- **{}:** {}",
            "Consistency".tr(ru),
            counted.headline(ru)
        );
        if let Some(spread) = counted.spread(ru) {
            let _ = writeln!(out, "- **{}:** {}", "Spread".tr(ru), spread);
        }
    }
    let fuel: Vec<f32> = valid
        .iter()
        .map(|lap| lap.fuel_used)
        .filter(|litres| *litres > 0.0)
        .collect();
    if !fuel.is_empty() {
        let _ = writeln!(
            out,
            "- **{}:** {:.2} L",
            "Fuel per lap".tr(ru),
            fuel.iter().sum::<f32>() / fuel.len() as f32
        );
    }

    // ---- Every lap.
    let best_ms = best.map(|lap| lap.lap_time_ms).unwrap_or(0);
    let best_sector = best_sectors_of(&own);
    let _ = writeln!(out, "\n## {}\n", "Laps".tr(ru));
    let mut header = format!("| {} | {} | Δ |", "Lap".tr(ru), "Time".tr(ru));
    let mut rule = "|---:|---:|---:|".to_string();
    for sector in 1..=sectors {
        let _ = write!(header, " S{sector} |");
        rule.push_str("---:|");
    }
    let _ = write!(header, " {} |", "Fuel".tr(ru));
    rule.push_str("---:|");
    let _ = writeln!(out, "{header}\n{rule}");
    for lap in &laps {
        let delta = if !lap.valid {
            "invalid".tr(ru).to_string()
        } else if lap.lap_time_ms == best_ms {
            "**best**".tr(ru).to_string()
        } else {
            format!("+{:.3}", (lap.lap_time_ms - best_ms) as f32 / 1000.0)
        };
        let time = if lap.valid {
            lap_time(lap.lap_time_ms)
        } else {
            format!("~~{}~~", lap_time(lap.lap_time_ms))
        };
        let mut row = format!("| {} | {} | {} |", lap.lap_number + 1, time, delta);
        for (index, sector) in lap.sectors.iter().take(sectors).enumerate() {
            // A purple sector is the session's best, as every timing screen
            // in motorsport marks it; here it is bold.
            let text = if *sector <= 0 {
                "—".to_string()
            } else if lap.valid && best_sector[index] == Some(*sector) {
                format!("**{:.3}**", *sector as f32 / 1000.0)
            } else {
                format!("{:.3}", *sector as f32 / 1000.0)
            };
            let _ = write!(row, " {text} |");
        }
        let fuel = if lap.fuel_used > 0.0 {
            format!("{:.2}", lap.fuel_used)
        } else {
            "—".to_string()
        };
        let _ = write!(row, " {fuel} |");
        let _ = writeln!(out, "{row}");
    }

    // ---- Corners to practise.
    if let Some(corners) = session.corners {
        let worst: Vec<_> = corners.worst().into_iter().take(CORNERS).collect();
        if !worst.is_empty() {
            let _ = writeln!(out, "\n## {}\n", "Corners worth practising".tr(ru));
            for section in worst {
                let _ = writeln!(
                    out,
                    "- **T{}** — {}",
                    section.number,
                    tr_fmt(
                        "±{0} lap to lap, {1} a lap to find against your own best through it",
                        ru,
                        &[
                            &format!("{:.2} s", section.spread_ms as f32 / 1000.0),
                            &format!("{:.2} s", section.to_gain_ms as f32 / 1000.0),
                        ],
                    )
                );
            }
        }
    }

    // ---- What the engineer said.
    let mut findings: Vec<&Recommendation> = session.findings.iter().collect();
    findings.sort_by_key(|finding| match finding.severity {
        Severity::Critical => 0,
        Severity::Warning => 1,
        Severity::Info => 2,
    });
    if !findings.is_empty() {
        let _ = writeln!(out, "\n## {}\n", "What the engineer said".tr(ru));
        for finding in findings.into_iter().take(FINDINGS) {
            let mark = match finding.severity {
                Severity::Critical => "🔴",
                Severity::Warning => "🟡",
                Severity::Info => "⚪",
            };
            let _ = write!(
                out,
                "- {mark} **{}** — {}",
                finding.component, finding.message
            );
            if !finding.action.trim().is_empty() {
                let _ = write!(out, " → {}", finding.action);
            }
            let _ = writeln!(out);
            if let Some(chain) = &finding.chain
                && !chain.confirm.trim().is_empty()
            {
                let _ = writeln!(out, "  - _{}: {}_", "check".tr(ru), chain.confirm);
            }
        }
    }

    footer(&mut out, ru);
    out
}

/// A car or track id, as something a person reads: `ks_ferrari_488_gt3` is
/// `ferrari 488 gt3`. The id is what the game publishes and the report has no
/// other name for it.
fn name(id: &str) -> String {
    let trimmed = id.trim();
    if trimmed.is_empty() {
        return "—".to_string();
    }
    trimmed
        .strip_prefix("ks_")
        .unwrap_or(trimmed)
        .replace('_', " ")
}

fn footer(out: &mut String, ru: bool) {
    let _ = writeln!(
        out,
        "\n---\n_{} [Pro Engineer](https://proengineer.app) v{}_",
        "Written by".tr(ru),
        env!("CARGO_PKG_VERSION")
    );
}

/// Where reports are kept: beside the CSV exports, in the data folder the
/// driver chose.
pub fn folder(config: &crate::config::AppConfig) -> PathBuf {
    config.resolve_data_path().join("reports")
}

/// A file name for this session that sorts by date and says what it is.
///
/// Every character a file system might refuse is replaced, because a car
/// model is whatever a mod author typed — the same reason the follow-up store
/// names its files by digest.
pub fn file_name(session: &Session<'_>) -> String {
    let when = session
        .laps
        .iter()
        .find(|lap| !lap.from_file && lap.lap_time_ms > 0)
        .map(|lap| {
            format!(
                "{} {}",
                lap.save_date,
                lap.timestamp.chars().take(5).collect::<String>()
            )
        })
        .unwrap_or_default();
    let raw = format!("{} {} {}", when.trim(), session.car, session.track);
    let safe: String = raw
        .trim()
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == ' ' || c == '-' || c == '_' || c == '.' {
                c
            } else {
                '-'
            }
        })
        .collect();
    format!(
        "{}.md",
        if safe.is_empty() {
            "session"
        } else {
            safe.as_str()
        }
    )
}

/// Write the report into [`folder`] and say where it went.
pub fn save(
    config: &crate::config::AppConfig,
    session: &Session<'_>,
    ru: bool,
) -> std::io::Result<PathBuf> {
    save_in(&folder(config), session, ru)
}

/// The same, into a folder of the caller's choosing — for the tests, which
/// must not write where a driver's own reports are.
pub fn save_in(dir: &std::path::Path, session: &Session<'_>, ru: bool) -> std::io::Result<PathBuf> {
    std::fs::create_dir_all(dir)?;
    let path = dir.join(file_name(session));
    crate::atomic_file::write_atomic(&path, markdown(session, ru).as_bytes())?;
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lap(number: i32, ms: i32, sectors: [i32; 3], valid: bool) -> LapData {
        LapData {
            lap_number: number,
            lap_time_ms: ms,
            sectors,
            valid,
            fuel_used: 2.5,
            car_model: "ks_ferrari_488_gt3".to_string(),
            track_name: "spa".to_string(),
            save_date: "2026-10-27".to_string(),
            timestamp: "19:42:10".to_string(),
            ..Default::default()
        }
    }

    fn session<'a>(laps: &'a [LapData], findings: &'a [Recommendation]) -> Session<'a> {
        Session {
            game: "A Simulator",
            car: "ks_ferrari_488_gt3",
            track: "spa",
            driver: "Rgosh",
            laps,
            findings,
            corners: None,
            sector_count: 3,
        }
    }

    /// **The whole evening on one page**, with the numbers the screens show.
    #[test]
    fn a_session_is_written_down_whole() {
        let laps = vec![
            lap(0, 140_500, [45_000, 50_500, 45_000], true),
            lap(1, 139_900, [44_800, 50_600, 44_500], true),
            lap(2, 138_000, [44_000, 50_000, 44_000], false),
        ];
        let findings = vec![Recommendation {
            component: "Tyres".to_string(),
            category: "Pressure".to_string(),
            severity: Severity::Warning,
            message: "Fronts over 28.4 psi".to_string(),
            action: "Take 0.3 psi out".to_string(),
            ..Default::default()
        }];
        let text = markdown(&session(&laps, &findings), false);

        assert!(text.starts_with("# ferrari 488 gt3 · spa"), "{text}");
        assert!(text.contains("2026-10-27 19:42"), "when it was");
        assert!(text.contains("Rgosh"));
        assert!(text.contains("**Best lap:** 2:19.900 — lap 2"), "{text}");
        assert!(
            text.contains("**Best sectors together:** 2:19.800"),
            "the cut lap's sectors are not a best: {text}"
        );
        assert!(
            text.contains("~~2:18.000~~"),
            "an invalid lap is struck through"
        );
        assert!(text.contains("| 2 | 2:19.900 | **best** |"), "{text}");
        assert!(text.contains("+0.600"), "the gap to the best");
        assert!(text.contains("🟡 **Tyres** — Fronts over 28.4 psi → Take 0.3 psi out"));
        assert!(text.contains("Consistency"));
        assert!(text.contains("Pro Engineer"));
    }

    /// A two-sector track has two columns, not an empty third.
    #[test]
    fn only_the_sectors_the_track_has() {
        let laps = vec![lap(0, 90_000, [45_000, 45_000, 0], true)];
        let mut two = session(&laps, &[]);
        two.sector_count = 2;
        let text = markdown(&two, false);
        assert!(text.contains("| S1 | S2 |"));
        assert!(!text.contains("S3"));
    }

    /// Nothing driven is said, rather than a table of nothing.
    #[test]
    fn a_session_with_no_laps_says_so() {
        let text = markdown(&session(&[], &[]), false);
        assert!(text.contains("No lap was finished"));
        assert!(!text.contains("| Lap |"));
    }

    /// A mod's car name cannot make a path.
    #[test]
    fn the_file_name_is_safe_and_sorts_by_date() {
        let laps = vec![lap(0, 90_000, [0; 3], true)];
        let mut odd = session(&laps, &[]);
        odd.car = "../../evil:car";
        let name = file_name(&odd);
        assert!(name.starts_with("2026-10-27 19-42"), "{name}");
        assert!(!name.contains('/') && !name.contains(':'), "{name}");
        assert!(name.ends_with(".md"));
    }

    /// And it lands where it says it did.
    #[test]
    fn a_report_is_saved() {
        let dir = std::env::temp_dir().join(format!("acpe-report-{}", std::process::id()));
        let laps = vec![lap(0, 90_000, [30_000; 3], true)];
        let path = save_in(&dir, &session(&laps, &[]), true).expect("it writes");
        let written = std::fs::read_to_string(&path).expect("it reads");
        assert!(written.contains("1:30.000"));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
