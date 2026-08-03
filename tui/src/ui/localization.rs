use ac_core::config::Language;
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::sync::OnceLock;

static EN_FALLBACK_JSON: &str = include_str!("../../../data/locales/en.json");
static RU_FALLBACK_JSON: &str = include_str!("../../../data/locales/ru.json");

static LOCALES: OnceLock<(HashMap<String, String>, HashMap<String, String>)> = OnceLock::new();

/// Parse the embedded copy of a locale. Compiled in, so it cannot fail in
/// practice; an empty map is the only sensible answer if it somehow does.
fn parse_embedded(lang_code: &str, fallback_json: &str) -> HashMap<String, String> {
    serde_json::from_str(fallback_json).unwrap_or_else(|error| {
        eprintln!("Embedded {lang_code} locale failed to parse: {error}");
        HashMap::new()
    })
}

/// Load a locale, preferring an override in `data/locales/` next to the
/// working directory.
///
/// A malformed override used to be swallowed by `unwrap_or_default()`,
/// producing an *empty* dictionary with nothing logged — and since `tr` falls
/// back to the raw key, the entire UI silently degraded to showing things like
/// `launch_nav_desc` instead of text. The embedded copy is a far better answer
/// than that, and the parse error is now visible.
fn load_locale_dict(lang_code: &str, fallback_json: &str) -> HashMap<String, String> {
    let file_path = format!("data/locales/{}.json", lang_code);
    let path = Path::new(&file_path);

    if !path.exists() {
        return parse_embedded(lang_code, fallback_json);
    }

    let content = match fs::read_to_string(path) {
        Ok(content) => content,
        Err(error) => {
            eprintln!("Could not read {file_path}: {error}. Using the built-in strings.");
            return parse_embedded(lang_code, fallback_json);
        }
    };

    match serde_json::from_str(&content) {
        Ok(dict) => dict,
        Err(error) => {
            eprintln!("{file_path} is not valid JSON: {error}. Using the built-in strings.");
            parse_embedded(lang_code, fallback_json)
        }
    }
}

fn get_locales() -> &'static (HashMap<String, String>, HashMap<String, String>) {
    LOCALES.get_or_init(|| {
        let en_dict = load_locale_dict("en", EN_FALLBACK_JSON);
        let ru_dict = load_locale_dict("ru", RU_FALLBACK_JSON);
        (en_dict, ru_dict)
    })
}

pub fn tr(key: &str, lang: &Language) -> String {
    let (en_map, ru_map) = get_locales();

    let target_map = match lang {
        Language::English => en_map,
        Language::Russian => ru_map,
    };

    if let Some(val) = target_map.get(key) {
        val.clone()
    } else if let Some(val) = en_map.get(key) {
        val.clone()
    } else {
        key.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn embedded(json: &str) -> HashMap<String, String> {
        serde_json::from_str(json).expect("the embedded locale must be valid JSON")
    }

    /// Both locale files ship with the binary, so a syntax error in either
    /// one is a build-time mistake that would otherwise only surface as a UI
    /// full of raw key names.
    #[test]
    fn both_embedded_locales_parse() {
        assert!(!embedded(EN_FALLBACK_JSON).is_empty());
        assert!(!embedded(RU_FALLBACK_JSON).is_empty());
    }

    /// `tr` falls back target -> en -> the raw key. A key present in one
    /// locale but not the other therefore renders as its own name in the
    /// other language, which is a visible bug the moment anyone wires it up.
    #[test]
    fn the_two_locales_define_the_same_keys() {
        let en = embedded(EN_FALLBACK_JSON);
        let ru = embedded(RU_FALLBACK_JSON);

        let mut missing_from_en: Vec<&str> = ru
            .keys()
            .filter(|k| !en.contains_key(*k))
            .map(|k| k.as_str())
            .collect();
        let mut missing_from_ru: Vec<&str> = en
            .keys()
            .filter(|k| !ru.contains_key(*k))
            .map(|k| k.as_str())
            .collect();
        missing_from_en.sort_unstable();
        missing_from_ru.sort_unstable();

        assert!(
            missing_from_en.is_empty(),
            "keys in ru.json with no en.json entry, which would render as the raw key in English: {missing_from_en:?}"
        );
        assert!(
            missing_from_ru.is_empty(),
            "keys in en.json with no ru.json entry: {missing_from_ru:?}"
        );
    }

    /// An unknown key must not panic; the raw key is the documented last
    /// resort.
    #[test]
    fn an_unknown_key_returns_itself() {
        assert_eq!(
            tr("no_such_key_anywhere", &Language::English),
            "no_such_key_anywhere"
        );
    }
}
