//! The application's words, in one place instead of five hundred.
//!
//! Every user-facing string used to be written twice, at the point it was used:
//!
//! ```ignore
//! component: "Tyres".tr(ru).to_string(),
//! ```
//!
//! That works, and it costs three things that add up. **The code stops being
//! readable in one language** — a rule about tyre pressure is half English and
//! half Russian, and a reader has to skip every other branch. **A translation
//! cannot be reviewed**, because it is scattered across twenty files and there
//! is no list of it to look at. And **a third language means touching every
//! call site again**, which is the point at which nobody adds one.
//!
//! So the code says what it means, in English:
//!
//! ```ignore
//! component: "Tyres".tr(ru).to_string(),
//! ```
//!
//! and the Russian for it lives in `data/locales/ru.json`, next to every other
//! Russian word in the program. This is the same split the in-game panel has
//! had since it was written — `assets/frontends/csp-panel/acpe/i18n.lua` — and
//! the desktop side was simply the half that never got it.
//!
//! ## One dictionary, not two
//!
//! The terminal used to have its own: `tui/src/ui/localization.rs`, keyed by
//! short names like `skill_smooth`, with an `en.json` beside the `ru.json`.
//! It worked, and it was invisible from `core`, which cannot depend on `tui` —
//! so the engineer's words ended up somewhere else and the program had two
//! answers to one question. They are merged here.
//!
//! Two things changed in the merge, both improvements the old shape could not
//! have. **The key is the English text**, so a reader of the code sees the
//! sentence rather than `anal_fuel_used`, and a missing entry degrades to
//! readable English instead of a raw key on screen. And **there is no
//! `en.json`**: English is the source, so there is no second file to keep in
//! step and no way for the two to disagree.
//!
//! ## What this is not
//!
//! It is not a general localisation framework. There is no plural handling, no
//! gender, no locale-aware number formatting — [`crate::config::Formatter`]
//! already owns units and decimals, and it stays there. This translates
//! *fragments*, exactly as the code did before, so the migration changed no
//! output at all.
//!
//! ## Adding a word
//!
//! Write the English at the call site and add one line to
//! `data/locales/ru.json`. A missing entry is not a compile error — it falls
//! back to the English, which is the right failure: an untranslated word is
//! readable, and a panic in the middle of a race is not.
//! `every_translation_is_reachable_from_the_code` catches the opposite mistake,
//! an entry nobody uses.

use crate::config::Language;
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::sync::OnceLock;

/// Every Russian word the program knows, read from `data/locales/ru.json`.
///
/// **English is the source and needs no file.** The key *is* the English, so
/// there is nothing to keep in step: a word that has no entry shows in English,
/// which is what the fallback in [`translate`] is for.
///
/// The file is embedded, and an override next to the working directory wins if
/// there is one — a translator can correct a word and see it without a Rust
/// toolchain, which is the whole reason the dictionary lives in JSON rather
/// than in a `const` in this file.
static EMBEDDED_RU: &str = include_str!("../../data/locales/ru.json");

static RUSSIAN: OnceLock<HashMap<String, String>> = OnceLock::new();

/// Load the dictionary, preferring an override in `data/locales/`.
///
/// A malformed override used to be swallowed, producing an *empty* dictionary
/// with nothing logged — and since a missing word falls back to English, the
/// whole interface silently reverted to English with no clue why. The embedded
/// copy is a far better answer than that, and the parse error is now visible.
fn dictionary() -> &'static HashMap<String, String> {
    RUSSIAN.get_or_init(|| {
        let path = Path::new("data/locales/ru.json");
        if path.exists() {
            match fs::read_to_string(path) {
                Ok(text) => match serde_json::from_str(&text) {
                    Ok(dict) => return dict,
                    Err(error) => {
                        eprintln!("data/locales/ru.json is not valid JSON: {error}. Using the built-in words.");
                    }
                },
                Err(error) => {
                    eprintln!("Could not read data/locales/ru.json: {error}. Using the built-in words.");
                }
            }
        }
        serde_json::from_str(EMBEDDED_RU).unwrap_or_else(|error| {
            // Compiled in, so this cannot happen in practice — and a test
            // parses it precisely so that it cannot start to.
            eprintln!("The embedded Russian locale failed to parse: {error}");
            HashMap::new()
        })
    })
}

/// How many words the dictionary holds, for tests and diagnostics.
pub fn word_count() -> usize {
    dictionary().len()
}

/// Look a string up, or hand back what it was given.
///
/// The fallback is deliberate. A word nobody has translated yet shows in
/// English, which a Russian-speaking driver can still read in context; the
/// alternative — failing, or printing a key — turns a missing translation into
/// a broken screen.
pub fn translate(text: &str, russian: bool) -> &str {
    // `"grip|short"` is one English word in two places that want two different
    // Russian ones — the whole word in a sentence, an abbreviation in a column
    // four characters wide. English shows what is before the bar and never the
    // context; Russian gets an entry of its own for each. Every translation
    // system grows some form of this, because one word in two places is
    // genuinely two words in a language that inflects.
    let english = text.split('|').next().unwrap_or(text);
    if !russian {
        return english;
    }
    dictionary()
        .get(text)
        .map(String::as_str)
        .unwrap_or(english)
}

