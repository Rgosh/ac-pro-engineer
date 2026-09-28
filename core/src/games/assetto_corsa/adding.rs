//! Putting a downloaded car, circuit or livery where the game will find it.
//!
//! A mod arrives as a folder or a zip, and putting it in place is one copy and
//! one thing you have to already know: whether what you were sent is a car, a
//! circuit or a livery, and — if it is a livery — which car it belongs to.
//! Everybody who has installed one has also once unpacked a skin into
//! `content/cars` and wondered why the game grew a car with no model.
//!
//! **The recognition is here because the layout is here.** `content.rs` knows
//! that a car keeps `ui/ui_car.json` and that a circuit may keep its
//! description one folder deeper; this is the same knowledge read in the other
//! direction, and splitting the two across two programs is how they come to
//! disagree. Nothing above this line decides what a folder is.
//!
//! **One rule, read from two kinds of thing.** A folder on disk and the index
//! of a zip are both reduced to a list of relative paths before anything looks
//! at them, so an archive and its extracted copy cannot be identified
//! differently. That is not tidiness: it is the only way the answer is the
//! same whichever way somebody happened to receive the mod.
//!
//! Nothing here deletes or moves what was dropped. It is copied, and the
//! original stays where it was — a mod manager that eats the download is one
//! people stop dropping things on.

use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

/// How deep the recogniser looks.
///
/// Everything any rule below asks about is within this many levels of the
/// drop, and a car mod is thousands of textures — reading all of them to find
/// out it is a car is work for nothing.
const DEEP: usize = 5;

/// The most paths any one recognition will consider, so a pathological archive
/// cannot be read into memory by pointing at it.
const MOST: usize = 40_000;

/// The largest description file worth reading to learn a mod's name.
const SMALL: u64 = 512 * 1024;

/// What a dropped folder turned out to hold.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// A car, which goes under `content/cars`.
    Car,
    /// A circuit, which goes under `content/tracks`.
    Track,
    /// A livery, which goes inside one car's `skins`.
    Skin,
    /// A weather preset, which goes under `content/weather`.
    Weather,
}

impl Kind {
    /// What to call it in a sentence.
    pub fn what(self) -> &'static str {
        match self {
            Self::Car => "car",
            Self::Track => "track",
            Self::Skin => "livery",
            Self::Weather => "weather",
        }
    }

    /// Which folder under `content` this kind lives in.
    ///
    /// A livery has no home of its own — it belongs to a car — which is why
    /// this is not the whole of [`Addition::where_it_goes`].
    fn folder(self) -> &'static str {
        match self {
            Self::Car | Self::Skin => "cars",
            Self::Track => "tracks",
            Self::Weather => "weather",
        }
    }

    /// The file this kind describes itself in, relative to its own folder.
    fn describes_itself_in(self) -> &'static str {
        match self {
            Self::Car => "ui/ui_car.json",
            Self::Track => "ui/ui_track.json",
            Self::Skin => "ui_skin.json",
            Self::Weather => "weather.ini",
        }
    }

    /// The field of that file that holds the name.
    fn calls_itself(self) -> &'static str {
        match self {
            Self::Skin => "skinname",
            _ => "name",
        }
    }
}

/// Where the thing is now: loose on disk, or inside an archive.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Source {
    /// A folder, which will be copied.
    Folder(PathBuf),
    /// A subtree of a zip, named by the prefix its entries share.
    ///
    /// The prefix is empty when the archive *is* the thing, and
    /// `content/cars/some_car/` when it was packed the way most of them are.
    InArchive {
        /// The archive on disk.
        archive: PathBuf,
        /// What every entry of this addition starts with. Ends in `/`, or is
        /// empty.
        under: String,
    },
}

/// One thing found in a drop, ready to be put in place.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Addition {
    /// What it is.
    pub kind: Kind,
    /// The folder name it will take.
    ///
    /// **Taken from the mod and never invented.** It is what the game is told
    /// and what the mod's own readme will name; tidying it into the game's
    /// house style would leave somebody looking for a folder that is not
    /// there.
    pub id: String,
    /// What it calls itself, where it says so. Only for showing.
    pub name: Option<String>,
    /// Which car a livery belongs to, when the drop said so.
    ///
    /// `None` means a bare livery folder, which names no car — somebody has to
    /// say, and that is why [`place`] takes one.
    pub car: Option<String>,
    /// Where it is now.
    pub from: Source,
}

