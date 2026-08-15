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

/// AC's three shared-memory pages. Nothing outside the game folder may name one.
const AC_STRUCTS: &[&str] = &["AcPhysics", "AcGraphics", "AcStatic"];

/// Files outside `games/` that legitimately speak Assetto Corsa, and why.
///
/// Both of these exist *because* of AC's byte layout rather than in spite of
/// it, which is the only reason to be on this list. Neither reads telemetry to
/// act on it: one writes the pages and one measures them.
const SPEAKS_AC: &[&str] = &[
    // Writes fake `acpmf_*` pages for developing without the game. It is a
    // stand-in for AC, so it has to lay the bytes out the way AC does.
    "tui/src/bin/simulator.rs",
];

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

/// Lines naming an AC struct in code — a comment may still discuss one.
fn ac_struct_lines(source: &str) -> Vec<(usize, String)> {
    source
        .lines()
        .enumerate()
        .filter(|(_, line)| {
            let trimmed = line.trim_start();
            !trimmed.starts_with("//") && AC_STRUCTS.iter().any(|name| line.contains(name))
        })
        .map(|(index, line)| (index + 1, line.trim().to_string()))
        .collect()
}

/// The reason §4 of the roadmap existed: the trait describing the boundary was
/// bypassed by everything that mattered.
#[test]
fn no_file_outside_the_game_folder_reads_assetto_corsas_layout() {
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
        for (line_number, text) in ac_struct_lines(&source) {
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
    let source = fs::read_to_string(workspace_root().join("core/src/games/assetto_corsa/mod.rs"))
        .expect("the Assetto Corsa module is in the tree");

    for accessor in ["fn physics(", "fn graphics(", "fn stat("] {
        assert!(
            !source.contains(accessor),
            "`{accessor})` is back on `AssettoCorsa`. Telemetry leaves a game \
             through `Source::poll`, which returns a `Reading`; an accessor \
             beside it is a way to read AC's own structs without the trait \
             noticing, which is how the boundary stopped carrying data."
        );
    }
}
