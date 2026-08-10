//! What every key does — and the only place that decides it.
//!
//! Before this, three things claimed to know the key map and none of them
//! agreed: the `match key.code` arms in `main.rs`, the hint lines drawn at the
//! bottom of two tabs, and the help overlay. The Setup tab's hint offered
//! `'D' - Download` on a screen where `D` was not handled at all; the help
//! overlay said settings categories were on `A / S / D` after a fourth
//! category had been added on `F`. Nothing failed when they drifted, because
//! nothing connected them.
//!
//! So: the bindings live in the config as text, [`resolve`] is the only thing
//! that turns a keypress into an [`Action`], and [`describe`] is the only
//! thing that turns a binding into something to print. A hint built from
//! `describe` cannot name a key that `resolve` does not honour, and
//! `the_hints_only_name_keys_that_do_something` in `ui::widgets` checks that
//! it does not.

use crate::AppTab;
use ac_core::config::KeyBindings;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

/// Something the application does when a key is pressed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    Help,
    Quit,
    Screenshot,
    Language,
    NextTab,
    PrevTab,
    GoToTab(AppTab),
    AnalysisSave,
    AnalysisLoad,
    AnalysisCompare,
    AnalysisExport,
    AnalysisFilter,
    SetupBrowser,
    SetupDownload,
    OverlayInstall,
    OverlayUninstall,
    OverlayDiagnostics,
}

/// A parsed binding: what to press, and with what held down.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Binding {
    pub code: KeyCode,
    pub modifiers: KeyModifiers,
}

/// Letters that share a physical key across the two layouts this application
/// is used in.
///
/// A driver in a race does not stop to switch layout, and until now the arms
/// in `main.rs` carried the Cyrillic twin of every letter they matched. That
/// belongs here, once, so a rebound letter keeps the same courtesy — bind
/// `analysis_save` to `w` and `ц` works too.
const LAYOUT_TWINS: &[(char, char)] = &[
    ('q', 'й'),
    ('w', 'ц'),
    ('e', 'у'),
    ('r', 'к'),
    ('t', 'е'),
    ('y', 'н'),
    ('u', 'г'),
    ('i', 'ш'),
    ('o', 'щ'),
    ('p', 'з'),
    ('a', 'ф'),
    ('s', 'ы'),
    ('d', 'в'),
    ('f', 'а'),
    ('g', 'п'),
    ('h', 'р'),
    ('j', 'о'),
    ('k', 'л'),
    ('l', 'д'),
    ('z', 'я'),
    ('x', 'ч'),
    ('c', 'с'),
    ('v', 'м'),
    ('b', 'и'),
    ('n', 'т'),
    ('m', 'ь'),
];

/// Keys that have always done one thing, in places a binding cannot reach.
///
/// The help modal says "PRESS ESC, ?, Q, OR F1 TO CLOSE" in nine of them, and
/// `?` has opened the help since the first release. Honoured after the
/// configured bindings, so rebinding a global action still wins, and reported
/// by [`conflict`] so nobody puts a tab-local action on one of these and
/// wonders why it never fires.
const RESERVED: &[(char, Action)] = &[('?', Action::Help), ('q', Action::Quit)];

/// The Cyrillic letter on the same key, if there is one.
fn layout_twin(latin: char) -> Option<char> {
    LAYOUT_TWINS
        .iter()
        .find(|(from, _)| *from == latin)
        .map(|(_, to)| *to)
}

