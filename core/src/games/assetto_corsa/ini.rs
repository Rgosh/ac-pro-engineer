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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_value_is_read_from_its_own_section_without_its_comment() {
        let text = "[VIDEO]\nWIDTH=1920 ; mine\n[OTHER]\nWIDTH=5\n";
        assert_eq!(value(text, "VIDEO", "WIDTH").as_deref(), Some("1920"));
        assert_eq!(value(text, "video", "width").as_deref(), Some("1920"));
        assert_eq!(value(text, "VIDEO", "HEIGHT"), None);
        assert_eq!(value("", "VIDEO", "WIDTH"), None);
    }
}