/// A translated sentence with values dropped into it.
///
/// [`translate`] handles a word. This handles the other half of what the code
/// used to write twice — a sentence with a number in the middle of it:
///
/// ```ignore
/// tr_fmt("Rear dropping too much at high speed (-{0} mm)", ru,
///        &[&format!("{rake_loss:.1}")])
/// ```
///
/// The placeholders are `{0}`, `{1}` and so on, and the **catalogue holds the
/// whole sentence** rather than the pieces of it. That matters more than it
/// looks: splitting a sentence into fragments and joining them with `format!`
/// gets the words translated and leaves everything between them in English, so
/// a Russian driver reads "-12.4 mm" where the old code said "-12.4 мм". Word
/// order goes the same way — a language that puts the number last cannot be
/// served by a fixed English skeleton.
///
/// Values arrive already formatted, because `{:.1}` and `{:>8}` belong to the
/// code and not to a translator. A key with no entry falls back to English,
/// exactly as a single word does.
pub fn tr_fmt(template: &str, russian: bool, args: &[&str]) -> String {
    let mut out = translate(template, russian).to_string();
    for (index, value) in args.iter().enumerate() {
        out = out.replace(&format!("{{{index}}}"), value);
    }
    out
}

/// `"Tyres".tr(ru)`, which is short enough to use everywhere it is needed.
///
/// An extension trait rather than a macro: the call site stays a plain
/// expression, so it works inside `format!` arguments, `match` arms and struct
/// literals without any of them having to know it is there.
pub trait Translate {
    /// The Russian for this, if `russian` and if there is one.
    fn tr(&self, russian: bool) -> &str;

    /// The same, from a [`Language`] rather than a flag — for the call sites
    /// that have one and would otherwise have to make the flag first.
    ///
    /// By reference, because that is how the terminal passes it around.
    fn tr_lang(&self, language: &Language) -> &str {
        self.tr(*language == Language::Russian)
    }
}

impl Translate for str {
    fn tr(&self, russian: bool) -> &str {
        translate(self, russian)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn english_is_returned_unchanged() {
        assert_eq!("Tyres".tr(false), "Tyres");
        // Including for words that do have a translation: the flag decides,
        // not the presence of an entry.
        assert_eq!("Brakes".tr(false), "Brakes");
    }

    #[test]
    fn russian_comes_from_the_catalogue() {
        assert_eq!("Tyres".tr(true), "Шины");
        assert_eq!("All four".tr(true), "Все шины");
    }

    /// A word with no entry reads in English rather than breaking the screen.
    #[test]
    fn a_missing_translation_falls_back_rather_than_failing() {
        assert_eq!("Kerb strike".tr(true), "Kerb strike");
    }

    /// A sentence keeps what is *between* its values, which is the whole reason
    /// the catalogue holds templates rather than fragments.
    #[test]
    fn a_template_translates_before_its_values_arrive() {
        // The unit is inside the sentence, so it is translated with it — glue
        // the pieces together in code instead and the Russian reads "mm".
        assert_eq!(
            tr_fmt("{0} laps on this set", false, &["7"]),
            "7 laps on this set"
        );
        // Unknown template: English, values still filled in.
        assert_eq!(
            tr_fmt("{0} of {1} slots used", true, &["3", "8"]),
            "3 of 8 slots used"
        );
    }

    /// The same value can appear twice, and an argument nobody references is
    /// not an error — a template may legitimately drop one in one language.
    #[test]
    fn a_placeholder_may_repeat_and_a_spare_argument_is_harmless() {
        assert_eq!(tr_fmt("{0}-{0}", false, &["x"]), "x-x");
        assert_eq!(tr_fmt("{0} only", false, &["a", "b"]), "a only");
    }

    /// The shipped dictionary is valid JSON and is not empty.
    ///
    /// It is compiled in, so a syntax error is a build-time mistake that would
    /// otherwise surface as an interface silently reverting to English with no
    /// clue why. A duplicate key cannot happen any more — JSON has one value
    /// per key, which is one class of mistake the file format rules out that a
    /// list of pairs did not.
    #[test]
    fn the_shipped_dictionary_parses_and_is_not_empty() {
        let parsed: HashMap<String, String> =
            serde_json::from_str(EMBEDDED_RU).expect("ru.json must be valid JSON");
        assert!(parsed.len() > 500, "only {} words shipped", parsed.len());
    }

    /// An entry that translates a word to itself is either a mistake or a
    /// reminder somebody left. Both are worth failing over: the first is wrong
    /// and the second belongs in a comment.
    #[test]
    fn nothing_is_translated_to_itself() {
        let parsed: HashMap<String, String> =
            serde_json::from_str(EMBEDDED_RU).expect("ru.json must be valid JSON");
        let same: Vec<&str> = parsed
            .iter()
            .filter(|(english, russian)| english == russian)
            .map(|(english, _)| english.as_str())
            .collect();
        assert!(
            same.is_empty(),
            "these are their own translation, which is not a translation: {same:?}"
        );
    }
}