/// Read a binding out of its written form.
///
/// `None` for anything unrecognised. An unparseable binding leaves that action
/// without a key rather than taking the application down — a config someone
/// edited by hand is a config with a typo in it eventually, and losing one
/// shortcut is a smaller problem than not starting.
pub fn parse(text: &str) -> Option<Binding> {
    let text = text.trim();
    if text.is_empty() {
        return None;
    }

    let mut modifiers = KeyModifiers::NONE;
    let mut rest = text;
    loop {
        let lower = rest.to_lowercase();
        if let Some(tail) = lower.strip_prefix("ctrl+") {
            modifiers |= KeyModifiers::CONTROL;
            rest = &rest[rest.len() - tail.len()..];
        } else if let Some(tail) = lower.strip_prefix("shift+") {
            modifiers |= KeyModifiers::SHIFT;
            rest = &rest[rest.len() - tail.len()..];
        } else if let Some(tail) = lower.strip_prefix("alt+") {
            modifiers |= KeyModifiers::ALT;
            rest = &rest[rest.len() - tail.len()..];
        } else {
            break;
        }
    }

    let name = rest.trim().to_lowercase();
    if name.is_empty() {
        return None;
    }

    // Shift+Tab is its own key code in crossterm, not Tab with a modifier, and
    // a terminal never reports it the other way. Folded here so the config can
    // spell it the way a person would.
    if name == "tab" && modifiers.contains(KeyModifiers::SHIFT) {
        return Some(Binding {
            code: KeyCode::BackTab,
            modifiers: modifiers - KeyModifiers::SHIFT,
        });
    }

    let code = match name.as_str() {
        "esc" | "escape" => KeyCode::Esc,
        "tab" => KeyCode::Tab,
        "backtab" => KeyCode::BackTab,
        "enter" | "return" => KeyCode::Enter,
        "space" => KeyCode::Char(' '),
        "up" => KeyCode::Up,
        "down" => KeyCode::Down,
        "left" => KeyCode::Left,
        "right" => KeyCode::Right,
        "pgup" | "pageup" => KeyCode::PageUp,
        "pgdn" | "pagedown" => KeyCode::PageDown,
        "home" => KeyCode::Home,
        "end" => KeyCode::End,
        "ins" | "insert" => KeyCode::Insert,
        "del" | "delete" => KeyCode::Delete,
        "backspace" => KeyCode::Backspace,
        other => {
            if let Some(number) = other.strip_prefix('f')
                && let Ok(index) = number.parse::<u8>()
                && (1..=12).contains(&index)
            {
                KeyCode::F(index)
            } else {
                let mut chars = other.chars();
                let first = chars.next()?;
                if chars.next().is_some() {
                    return None;
                }
                KeyCode::Char(first)
            }
        }
    };

    Some(Binding { code, modifiers })
}

/// Write a binding the way it should appear on screen.
///
/// Falls back to the raw text uppercased, so a binding this module cannot read
/// still shows the driver what is in their config rather than a blank.
pub fn describe(text: &str) -> String {
    let Some(binding) = parse(text) else {
        return text.trim().to_uppercase();
    };

    let mut out = String::new();
    if binding.modifiers.contains(KeyModifiers::CONTROL) {
        out.push_str("CTRL+");
    }
    if binding.modifiers.contains(KeyModifiers::ALT) {
        out.push_str("ALT+");
    }
    if binding.modifiers.contains(KeyModifiers::SHIFT) {
        out.push_str("SHIFT+");
    }

    match binding.code {
        KeyCode::Char(' ') => out.push_str("SPACE"),
        KeyCode::Char(c) => out.extend(c.to_uppercase()),
        KeyCode::F(n) => out.push_str(&format!("F{n}")),
        KeyCode::Esc => out.push_str("ESC"),
        KeyCode::Tab => out.push_str("TAB"),
        KeyCode::BackTab => {
            out.clear();
            out.push_str("SHIFT+TAB");
        }
        KeyCode::Enter => out.push_str("ENTER"),
        KeyCode::Up => out.push('↑'),
        KeyCode::Down => out.push('↓'),
        KeyCode::Left => out.push('←'),
        KeyCode::Right => out.push('→'),
        KeyCode::PageUp => out.push_str("PGUP"),
        KeyCode::PageDown => out.push_str("PGDN"),
        KeyCode::Home => out.push_str("HOME"),
        KeyCode::End => out.push_str("END"),
        KeyCode::Insert => out.push_str("INS"),
        KeyCode::Delete => out.push_str("DEL"),
        KeyCode::Backspace => out.push_str("BACKSPACE"),
        other => out.push_str(&format!("{other:?}").to_uppercase()),
    }

    out
}

/// Turn a keypress into the written form a binding uses.
///
/// The other direction from [`parse`], and the reason the Settings screen can
/// capture a key rather than asking anyone to type `ctrl+s`.
pub fn spell(key: KeyEvent) -> Option<String> {
    let mut out = String::new();
    if key.modifiers.contains(KeyModifiers::CONTROL) {
        out.push_str("ctrl+");
    }
    if key.modifiers.contains(KeyModifiers::ALT) {
        out.push_str("alt+");
    }

    match key.code {
        KeyCode::Char(' ') => out.push_str("space"),
        KeyCode::Char(c) => {
            // A shifted letter arrives as the upper-case character with SHIFT
            // set. Recording "shift+S" would then never match, because the
            // next press reports the same thing and the comparison is against
            // a lower-case name.
            for lower in c.to_lowercase() {
                out.push(lower);
            }
        }
        KeyCode::F(n) => out.push_str(&format!("f{n}")),
        KeyCode::Esc => out.push_str("esc"),
        KeyCode::Tab => out.push_str("tab"),
        KeyCode::BackTab => {
            out.clear();
            out.push_str("shift+tab");
        }
        KeyCode::Enter => out.push_str("enter"),
        KeyCode::Up => out.push_str("up"),
        KeyCode::Down => out.push_str("down"),
        KeyCode::Left => out.push_str("left"),
        KeyCode::Right => out.push_str("right"),
        KeyCode::PageUp => out.push_str("pgup"),
        KeyCode::PageDown => out.push_str("pgdn"),
        KeyCode::Home => out.push_str("home"),
        KeyCode::End => out.push_str("end"),
        KeyCode::Insert => out.push_str("ins"),
        KeyCode::Delete => out.push_str("del"),
        KeyCode::Backspace => out.push_str("backspace"),
        _ => return None,
    }

    Some(out)
}

