//! A car's class and tags changed in its `ui_car.json` — one car or eight
//! hundred — without touching anything else in the file.
//!
//! **What a library of mods needs and the game never offered.** Mods arrive
//! with classes like `street`, `race`, `GT3`, `gt3 `, `Gt-3` and nothing at
//! all, and the tags every list filters by are whatever each author typed.
//! Content Manager lets a driver fix that a car at a time, or in a batch.
//!
//! # Only the two values, in place
//!
//! A `ui_car.json` is the author's file: their key order, their spacing,
//! sometimes a comment, often a raw line break inside the description that
//! strict JSON refuses and the game reads anyway. Parsing it and writing it
//! back would "fix" all of that — and turn a one-word change into a file
//! the author no longer recognises. So the file is scanned, not rewritten:
//! the value of `class` and of `tags` at the top level is found and replaced,
//! or added after the opening brace when there is none, and every other byte
//! stays where it was.
//!
//! # Never worse than it was
//!
//! The result is read back the way the game reads it before anything is
//! written; one that does not read, or does not say what was asked, is
//! refused and the file left alone. The write is atomic. And the first time
//! a car is changed its original is kept elsewhere, so [`restore`] can put
//! back exactly what the author shipped — whatever has been changed since.

use crate::atomic_file::write_atomic;
use std::path::{Path, PathBuf};

/// What to change.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Edit {
    /// The class to set, or `None` to leave it.
    pub class: Option<String>,
    /// Tags to add, if a car does not have them already (any case).
    pub add: Vec<String>,
    /// Tags to take away, whatever their case.
    pub remove: Vec<String>,
}

impl Edit {
    /// Whether it changes anything at all.
    pub fn is_empty(&self) -> bool {
        self.class.is_none() && self.add.is_empty() && self.remove.is_empty()
    }
}

/// A top-level key and the byte range of its value.
#[derive(Debug)]
struct Slot {
    key: String,
    value: std::ops::Range<usize>,
}

/// The top-level keys of a JSON object, with where each value is — or `None`
/// for text that is not one object. Strings are read escape-aware; comments
/// (`//` and `/* */`) are stepped over, since mods carry them.
fn slots(text: &str) -> Option<(usize, Vec<Slot>)> {
    let bytes = text.as_bytes();
    let mut at = skip_space(bytes, 0);
    if bytes.get(at) != Some(&b'{') {
        return None;
    }
    let open = at;
    at += 1;
    let mut found = Vec::new();
    loop {
        at = skip_space(bytes, at);
        match bytes.get(at)? {
            b'}' => return Some((open, found)),
            b',' => {
                at += 1;
                continue;
            }
            b'"' => {}
            _ => return None,
        }
        let key_end = string_end(bytes, at)?;
        let key = text.get(at + 1..key_end - 1)?.to_string();
        at = skip_space(bytes, key_end);
        if bytes.get(at) != Some(&b':') {
            return None;
        }
        let start = skip_space(bytes, at + 1);
        let end = value_end(bytes, start)?;
        found.push(Slot {
            key,
            value: start..end,
        });
        at = end;
    }
}

/// Past whitespace and comments.
fn skip_space(bytes: &[u8], mut at: usize) -> usize {
    loop {
        while bytes.get(at).is_some_and(u8::is_ascii_whitespace) {
            at += 1;
        }
        match (bytes.get(at), bytes.get(at + 1)) {
            (Some(b'/'), Some(b'/')) => {
                while bytes.get(at).is_some_and(|b| *b != b'\n') {
                    at += 1;
                }
            }
            (Some(b'/'), Some(b'*')) => {
                at += 2;
                while at < bytes.len() && !(bytes[at] == b'*' && bytes.get(at + 1) == Some(&b'/')) {
                    at += 1;
                }
                at = (at + 2).min(bytes.len());
            }
            _ => return at,
        }
    }
}

/// One past the closing quote of the string that opens at `at`.
fn string_end(bytes: &[u8], at: usize) -> Option<usize> {
    let mut i = at + 1;
    while i < bytes.len() {
        match bytes[i] {
            b'\\' => i += 2,
            b'"' => return Some(i + 1),
            _ => i += 1,
        }
    }
    None
}

