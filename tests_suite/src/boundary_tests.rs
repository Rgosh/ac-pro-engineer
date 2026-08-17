//! One game's memory layout stays inside that game's folder.
//!
//! `core/src/games/` was drawn as the boundary between a simulator and the rest
//! of the program, and until v0.3.7 it did not carry data: `Source` returned
//! whether the game was running, and the telemetry came out beside it through
//! `physics()`, `graphics()` and `stat()` handing over Assetto Corsa's own
//! shared-memory structs. The engineer, the analyser and five screens read AC's
//! layout directly — 109 references across ten files — so the boundary was real
//! as a folder and nothing more.
//!
//! That is a mistake nobody can catch by reading a diff. Reaching for
//! `AcPhysics` compiles, runs, and looks exactly like the code beside it; it is
//! only wrong in the sense that it puts one simulator's field names somewhere a
//! second simulator cannot follow. So it is a test, and it walks the tree.

use std::fs;
use std::path::{Path, PathBuf};

/// The shared-memory pages of both readable games. Nothing outside the game
/// folder may name one.
///
/// Competizione's are on this list from the day they were written rather than
/// after the fact, which is the whole value of having the test: `AcPhysics`
/// spread to 109 references across ten files before anybody noticed, and it
/// spread because reaching for it compiles and looks exactly like the code
/// beside it.
const GAME_STRUCTS: &[&str] = &[
    "AcPhysics",
    "AcGraphics",
    "AcStatic",
    "AccPhysics",
    "AccGraphics",
    "AccStatic",
];

/// How each game is laid out on disk and in the process table.
///
/// The structs were only half of it. A neutral module that knows setups are
/// INI under `Documents/Assetto Corsa/setups`, that cars are JSON under
/// `content/cars`, that the process is `acs.exe` or that Steam calls this game
/// 244210 is a module a second simulator cannot reuse — and none of that
/// announces itself in a type name.
const GAME_LAYOUT: &[&str] = &[
    "\"Assetto Corsa\"",
    "acs.exe",
    "ui_car.json",
    "acpmf_",
    "244210",
    // And Competizione's, which is the same rule and a sharper case: it shares
    // `acpmf_` with Assetto Corsa, so the only things that tell the two apart
    // outside the pages themselves are the process name and the appid.
    "\"Assetto Corsa Competizione\"",
    "AC2-Win64-Shipping.exe",
    "805550",
];

/// Files outside `games/` that legitimately speak Assetto Corsa, and why.
///
/// Both of these exist *because* of AC's byte layout rather than in spite of
/// it, which is the only reason to be on this list. Neither reads telemetry to
/// act on it: one writes the pages and one measures them.
const SPEAKS_AC: &[&str] = &[
    // Writes fake `acpmf_*` pages for developing without the game. It is a
    // stand-in for AC, so it has to lay the bytes out the way AC does.
    "tui/src/bin/simulator.rs",
    // The generic process matcher, whose *tests* use `acs.exe` as the string
    // to match. The matcher itself takes the name as an argument.
    "core/src/process.rs",
];

/// Folders exempt from the layout rule, and why.
///
/// `overlay/` installs a Custom Shaders Patch Lua app into the game's own
/// folder and drives `shm-bridge.exe` inside its Proton prefix. CSP is an
/// Assetto Corsa mod and ACC has none, so this is not a boundary a second game
/// crosses — it is a feature one game has. See §8 of `docs/roadmap.md`, which
/// is where that decision is written down.
///
/// `tui/src/platform/` is deliberately **not** on this list any more. It
/// launches `shm-bridge.exe` inside a Proton prefix, and it used to be given
/// Assetto Corsa's appid as a constant — which is the same as saying this
/// program reads one game. It asks the registry which prefix now, so it has
/// stopped knowing what a simulator is, and the test is what keeps it that
/// way.
const SPEAKS_AC_FOLDERS: &[&str] = &["core/src/overlay/"];

/// The terminal must not name a simulator.
///
/// This is the rule the registry exists for. Whichever game is being read is
/// an entry in `games::registry`, and a screen that reaches past it for
/// `assetto_corsa::` is a screen that has to be edited again for the second
/// game — which is exactly the ten call sites the registry replaced.
const INTERFACE_DIRS: &[&str] = &["tui/src"];

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("tests_suite sits inside the workspace")
        .to_path_buf()
}

fn rust_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            rust_files(&path, out);
        } else if path.extension().is_some_and(|e| e == "rs") {
            out.push(path);
        }
    }
}

/// Lines naming a game's page struct in code — a comment may still discuss one.
fn game_struct_lines(source: &str) -> Vec<(usize, String)> {
    source
        .lines()
        .enumerate()
        .filter(|(_, line)| {
            let trimmed = line.trim_start();
            !trimmed.starts_with("//") && GAME_STRUCTS.iter().any(|name| line.contains(name))
        })
        .map(|(index, line)| (index + 1, line.trim().to_string()))
        .collect()
}

