//! What a driver built up in Content Manager, found so it can come along.
//!
//! **Moving launcher should cost nothing.** Most of what matters is already
//! shared — Content Manager installs mods into the game folder and writes
//! the game's own settings and setups in Documents, which any launcher
//! reads. What is Content Manager's alone lives in its own folder,
//! `AppData/Local/AcTools Content Manager`: the servers starred, joined and
//! hidden, lists of servers a driver made, and the patch's presets. This
//! finds them — on Windows in the user's folder, on Linux inside the game's
//! Proton prefix where Content Manager runs — so they can be brought over
//! with one press instead of rebuilt by hand.
//!
//! Server lists are text: `Online servers/<List>.txt`, a server a line as
//! `IP:HTTP port;Name`, lines starting with `#` its own notes. Nothing here
//! writes into Content Manager's folder.

use std::path::{Path, PathBuf};

/// Content Manager's own folder, found beside the game's Documents:
/// `…/Documents/Assetto Corsa` → `…/AppData/Local/AcTools Content Manager`.
pub fn folder(game_documents: &Path) -> Option<PathBuf> {
    let home = game_documents.parent()?.parent()?;
    let at = home
        .join("AppData")
        .join("Local")
        .join("AcTools Content Manager");
    at.is_dir().then_some(at)
}

/// One server in a list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Server {
    pub ip: String,
    /// The server's HTTP port — the one a list names a server by.
    pub http: u16,
    pub name: Option<String>,
}

impl Server {
    /// `ip:http`, the way a server is keyed everywhere here.
    pub fn key(&self) -> String {
        format!("{}:{}", self.ip, self.http)
    }
}

/// One list of servers: favourites, recent, hidden, or one of the driver's.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct List {
    /// The file's name without `.txt`, as Content Manager shows it.
    pub name: String,
    pub servers: Vec<Server>,
}

impl List {
    /// Which of Content Manager's own lists this is, by its name.
    pub fn is(&self, which: &str) -> bool {
        self.name.eq_ignore_ascii_case(which)
    }
}

/// Read one list's text.
pub fn parse_list(text: &str) -> Vec<Server> {
    let mut out: Vec<Server> = Vec::new();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let (address, name) = match line.split_once(';') {
            Some((address, name)) => (address.trim(), Some(name.trim().to_string())),
            None => (line, None),
        };
        let Some((ip, port)) = address.rsplit_once(':') else {
            continue;
        };
        let Ok(http) = port.trim().parse::<u16>() else {
            continue;
        };
        let ip = ip.trim().to_string();
        let addressable = !ip.is_empty()
            && ip
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | ':' | '[' | ']'));
        if http == 0 || !addressable {
            continue;
        }
        let server = Server {
            ip,
            http,
            name: name.filter(|n| !n.is_empty()),
        };
        if !out.iter().any(|s| s.key() == server.key()) {
            out.push(server);
        }
    }
    out
}

/// Every server list Content Manager keeps.
pub fn lists(folder: &Path) -> Vec<List> {
    let Ok(entries) = std::fs::read_dir(folder.join("Online servers")) else {
        return Vec::new();
    };
    let mut found: Vec<List> = entries
        .flatten()
        .filter_map(|entry| {
            let name = entry.file_name().to_string_lossy().into_owned();
            let stem = name
                .strip_suffix(".txt")
                .or_else(|| name.strip_suffix(".TXT"))?;
            let text = std::fs::read_to_string(entry.path()).ok()?;
            Some(List {
                name: stem.to_string(),
                servers: parse_list(&text),
            })
        })
        .filter(|list| !list.servers.is_empty())
        .collect();
    found.sort_by_key(|list| list.name.to_lowercase());
    found
}

/// What there is to bring over.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Found {
    pub folder: Option<PathBuf>,
    pub lists: Vec<List>,
    /// How many of the patch's presets Content Manager keeps.
    pub csp_presets: usize,
    /// Its driving-aid presets, read into the game's own keys.
    pub assists: Vec<AssistsPreset>,
}

/// One of Content Manager's driving-aid presets, as `cfg/assists.ini`
/// values the game reads.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssistsPreset {
    pub name: String,
    /// `(KEY, value)` under `[ASSISTS]`.
    pub values: Vec<(&'static str, String)>,
}

