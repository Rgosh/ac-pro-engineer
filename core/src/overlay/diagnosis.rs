//! Everything the application knows about whether the overlay can work, as
//! data rather than as printed lines.
//!
//! Three pieces have to agree about a frame — the application, the panel and
//! the bridge — and every failure looks identical from the driving seat: the
//! panel saying *waiting for Pro Engineer* with the file sitting in
//! `/dev/shm`, at the right size, with the application running.
//!
//! That question had exactly one answer, and it was
//! `cargo run -p ac_core --example bridge_probe` — a command a driver who
//! downloaded a release cannot run, and would have no reason to know about.
//! The report lives here now, and the example and the Settings screen both
//! render the same thing, so the answer a user reads is the answer a bug
//! report quotes.

use std::fmt::Write as _;

use crate::overlay::bridge::{self, BridgeStatus};
use crate::overlay::frame::{OVERLAY_MMF_NAME, OVERLAY_VERSION, OverlayFrame};
use crate::updater::CURRENT_VERSION;

/// How a line should read, so the caller can colour it without parsing it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tone {
    /// A section title.
    Heading,
    /// A fact with no verdict attached.
    Plain,
    /// Working.
    Good,
    /// Works, but not what this build ships.
    Warn,
    /// The overlay cannot work until this is dealt with.
    Bad,
    /// Something to type or do.
    Action,
}

/// One line of the report. `label` is empty for headings and actions.
#[derive(Debug, Clone)]
pub struct Line {
    pub tone: Tone,
    pub label: String,
    pub value: String,
}

impl Line {
    fn new(tone: Tone, label: &str, value: impl Into<String>) -> Self {
        Self {
            tone,
            label: label.to_string(),
            value: value.into(),
        }
    }

    fn heading(text: &str) -> Self {
        Self::new(Tone::Heading, "", text)
    }

    fn action(text: impl Into<String>) -> Self {
        Self::new(Tone::Action, "", text)
    }
}

/// The whole answer: the lines to show, and the one-sentence verdict.
#[derive(Debug, Clone)]
pub struct Report {
    pub lines: Vec<Line>,
    pub verdict: String,
    pub workable: bool,
}

impl Report {
    /// The report as the text a bug report should carry.
    ///
    /// Same content as the screen, so "quote what it said" needs no
    /// transcription.
    pub fn to_plain_text(&self) -> String {
        let mut out = String::new();
        for line in &self.lines {
            match line.tone {
                Tone::Heading => {
                    let _ = writeln!(out, "\n{}", line.value);
                }
                Tone::Action => {
                    let _ = writeln!(out, "    {}", line.value);
                }
                _ => {
                    let _ = writeln!(out, "  {:<17}{}", line.label, line.value);
                }
            }
        }
        let _ = writeln!(out, "\noverlay {}", self.verdict);
        out
    }
}

