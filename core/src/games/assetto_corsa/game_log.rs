//! Why a session never began — read out of the game's own log.
//!
//! **The most common complaint about every launcher for this game is a
//! session that ends before it starts, with nothing said but "race
//! cancelled".** The reason is nearly always written down: the game logs
//! every file it fails to open and every error it stops on, in
//! `Documents/Assetto Corsa/logs/log.txt`, rewritten on each start. People are
//! told to go and read it. This reads it for them and says the reason in a
//! sentence — which car or circuit, Steam, the patch, or the last error the
//! game wrote before it stopped.
//!
//! **A reading, not a verdict.** The log's wording is the game's, the patch's
//! and every mod's, and none of it is a contract. Each rule here looks for
//! something specific and says only what that line says; when nothing
//! specific is found, the last error is quoted as it is rather than guessed
//! at.

use std::path::{Path, PathBuf};

/// What the reason is about, when it is about one thing in the install.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Subject {
    /// A car, by its folder.
    Car(String),
    /// A circuit, by its folder.
    Track(String),
    /// Steam: not running, or not signed in.
    Steam,
    /// Custom Shaders Patch.
    Patch,
}

/// Why the game stopped, as far as its log says.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Reason {
    /// One sentence for a person.
    pub said: String,
    /// What it is about, when it is about one thing.
    pub subject: Option<Subject>,
    /// The log's own line, for whoever wants the detail.
    pub line: String,
}

/// Where the log is.
pub fn file(documents: &Path) -> PathBuf {
    documents.join("logs").join("log.txt")
}

/// The log as the last start left it, or nothing when there is none.
pub fn read(documents: &Path) -> Option<String> {
    let bytes = std::fs::read(file(documents)).ok()?;
    Some(String::from_utf8_lossy(&bytes).into_owned())
}

/// The log as it is now, when the game wrote it after `since` — or, when it
/// has not been written since, the reason that is: the game closed before it
/// got as far as its log.
///
/// **A log from the session before is not this one's reason.** The game
/// rewrites it early on each start, so a start that dies sooner leaves the
/// last one in place — a session that went well, read as the reason this one
/// did not. The commonest such start is a game that found Steam still
/// signing in, asked Steam to start it again and closed.
pub fn why_since(documents: &Path, since: std::time::SystemTime) -> Option<Reason> {
    let at = file(documents);
    let written = std::fs::metadata(&at).and_then(|m| m.modified()).ok();
    if written.is_some_and(|written| written < since) {
        return Some(Reason {
            said: "The game closed before it wrote its log — most often Steam was still \
                   signing in. Press DRIVE again once Steam is up."
                .to_string(),
            subject: Some(Subject::Steam),
            line: "log.txt is still the one from the session before".to_string(),
        });
    }
    why(&read(documents)?)
}

/// Whether a line reports something going wrong. Not the INI reader's notes
/// about keys a file lacks: every session writes dozens of those, and they
/// are the game falling back to a default, not failing. Not a `WARNING`
/// either — the game and the patch write them in sessions that go well,
/// `TaskBarIcon :: COULD NOT FIND TEXTURES FOR …` for every app without an
/// icon among them.
fn is_error(line: &str) -> bool {
    let lower = line.to_ascii_lowercase();
    if lower.contains("key_not_found")
        || lower.contains("inireader")
        || lower.starts_with("warning")
    {
        return false;
    }
    [
        "error",
        "exception",
        "crash",
        "could not",
        "cannot",
        "can't",
        "failed",
        "not found",
        "missing",
    ]
    .iter()
    .any(|word| lower.contains(word))
}