impl Addition {
    /// Where this would be written under an install root.
    ///
    /// `None` for a livery whose car is known neither here nor by the caller,
    /// which is the one case that cannot be resolved without asking.
    pub fn where_it_goes(&self, root: &Path, car: Option<&str>) -> Option<PathBuf> {
        let content = root.join("content").join(self.kind.folder());
        match self.kind {
            Kind::Skin => {
                let car = self.car.as_deref().or(car)?;
                Some(content.join(car).join("skins").join(&self.id))
            }
            _ => Some(content.join(&self.id)),
        }
    }

    /// What this would replace, if anything is there already.
    ///
    /// **Asked before writing, and shown.** Overwriting a car somebody has
    /// spent an evening tuning, without saying so, is the difference between a
    /// tool and an accident.
    pub fn replaces(&self, root: &Path, car: Option<&str>) -> Option<PathBuf> {
        self.where_it_goes(root, car).filter(|at| at.exists())
    }

    /// The name to show: what it calls itself, or the folder it will take.
    pub fn title(&self) -> &str {
        self.name.as_deref().unwrap_or(&self.id)
    }
}

/// What went wrong, in terms that name the thing.
#[derive(Debug)]
pub enum Trouble {
    /// A livery was dropped on its own and nothing said which car it is for.
    WhichCar,
    /// Something is already there and replacing was not asked for.
    AlreadyThere(PathBuf),
    /// The disk, or the archive, said no.
    Disk(String),
}

impl std::fmt::Display for Trouble {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::WhichCar => write!(f, "a livery has to belong to a car, and none was named"),
            Self::AlreadyThere(at) => write!(f, "{} is already there", at.display()),
            Self::Disk(why) => write!(f, "{why}"),
        }
    }
}

impl std::error::Error for Trouble {}

/// Everything a dropped path turns out to hold.
///
/// An empty answer means it was not recognised, which is the honest answer for
/// a folder of screenshots. It is never a guess: a drop that cannot be named
/// is refused by the caller rather than put somewhere plausible, because a
/// wrong guess here writes hundreds of megabytes into the wrong folder.
pub fn whats_in(dropped: &Path) -> Vec<Addition> {
    let (paths, from) = if dropped.is_dir() {
        (walk(dropped), Source::Folder(dropped.to_path_buf()))
    } else if is_zip(dropped) {
        let Some(paths) = peek(dropped) else {
            return Vec::new();
        };
        (
            paths,
            Source::InArchive {
                archive: dropped.to_path_buf(),
                under: String::new(),
            },
        )
    } else {
        return Vec::new();
    };

    let mut found = read(&paths, &from, &name_of(dropped));
    for one in &mut found {
        one.name = describes(one);
    }
    found
}

/// Put one addition in place, returning where it landed.
///
/// `car` answers [`Trouble::WhichCar`] for a bare livery and is ignored
/// otherwise. `replacing` has to be asked for: without it an addition that is
/// already installed is refused rather than written over.
pub fn place(
    root: &Path,
    addition: &Addition,
    car: Option<&str>,
    replacing: bool,
) -> Result<PathBuf, Trouble> {
    let to = addition.where_it_goes(root, car).ok_or(Trouble::WhichCar)?;
    if to.exists() {
        if !replacing {
            return Err(Trouble::AlreadyThere(to));
        }
        fs::remove_dir_all(&to).map_err(|e| Trouble::Disk(format!("{}: {e}", to.display())))?;
    }
    if let Some(above) = to.parent() {
        fs::create_dir_all(above)
            .map_err(|e| Trouble::Disk(format!("{}: {e}", above.display())))?;
    }
    match &addition.from {
        Source::Folder(from) => copy_tree(from, &to).map_err(|e| Trouble::Disk(e.to_string()))?,
        Source::InArchive { archive, under } => unpack(archive, under, &to)?,
    }
    Ok(to)
}

// ---------------------------------------------------------------- recognition

/// The recognition itself: a list of relative paths, and what they say.
///
/// Three shapes, tried in this order, because a later one would misread an
/// earlier one:
///
/// 1. the drop **is** the thing — a car folder, a circuit, a livery
/// 2. the drop **contains** `content/cars/…`, which is how most archives are
///    packed and is also the only shape that says which car a livery is for
/// 3. the drop is a **bag** of several of them, which is how packs arrive
///
/// A car mod holds liveries and a circuit holds a `data` folder, so asking
/// "does this contain a livery" first would find one inside every car.
fn read(paths: &[String], from: &Source, called: &str) -> Vec<Addition> {
    if let Some(kind) = kind_at(paths, "") {
        return vec![Addition {
            kind,
            id: called.trim().to_string(),
            name: None,
            car: None,
            from: from.clone(),
        }];
    }
    let packed = under_content(paths, from);
    if !packed.is_empty() {
        return packed;
    }
    bag(paths, from)
}