/// Does this keypress fire this binding?
pub fn matches(binding: &str, key: KeyEvent) -> bool {
    let Some(wanted) = parse(binding) else {
        return false;
    };

    // Only the modifiers a binding names are required to be down. Shift is not
    // compared for characters at all: a terminal reports `!` as Char('!') with
    // SHIFT set on some platforms and without it on others, and a shortcut
    // that works on Linux and not on Windows is worse than one that ignores a
    // modifier nobody meant to type.
    let control_ok = wanted.modifiers.contains(KeyModifiers::CONTROL)
        == key.modifiers.contains(KeyModifiers::CONTROL);
    let alt_ok =
        wanted.modifiers.contains(KeyModifiers::ALT) == key.modifiers.contains(KeyModifiers::ALT);
    if !control_ok || !alt_ok {
        return false;
    }

    match (wanted.code, key.code) {
        (KeyCode::Char(want), KeyCode::Char(got)) => {
            let got_lower = got.to_lowercase().next().unwrap_or(got);
            if want == got_lower {
                return true;
            }
            layout_twin(want) == Some(got_lower)
        }
        (a, b) => a == b,
    }
}

/// Which action, if any, this keypress asks for.
///
/// Global bindings first, then the ones that belong to the tab on screen: a
/// letter means different things on different tabs, and a modifier or an F-key
/// means the same thing everywhere.
pub fn resolve(key: KeyEvent, keys: &KeyBindings, tab: AppTab) -> Option<Action> {
    let global = [
        (&keys.screenshot, Action::Screenshot),
        (&keys.language, Action::Language),
        (&keys.help, Action::Help),
        (&keys.quit, Action::Quit),
        (&keys.next_tab, Action::NextTab),
        (&keys.prev_tab, Action::PrevTab),
        (&keys.tab_dashboard, Action::GoToTab(AppTab::Dashboard)),
        (&keys.tab_telemetry, Action::GoToTab(AppTab::Telemetry)),
        (&keys.tab_engineer, Action::GoToTab(AppTab::Engineer)),
        (&keys.tab_setup, Action::GoToTab(AppTab::Setup)),
        (&keys.tab_analysis, Action::GoToTab(AppTab::Analysis)),
        (&keys.tab_strategy, Action::GoToTab(AppTab::Strategy)),
        (&keys.tab_ffb, Action::GoToTab(AppTab::Ffb)),
        (&keys.tab_settings, Action::GoToTab(AppTab::Settings)),
        (&keys.tab_guide, Action::GoToTab(AppTab::Guide)),
    ];

    for (binding, action) in global {
        if matches(binding, key) {
            return Some(action);
        }
    }

    for (character, action) in RESERVED {
        if matches(&character.to_string(), key) {
            return Some(*action);
        }
    }

    let local: &[(&String, Action)] = match tab {
        AppTab::Analysis => &[
            (&keys.analysis_save, Action::AnalysisSave),
            (&keys.analysis_load, Action::AnalysisLoad),
            (&keys.analysis_compare, Action::AnalysisCompare),
            (&keys.analysis_export, Action::AnalysisExport),
            (&keys.analysis_filter, Action::AnalysisFilter),
        ],
        AppTab::Setup => &[
            (&keys.setup_browser, Action::SetupBrowser),
            (&keys.setup_download, Action::SetupDownload),
        ],
        // Only on Settings, and in practice only while the OVERLAY category is
        // showing — but that is a screen the key handler knows about and this
        // table does not, and a letter that does nothing on four of five
        // categories is better than a letter written into a string.
        AppTab::Settings => &[
            (&keys.overlay_install, Action::OverlayInstall),
            (&keys.overlay_uninstall, Action::OverlayUninstall),
            (&keys.overlay_diagnostics, Action::OverlayDiagnostics),
        ],
        _ => &[],
    };

    for (binding, action) in local {
        if matches(binding, key) {
            return Some(*action);
        }
    }

    None
}