/// The folder a path in the line names under `content/<kind>/`, if any.
fn named(line: &str, kind: &str) -> Option<String> {
    let lower = line.to_ascii_lowercase().replace('\\', "/");
    let marker = format!("content/{kind}/");
    let at = lower.find(&marker)? + marker.len();
    // The original spelling, from the same place: folder names are matched
    // by the game without regard to case, but said back as written.
    let original = line.replace('\\', "/");
    let rest = original.get(at..)?;
    let name: String = rest
        .chars()
        .take_while(|c| !matches!(c, '/' | '"' | '\'' | ' ' | ')' | ']' | ','))
        .collect();
    (!name.is_empty()).then_some(name)
}

/// Why the game stopped, from its log. `None` when the log holds no error at
/// all — a session somebody quit on purpose.
pub fn why(text: &str) -> Option<Reason> {
    // A session that ended as the game ends one on purpose went as far as it
    // was meant to.
    if text.lines().rev().take(8).any(|l| l.trim() == "CLEAN EXIT") {
        return None;
    }
    let errors: Vec<&str> = text
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && is_error(l))
        .collect();
    let last = errors.last()?;

    // Steam first: without it nothing else in the log matters.
    if let Some(line) = errors.iter().find(|l| {
        let lower = l.to_ascii_lowercase();
        lower.contains("steam") && !lower.contains("steamapps")
    }) {
        return Some(Reason {
            said: "Steam was not ready — start Steam and sign in, then drive again.".to_string(),
            subject: Some(Subject::Steam),
            line: (*line).to_string(),
        });
    }
    // A car or a circuit the game could not load: the line names its folder.
    if let Some((line, car)) = errors
        .iter()
        .find_map(|l| named(l, "cars").map(|c| (*l, c)))
    {
        return Some(Reason {
            said: format!(
                "The car {car} could not be loaded — a file it needs is missing or broken. \
                 Reinstall it, or choose another."
            ),
            subject: Some(Subject::Car(car)),
            line: line.to_string(),
        });
    }
    if let Some((line, track)) = errors
        .iter()
        .find_map(|l| named(l, "tracks").map(|t| (*l, t)))
    {
        return Some(Reason {
            said: format!(
                "The circuit {track} could not be loaded — a file it needs is missing or \
                 broken. Reinstall it, or choose another."
            ),
            subject: Some(Subject::Track(track)),
            line: line.to_string(),
        });
    }
    if let Some(line) = errors.iter().find(|l| {
        let lower = l.to_ascii_lowercase();
        lower.contains("shaders patch") || lower.contains("extension/") || has_word(&lower, "csp")
    }) {
        return Some(Reason {
            said: "Custom Shaders Patch stopped the session. Try another build of it, or \
                   switch it off for this car or circuit."
                .to_string(),
            subject: Some(Subject::Patch),
            line: (*line).to_string(),
        });
    }
    Some(Reason {
        said: "The game stopped before the session began. Its last error is below.".to_string(),
        subject: None,
        line: (*last).to_string(),
    })
}