/// What the paths under `at` say that folder is, if anything.
///
/// Every sign here is one the game itself relies on. `data.acd` is a car's
/// packed physics; `ui_track.json` is what the track picker reads, and it sits
/// one folder deeper on a circuit with several layouts; `fast_lane.ai` is the
/// racing line, which only a circuit has.
fn kind_at(paths: &[String], at: &str) -> Option<Kind> {
    let here = |name: &str| has(paths, &format!("{at}{name}"));

    if here("ui/ui_car.json") || here("data.acd") || (here("sfx/") && beside(paths, at, "kn5")) {
        return Some(Kind::Car);
    }
    if here("ui/ui_track.json")
        || here("ai/fast_lane.ai")
        || here("data/surfaces.ini")
        || deeper_ui_track(paths, at)
    {
        return Some(Kind::Track);
    }
    if here("weather.ini") {
        return Some(Kind::Weather);
    }
    if here("ui_skin.json") || beside(paths, at, "dds") {
        return Some(Kind::Skin);
    }
    None
}

/// A circuit with more than one layout keeps `ui/<layout>/ui_track.json`.
fn deeper_ui_track(paths: &[String], at: &str) -> bool {
    let under = format!("{at}ui/");
    paths
        .iter()
        .any(|p| p.starts_with(&under) && p.ends_with("/ui_track.json"))
}

/// Whether an exact relative path is in the list.
fn has(paths: &[String], what: &str) -> bool {
    let folder = format!("{what}/");
    paths.iter().any(|p| p == what || *p == folder)
        || (what.ends_with('/') && paths.iter().any(|p| p.starts_with(what)))
}

/// Whether a file with this extension sits directly in `at`, and not deeper.
///
/// **Directly matters.** Every car holds `.dds` inside its liveries; only a
/// livery holds them at its own top level, and reading the two the same way is
/// how a car gets installed as a skin.
fn beside(paths: &[String], at: &str, ext: &str) -> bool {
    let dot = format!(".{ext}");
    paths.iter().any(|p| {
        p.strip_prefix(at)
            .is_some_and(|rest| !rest.contains('/') && rest.ends_with(&dot))
    })
}

/// Additions packed the way most archives are: `content/cars/<id>/…`.
///
/// The `content` folder is looked for anywhere in the tree, because plenty of
/// archives wrap it in `assettocorsa/` or the name of the mod.
fn under_content(paths: &[String], from: &Source) -> Vec<Addition> {
    let mut found: Vec<Addition> = Vec::new();
    for (prefix, kind) in content_roots(paths) {
        for id in children(paths, &prefix) {
            let at = format!("{prefix}{id}/");
            if let Some(seen) = kind_at(paths, &at)
                && seen == kind
            {
                push(&mut found, made(kind, &id, None, from, &at));
                continue;
            }
            // A livery archive names its car by where it put the folder:
            // `content/cars/<car>/skins/<livery>/`. This is the only shape
            // that answers the question a bare livery folder cannot.
            if kind == Kind::Car {
                let skins = format!("{at}skins/");
                for livery in children(paths, &skins) {
                    let at = format!("{skins}{livery}/");
                    push(
                        &mut found,
                        made(Kind::Skin, &livery, Some(id.clone()), from, &at),
                    );
                }
            }
        }
    }
    found
}

/// Every `…content/<cars|tracks|weather>/` prefix the paths contain.
fn content_roots(paths: &[String]) -> Vec<(String, Kind)> {
    let mut roots: Vec<(String, Kind)> = Vec::new();
    for path in paths {
        for (folder, kind) in [
            ("cars", Kind::Car),
            ("tracks", Kind::Track),
            ("weather", Kind::Weather),
        ] {
            let mark = format!("content/{folder}/");
            if let Some(at) = path.find(&mark) {
                let prefix = format!("{}{mark}", &path[..at]);
                if !roots.iter().any(|(seen, _)| *seen == prefix) {
                    roots.push((prefix, kind));
                }
            }
        }
    }
    roots
}