/// No screen names a simulator: they ask the registry which game is being
/// read, and take what it hands back.
#[test]
fn the_interface_does_not_name_a_game() {
    let root = workspace_root();

    let mut files = Vec::new();
    for dir in INTERFACE_DIRS {
        rust_files(&root.join(dir), &mut files);
    }
    files.sort();

    let mut problems = Vec::new();
    for file in &files {
        let relative = file
            .strip_prefix(&root)
            .expect("walked from the root")
            .to_string_lossy()
            .replace('\\', "/");

        if SPEAKS_AC.contains(&relative.as_str())
            || SPEAKS_AC_FOLDERS
                .iter()
                .any(|folder| relative.starts_with(folder))
        {
            continue;
        }
        let Ok(source) = fs::read_to_string(file) else {
            continue;
        };
        for (number, line) in source.lines().enumerate() {
            let trimmed = line.trim_start();
            if trimmed.starts_with("//") {
                continue;
            }
            if line.contains("games::assetto_corsa") || line.contains("assetto_corsa::") {
                problems.push(format!("{relative}:{}: {trimmed}", number + 1));
            }
        }
    }

    assert!(
        problems.is_empty(),
        "the interface names a simulator:\n{}\n\n\
         Ask `games::registry` instead — `AppState::game` is the entry being \
         read, and its `Backend` holds the connection, the process names, the \
         car scan and the setup store.",
        problems.join("\n")
    );
}

/// A game's file layout, its process and its Steam appid stay in its own
/// folder, so that a second game is a folder beside it — which it now is.
#[test]
fn no_file_outside_the_game_folder_knows_where_a_game_keeps_things() {
    let root = workspace_root();

    let mut files = Vec::new();
    rust_files(&root.join("core/src"), &mut files);
    rust_files(&root.join("tui/src"), &mut files);
    files.sort();

    let mut problems = Vec::new();
    for file in &files {
        let relative = file
            .strip_prefix(&root)
            .expect("walked from the root")
            .to_string_lossy()
            .replace('\\', "/");

        if relative.starts_with("core/src/games/")
            || SPEAKS_AC.contains(&relative.as_str())
            || SPEAKS_AC_FOLDERS
                .iter()
                .any(|folder| relative.starts_with(folder))
        {
            continue;
        }
        let Ok(source) = fs::read_to_string(file) else {
            continue;
        };
        for (number, line) in source.lines().enumerate() {
            let trimmed = line.trim_start();
            if trimmed.starts_with("//") {
                continue;
            }
            if let Some(marker) = GAME_LAYOUT.iter().find(|marker| line.contains(**marker)) {
                problems.push(format!(
                    "{relative}:{}: {marker} in {}",
                    number + 1,
                    trimmed
                ));
            }
        }
    }

    assert!(
        problems.is_empty(),
        "Assetto Corsa's own layout outside `core/src/games/`:\n{}\n\n\
         Where a game keeps its files, what its process is called and what \
         Steam numbers it are facts about that game. Put them in its folder \
         and hand back the neutral shape — `CarSpecs`, `CarSetup`, a `bool`.",
        problems.join("\n")
    );
}

/// The reason §4 of the roadmap existed: the trait describing the boundary was
/// bypassed by everything that mattered.
#[test]
fn no_file_outside_the_game_folder_reads_a_games_layout() {
    let root = workspace_root();

    let mut files = Vec::new();
    rust_files(&root.join("core/src"), &mut files);
    rust_files(&root.join("tui/src"), &mut files);
    files.sort();

    let mut problems = Vec::new();
    for file in &files {
        let relative = file
            .strip_prefix(&root)
            .expect("walked from the root")
            .to_string_lossy()
            .replace('\\', "/");

        if relative.starts_with("core/src/games/") || SPEAKS_AC.contains(&relative.as_str()) {
            continue;
        }
        let Ok(source) = fs::read_to_string(file) else {
            continue;
        };
        for (line_number, text) in game_struct_lines(&source) {
            problems.push(format!("{relative}:{line_number}: {text}"));
        }
    }

    assert!(
        problems.is_empty(),
        "Assetto Corsa's shared-memory structs outside `core/src/games/`:\n{}\n\n\
         Take the reading instead — `games::Reading` and its `Car`, `Session` \
         and `Fixed` — and convert in `games/assetto_corsa/reading.rs`. \
         A second simulator cannot follow a field named after AC's page.",
        problems.join("\n")
    );
}

/// The other half of the same rule, and the one that let the first half happen:
/// telemetry has to leave the game through the trait.
///
/// The accessors this replaced were on the concrete type, so the trait could be
/// held and ignored at the same time — `tui/src/lib.rs` did exactly that, with
/// an `AssettoCorsa` in a field and five direct calls into it.
#[test]
fn the_only_way_out_of_a_game_is_the_trait() {
    // Every game, not only the one it happened to: the rule is about the
    // shape of a `Source`, and a second game is exactly where a shortcut back
    // to the raw pages would be reintroduced.
    for game in ["assetto_corsa", "assetto_corsa_competizione"] {
        let module = workspace_root().join(format!("core/src/games/{game}/mod.rs"));
        let source = fs::read_to_string(&module).expect("the game's module is in the tree");

        for accessor in ["fn physics(", "fn graphics(", "fn stat("] {
            assert!(
                !source.contains(accessor),
                "`{accessor})` is back on {game}'s source. Telemetry leaves a \
                 game through `Source::poll`, which returns a `Reading`; an \
                 accessor beside it is a way to read the game's own structs \
                 without the trait noticing, which is how the boundary stopped \
                 carrying data."
            );
        }
    }
}