/// Ask every piece where it stands.
pub fn report() -> Report {
    let mut lines = Vec::new();

    lines.push(Line::heading("this application"));
    lines.push(Line::new(Tone::Plain, "release", CURRENT_VERSION));
    lines.push(Line::new(
        Tone::Plain,
        "frame version",
        OVERLAY_VERSION.to_string(),
    ));
    lines.push(Line::new(
        Tone::Plain,
        "frame size",
        format!("{} bytes", size_of::<OverlayFrame>()),
    ));
    lines.push(Line::new(Tone::Plain, "mapping", OVERLAY_MMF_NAME));
    lines.push(Line::new(
        Tone::Plain,
        "bridge protocol",
        bridge::BRIDGE_PROTOCOL.to_string(),
    ));

    lines.push(Line::heading("shm-bridge.exe on disk"));
    match bridge::installed_executable() {
        Some(path) => {
            lines.push(Line::new(Tone::Plain, "found", path.display().to_string()));
            match bridge::version_in_executable(&path) {
                Some(version) => {
                    let tone = if version == CURRENT_VERSION {
                        Tone::Good
                    } else {
                        Tone::Warn
                    };
                    lines.push(Line::new(tone, "version", version));
                }
                // A bridge built before the marker existed is, by that fact,
                // older than this check — which is itself the answer.
                None => lines.push(Line::new(
                    Tone::Warn,
                    "version",
                    "unknown — built before the version marker, so it predates this check",
                )),
            }
        }
        None => lines.push(Line::new(
            Tone::Bad,
            "not found",
            "looked beside this executable and in the working directory",
        )),
    }

    lines.push(Line::heading("shm-bridge.exe running"));
    lines.push(Line::new(
        Tone::Plain,
        "announced in",
        bridge::info_path().display().to_string(),
    ));

    let status = bridge::status(CURRENT_VERSION);
    match &status {
        BridgeStatus::NotRequired => lines.push(Line::new(
            Tone::Good,
            "not required",
            "Windows maps this directly; there is no bridge",
        )),
        BridgeStatus::NotRunning => {
            lines.push(Line::new(
                Tone::Bad,
                "not running",
                "nothing has announced itself",
            ));
            lines.push(Line::action("Start it in the game's Proton prefix:"));
            lines.push(Line::action(
                "protontricks-launch --appid 244210 shm-bridge.exe",
            ));
        }
        BridgeStatus::Unannounced => {
            lines.push(Line::new(
                Tone::Bad,
                "no announcement",
                "but AC's pages are mapped, so a bridge is running",
            ));
            lines.push(Line::new(
                Tone::Bad,
                "TOO OLD",
                "it predates the announcement, and every bridge that old maps AC's four \
                 pages and nothing else — no overlay mapping is ever created, so the panel \
                 waits forever",
            ));
            lines.push(Line::action("Build one from a checkout:"));
            lines.push(Line::action(
                "cargo build --release -p shm-bridge --target x86_64-pc-windows-gnu",
            ));
        }
        BridgeStatus::Unreadable(why) => {
            lines.push(Line::new(Tone::Bad, "unreadable", why.clone()))
        }
        BridgeStatus::Incompatible { info, complaint } => {
            describe(&mut lines, info);
            lines.push(Line::new(Tone::Bad, "INCOMPATIBLE", complaint.describe()));
            lines.push(Line::action(
                "The panel will wait forever. Press [B] on the launcher's overlay card to \
                 fetch a published bridge, or build one:",
            ));
            lines.push(Line::action(
                "cargo build --release -p shm-bridge --target x86_64-pc-windows-gnu",
            ));
        }
        BridgeStatus::Behind {
            info,
            expected_version,
        } => {
            describe(&mut lines, info);
            lines.push(Line::new(
                Tone::Warn,
                "BEHIND",
                format!(
                    "this application is v{expected_version}. The frame still fits, so \
                     nothing is broken."
                ),
            ));
        }
        BridgeStatus::Current(info) => {
            describe(&mut lines, info);
            lines.push(Line::new(
                Tone::Good,
                "CURRENT",
                "same release as this application",
            ));
        }
    }

    // "Not workable" covers two different problems with one remedy each, so
    // the verdict names which rather than sending everyone to rebuild.
    let workable = status.is_workable();
    let verdict = match &status {
        _ if workable => "can work as things stand",
        BridgeStatus::NotRunning => "cannot work until the bridge is started",
        BridgeStatus::Unannounced | BridgeStatus::Incompatible { .. } => {
            "cannot work until the bridge is replaced"
        }
        _ => "cannot work; the bridge could not be identified",
    }
    .to_string();

    Report {
        lines,
        verdict,
        workable,
    }
}

fn describe(lines: &mut Vec<Line>, info: &bridge::BridgeInfo) {
    lines.push(Line::new(Tone::Plain, "version", info.version.clone()));
    lines.push(Line::new(
        Tone::Plain,
        "bridge protocol",
        info.protocol.to_string(),
    ));
    lines.push(Line::new(
        Tone::Plain,
        "maps",
        format!("{} bytes", info.frame_bytes),
    ));
    lines.push(Line::new(Tone::Plain, "as", info.mmf.clone()));
    lines.push(Line::new(Tone::Plain, "wine pid", info.pid.to_string()));
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Whatever this machine's bridge situation is, the report has to describe
    /// all three pieces and reach a verdict — an empty or half-built answer is
    /// worse than no screen at all, because it looks like the check ran.
    #[test]
    fn the_report_covers_all_three_pieces_and_ends_with_a_verdict() {
        let report = report();

        let headings: Vec<&str> = report
            .lines
            .iter()
            .filter(|line| line.tone == Tone::Heading)
            .map(|line| line.value.as_str())
            .collect();
        assert_eq!(
            headings,
            vec![
                "this application",
                "shm-bridge.exe on disk",
                "shm-bridge.exe running"
            ]
        );

        assert!(!report.verdict.is_empty(), "there is always a verdict");

        // The application's own facts are not conditional on anything.
        let text = report.to_plain_text();
        assert!(text.contains(CURRENT_VERSION), "{text}");
        assert!(text.contains(OVERLAY_MMF_NAME), "{text}");
        assert!(
            text.contains(&size_of::<OverlayFrame>().to_string()),
            "the frame size is the number every mismatch is about\n{text}"
        );
    }

    /// A screen that says something is wrong and does not say what to do is a
    /// screen that generates a support question.
    #[test]
    fn anything_that_stops_the_overlay_comes_with_something_to_do() {
        let report = report();
        if report.workable {
            return;
        }
        assert!(
            report.lines.iter().any(|line| line.tone == Tone::Action),
            "not workable, and nothing to act on:\n{}",
            report.to_plain_text()
        );
    }
}