/// A folder of several cars, or several circuits, which is how packs arrive.
fn bag(paths: &[String], from: &Source) -> Vec<Addition> {
    let mut found = Vec::new();
    for id in children(paths, "") {
        let at = format!("{id}/");
        if let Some(kind) = kind_at(paths, &at) {
            // A bag of liveries is a car's `skins` folder handed over whole,
            // and it still does not say which car. It is offered, and the
            // caller asks.
            push(&mut found, made(kind, &id, None, from, &at));
        }
    }
    found
}

/// The distinct folder names directly under a prefix.
fn children(paths: &[String], prefix: &str) -> Vec<String> {
    let mut names: Vec<String> = Vec::new();
    for path in paths {
        let Some(rest) = path.strip_prefix(prefix) else {
            continue;
        };
        let Some((head, _)) = rest.split_once('/') else {
            continue;
        };
        if !head.is_empty() && !names.iter().any(|seen| seen == head) {
            names.push(head.to_string());
        }
    }
    names.sort();
    names
}

/// One addition, rooted at `at` inside whatever the drop was.
fn made(kind: Kind, id: &str, car: Option<String>, from: &Source, at: &str) -> Addition {
    let from = match from {
        Source::Folder(root) => Source::Folder(root.join(at.trim_end_matches('/'))),
        Source::InArchive { archive, under } => Source::InArchive {
            archive: archive.clone(),
            under: format!("{under}{at}"),
        },
    };
    Addition {
        kind,
        id: id.to_string(),
        name: None,
        car,
        from,
    }
}

/// Add unless the same thing is already in the list.
fn push(found: &mut Vec<Addition>, one: Addition) {
    if !found
        .iter()
        .any(|seen| seen.kind == one.kind && seen.id == one.id && seen.car == one.car)
    {
        found.push(one);
    }
}

// ------------------------------------------------------------------- the name

/// What the addition calls itself, read from its own description file.
///
/// The same file the game reads, through the same lenient parser — AC's own
/// cars are not valid JSON and neither are most mods.
fn describes(one: &Addition) -> Option<String> {
    let said = fetch(&one.from, one.kind.describes_itself_in())?;
    let text = String::from_utf8_lossy(&said);
    if one.kind == Kind::Weather {
        return ini_name(&text);
    }
    let json = super::content::read_json_text(&text)?;
    json[one.kind.calls_itself()]
        .as_str()
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .map(str::to_string)
}

/// `NAME=` out of a weather preset's ini.
fn ini_name(text: &str) -> Option<String> {
    text.lines()
        .filter_map(|line| line.split_once('='))
        .find(|(key, _)| key.trim().eq_ignore_ascii_case("name"))
        .map(|(_, value)| value.trim().to_string())
        .filter(|name| !name.is_empty())
}

/// One small file out of whatever the addition is in.
///
/// **Both sources, one caller.** A zip and a folder must answer the same
/// question the same way, or an archive would show its folder name and the
/// unpacked copy its real one.
fn fetch(from: &Source, relative: &str) -> Option<Vec<u8>> {
    match from {
        Source::Folder(root) => {
            let at = relative
                .split('/')
                .fold(root.clone(), |at, part| at.join(part));
            if fs::metadata(&at).ok()?.len() > SMALL {
                return None;
            }
            fs::read(at).ok()
        }
        Source::InArchive { archive, under } => {
            let file = fs::File::open(archive).ok()?;
            let mut zip = zip::ZipArchive::new(std::io::BufReader::new(file)).ok()?;
            let wanted = format!("{under}{relative}");
            let at = zip
                .file_names()
                .position(|name| name.replace('\\', "/").to_lowercase() == wanted)?;
            let mut entry = zip.by_index(at).ok()?;
            if entry.size() > SMALL {
                return None;
            }
            let mut said = Vec::new();
            entry.read_to_end(&mut said).ok()?;
            Some(said)
        }
    }
}

// ------------------------------------------------------------------ the paths

/// Reduce a folder to the relative paths inside it, lowercased.
fn walk(root: &Path) -> Vec<String> {
    walkdir::WalkDir::new(root)
        .max_depth(DEEP)
        .into_iter()
        .filter_map(Result::ok)
        .take(MOST)
        .filter_map(|entry| {
            let rest = entry.path().strip_prefix(root).ok()?;
            let mut said = rest
                .components()
                .map(|part| part.as_os_str().to_string_lossy().to_lowercase())
                .collect::<Vec<_>>()
                .join("/");
            if said.is_empty() {
                return None;
            }
            if entry.file_type().is_dir() {
                said.push('/');
            }
            Some(said)
        })
        .collect()
}

