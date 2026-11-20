//! The game's ini files: one value read, one value written, and nothing
//! else in the file disturbed.
//!
//! **Written back as it was found.** These are files a driver, Content
//! Manager and the patch all edit, with comments and keys this program has
//! never heard of. Rewriting the whole file from a parsed table would lose
//! the comments and reorder the rest; changing one line in place loses
//! nothing.

/// What `key` under `[section]` says, trimmed, with any comment after it
/// removed. The last one wins, as it does when the game reads it.
pub fn value(text: &str, section: &str, key: &str) -> Option<String> {
    let mut in_section = false;
    let mut said = None;
    for line in text.lines() {
        let line = line.split([';', '#']).next().unwrap_or("").trim();
        if let Some(name) = line.strip_prefix('[').and_then(|l| l.strip_suffix(']')) {
            in_section = name.trim().eq_ignore_ascii_case(section);
            continue;
        }
        if in_section
            && let Some((k, v)) = line.split_once('=')
            && k.trim().eq_ignore_ascii_case(key)
        {
            said = Some(v.trim().to_string());
        }
    }
    said
}

/// One `key=value` in a file, under its section, with the comment after it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    pub section: String,
    pub key: String,
    pub value: String,
    pub comment: String,
}

/// Every `key=value` in a file, in order, under the section it is in —
/// for a page that shows a whole file. A key outside any section is under
/// the empty one; a commented-out line is not an entry.
pub fn entries(text: &str) -> Vec<Entry> {
    let mut section = String::new();
    let mut out = Vec::new();
    for line in text.lines() {
        let bare = line.trim();
        if bare.starts_with(';') || bare.starts_with('#') || bare.starts_with("//") {
            continue;
        }
        if let Some(name) = bare.strip_prefix('[').and_then(|l| l.split(']').next()) {
            section = name.trim().to_string();
            continue;
        }
        let (body, comment) = match bare.find([';', '#']) {
            Some(at) => (&bare[..at], bare[at + 1..].trim()),
            None => (bare, ""),
        };
        if let Some((k, v)) = body.split_once('=') {
            let key = k.trim();
            if !key.is_empty() {
                out.push(Entry {
                    section: section.clone(),
                    key: key.to_string(),
                    value: v.trim().to_string(),
                    comment: comment.to_string(),
                });
            }
        }
    }
    out
}

/// The text with `key` under `[section]` set to `value`: replaced where it
/// is, added to the section where it is not, and the section added at the
/// end when the file has none.
pub fn with_value(text: &str, section: &str, key: &str, value: &str) -> String {
    let mut out: Vec<String> = Vec::new();
    let mut in_section = false;
    let mut seen_section = false;
    let mut done = false;
    for line in text.lines() {
        let bare = line.trim();
        if let Some(name) = bare.strip_prefix('[').and_then(|l| l.strip_suffix(']')) {
            // Leaving the section without having found the key: it goes at
            // the section's end, before the next one starts.
            if in_section && !done {
                out.push(format!("{key}={value}"));
                done = true;
            }
            in_section = name.trim().eq_ignore_ascii_case(section);
            seen_section |= in_section;
            out.push(line.to_string());
            continue;
        }
        if in_section
            && !done
            && let Some((k, _)) = bare.split_once('=')
            && k.trim().eq_ignore_ascii_case(key)
            && !bare.starts_with(';')
        {
            out.push(format!("{key}={value}"));
            done = true;
            continue;
        }
        out.push(line.to_string());
    }
    if !done {
        if !seen_section {
            if out.last().is_some_and(|l| !l.trim().is_empty()) {
                out.push(String::new());
            }
            out.push(format!("[{section}]"));
        }
        out.push(format!("{key}={value}"));
    }
    let mut joined = out.join("\n");
    joined.push('\n');
    joined
}

/// Set one value in a file, creating the file and its folder when they are
/// not there, every other line left as it was.
pub fn set_in_file(
    at: &std::path::Path,
    section: &str,
    key: &str,
    value: &str,
) -> Result<std::path::PathBuf, String> {
    let before = std::fs::read_to_string(at).unwrap_or_default();
    let after = with_value(&before, section, key, value);
    if let Some(folder) = at.parent() {
        std::fs::create_dir_all(folder).map_err(|e| format!("{}: {e}", folder.display()))?;
    }
    std::fs::write(at, after).map_err(|e| format!("{}: {e}", at.display()))?;
    Ok(at.to_path_buf())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_entry_is_listed_under_its_section() {
        let text = "; top\n[A]\nX=1 ; one\n;Y=2\n[B]\nZ = hello\n";
        let all = entries(text);
        assert_eq!(all.len(), 2);
        assert_eq!(
            (
                all[0].section.as_str(),
                all[0].key.as_str(),
                all[0].value.as_str()
            ),
            ("A", "X", "1")
        );
        assert_eq!(all[0].comment, "one");
        assert_eq!(
            (all[1].section.as_str(), all[1].value.as_str()),
            ("B", "hello")
        );
    }

    #[test]
    fn a_value_is_read_from_its_own_section_without_its_comment() {
        let text = "[VIDEO]\nWIDTH=1920 ; mine\n[OTHER]\nWIDTH=5\n";
        assert_eq!(value(text, "VIDEO", "WIDTH").as_deref(), Some("1920"));
        assert_eq!(value(text, "video", "width").as_deref(), Some("1920"));
        assert_eq!(value(text, "VIDEO", "HEIGHT"), None);
        assert_eq!(value("", "VIDEO", "WIDTH"), None);
    }
}