/// Read one of Content Manager's aid presets — JSON with its own names for
/// each aid — into the game's keys. Aids it does not name are left out, so
/// applying it leaves them as they were.
pub fn parse_assists(json: &str) -> Vec<(&'static str, String)> {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(json) else {
        return Vec::new();
    };
    let flag = |b: bool| if b { "1" } else { "0" }.to_string();
    let number = |v: &serde_json::Value| -> Option<f64> {
        v.as_f64()
            .or_else(|| v.as_bool().map(|b| if b { 1.0 } else { 0.0 }))
    };
    let mut out = Vec::new();
    for (theirs, ours) in [
        ("Abs", "ABS"),
        ("TractionControl", "TRACTION_CONTROL"),
        ("StabilityControl", "STABILITY_CONTROL"),
        ("AutoClutch", "AUTO_CLUTCH"),
        ("AutoBlip", "AUTO_BLIP"),
        ("AutoShifter", "AUTO_SHIFTER"),
        ("IdealLine", "IDEAL_LINE"),
        ("Damage", "DAMAGE"),
        ("TyreWear", "TYRE_WEAR"),
        ("FuelConsumption", "FUEL_RATE"),
        ("TyreBlankets", "TYRE_BLANKETS"),
        ("VisualDamage", "VISUALDAMAGE"),
        ("SlipSteam", "SLIPSTREAM"),
    ] {
        let Some(v) = value.get(theirs) else { continue };
        let written = match (ours, v) {
            (_, serde_json::Value::Bool(b)) => flag(*b),
            // Percentages and multipliers as the game writes them: whole
            // numbers where they are whole.
            (_, v) => match number(v) {
                Some(n) if n.fract() == 0.0 => format!("{}", n as i64),
                Some(n) => format!("{n}"),
                None => continue,
            },
        };
        out.push((ours, written));
    }
    out
}

/// Content Manager's aid presets, by name.
pub fn assists_presets(folder: &Path) -> Vec<AssistsPreset> {
    let Ok(entries) = std::fs::read_dir(folder.join("Presets").join("Assists")) else {
        return Vec::new();
    };
    let mut found: Vec<AssistsPreset> = entries
        .flatten()
        .filter_map(|entry| {
            let name = entry.file_name().to_string_lossy().into_owned();
            let stem = name.strip_suffix(".cmpreset")?;
            let values = parse_assists(&std::fs::read_to_string(entry.path()).ok()?);
            (!values.is_empty()).then(|| AssistsPreset {
                name: stem.to_string(),
                values,
            })
        })
        .collect();
    found.sort_by_key(|p| p.name.to_lowercase());
    found
}

/// Apply an aid preset to the game's `cfg/assists.ini`, every other line
/// left as it was.
pub fn apply_assists(game_documents: &Path, preset: &AssistsPreset) -> Result<(), String> {
    for (key, value) in &preset.values {
        super::ini::set_in_file(&super::assists::file(game_documents), "ASSISTS", key, value)?;
    }
    Ok(())
}

impl Found {
    /// Whether Content Manager was found at all.
    pub fn any(&self) -> bool {
        self.folder.is_some()
    }

    /// The servers of the lists named, or of the driver's own lists when
    /// `which` is `None` (every list but recent and hidden).
    pub fn servers(&self, which: Option<&str>) -> Vec<&Server> {
        self.lists
            .iter()
            .filter(|list| match which {
                Some(name) => list.is(name),
                None => !list.is("recent") && !list.is("hidden"),
            })
            .flat_map(|list| list.servers.iter())
            .collect()
    }
}

