//! Re-launching the application inside a terminal emulator.
//!
//! A TUI double-clicked from a file manager has no terminal attached: stdout
//! is a pipe or /dev/null, `crossterm` cannot enter raw mode, and the process
//! either dies immediately or runs invisibly. From the user's point of view
//! nothing happens at all.
//!
//! When there is no controlling terminal, the application starts one and runs
//! itself inside it. The terminal is asked for a size large enough for the UI
//! up front, so the layout is right on the first frame rather than after the
//! user drags the window bigger.

use std::path::PathBuf;
use std::process::Command;

/// Environment variable set on the child so it never tries to relaunch again.
///
/// A terminal emulator that starts but cannot run the command, or a stdout
/// that is still not a tty for some reason we did not anticipate, would
/// otherwise fork a new terminal forever.
const GUARD_VAR: &str = "AC_PRO_ENGINEER_RELAUNCHED";

/// Columns and rows requested from the terminal. Matches the size the UI
/// asks for at startup, so nothing has to be resized after the fact.
const COLS: u16 = 140;
const ROWS: u16 = 40;

/// How a given terminal emulator is asked for a size and a command.
///
/// The `-e`/`--` convention is not shared: some take the command as separate
/// arguments after a flag, others take a single string. Getting this wrong
/// means the terminal opens to a shell instead of the application, which is
/// why each is spelled out rather than guessed at.
struct TerminalSpec {
    binary: &'static str,
    /// Arguments before the command, including any size request.
    args: &'static [&'static str],
    /// Whether the size arguments use the `WxH` geometry form.
    geometry: Option<GeometryStyle>,
}

enum GeometryStyle {
    /// `-geometry 140x40`, as X11 clients have always taken it.
    XGeometry,
    /// `--geometry=140x40`.
    LongGeometry,
}

/// Terminals to try, in order.
///
/// `x-terminal-emulator` first because on Debian derivatives it is the user's
/// configured choice rather than ours. After that, the terminals most likely
/// to be present on a gaming Linux box, ending with xterm, which is the one
/// thing nearly always installed.
const TERMINALS: &[TerminalSpec] = &[
    TerminalSpec {
        binary: "x-terminal-emulator",
        args: &["-e"],
        geometry: Some(GeometryStyle::XGeometry),
    },
    TerminalSpec {
        binary: "konsole",
        args: &["-e"],
        geometry: None,
    },
    TerminalSpec {
        binary: "alacritty",
        args: &["-e"],
        geometry: None,
    },
    TerminalSpec {
        binary: "kitty",
        args: &[],
        geometry: None,
    },
    TerminalSpec {
        binary: "wezterm",
        args: &["start", "--"],
        geometry: None,
    },
    TerminalSpec {
        binary: "foot",
        args: &[],
        geometry: Some(GeometryStyle::LongGeometry),
    },
    TerminalSpec {
        binary: "gnome-terminal",
        args: &["--"],
        geometry: Some(GeometryStyle::LongGeometry),
    },
    TerminalSpec {
        binary: "xfce4-terminal",
        args: &["-e"],
        geometry: Some(GeometryStyle::LongGeometry),
    },
    TerminalSpec {
        binary: "xterm",
        args: &["-e"],
        geometry: Some(GeometryStyle::XGeometry),
    },
];

/// Whether this process should relaunch itself inside a terminal.
///
/// Only when there is no terminal to draw on, and only once — the guard
/// variable stops a child that still has no tty from forking endlessly.
pub fn needs_terminal() -> bool {
    if std::env::var_os(GUARD_VAR).is_some() {
        return false;
    }
    !std::io::IsTerminal::is_terminal(&std::io::stdout())
}

/// Extra arguments a terminal needs to open at the requested size.
fn size_args(spec: &TerminalSpec) -> Vec<String> {
    match spec.geometry {
        Some(GeometryStyle::XGeometry) => {
            vec!["-geometry".to_string(), format!("{COLS}x{ROWS}")]
        }
        Some(GeometryStyle::LongGeometry) => vec![format!("--geometry={COLS}x{ROWS}")],
        // kitty, konsole, alacritty and wezterm all size from their own
        // config or from the shell's SIGWINCH; the app's own resize request
        // covers them, and passing a flag they do not know is fatal.
        None => Vec::new(),
    }
}

/// Start `exe` inside the first terminal emulator that works.
///
/// Returns the name of the terminal that accepted the job. `Err` means every
/// candidate was missing or refused to start, in which case the caller should
/// carry on and let the application fail visibly rather than silently.
pub fn relaunch_in_terminal(exe: &PathBuf, args: &[String]) -> Result<&'static str, String> {
    let mut attempts = Vec::new();

    for spec in TERMINALS {
        if which(spec.binary).is_none() {
            continue;
        }

        let mut command = Command::new(spec.binary);
        command.args(size_args(spec));
        command.args(spec.args);
        command.arg(exe);
        command.args(args);
        command.env(GUARD_VAR, "1");

        match command.spawn() {
            Ok(_) => return Ok(spec.binary),
            Err(error) => attempts.push(format!("{}: {error}", spec.binary)),
        }
    }

    if attempts.is_empty() {
        Err("no terminal emulator found".to_string())
    } else {
        Err(attempts.join("; "))
    }
}

/// Whether a binary is on PATH.
///
/// A three-line lookup rather than a dependency: this is the only thing the
/// crate would use one for.
fn which(binary: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path)
        .map(|dir| dir.join(binary))
        .find(|candidate| candidate.is_file())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_guard_variable_stops_a_relaunch_loop() {
        // Safety: single-threaded test, and the variable is only read by
        // `needs_terminal`.
        unsafe { std::env::set_var(GUARD_VAR, "1") };
        assert!(
            !needs_terminal(),
            "a process we already relaunched must never relaunch again"
        );
        unsafe { std::env::remove_var(GUARD_VAR) };
    }

    #[test]
    fn x_geometry_terminals_get_the_traditional_flag() {
        let spec = TerminalSpec {
            binary: "xterm",
            args: &["-e"],
            geometry: Some(GeometryStyle::XGeometry),
        };
        assert_eq!(size_args(&spec), vec!["-geometry", "140x40"]);
    }

    #[test]
    fn long_geometry_terminals_get_the_joined_form() {
        let spec = TerminalSpec {
            binary: "foot",
            args: &[],
            geometry: Some(GeometryStyle::LongGeometry),
        };
        assert_eq!(size_args(&spec), vec!["--geometry=140x40"]);
    }

    /// Passing a size flag a terminal does not understand is fatal, so the
    /// ones that size themselves get nothing.
    #[test]
    fn terminals_without_a_geometry_flag_get_no_size_arguments() {
        let spec = TerminalSpec {
            binary: "kitty",
            args: &[],
            geometry: None,
        };
        assert!(size_args(&spec).is_empty());
    }

    #[test]
    fn which_finds_a_binary_that_exists_and_not_one_that_does_not() {
        assert!(which("sh").is_some(), "sh is on PATH everywhere");
        assert!(which("definitely-not-a-real-binary-xyzzy").is_none());
    }

    /// The requested size has to match what the UI asks for, or the terminal
    /// opens at one size and is immediately resized to another.
    #[test]
    fn the_requested_size_is_the_size_the_ui_wants() {
        assert_eq!((COLS, ROWS), (140, 40));
    }
}