/// Every binding, as (field name, label, current value).
///
/// The Settings screen lists these and the conflict check walks them, so a
/// binding added to [`KeyBindings`] and forgotten here is a binding with no way
/// to change it — which `every_binding_is_listed_for_the_settings_screen`
/// catches by counting the fields in the serialised form.
pub fn all(keys: &KeyBindings) -> Vec<(&'static str, &'static str, &str)> {
    vec![
        ("help", "Help", keys.help.as_str()),
        ("quit", "Back / quit", keys.quit.as_str()),
        ("screenshot", "Screenshot", keys.screenshot.as_str()),
        (
            "overlay_install",
            "Install the panel",
            keys.overlay_install.as_str(),
        ),
        (
            "overlay_uninstall",
            "Remove the panel",
            keys.overlay_uninstall.as_str(),
        ),
        (
            "overlay_diagnostics",
            "Overlay diagnostics",
            keys.overlay_diagnostics.as_str(),
        ),
        ("language", "Switch language", keys.language.as_str()),
        ("next_tab", "Next tab", keys.next_tab.as_str()),
        ("prev_tab", "Previous tab", keys.prev_tab.as_str()),
        (
            "tab_dashboard",
            "Go to Dashboard",
            keys.tab_dashboard.as_str(),
        ),
        (
            "tab_telemetry",
            "Go to Telemetry",
            keys.tab_telemetry.as_str(),
        ),
        ("tab_engineer", "Go to Engineer", keys.tab_engineer.as_str()),
        ("tab_setup", "Go to Setup", keys.tab_setup.as_str()),
        ("tab_analysis", "Go to Analysis", keys.tab_analysis.as_str()),
        ("tab_strategy", "Go to Strategy", keys.tab_strategy.as_str()),
        ("tab_ffb", "Go to FFB", keys.tab_ffb.as_str()),
        ("tab_settings", "Go to Settings", keys.tab_settings.as_str()),
        ("tab_guide", "Go to Guide", keys.tab_guide.as_str()),
        (
            "analysis_save",
            "Analysis: save lap",
            keys.analysis_save.as_str(),
        ),
        (
            "analysis_load",
            "Analysis: load lap",
            keys.analysis_load.as_str(),
        ),
        (
            "analysis_compare",
            "Analysis: compare ghost",
            keys.analysis_compare.as_str(),
        ),
        (
            "analysis_export",
            "Analysis: export CSV",
            keys.analysis_export.as_str(),
        ),
        (
            "analysis_filter",
            "Analysis: only real losses",
            keys.analysis_filter.as_str(),
        ),
        (
            "setup_browser",
            "Setup: open browser",
            keys.setup_browser.as_str(),
        ),
        (
            "setup_download",
            "Setup: download",
            keys.setup_download.as_str(),
        ),
    ]
}

/// What each binding does, by the name [`all`] gives it.
///
/// The hints are built from this, and `the_hints_only_name_keys_that_do_
/// something` walks it, so a field named in a hint whose action does not fire
/// on that tab fails a test rather than misleading a driver.
pub fn action_of(field: &str) -> Option<Action> {
    Some(match field {
        "help" => Action::Help,
        "quit" => Action::Quit,
        "screenshot" => Action::Screenshot,
        "overlay_install" => Action::OverlayInstall,
        "overlay_uninstall" => Action::OverlayUninstall,
        "overlay_diagnostics" => Action::OverlayDiagnostics,
        "language" => Action::Language,
        "next_tab" => Action::NextTab,
        "prev_tab" => Action::PrevTab,
        "tab_dashboard" => Action::GoToTab(AppTab::Dashboard),
        "tab_telemetry" => Action::GoToTab(AppTab::Telemetry),
        "tab_engineer" => Action::GoToTab(AppTab::Engineer),
        "tab_setup" => Action::GoToTab(AppTab::Setup),
        "tab_analysis" => Action::GoToTab(AppTab::Analysis),
        "tab_strategy" => Action::GoToTab(AppTab::Strategy),
        "tab_ffb" => Action::GoToTab(AppTab::Ffb),
        "tab_settings" => Action::GoToTab(AppTab::Settings),
        "tab_guide" => Action::GoToTab(AppTab::Guide),
        "analysis_save" => Action::AnalysisSave,
        "analysis_load" => Action::AnalysisLoad,
        "analysis_compare" => Action::AnalysisCompare,
        "analysis_export" => Action::AnalysisExport,
        "analysis_filter" => Action::AnalysisFilter,
        "setup_browser" => Action::SetupBrowser,
        "setup_download" => Action::SetupDownload,
        _ => return None,
    })
}