/// One past the end of the value that starts at `at`.
fn value_end(bytes: &[u8], at: usize) -> Option<usize> {
    match bytes.get(at)? {
        b'"' => string_end(bytes, at),
        b'{' | b'[' => {
            let mut depth = 0usize;
            let mut i = at;
            while i < bytes.len() {
                match bytes[i] {
                    b'"' => {
                        i = string_end(bytes, i)?;
                        continue;
                    }
                    b'{' | b'[' => depth += 1,
                    b'}' | b']' => {
                        depth -= 1;
                        if depth == 0 {
                            return Some(i + 1);
                        }
                    }
                    _ => {}
                }
                i += 1;
            }
            None
        }
        _ => {
            let mut i = at;
            while i < bytes.len() && !matches!(bytes[i], b',' | b'}' | b'\n' | b'\r') {
                i += 1;
            }
            // Trailing spaces belong to the gap, not the value.
            while i > at && bytes[i - 1].is_ascii_whitespace() {
                i -= 1;
            }
            Some(i)
        }
    }
}

/// The file read as the game reads it: control characters inside strings
/// are tolerated, so they are blanked before a strict reader looks.
fn read_leniently(text: &str) -> Option<serde_json::Value> {
    // Comments first, while the line breaks that end them are still there.
    let text = text.trim_start_matches('\u{feff}');
    let bytes = text.as_bytes();
    let mut plain = String::with_capacity(text.len());
    let mut at = 0;
    while at < bytes.len() {
        match bytes[at] {
            b'"' => {
                let end = string_end(bytes, at).unwrap_or(bytes.len());
                plain.push_str(text.get(at..end).unwrap_or(""));
                at = end;
            }
            b'/' if matches!(bytes.get(at + 1), Some(b'/') | Some(b'*')) => {
                at = skip_space(bytes, at);
            }
            _ => {
                let next = text[at..].chars().next().map_or(1, char::len_utf8);
                plain.push_str(&text[at..at + next]);
                at += next;
            }
        }
    }
    // Then the game's own leniency: control characters inside strings.
    let cleaned: String = plain
        .chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect();
    serde_json::from_str(&cleaned).ok()
}