/// The same, from a zip's index. Nothing is extracted to answer this.
fn peek(archive: &Path) -> Option<Vec<String>> {
    let file = fs::File::open(archive).ok()?;
    let zip = zip::ZipArchive::new(std::io::BufReader::new(file)).ok()?;
    Some(
        zip.file_names()
            .take(MOST)
            .map(|name| name.replace('\\', "/").to_lowercase())
            .collect(),
    )
}

/// Whether a path is worth opening as an archive.
fn is_zip(path: &Path) -> bool {
    path.extension()
        .is_some_and(|e| e.to_string_lossy().eq_ignore_ascii_case("zip"))
}

/// The last part of a path, without its extension.
fn name_of(path: &Path) -> String {
    path.file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default()
}

// ---------------------------------------------------------------- the copying

/// Copy a tree, making folders as it goes.
fn copy_tree(from: &Path, to: &Path) -> std::io::Result<()> {
    fs::create_dir_all(to)?;
    for entry in walkdir::WalkDir::new(from)
        .into_iter()
        .filter_map(Result::ok)
    {
        let Ok(rest) = entry.path().strip_prefix(from) else {
            continue;
        };
        let at = to.join(rest);
        if entry.file_type().is_dir() {
            fs::create_dir_all(&at)?;
        } else if entry.file_type().is_file() {
            if let Some(above) = at.parent() {
                fs::create_dir_all(above)?;
            }
            fs::copy(entry.path(), &at)?;
        }
    }
    Ok(())
}