/// The current value of a binding, by the name [`all`] gives it.
pub fn value_of<'a>(keys: &'a KeyBindings, field: &str) -> Option<&'a str> {
    all(keys)
        .into_iter()
        .find(|(name, _, _)| *name == field)
        .map(|(_, _, value)| value)
}

/// What to offer at the bottom right of a tab: the binding to name, and what
/// to call it in each language.
///
/// Short on purpose — this shares a row with the status chips. The keys that
/// work everywhere (tab switching, the digits, the screenshot) are in the help
/// overlay rather than here; these are the ones that do something on *this*
/// screen and would otherwise go undiscovered.
pub fn hints(tab: AppTab) -> &'static [(&'static str, &'static str, &'static str)] {
    match tab {
        AppTab::Analysis => &[
            ("analysis_save", "Save", "Сохр"),
            ("analysis_load", "Load", "Загр"),
            ("analysis_compare", "Ghost", "Призрак"),
            ("analysis_export", "CSV", "CSV"),
            ("analysis_filter", "Losses", "Потери"),
            ("help", "Help", "Помощь"),
        ],
        AppTab::Setup => &[
            ("setup_browser", "Browser", "Браузер"),
            ("setup_download", "Download", "Скачать"),
            ("help", "Help", "Помощь"),
        ],
        _ => &[
            ("screenshot", "Screenshot", "Снимок"),
            ("language", "Language", "Язык"),
            ("help", "Help", "Помощь"),
        ],
    }
}

/// Write a binding by the name [`all`] gives it.
pub fn set(keys: &mut KeyBindings, field: &str, value: String) {
    match field {
        "help" => keys.help = value,
        "quit" => keys.quit = value,
        "screenshot" => keys.screenshot = value,
        "overlay_install" => keys.overlay_install = value,
        "overlay_uninstall" => keys.overlay_uninstall = value,
        "overlay_diagnostics" => keys.overlay_diagnostics = value,
        "language" => keys.language = value,
        "next_tab" => keys.next_tab = value,
        "prev_tab" => keys.prev_tab = value,
        "tab_dashboard" => keys.tab_dashboard = value,
        "tab_telemetry" => keys.tab_telemetry = value,
        "tab_engineer" => keys.tab_engineer = value,
        "tab_setup" => keys.tab_setup = value,
        "tab_analysis" => keys.tab_analysis = value,
        "tab_strategy" => keys.tab_strategy = value,
        "tab_ffb" => keys.tab_ffb = value,
        "tab_settings" => keys.tab_settings = value,
        "tab_guide" => keys.tab_guide = value,
        "analysis_save" => keys.analysis_save = value,
        "analysis_load" => keys.analysis_load = value,
        "analysis_compare" => keys.analysis_compare = value,
        "analysis_export" => keys.analysis_export = value,
        "analysis_filter" => keys.analysis_filter = value,
        "setup_browser" => keys.setup_browser = value,
        "setup_download" => keys.setup_download = value,
        _ => {}
    }
}

/// Which other binding a value collides with, if any.
///
/// Two actions on one key is not an error — the tab-local ones deliberately
/// reuse letters the global ones do not want — but silently shadowing a
/// binding that already wins is, because the losing one simply stops working
/// with nothing on screen to say why.
/// Which tab a binding belongs to, or `None` when it fires everywhere.
///
/// Kept beside [`resolve`]'s tab table and checked against it by
/// `every_tab_local_binding_knows_which_tab_it_is_on`, because the two
/// disagreeing is a clash reported for keys that cannot meet, or a real clash
/// waved through.
fn scope_of(field: &str) -> Option<AppTab> {
    match field {
        "analysis_save" | "analysis_load" | "analysis_compare" | "analysis_export"
        | "analysis_filter" => Some(AppTab::Analysis),
        "setup_browser" | "setup_download" => Some(AppTab::Setup),
        "overlay_install" | "overlay_uninstall" | "overlay_diagnostics" => Some(AppTab::Settings),
        _ => None,
    }
}

/// Can these two bindings ever be pressed in the same place?
///
/// Two tab-local keys on different tabs cannot: a letter meaning one thing on
/// Analysis and another on Setup is the design. A global one shadows a
/// tab-local one everywhere, because [`resolve`] checks the globals first, so
/// that pair *is* a clash.
fn can_collide(a: &str, b: &str) -> bool {
    match (scope_of(a), scope_of(b)) {
        (Some(left), Some(right)) => left == right,
        _ => true,
    }
}

