use ac_core::config::Language;
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::sync::OnceLock;

static EN_FALLBACK_JSON: &str = include_str!("../../../data/locales/en.json");
static RU_FALLBACK_JSON: &str = include_str!("../../../data/locales/ru.json");

static LOCALES: OnceLock<(HashMap<String, String>, HashMap<String, String>)> = OnceLock::new();

fn load_locale_dict(lang_code: &str, fallback_json: &str) -> HashMap<String, String> {
    let file_path = format!("data/locales/{}.json", lang_code);
    let path = Path::new(&file_path);

    let content = if path.exists() {
        fs::read_to_string(path).unwrap_or_else(|_| fallback_json.to_string())
    } else {
        fallback_json.to_string()
    };

    serde_json::from_str(&content).unwrap_or_default()
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