/// Extract one subtree of an archive.
///
/// **Entry names are not trusted.** A zip may name a path `../../` and unpack
/// itself over the game's executable; `enclosed_name` is what refuses that,
/// and an entry it refuses is skipped rather than guessed at.
fn unpack(archive: &Path, under: &str, to: &Path) -> Result<(), Trouble> {
    let file = fs::File::open(archive)
        .map_err(|e| Trouble::Disk(format!("{}: {e}", archive.display())))?;
    let mut zip = zip::ZipArchive::new(std::io::BufReader::new(file))
        .map_err(|e| Trouble::Disk(format!("{}: {e}", archive.display())))?;
    fs::create_dir_all(to).map_err(|e| Trouble::Disk(format!("{}: {e}", to.display())))?;

    let depth = under.split('/').filter(|part| !part.is_empty()).count();
    for at in 0..zip.len() {
        let mut entry = zip.by_index(at).map_err(|e| Trouble::Disk(e.to_string()))?;
        let Some(safe) = entry.enclosed_name() else {
            continue;
        };
        if !safe
            .to_string_lossy()
            .replace('\\', "/")
            .to_lowercase()
            .starts_with(under)
        {
            continue;
        }
        // **The prefix is matched in lowercase and the name is written as it
        // was.** Lowercasing what is written would rename every texture, and a
        // model that asks for `Body_D.dds` would then find nothing.
        let out = safe
            .components()
            .skip(depth)
            .fold(to.to_path_buf(), |at, part| at.join(part));
        if out == to {
            continue;
        }
        if entry.is_dir() {
            fs::create_dir_all(&out)
                .map_err(|e| Trouble::Disk(format!("{}: {e}", out.display())))?;
            continue;
        }
        if let Some(above) = out.parent() {
            fs::create_dir_all(above)
                .map_err(|e| Trouble::Disk(format!("{}: {e}", above.display())))?;
        }
        let mut wrote =
            fs::File::create(&out).map_err(|e| Trouble::Disk(format!("{}: {e}", out.display())))?;
        std::io::copy(&mut entry, &mut wrote)
            .map_err(|e| Trouble::Disk(format!("{}: {e}", out.display())))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("acpe-adding-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("a scratch folder");
        dir
    }

    /// Write a file and every folder above it.
    fn file(at: &Path, relative: &str, said: &str) {
        let full = relative
            .split('/')
            .fold(at.to_path_buf(), |p, part| p.join(part));
        if let Some(above) = full.parent() {
            fs::create_dir_all(above).expect("a fixture");
        }
        fs::write(full, said).expect("a fixture");
    }

    /// A zip whose entries are exactly these names.
    fn archive(at: &Path, named: &str, entries: &[(&str, &str)]) -> PathBuf {
        let path = at.join(named);
        let file = fs::File::create(&path).expect("a fixture");
        let mut zip = zip::ZipWriter::new(file);
        let plain: zip::write::FileOptions<'_, ()> =
            zip::write::FileOptions::default().compression_method(zip::CompressionMethod::Stored);
        for (name, said) in entries {
            zip.start_file(*name, plain).expect("a fixture");
            zip.write_all(said.as_bytes()).expect("a fixture");
        }
        zip.finish().expect("a fixture");
        path
    }

    /// **The plainest drop there is, and the one everything else is judged
    /// against.** A folder with `ui/ui_car.json` in it is a car, and it goes
    /// under `content/cars` with the name it already had.
    #[test]
    fn a_car_folder_is_a_car() {
        let dir = scratch("car");
        let car = dir.join("ks_ferrari_488");
        file(&car, "ui/ui_car.json", r#"{"name":"Ferrari 488 GT3"}"#);
        file(&car, "data.acd", "");

        let found = whats_in(&car);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].kind, Kind::Car);
        assert_eq!(found[0].id, "ks_ferrari_488");
        assert_eq!(found[0].name.as_deref(), Some("Ferrari 488 GT3"));
        assert_eq!(
            found[0].where_it_goes(Path::new("/game"), None),
            Some(PathBuf::from("/game/content/cars/ks_ferrari_488"))
        );
    }

    /// **A circuit with several layouts keeps its description a folder
    /// deeper**, and half of the ones that ship with the game do. Reading only
    /// `ui/ui_track.json` would refuse every one of them.
    #[test]
    fn a_circuit_with_layouts_is_still_a_circuit() {
        let dir = scratch("track");
        let track = dir.join("ks_nordschleife");
        file(
            &track,
            "ui/endurance/ui_track.json",
            r#"{"name":"Nordschleife"}"#,
        );
        file(&track, "ui/tourist/ui_track.json", r#"{"name":"Tourist"}"#);

        let found = whats_in(&track);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].kind, Kind::Track);
        assert_eq!(
            found[0].where_it_goes(Path::new("/game"), None),
            Some(PathBuf::from("/game/content/tracks/ks_nordschleife"))
        );
    }

    /// **A car holds liveries, and a livery holds textures.** Asking "are
    /// there textures in here" before "is this a car" finds one inside every
    /// car mod there is, and installs a three hundred megabyte car as a skin.
    #[test]
    fn a_car_full_of_liveries_is_not_a_livery() {
        let dir = scratch("not-a-skin");
        let car = dir.join("some_car");
        file(&car, "ui/ui_car.json", r#"{"name":"Some Car"}"#);
        file(&car, "skins/00_red/ui_skin.json", r#"{"skinname":"Red"}"#);
        file(&car, "skins/00_red/body.dds", "");

        let found = whats_in(&car);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].kind, Kind::Car);
    }

    /// A livery on its own is recognised, and cannot say which car it is for.
    /// **That has to be asked rather than guessed**: a livery in the wrong
    /// car's folder is a car that will not load.
    #[test]
    fn a_bare_livery_names_no_car() {
        let dir = scratch("bare-skin");
        let skin = dir.join("my_team");
        file(&skin, "ui_skin.json", r#"{"skinname":"My Team"}"#);
        file(&skin, "body_d.dds", "");

        let found = whats_in(&skin);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].kind, Kind::Skin);
        assert_eq!(found[0].car, None);
        assert_eq!(found[0].where_it_goes(Path::new("/game"), None), None);
        assert_eq!(
            found[0].where_it_goes(Path::new("/game"), Some("ks_ferrari_488")),
            Some(PathBuf::from(
                "/game/content/cars/ks_ferrari_488/skins/my_team"
            ))
        );
    }

    /// **The one shape that answers the question a bare livery cannot.** A
    /// skin packed as `content/cars/<car>/skins/<livery>` says which car it is
    /// for, and nothing has to be asked.
    #[test]
    fn a_livery_packed_under_its_car_names_it() {
        let dir = scratch("packed-skin");
        let drop = dir.join("cool_livery_pack");
        file(
            &drop,
            "content/cars/ks_porsche_911/skins/rothmans/ui_skin.json",
            r#"{"skinname":"Rothmans"}"#,
        );
        file(
            &drop,
            "content/cars/ks_porsche_911/skins/rothmans/body.dds",
            "",
        );

        let found = whats_in(&drop);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].kind, Kind::Skin);
        assert_eq!(found[0].car.as_deref(), Some("ks_porsche_911"));
        assert_eq!(found[0].name.as_deref(), Some("Rothmans"));
        assert_eq!(
            found[0].where_it_goes(Path::new("/game"), None),
            Some(PathBuf::from(
                "/game/content/cars/ks_porsche_911/skins/rothmans"
            ))
        );
    }

    /// A pack of several cars is several additions, not one unrecognised drop.
    #[test]
    fn a_bag_of_cars_is_every_car_in_it() {
        let dir = scratch("bag");
        let pack = dir.join("urd_pack");
        file(&pack, "one_car/ui/ui_car.json", r#"{"name":"One"}"#);
        file(&pack, "two_car/ui/ui_car.json", r#"{"name":"Two"}"#);
        file(&pack, "readme.txt", "thanks for downloading");

        let mut found = whats_in(&pack);
        found.sort_by(|a, b| a.id.cmp(&b.id));
        assert_eq!(found.len(), 2);
        assert_eq!(found[0].id, "one_car");
        assert_eq!(found[1].id, "two_car");
        assert!(found.iter().all(|one| one.kind == Kind::Car));
    }

    /// **A zip and the folder inside it must be read the same way.** Somebody
    /// who unpacks the archive first and somebody who does not are holding the
    /// same mod, and a launcher that names them differently is one of them
    /// being told something untrue.
    #[test]
    fn an_archive_reads_as_what_is_inside_it() {
        let dir = scratch("zip");
        let zipped = archive(
            &dir,
            "some_mod.zip",
            &[
                (
                    "content/cars/ks_mazda_mx5/ui/ui_car.json",
                    r#"{"name":"Mazda MX-5"}"#,
                ),
                ("content/cars/ks_mazda_mx5/data.acd", "x"),
            ],
        );

        let found = whats_in(&zipped);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].kind, Kind::Car);
        assert_eq!(found[0].id, "ks_mazda_mx5");
        assert_eq!(found[0].name.as_deref(), Some("Mazda MX-5"));
    }

    /// **The game's own files are not valid JSON**, and neither are most
    /// mods': a description runs over several lines with the breaks written
    /// raw. Ninety-two of ninety-six installed cars failed a strict parse. The
    /// name has to survive that here too, and through the same parser.
    #[test]
    fn a_name_survives_a_description_that_is_not_valid_json() {
        let dir = scratch("lenient");
        let car = dir.join("broken_mod");
        file(
            &car,
            "ui/ui_car.json",
            "{\"name\":\"Lancia Delta\",\"description\":\"a car\nwith a tall\ndescription\"}",
        );

        let found = whats_in(&car);
        assert_eq!(found[0].name.as_deref(), Some("Lancia Delta"));
    }

    /// Nothing is guessed. A folder of photographs is not a car.
    #[test]
    fn something_that_is_not_a_mod_is_not_offered() {
        let dir = scratch("nonsense");
        let drop = dir.join("screenshots");
        file(&drop, "one.png", "");
        file(&drop, "two.png", "");

        assert!(whats_in(&drop).is_empty());
    }

    /// **Installing over something is a decision somebody has to make.** A car
    /// that has been tuned, replaced without a word, is the difference between
    /// a tool and an accident.
    #[test]
    fn it_refuses_to_replace_unless_replacing_was_asked_for() {
        let dir = scratch("clash");
        let car = dir.join("ks_car");
        file(&car, "ui/ui_car.json", r#"{"name":"A Car"}"#);
        let root = dir.join("game");
        file(
            &root,
            "content/cars/ks_car/ui/ui_car.json",
            r#"{"name":"The old one"}"#,
        );

        let found = whats_in(&car);
        assert!(found[0].replaces(&root, None).is_some());
        let refused = place(&root, &found[0], None, false);
        assert!(matches!(refused, Err(Trouble::AlreadyThere(_))));

        let landed = place(&root, &found[0], None, true).expect("replacing was asked for");
        let said = fs::read_to_string(landed.join("ui/ui_car.json")).expect("read back");
        assert!(
            said.contains("A Car"),
            "the new one should be there, got {said}"
        );
    }

    /// A livery with no car named cannot be placed, and says so rather than
    /// landing somewhere plausible.
    #[test]
    fn a_livery_with_no_car_refuses_by_name() {
        let dir = scratch("no-car");
        let skin = dir.join("my_team");
        file(&skin, "ui_skin.json", r#"{"skinname":"My Team"}"#);

        let found = whats_in(&skin);
        assert!(matches!(
            place(&dir.join("game"), &found[0], None, false),
            Err(Trouble::WhichCar)
        ));
    }

    /// **An archive is extracted with its names as they were written.** AC's
    /// models ask for `Body_D.dds` by name, and a texture lowercased on the
    /// way out of the zip is a texture the car will not find on a filesystem
    /// that cares.
    #[test]
    fn unpacking_keeps_the_names_it_was_given() {
        let dir = scratch("case");
        let zipped = archive(
            &dir,
            "cased.zip",
            &[
                ("content/cars/My_Car/ui/ui_car.json", r#"{"name":"My Car"}"#),
                ("content/cars/My_Car/Body_D.dds", "texture"),
            ],
        );
        let root = dir.join("game");

        let found = whats_in(&zipped);
        assert_eq!(found.len(), 1);
        let landed = place(&root, &found[0], None, false).expect("place it");
        assert!(
            landed.join("Body_D.dds").exists(),
            "the texture should keep its name, {:?}",
            fs::read_dir(&landed)
                .map(|d| d.flatten().map(|e| e.file_name()).collect::<Vec<_>>())
                .unwrap_or_default()
        );
    }

    /// **A zip may name an entry `../../` and unpack itself over the game.**
    /// The refusal is the archive reader's, and what matters here is that a
    /// refused entry is skipped rather than guessed at.
    #[test]
    fn an_archive_cannot_write_outside_where_it_was_told() {
        let dir = scratch("slip");
        let zipped = archive(
            &dir,
            "nasty.zip",
            &[
                ("content/cars/ok_car/ui/ui_car.json", r#"{"name":"Fine"}"#),
                (
                    "content/cars/ok_car/../../../escaped.txt",
                    "should not land",
                ),
            ],
        );
        let root = dir.join("game");

        let found = whats_in(&zipped);
        let _ = place(&root, &found[0], None, false);
        assert!(!dir.join("escaped.txt").exists());
        assert!(!root.join("escaped.txt").exists());
    }

    /// **The two directions of the same knowledge have to agree.**
    ///
    /// `content.rs` reads an installed car out of the game; this reads one on
    /// its way in. They are the same rule written twice, which is the thing
    /// this whole family of repositories is arranged to prevent — so where
    /// there is a real install, every car and every circuit the scan lists
    /// must be recognised by the recogniser, and as the right kind.
    ///
    /// Run against the ninety-six cars and twenty-one circuits on the machine
    /// this was written on it also settles a thing that looks like a fault and
    /// is not: eighty-four more car folders and three more circuit folders
    /// hold nothing but `ui/dlc_ui_car.json` and a badge. They are unowned DLC
    /// and there is no car in them. Both sides refuse them, which is right.
    ///
    /// **Skipped where there is no game**, because the alternative is a test
    /// nobody can run.
    #[test]
    fn what_is_installed_is_what_the_recogniser_sees() {
        let Some(root) = crate::games::assetto_corsa::paths::ac_install_root(None) else {
            return;
        };

        for car in crate::games::assetto_corsa::content::scan_cars(&root) {
            let at = root.join("content").join("cars").join(&car.id);
            let found = whats_in(&at);
            assert_eq!(
                found.first().map(|one| one.kind),
                Some(Kind::Car),
                "the scan lists {} as a car and the recogniser did not",
                car.id
            );
        }

        for track in crate::games::assetto_corsa::content::scan_tracks(&root) {
            let at = root.join("content").join("tracks").join(&track.id);
            let found = whats_in(&at);
            assert_eq!(
                found.first().map(|one| one.kind),
                Some(Kind::Track),
                "the scan lists {} as a circuit and the recogniser did not",
                track.id
            );
        }
    }

    /// A weather preset is a folder with `weather.ini`, and it says its own
    /// name in an ini rather than a json.
    #[test]
    fn a_weather_preset_is_recognised_and_names_itself() {
        let dir = scratch("weather");
        let preset = dir.join("sol_clear");
        file(&preset, "weather.ini", "[LAUNCHER]\nNAME=Sol Clear\n");

        let found = whats_in(&preset);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].kind, Kind::Weather);
        assert_eq!(found[0].name.as_deref(), Some("Sol Clear"));
        assert_eq!(
            found[0].where_it_goes(Path::new("/game"), None),
            Some(PathBuf::from("/game/content/weather/sol_clear"))
        );
    }
}