pub fn conflict(keys: &KeyBindings, field: &str, value: &str) -> Option<&'static str> {
    let wanted = parse(value)?;

    // The reserved aliases first: they win over anything tab-local, and a
    // binding shadowed by one is the hardest kind to work out from the outside.
    if let KeyCode::Char(character) = wanted.code
        && wanted.modifiers == KeyModifiers::NONE
        && let Some((_, action)) = RESERVED.iter().find(|(c, _)| *c == character)
    {
        return Some(match action {
            Action::Help => "Help (? is fixed)",
            _ => "Back / quit (Q is fixed)",
        });
    }

    all(keys)
        .into_iter()
        .find(|(name, _, current)| {
            *name != field && can_collide(field, name) && parse(current) == Some(wanted)
        })
        .map(|(_, label, _)| label)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn press(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    #[test]
    fn the_written_form_survives_a_round_trip() {
        for text in [
            "f1",
            "f12",
            "esc",
            "tab",
            "shift+tab",
            "ctrl+s",
            "ctrl+l",
            "1",
            "s",
            "space",
            "pgup",
            "del",
            "up",
        ] {
            let parsed = parse(text).expect("a default binding has to parse");
            let printed = describe(text);
            assert!(!printed.is_empty(), "{text} printed as nothing");
            let key = KeyEvent::new(parsed.code, parsed.modifiers);
            assert!(
                matches(text, key),
                "{text} parsed to something that does not match itself"
            );
        }
    }

    /// Shift+Tab is a key code of its own, never Tab with a modifier, so a
    /// config spelling it the way a person would has to fold to BackTab.
    #[test]
    fn shift_tab_is_its_own_key() {
        assert_eq!(
            parse("shift+tab"),
            Some(Binding {
                code: KeyCode::BackTab,
                modifiers: KeyModifiers::NONE
            })
        );
        assert!(matches("shift+tab", press(KeyCode::BackTab)));
        assert!(!matches("tab", press(KeyCode::BackTab)));
    }

    /// The one thing that has to work identically on Linux and Windows: a
    /// letter typed with shift down arrives upper-case on one and not the
    /// other, and a shortcut that depends on which is a shortcut that works on
    /// one platform.
    #[test]
    fn a_letter_matches_whatever_case_it_arrives_in() {
        assert!(matches("s", press(KeyCode::Char('s'))));
        assert!(matches(
            "s",
            KeyEvent::new(KeyCode::Char('S'), KeyModifiers::SHIFT)
        ));
    }

    /// The Cyrillic twin comes along with a rebound letter, rather than being
    /// spelled out per arm the way it used to be.
    #[test]
    fn a_rebound_letter_keeps_its_layout_twin() {
        assert!(matches("s", press(KeyCode::Char('ы'))));
        assert!(matches("w", press(KeyCode::Char('ц'))));
        assert!(!matches("s", press(KeyCode::Char('в'))));
    }

    /// Ctrl+S is the screenshot and S is the analysis save. Confusing them
    /// writes a file every time someone saves a lap.
    #[test]
    fn a_modifier_is_part_of_the_binding() {
        assert!(!matches("ctrl+s", press(KeyCode::Char('s'))));
        assert!(!matches(
            "s",
            KeyEvent::new(KeyCode::Char('s'), KeyModifiers::CONTROL)
        ));
        assert!(matches(
            "ctrl+s",
            KeyEvent::new(KeyCode::Char('s'), KeyModifiers::CONTROL)
        ));
    }

    /// A hand-edited config with a typo in it costs one shortcut, not the
    /// application.
    #[test]
    fn nonsense_leaves_the_action_unbound() {
        assert_eq!(parse("f13"), None);
        assert_eq!(parse("wat"), None);
        assert_eq!(parse(""), None);
        assert!(!matches("wat", press(KeyCode::Char('w'))));
        // And still shows the driver what is in their file.
        assert_eq!(describe("wat"), "WAT");
    }

    #[test]
    fn a_captured_key_spells_itself_back() {
        let captured =
            spell(KeyEvent::new(KeyCode::Char('S'), KeyModifiers::CONTROL)).expect("ctrl+S spells");
        assert_eq!(captured, "ctrl+s");
        assert!(matches(
            &captured,
            KeyEvent::new(KeyCode::Char('s'), KeyModifiers::CONTROL)
        ));
    }

    /// The defaults have to be the map that shipped, or an existing user's
    /// muscle memory breaks on upgrade.
    #[test]
    fn the_defaults_are_the_map_that_always_shipped() {
        let keys = KeyBindings::default();
        assert_eq!(
            resolve(press(KeyCode::F(1)), &keys, AppTab::Dashboard),
            Some(Action::Help)
        );
        assert_eq!(
            resolve(
                KeyEvent::new(KeyCode::Char('s'), KeyModifiers::CONTROL),
                &keys,
                AppTab::Dashboard
            ),
            Some(Action::Screenshot)
        );
        assert_eq!(
            resolve(press(KeyCode::Char('3')), &keys, AppTab::Dashboard),
            Some(Action::GoToTab(AppTab::Engineer))
        );
        assert_eq!(
            resolve(press(KeyCode::Tab), &keys, AppTab::Dashboard),
            Some(Action::NextTab)
        );
        assert_eq!(
            resolve(press(KeyCode::Esc), &keys, AppTab::Dashboard),
            Some(Action::Quit)
        );
    }

    /// A letter means one thing on Analysis and another on Setup, and nothing
    /// at all on the tabs that do not claim it.
    #[test]
    fn tab_local_keys_only_fire_on_their_tab() {
        let keys = KeyBindings::default();
        assert_eq!(
            resolve(press(KeyCode::Char('s')), &keys, AppTab::Analysis),
            Some(Action::AnalysisSave)
        );
        assert_eq!(
            resolve(press(KeyCode::Char('s')), &keys, AppTab::Setup),
            None
        );
        assert_eq!(
            resolve(press(KeyCode::Char('b')), &keys, AppTab::Setup),
            Some(Action::SetupBrowser)
        );
        assert_eq!(
            resolve(press(KeyCode::Char('b')), &keys, AppTab::Dashboard),
            None
        );
    }

    /// Rebinding is the point, so it has to actually take effect.
    #[test]
    fn a_rebound_key_replaces_the_default() {
        let mut keys = KeyBindings::default();
        set(&mut keys, "screenshot", "ctrl+o".to_string());
        assert_eq!(
            resolve(
                KeyEvent::new(KeyCode::Char('s'), KeyModifiers::CONTROL),
                &keys,
                AppTab::Dashboard
            ),
            None
        );
        assert_eq!(
            resolve(
                KeyEvent::new(KeyCode::Char('o'), KeyModifiers::CONTROL),
                &keys,
                AppTab::Dashboard
            ),
            Some(Action::Screenshot)
        );
    }

    /// Binding two actions to one key hides one of them with nothing on screen
    /// to say so, which is exactly the failure the Settings screen has to warn
    /// about before writing it.
    #[test]
    fn a_clash_is_reported_against_the_binding_it_shadows() {
        let keys = KeyBindings::default();
        assert_eq!(conflict(&keys, "language", "ctrl+s"), Some("Screenshot"));
        assert_eq!(conflict(&keys, "screenshot", "ctrl+s"), None);
        assert_eq!(conflict(&keys, "language", "f9"), None);

        // The fixed aliases shadow anything tab-local, and that is the hardest
        // clash to work out from the outside.
        assert!(conflict(&keys, "analysis_save", "q").is_some());
        assert!(conflict(&keys, "analysis_save", "?").is_some());
    }

    /// `scope_of` and the tab table inside `resolve` are two statements about
    /// the same thing, and they disagreeing is either a clash reported between
    /// keys that can never meet, or a real one waved through.
    #[test]
    fn every_tab_local_binding_knows_which_tab_it_is_on() {
        let keys = KeyBindings::default();
        for (field, _, _) in all(&keys) {
            let Some(tab) = scope_of(field) else { continue };
            let action = action_of(field).expect("a listed binding has an action");
            // It fires on its own tab...
            let value = all(&keys)
                .into_iter()
                .find(|(name, _, _)| *name == field)
                .map(|(_, _, current)| current.to_string())
                .expect("listed");
            let binding = parse(&value).expect("a default binding parses");
            assert_eq!(
                resolve(KeyEvent::new(binding.code, binding.modifiers), &keys, tab),
                Some(action),
                "{field} claims {tab:?} and does not fire there"
            );
        }
    }

    /// Two tab-local keys on different tabs are not a clash. `C` compares laps
    /// on Analysis and opens the overlay diagnostics on Settings, and refusing
    /// one because of the other is refusing a binding that already works.
    #[test]
    fn a_letter_on_two_different_tabs_is_not_a_clash() {
        let keys = KeyBindings::default();
        assert_eq!(keys.analysis_compare, "c");
        assert_eq!(keys.overlay_diagnostics, "c");
        assert_eq!(conflict(&keys, "overlay_diagnostics", "c"), None);
        assert_eq!(conflict(&keys, "analysis_compare", "c"), None);

        // A global one still shadows a tab-local one, because `resolve` checks
        // the globals first.
        assert!(conflict(&keys, "screenshot", "c").is_some());
    }

    /// Q and ? have closed the help since the first release, and the modal
    /// says so in nine places that no binding reaches.
    #[test]
    fn the_fixed_aliases_still_work() {
        let keys = KeyBindings::default();
        assert_eq!(
            resolve(press(KeyCode::Char('q')), &keys, AppTab::Dashboard),
            Some(Action::Quit)
        );
        assert_eq!(
            resolve(press(KeyCode::Char('й')), &keys, AppTab::Dashboard),
            Some(Action::Quit)
        );
        assert_eq!(
            resolve(press(KeyCode::Char('?')), &keys, AppTab::Dashboard),
            Some(Action::Help)
        );
    }

    /// A configured binding beats the fixed alias, or rebinding is a lie.
    #[test]
    fn a_configured_binding_beats_a_fixed_alias() {
        let mut keys = KeyBindings::default();
        set(&mut keys, "screenshot", "q".to_string());
        assert_eq!(
            resolve(press(KeyCode::Char('q')), &keys, AppTab::Dashboard),
            Some(Action::Screenshot)
        );
    }

    /// Every field of KeyBindings has to be listed, or it is a binding with no
    /// way to change it and no way to see it. Counted off the serialised form
    /// so adding a field is what fails, not remembering to update a number.
    #[test]
    fn every_binding_is_listed_for_the_settings_screen() {
        let keys = KeyBindings::default();
        let value = serde_json::to_value(&keys).expect("KeyBindings serialises");
        let fields = value
            .as_object()
            .expect("KeyBindings is a struct, so it serialises as an object");

        let listed: Vec<&str> = all(&keys).into_iter().map(|(name, _, _)| name).collect();
        for field in fields.keys() {
            assert!(
                listed.contains(&field.as_str()),
                "{field} is a binding the Settings screen cannot show or change"
            );
        }
        assert_eq!(listed.len(), fields.len());
    }

    /// The check this module exists for.
    ///
    /// Every key named in a hint has to do, on the tab the hint is drawn on,
    /// the thing the hint says it does. The Setup tab offered `'D' - Download`
    /// on a screen where D was not handled at all, and nothing caught it
    /// because the text and the handler had no connection.
    #[test]
    fn the_hints_only_name_keys_that_do_something() {
        let keys = KeyBindings::default();
        let tabs = [
            AppTab::Dashboard,
            AppTab::Telemetry,
            AppTab::Engineer,
            AppTab::Setup,
            AppTab::Analysis,
            AppTab::Strategy,
            AppTab::Ffb,
            AppTab::Settings,
            AppTab::Guide,
        ];

        for tab in tabs {
            for (field, label, _) in hints(tab) {
                let binding = value_of(&keys, field);
                assert!(
                    binding.is_some(),
                    "{tab:?} names {field} in a hint, and it is not a binding"
                );
                let binding = binding.unwrap_or_default();

                let parsed = parse(binding);
                assert!(
                    parsed.is_some(),
                    "{tab:?} names {field} in a hint, and it is bound to \
                     something unreadable: {binding}"
                );
                let parsed = parsed.unwrap_or(Binding {
                    code: KeyCode::Null,
                    modifiers: KeyModifiers::NONE,
                });
                let key = KeyEvent::new(parsed.code, parsed.modifiers);

                assert_eq!(
                    resolve(key, &keys, tab),
                    action_of(field),
                    "{tab:?} offers {label} on {}, which does something else there",
                    describe(binding)
                );
            }
        }
    }

    /// And it has to keep holding after a rebind, or the hint is only true
    /// until someone uses the feature this release added.
    #[test]
    fn the_hints_follow_a_rebound_key() {
        let mut keys = KeyBindings::default();
        set(&mut keys, "analysis_save", "f6".to_string());

        let binding = value_of(&keys, "analysis_save").expect("bound");
        assert_eq!(describe(binding), "F6");

        let parsed = parse(binding).expect("readable");
        assert_eq!(
            resolve(
                KeyEvent::new(parsed.code, parsed.modifiers),
                &keys,
                AppTab::Analysis
            ),
            Some(Action::AnalysisSave)
        );
    }

    /// `set` has to reach every field, or a binding is editable on screen and
    /// silently discarded on the way to the config.
    #[test]
    fn set_reaches_every_binding() {
        for (field, _, _) in all(&KeyBindings::default()) {
            let mut keys = KeyBindings::default();
            set(&mut keys, field, "f9".to_string());
            let written = all(&keys)
                .into_iter()
                .find(|(name, _, _)| *name == field)
                .map(|(_, _, value)| value.to_string());
            assert_eq!(
                written.as_deref(),
                Some("f9"),
                "set() does not write {field}"
            );
        }
    }
}