/// The tags a value holds, as strings.
fn tags_of(value: &serde_json::Value) -> Vec<String> {
    value["tags"]
        .as_array()
        .map(|tags| {
            tags.iter()
                .filter_map(|tag| tag.as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default()
}

/// The text with the edit made, or why not. Unchanged text comes back as it
/// went in.
pub fn edited(text: &str, edit: &Edit) -> Result<String, String> {
    let before = read_leniently(text).ok_or("ui_car.json does not read")?;
    let (open, found) = slots(text).ok_or("ui_car.json is not one object")?;

    let mut tags = tags_of(&before);
    tags.retain(|tag| {
        !edit
            .remove
            .iter()
            .any(|gone| gone.eq_ignore_ascii_case(tag))
    });
    for tag in &edit.add {
        let tag = tag.trim();
        if !tag.is_empty() && !tags.iter().any(|kept| kept.eq_ignore_ascii_case(tag)) {
            tags.push(tag.to_string());
        }
    }
    let tags_changed = tags != tags_of(&before);
    let class = edit
        .class
        .as_deref()
        .map(str::trim)
        .filter(|class| before["class"].as_str() != Some(*class));

    // Replacements, the furthest along first so earlier ranges stay true.
    let mut changes: Vec<(std::ops::Range<usize>, String)> = Vec::new();
    // Keys the file does not have yet, added together after the brace —
    // with a comma after each only if something follows, since a comma
    // before `}` is a file strict readers refuse.
    let mut new_keys: Vec<String> = Vec::new();
    let quoted = |s: &str| serde_json::to_string(s).unwrap_or_else(|_| "\"\"".to_string());
    if let Some(class) = class {
        let value = quoted(class);
        match found.iter().find(|slot| slot.key == "class") {
            Some(slot) => changes.push((slot.value.clone(), value)),
            None => new_keys.push(format!("\"class\": {value}")),
        }
    }
    if tags_changed {
        let value = format!(
            "[{}]",
            tags.iter()
                .map(|tag| quoted(tag))
                .collect::<Vec<_>>()
                .join(", ")
        );
        match found.iter().find(|slot| slot.key == "tags") {
            Some(slot) => changes.push((slot.value.clone(), value)),
            None => new_keys.push(format!("\"tags\": {value}")),
        }
    }
    if !new_keys.is_empty() {
        let mut added: String = new_keys.iter().map(|key| format!("\n  {key},")).collect();
        if found.is_empty() {
            added.pop();
            added.push('\n');
        }
        changes.push((open + 1..open + 1, added));
    }
    if changes.is_empty() {
        return Ok(text.to_string());
    }
    changes.sort_by_key(|(range, _)| std::cmp::Reverse(range.start));
    let mut out = text.to_string();
    for (range, value) in changes {
        out.replace_range(range, &value);
    }

    // Read back the way the game will, and say what was asked — or nothing
    // is written.
    let after = read_leniently(&out).ok_or("the change would not read back")?;
    if let Some(class) = class
        && after["class"].as_str() != Some(class)
    {
        return Err("the class did not read back".to_string());
    }
    if tags_of(&after) != tags {
        return Err("the tags did not read back".to_string());
    }
    Ok(out)
}

/// Where a car's description is, whatever its case.
pub fn file_of(car_dir: &Path) -> Option<PathBuf> {
    let ui = std::fs::read_dir(car_dir)
        .ok()?
        .flatten()
        .find(|e| e.file_name().to_string_lossy().eq_ignore_ascii_case("ui"))?
        .path();
    std::fs::read_dir(&ui)
        .ok()?
        .flatten()
        .find(|e| {
            e.file_name()
                .to_string_lossy()
                .eq_ignore_ascii_case("ui_car.json")
        })
        .map(|e| e.path())
}

/// Make the same edit to many cars. Each car's original is kept in
/// `originals/<id>.json` the first time it is changed. Returns, per car,
/// whether it changed or why it did not.
pub fn edit_cars(
    cars_dir: &Path,
    ids: &[String],
    edit: &Edit,
    originals: &Path,
) -> Vec<(String, Result<bool, String>)> {
    ids.iter()
        .map(|id| {
            (
                id.clone(),
                edit_one(&cars_dir.join(id), id, edit, originals),
            )
        })
        .collect()
}

fn edit_one(car_dir: &Path, id: &str, edit: &Edit, originals: &Path) -> Result<bool, String> {
    let at = file_of(car_dir).ok_or("no ui_car.json")?;
    let bytes = std::fs::read(&at).map_err(|why| why.to_string())?;
    let text = String::from_utf8_lossy(&bytes).into_owned();
    let out = edited(&text, edit)?;
    if out == text {
        return Ok(false);
    }
    let kept = originals.join(format!("{id}.json"));
    if !kept.exists() {
        write_atomic(&kept, &bytes).map_err(|why| format!("keeping the original: {why}"))?;
    }
    write_atomic(&at, out.as_bytes()).map_err(|why| why.to_string())?;
    Ok(true)
}

/// Whether a car has an original kept to go back to.
pub fn has_original(id: &str, originals: &Path) -> bool {
    originals.join(format!("{id}.json")).is_file()
}

/// Put a car's description back exactly as it was before the first change.
pub fn restore(cars_dir: &Path, id: &str, originals: &Path) -> Result<(), String> {
    let kept = originals.join(format!("{id}.json"));
    let bytes = std::fs::read(&kept).map_err(|_| "no original kept for this car")?;
    let at = file_of(&cars_dir.join(id)).ok_or("no ui_car.json")?;
    write_atomic(&at, &bytes).map_err(|why| why.to_string())?;
    std::fs::remove_file(&kept).map_err(|why| why.to_string())
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    const AUTHOR: &str = "{\n    // made by somebody\n    \"name\": \"Thing GT3\",\n    \"brand\": \"Thing\",\n    \"description\": \"Line one\nline two, raw\",\n    \"tags\": [\"#GT3\", \"race\"],\n    \"class\": \"race\",\n    \"specs\": {\"bhp\": \"500bhp\", \"weight\": \"1250kg\"}\n}\n";

    /// **Only the two values change**: the comment, the raw line break, the
    /// key order and the spacing are the author's and stay.
    #[test]
    fn only_class_and_tags_change() {
        let edit = Edit {
            class: Some("GT3".to_string()),
            add: vec!["drift".to_string(), "RACE".to_string()],
            remove: vec!["#gt3".to_string()],
        };
        let out = edited(AUTHOR, &edit).unwrap();
        let expected = AUTHOR
            .replace("[\"#GT3\", \"race\"]", "[\"race\", \"drift\"]")
            .replace("\"class\": \"race\"", "\"class\": \"GT3\"");
        assert_eq!(out, expected);
    }

    /// A file with neither gets both, and still reads.
    #[test]
    fn missing_keys_are_added_after_the_brace() {
        let out = edited(
            "{\"name\": \"Bare\"}",
            &Edit {
                class: Some("street".to_string()),
                add: vec!["drift".to_string()],
                ..Default::default()
            },
        )
        .unwrap();
        let value = read_leniently(&out).unwrap();
        assert_eq!(value["class"], "street");
        assert_eq!(value["tags"][0], "drift");
        assert_eq!(value["name"], "Bare");
    }

    /// An empty object gets both and no comma before its brace.
    #[test]
    fn an_empty_object_stays_valid() {
        let out = edited(
            "{}",
            &Edit {
                class: Some("race".to_string()),
                add: vec!["a".to_string()],
                ..Default::default()
            },
        )
        .unwrap();
        assert!(
            serde_json::from_str::<serde_json::Value>(&out).is_ok(),
            "{out}"
        );
    }

    /// Nothing asked is nothing written.
    #[test]
    fn an_edit_that_changes_nothing_returns_the_same_text() {
        let same = Edit {
            class: Some("race".to_string()),
            add: vec!["Race".to_string()],
            ..Default::default()
        };
        assert_eq!(edited(AUTHOR, &same).unwrap(), AUTHOR);
    }

    /// A file that does not read is refused, not "repaired".
    #[test]
    fn a_broken_file_is_refused() {
        assert!(edited("{ \"name\": ", &Edit::default()).is_err());
        assert!(edited("[1, 2]", &Edit::default()).is_err());
    }

    /// A class with quotes in it is written as JSON, not as a hole.
    #[test]
    fn values_are_escaped() {
        let out = edited(
            AUTHOR,
            &Edit {
                class: Some("a \"b\" \\ c".to_string()),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(read_leniently(&out).unwrap()["class"], "a \"b\" \\ c");
    }

    /// **Many cars, and back again.** The original is kept the first time
    /// only, so a second edit and then a restore return the author's file.
    #[test]
    fn many_cars_change_and_one_goes_back_exactly() {
        let root = std::env::temp_dir().join(format!("ac-ui-car-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let cars = root.join("cars");
        let originals = root.join("originals");
        for id in ["a", "b"] {
            std::fs::create_dir_all(cars.join(id).join("UI")).unwrap();
            std::fs::write(cars.join(id).join("UI/UI_car.json"), AUTHOR).unwrap();
        }
        std::fs::create_dir_all(cars.join("none")).unwrap();
        let ids = vec!["a".to_string(), "b".to_string(), "none".to_string()];

        let first = Edit {
            class: Some("GT3".to_string()),
            ..Default::default()
        };
        let done = edit_cars(&cars, &ids, &first, &originals);
        assert_eq!(done[0].1, Ok(true));
        assert_eq!(done[1].1, Ok(true));
        assert!(done[2].1.is_err());
        let second = Edit {
            add: vec!["night".to_string()],
            ..Default::default()
        };
        edit_cars(&cars, &ids[..1], &second, &originals);
        assert!(has_original("a", &originals));

        restore(&cars, "a", &originals).unwrap();
        assert_eq!(
            std::fs::read_to_string(cars.join("a/UI/UI_car.json")).unwrap(),
            AUTHOR
        );
        assert!(!has_original("a", &originals));
        let _ = std::fs::remove_dir_all(&root);
    }
}