/// Look for everything at once.
pub fn find(game_documents: &Path) -> Found {
    let Some(folder) = folder(game_documents) else {
        return Found::default();
    };
    let csp_presets =
        std::fs::read_dir(folder.join("Presets").join("Custom Shaders Patch Presets"))
            .map(|entries| {
                entries
                    .flatten()
                    .filter(|e| {
                        e.file_name()
                            .to_string_lossy()
                            .to_lowercase()
                            .ends_with(".ini")
                    })
                    .count()
            })
            .unwrap_or(0);
    Found {
        lists: lists(&folder),
        assists: assists_presets(&folder),
        folder: Some(folder),
        csp_presets,
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::indexing_slicing)]
    use super::*;

    /// **Content Manager's own format**: `IP:HTTP port;Name`, its notes
    /// skipped, a line that is not a server left out, a server twice kept
    /// once.
    #[test]
    fn a_list_reads_as_content_manager_writes_it() {
        let servers = parse_list(
            "#version=2\n# a note\n\n203.0.113.7:8081;Friday Night League\n\
             race.example.org:9600\n203.0.113.7:8081;again\nnot an address\n\
             10.0.0.1:0\n10.0.0.2:port\n../x:80\n",
        );
        assert_eq!(
            servers,
            vec![
                Server {
                    ip: "203.0.113.7".into(),
                    http: 8081,
                    name: Some("Friday Night League".into())
                },
                Server {
                    ip: "race.example.org".into(),
                    http: 9600,
                    name: None
                },
            ]
        );
        assert_eq!(servers[0].key(), "203.0.113.7:8081");
    }

    /// **Content Manager's aid presets in the game's own words**: its JSON
    /// names turned into `assists.ini` keys, written into the game's file
    /// with the rest of it left alone.
    #[test]
    fn an_aid_preset_becomes_the_games_own_values() {
        let pro = r#"{"IdealLine":false,"AutoBlip":true,"StabilityControl":0.0,"Abs":1,
            "TractionControl":2,"Damage":100.0,"TyreWear":1.0,"FuelConsumption":1.5,
            "TyreBlankets":false,"Unknown":7}"#;
        let values = parse_assists(pro);
        let get = |k: &str| {
            values
                .iter()
                .find(|(key, _)| *key == k)
                .map(|(_, v)| v.as_str())
        };
        assert_eq!(get("IDEAL_LINE"), Some("0"));
        assert_eq!(get("AUTO_BLIP"), Some("1"));
        assert_eq!(get("ABS"), Some("1"));
        assert_eq!(get("TRACTION_CONTROL"), Some("2"));
        assert_eq!(get("DAMAGE"), Some("100"));
        assert_eq!(get("FUEL_RATE"), Some("1.5"));
        assert_eq!(get("STABILITY_CONTROL"), Some("0"));
        assert!(parse_assists("not json").is_empty());
        let dir = std::env::temp_dir().join(format!("acpe-cm-aids-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("cfg")).expect("a fixture");
        std::fs::write(dir.join("cfg/assists.ini"), "[ASSISTS]\nABS=0\nKEEP=me\n")
            .expect("a fixture");
        let preset = AssistsPreset {
            name: "Pro".into(),
            values,
        };
        apply_assists(&dir, &preset).expect("it is written");
        let text = std::fs::read_to_string(dir.join("cfg/assists.ini")).unwrap_or_default();
        assert!(text.contains("ABS=1") && text.contains("KEEP=me") && text.contains("DAMAGE=100"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// **Found where Content Manager keeps it** — in a Proton prefix's
    /// layout here — with the driver's own lists told from recent and
    /// hidden.
    #[test]
    fn everything_is_found_beside_the_documents() {
        let dir = std::env::temp_dir().join(format!("acpe-cm-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let docs = dir.join("users/steamuser/Documents/Assetto Corsa");
        std::fs::create_dir_all(&docs).expect("a fixture");
        assert!(!find(&docs).any());
        let cm = dir.join("users/steamuser/AppData/Local/AcTools Content Manager");
        let lists_at = cm.join("Online servers");
        std::fs::create_dir_all(&lists_at).expect("a fixture");
        std::fs::create_dir_all(cm.join("Presets/Custom Shaders Patch Presets"))
            .expect("a fixture");
        std::fs::write(
            cm.join("Presets/Custom Shaders Patch Presets/Night.ini"),
            "[CONFIGURATION]\n",
        )
        .expect("a fixture");
        for (name, text) in [
            ("Favourites.txt", "1.2.3.4:8081;A\n"),
            ("Recent.txt", "5.6.7.8:8081;B\n"),
            ("Hidden.txt", "9.9.9.9:8081;C\n"),
            ("League nights.txt", "1.2.3.5:8090;D\n"),
            ("Empty.txt", "# nothing\n"),
        ] {
            std::fs::write(lists_at.join(name), text).expect("a fixture");
        }
        let found = find(&docs);
        assert!(found.any());
        assert_eq!(found.csp_presets, 1);
        let names: Vec<&str> = found.lists.iter().map(|l| l.name.as_str()).collect();
        assert_eq!(
            names,
            vec!["Favourites", "Hidden", "League nights", "Recent"]
        );
        let keys = |servers: Vec<&Server>| servers.iter().map(|s| s.key()).collect::<Vec<_>>();
        assert_eq!(
            keys(found.servers(None)),
            vec!["1.2.3.4:8081", "1.2.3.5:8090"]
        );
        assert_eq!(keys(found.servers(Some("recent"))), vec!["5.6.7.8:8081"]);
        assert_eq!(keys(found.servers(Some("hidden"))), vec!["9.9.9.9:8081"]);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