/// Whether `word` is in `text` on its own, not inside a longer name:
/// `csp` is the patch, `csprstats` is an app's.
fn has_word(text: &str, word: &str) -> bool {
    text.match_indices(word).any(|(at, _)| {
        let before = text[..at].chars().next_back();
        let after = text[at + word.len()..].chars().next();
        !before.is_some_and(|c| c.is_ascii_alphanumeric() || c == '_')
            && !after.is_some_and(|c| c.is_ascii_alphanumeric() || c == '_')
    })
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use super::*;

    #[test]
    fn a_clean_log_is_no_reason() {
        let text = "Assetto Corsa started\nINIReader: cfg/video.ini > KEY_NOT_FOUND: [X] Y\n\
                    LOADING CAR ks_ferrari_488_gt3\nRACE STARTED\n";
        assert_eq!(why(text), None);
        assert_eq!(why(""), None);
    }

    /// What stopped the owner's session read as the patch: a warning about
    /// an app's icon, in a log that ended as a clean session ends.
    #[test]
    fn a_warning_about_an_icon_is_not_the_patch_stopping_anything() {
        let text = "WARNING: TaskBarIcon :: COULD NOT FIND TEXTURES FOR CSPRSTATS_ICON\n\
                    LAP VALID: 1 CUTS:0, COUNT: 1\n";
        assert_eq!(why(text), None);
        let text = "ERROR: something about csprstats_app could not start\n";
        assert_ne!(why(text).unwrap().subject, Some(Subject::Patch));
        let text = "ERROR: CSP could not load its config\n";
        assert_eq!(why(text).unwrap().subject, Some(Subject::Patch));
    }

    #[test]
    fn a_clean_exit_is_no_reason() {
        let text = "ERROR: cannot open file content/cars/x/data.acd\nSaving replay\nCLEAN EXIT\n";
        assert_eq!(why(text), None);
    }

    /// A start that died before the log was written is not explained by the
    /// session before it.
    #[test]
    fn a_log_from_before_the_start_is_not_this_start_s_reason() {
        let documents = std::env::temp_dir().join("game_log_stale");
        let _ = std::fs::remove_dir_all(&documents);
        std::fs::create_dir_all(documents.join("logs")).unwrap();
        std::fs::write(
            file(&documents),
            "ERROR: cannot open content/cars/x/data.acd\n",
        )
        .unwrap();
        let later = std::time::SystemTime::now() + std::time::Duration::from_secs(60);
        assert_eq!(
            why_since(&documents, later).unwrap().subject,
            Some(Subject::Steam)
        );
        let earlier = std::time::SystemTime::now() - std::time::Duration::from_secs(60);
        assert_eq!(
            why_since(&documents, earlier).unwrap().subject,
            Some(Subject::Car("x".to_string()))
        );
    }

    #[test]
    fn a_car_that_did_not_load_is_named_as_written() {
        let text = "loading\nERROR: cannot open file content\\cars\\Ks_Ferrari_488_GT3\\data.acd\n\
                    something else\n";
        let reason = why(text).unwrap();
        assert_eq!(
            reason.subject,
            Some(Subject::Car("Ks_Ferrari_488_GT3".to_string()))
        );
        assert!(reason.said.contains("Ks_Ferrari_488_GT3"));
        assert!(reason.line.contains("data.acd"));
    }

    #[test]
    fn a_circuit_that_did_not_load_is_named() {
        let text = "ERROR: Could not find file \"content/tracks/spa/data/surfaces.ini\"\n";
        let reason = why(text).unwrap();
        assert_eq!(reason.subject, Some(Subject::Track("spa".to_string())));
    }

    #[test]
    fn steam_comes_before_everything_else() {
        let text = "ERROR: content/cars/x/data.acd missing\nSteam API failed to initialise\n";
        let reason = why(text).unwrap();
        assert_eq!(reason.subject, Some(Subject::Steam));
        // A path through steamapps is not about Steam.
        let text = "ERROR: could not open steamapps/common/assettocorsa/content/tracks/imola/x\n";
        let reason = why(text).unwrap();
        assert_eq!(reason.subject, Some(Subject::Track("imola".to_string())));
    }

    #[test]
    fn the_patch_is_named_and_anything_else_is_quoted() {
        let text = "Custom Shaders Patch: fatal error in module X\n";
        let reason = why(text).unwrap();
        assert_eq!(reason.subject, Some(Subject::Patch));
        let text = "first\nException: something odd\n";
        let reason = why(text).unwrap();
        assert_eq!(reason.subject, None);
        assert_eq!(reason.line, "Exception: something odd");
    }

    #[test]
    fn the_log_is_read_even_when_it_is_not_utf8() {
        let documents = std::env::temp_dir().join(format!("ac-log-{}", std::process::id()));
        let _ = std::fs::create_dir_all(documents.join("logs"));
        let _ = std::fs::write(file(&documents), b"ERROR: bad \xff byte\n");
        assert!(read(&documents).is_some_and(|t| t.contains("bad")));
        let _ = std::fs::remove_dir_all(&documents);
    }
}
