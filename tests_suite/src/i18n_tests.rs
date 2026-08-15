//! Russian belongs in the dictionary, not in the code.
//!
//! `data/locales/ru.json` holds every translated word; the code says what it
//! means in English and asks for the Russian at the point it draws. This walks
//! the tree and insists on that, because the rule is one nobody can enforce by
//! reading a diff — a new `if ru { "…" } else { "…" }` looks exactly like the
//! five hundred that used to be there.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

/// Files allowed to contain Cyrillic, and why.
///
/// * `i18n.rs` is the loader, and its tests quote Russian to check it.
/// * `keys.rs` maps a Cyrillic key to the Latin one under the same finger, so a
///   rebound letter keeps working when the driver's layout is Russian. Those
///   are keys on a keyboard, not words on a screen, and translating them would
///   be nonsense.
const EXEMPT: &[&str] = &["core/src/i18n.rs", "tui/src/keys.rs"];

/// The Cyrillic that is left, per file, and why each line is not a translation.
///
/// Every translatable string is in the dictionary now. What
/// remains is text that happens to be Russian for a reason other than being a
/// message to a driver, and moving any of it into `i18n.rs` would make the
/// program wrong rather than tidier.
///
/// **Lower a number when a line genuinely goes. Never raise one**, and never
/// add a file without a reason beside it: a new line of Russian in the code
/// fails this test, which is the entire point of the list.
const REMAINING: &[(&str, usize)] = &[
    // Test fixtures that exist *because* the text is Cyrillic: a filename to
    // sanitise, a string whose characters are two bytes each, and the
    // truncation that has to count characters rather than bytes.
    ("tui/src/ui/tabs/analysis/mod.rs", 3),
    ("core/src/setup_manager.rs", 1),
    ("core/src/overlay/frame.rs", 1),
    // A test of the translation itself: it passes a Russian unit in and asserts
    // the Russian phrasing comes back.
    ("core/src/confidence.rs", 2),
    // Keys on a keyboard, like `keys.rs` — quitting works on `й` because that
    // is where `q` is on a Russian layout, and a driver in a race does not stop
    // to switch layout.
    ("tui/src/main.rs", 2),
    // The language selector, which names each language in that language. It has
    // to read the same whichever one is currently active, so both arms are
    // deliberately not translated — that is what makes it findable by somebody
    // who set the wrong one by accident.
    ("tui/src/ui/tabs/settings.rs", 3),
    ("tui/src/ui/launcher.rs", 1),
];

fn workspace_root() -> PathBuf {
    // `tests_suite/` is one below the root, and this crate is never published,
    // so the manifest directory is a stable place to start from.
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

/// Join a string literal that was written across two lines.
///
/// A `\` at the end of a line inside a literal swallows the newline *and* the
/// indentation after it, so one dictionary key can be two lines of source. rustc
/// sees one string; a text search has to do the same or it will report a key
/// nobody uses when the truth is that it is spelled across a line break.
fn join_continuations(source: &str) -> String {
    let mut out = String::with_capacity(source.len());
    let mut chars = source.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\\' && matches!(chars.peek(), Some('\n') | Some('\r')) {
            while chars.peek().is_some_and(|n| n.is_whitespace()) {
                chars.next();
            }
            continue;
        }
        out.push(c);
    }
    out
}

/// Lines with Cyrillic in them, ignoring comments — a comment in Russian is a
/// note to a maintainer, not a string a driver will ever see.
fn cyrillic_lines(source: &str) -> usize {
    source
        .lines()
        .filter(|line| {
            let trimmed = line.trim_start();
            !trimmed.starts_with("//")
                && line.chars().any(|c| ('\u{0400}'..='\u{04FF}').contains(&c))
        })
        .count()
}

#[test]
fn no_file_grows_new_russian_in_the_code() {
    let root = workspace_root();
    let expected: BTreeMap<&str, usize> = REMAINING.iter().copied().collect();

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
        if EXEMPT.contains(&relative.as_str()) {
            continue;
        }
        let Ok(source) = fs::read_to_string(file) else {
            continue;
        };
        let found = cyrillic_lines(&source);
        let allowed = expected.get(relative.as_str()).copied().unwrap_or(0);
        if found > allowed {
            problems.push(format!(
                "{relative}: {found} lines of Russian, {allowed} allowed. \
                 Say it in English and put the translation in data/locales/ru.json."
            ));
        } else if found < allowed {
            problems.push(format!(
                "{relative}: down to {found} from {allowed} — lower the number \
                 in REMAINING so it cannot drift back up."
            ));
        }
    }

    assert!(problems.is_empty(), "\n{}", problems.join("\n"));
}

/// The dictionary is only useful if what is in it is what the code asks for.
///
/// An entry nobody looks up is either a word that was removed and left behind,
/// or a key that was edited on one side only — and the second is the one that
/// matters, because it fails silently by falling back to English.
#[test]
fn every_translation_is_reachable_from_the_code() {
    let root = workspace_root();
    let mut files = Vec::new();
    rust_files(&root.join("core/src"), &mut files);
    rust_files(&root.join("tui/src"), &mut files);

    let raw: String = files
        .iter()
        .filter(|f| !f.ends_with("i18n.rs"))
        .filter_map(|f| fs::read_to_string(f).ok())
        .collect();
    let sources = join_continuations(&raw);

    let dictionary: std::collections::HashMap<String, String> = serde_json::from_str(
        &fs::read_to_string(root.join("data/locales/ru.json")).expect("ru.json"),
    )
    .expect("ru.json is valid JSON");

    let orphans: Vec<&str> = dictionary
        .keys()
        .map(|english| english.as_str())
        // The literal as it appears in the code, escaped the same way. Anything
        // with a newline or a line continuation in it is spelled differently in
        // the source than in the dictionary, and matching those on text would be
        // a test about escaping rather than about translations.
        .filter(|english| !english.contains('\n'))
        .filter(|english| !sources.contains(&format!("\"{english}\"")))
        .collect();

    assert!(
        orphans.is_empty(),
        "the dictionary translates {} words nothing asks for:\n  {}",
        orphans.len(),
        orphans.join("\n  ")
    );
}
