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
///
/// **Eight, not five.** Archives from download sites wrap the mod in its own
/// name, often twice — `Mod v0.99/Mod/content/cars/<car>/ui/ui_car.json` is
/// seven levels — and at five such a car was "nothing recognisable".
///
/// **And twenty-four, not eight.** A pack unpacked by hand, then zipped
/// again with the folder it was unpacked into, then put in a "Mods" folder
/// of somebody's own, is four more wrappers than any author meant; eight was
/// enough for every archive seen and not for every drop. Twenty-four is
/// deeper than anybody nests folders on purpose, and still a bound: the
/// walk never follows links, and [`MOST`] caps what it reads.
const DEEP: usize = 24;

/// The most paths any one recognition will consider, so a pathological archive
/// cannot be read into memory by pointing at it.
///
/// **Two hundred thousand**: a car is a few hundred files, a circuit with
/// its liveries of grass a few thousand, and a folder of a hundred cars
/// dropped at once is past forty thousand — which was the old bound, and
/// the cars after it were not there. Two hundred thousand paths are a few
/// tens of megabytes, once, while a drop is read.
const MOST: usize = 200_000;

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
    /// Custom Shaders Patch, which goes into the game's own folder.
    ///
    /// **The one kind that is not content.** Everything else here is a folder
    /// under `content`; this is a loader beside `acs.exe` and a folder called
    /// `extension` — see [`super::csp`], which owns what it is and how it
    /// comes out again.
    Csp,
}

impl Kind {
    /// What to call it in a sentence.
    pub fn what(self) -> &'static str {
        match self {
            Self::Car => "car",
            Self::Track => "track",
            Self::Skin => "livery",
            Self::Weather => "weather",
            Self::Csp => "custom shaders patch",
        }
    }

    /// Whether this kind is put into the game's folder rather than under
    /// `content`, and merged with what is there rather than replacing it.
    ///
    /// **This is a guard, not a category.** The destination for everything
    /// else is a folder of its own, and replacing one means deleting it first.
    /// The patch's destination is the game itself; deleting that first would
    /// delete the game.
    fn merges_into_the_game(self) -> bool {
        matches!(self, Self::Csp)
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
            // Never reached: the patch does not live under `content`, and
            // `where_it_goes` answers for it before this is asked.
            Self::Csp => "",
        }
    }

    /// The file this kind describes itself in, relative to its own folder.
    fn describes_itself_in(self) -> &'static str {
        match self {
            Self::Car => "ui/ui_car.json",
            Self::Track => "ui/ui_track.json",
            Self::Skin => "ui_skin.json",
            Self::Weather => "weather.ini",
            Self::Csp => "extension/config/data_manifest.ini",
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
        if self.kind.merges_into_the_game() {
            return Some(root.to_path_buf());
        }
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
        // **The patch is never "already there" as a folder.** Its destination
        // is the game, which always exists; what it might be replacing is a
        // patch, and that is [`super::csp::look`]'s question rather than a
        // path check.
        if self.kind.merges_into_the_game() {
            return super::csp::look(root)
                .is_anywhere()
                .then(|| root.join(super::csp::FOLDER));
        }
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
    whats_in_at(dropped, 0)
}

/// How many archives inside archives are opened, one inside the next.
///
/// **Archives arrive inside archives.** A download from a mod site is as
/// often a zip holding a readme, a livery template and the car's own `.rar`
/// as it is the car — and that inner archive was left unopened, so a car
/// that was there was "nothing found". Three deep is every real case seen,
/// and six is the bound — what keeps an archive that contains itself from
/// going on, with room for a pack re-zipped by whoever passed it along.
const NESTED: usize = 6;

/// The most archives opened inside any one archive or folder.
///
/// **A pack is a folder of a hundred zips**, one per car, and at
/// twenty-four the twenty-fifth car onwards was simply not there. Each one
/// is opened only to be read, into a temporary folder, so the bound is
/// about time, not memory.
const MOST_NESTED: usize = 500;

/// Whether a path names an archive of any kind this reads.
fn is_any_archive(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    [".zip", ".7z", ".rar"]
        .iter()
        .any(|ext| lower.ends_with(ext))
}

fn whats_in_at(dropped: &Path, depth: usize) -> Vec<Addition> {
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
    } else if is_other_archive(dropped) {
        // **7z and rar through the tools that read them**, unpacked to a
        // folder of their own and read as a folder drop. Most mods on the
        // sites people download from are 7z; the readers for both formats
        // that a program could carry inside itself are either copyleft or
        // under licences a closed program cannot take, and every machine
        // that has these files has one of the tools.
        return match unpack_elsewhere(dropped) {
            Ok(folder) => whats_in_at(&folder, depth),
            Err(_) => Vec::new(),
        };
    } else {
        return Vec::new();
    };

    let mut found = read(&paths, &from, &name_of(dropped));
    for one in &mut found {
        one.name = describes(one);
    }
    // **Every archive inside, at any level, opened and read the same way**
    // — a pack of cars as a folder of zips, a car as a zip inside a zip.
    if depth < NESTED {
        let inner: Vec<&String> = paths
            .iter()
            .filter(|p| !p.ends_with('/') && is_any_archive(p))
            .take(MOST_NESTED)
            .collect();
        for rel in inner {
            let opened = match &from {
                Source::Folder(root) => {
                    Some(rel.split('/').fold(root.clone(), |at, part| at.join(part)))
                }
                Source::InArchive { archive, .. } => extract_one(archive, rel),
            };
            if let Some(opened) = opened {
                for one in whats_in_at(&opened, depth + 1) {
                    push(&mut found, one);
                }
            }
        }
    }
    found
}

/// One file out of a zip, into a folder of its own under the system's
/// temporary folder, for an archive inside an archive to be opened.
fn extract_one(archive: &Path, wanted: &str) -> Option<PathBuf> {
    let file = fs::File::open(archive).ok()?;
    let mut zip = zip::ZipArchive::new(std::io::BufReader::new(file)).ok()?;
    let at = zip.file_names().position(|name| {
        name.is_ok_and(|name| name.replace('\\', "/").to_lowercase() == wanted.to_lowercase())
    })?;
    let mut entry = zip.by_index(at).ok()?;
    let size = fs::metadata(archive).map(|m| m.len()).unwrap_or_default();
    let base = Path::new(wanted)
        .file_name()?
        .to_string_lossy()
        .into_owned();
    let into = std::env::temp_dir()
        .join("pro-unpacked")
        .join(format!("{}-{size}-{at}", name_of(archive)));
    fs::create_dir_all(&into).ok()?;
    let to = into.join(base);
    if !to.is_file() {
        let mut out = fs::File::create(&to).ok()?;
        std::io::copy(&mut entry, &mut out).ok()?;
    }
    Some(to)
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

    // **The one destination that is never cleared first.** Everything else
    // lands in a folder of its own, and replacing one means deleting it. The
    // patch lands in the game's own folder — deleting that first would delete
    // the game, with the cars and the circuits in it.
    if addition.kind.merges_into_the_game() {
        if !replacing && let Some(already) = addition.replaces(root, car) {
            return Err(Trouble::AlreadyThere(already));
        }
    } else if to.exists() {
        if !replacing {
            return Err(Trouble::AlreadyThere(to));
        }
        // **Set aside, not deleted**, so the install can be undone: the one
        // copy before this, under `content-replaced`. An older copy there is
        // the install before last, and goes.
        match mirrored(root, REPLACED, &to) {
            Some(aside) => {
                if aside.exists() {
                    fs::remove_dir_all(&aside)
                        .map_err(|e| Trouble::Disk(format!("{}: {e}", aside.display())))?;
                }
                move_tree(&to, &aside).map_err(Trouble::Disk)?;
            }
            None => fs::remove_dir_all(&to)
                .map_err(|e| Trouble::Disk(format!("{}: {e}", to.display())))?,
        }
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

    if here("ui/ui_car.json") || here("data.acd") || here("data/car.ini") {
        return Some(Kind::Car);
    }
    if here("ui/ui_track.json")
        || here("ai/fast_lane.ai")
        || here("data/surfaces.ini")
        || deeper_ui_track(paths, at)
    {
        return Some(Kind::Track);
    }
    // **After the circuit's signs, not before.** A model beside `skins/` is
    // what a car without its ui looks like, and also what Kunos's Black Cat
    // County is: three layouts, a `skins/` of billboards and its `.kn5`s at
    // the top. Asked first, this installed a circuit into the cars.
    if (here("sfx/") || here("skins/")) && beside(paths, at, "kn5") {
        return Some(Kind::Car);
    }
    // **Before the livery test, and before weather.** The patch carries
    // hundreds of `.dds` at various depths and a `weather.ini` of its own
    // among its configuration; asked in the other order it would be installed
    // as a livery, into a car, at the wrong end of the game.
    if here(super::csp::LOADER) && here(&format!("{}/", super::csp::FOLDER)) {
        return Some(Kind::Csp);
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
    let under = format!("{at}ui/").to_ascii_lowercase();
    paths.iter().any(|p| {
        let p = p.to_ascii_lowercase();
        p.starts_with(&under) && p.ends_with("/ui_track.json")
    })
}

/// Whether an exact relative path is in the list.
///
/// **Whatever the case.** A mod packed on Windows writes `UI/ui_car.json` or
/// `Data.acd` as often as not, and the game, reading through Windows's file
/// system or Wine's, finds it either way; so does this.
fn has(paths: &[String], what: &str) -> bool {
    let what = what.to_ascii_lowercase();
    let folder = format!("{what}/");
    paths.iter().any(|p| {
        let p = p.to_ascii_lowercase();
        p == what || p == folder || (what.ends_with('/') && p.starts_with(&what))
    })
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
            .is_some_and(|rest| !rest.contains('/') && rest.to_ascii_lowercase().ends_with(&dot))
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
            // Found in the lowercase, cut from the path as written: an ASCII
            // lowercase keeps every byte where it was.
            if let Some(at) = path.to_ascii_lowercase().find(&mark) {
                let prefix = path.get(..at + mark.len()).unwrap_or_default().to_string();
                if !roots.iter().any(|(seen, _)| *seen == prefix) {
                    roots.push((prefix, kind));
                }
            }
        }
    }
    roots
}

/// A folder of several cars, or several circuits, which is how packs arrive.
///
/// **Looked for below the top as well**, as far down as the walk goes: a
/// download is as often `Mod v0.99/Mod/<car>/` as `<car>/`, and the folders
/// that wrap it are names, not content. A folder found to be something is
/// not looked inside — a car's liveries are the car's.
fn bag(paths: &[String], from: &Source) -> Vec<Addition> {
    let mut found = Vec::new();
    let mut look: Vec<(String, usize)> = vec![(String::new(), 0)];
    while let Some((under, depth)) = look.pop() {
        for id in children(paths, &under) {
            let at = format!("{under}{id}/");
            match kind_at(paths, &at) {
                // A bag of liveries is a car's `skins` folder handed over
                // whole, and it still does not say which car. It is offered,
                // and the caller asks.
                Some(kind) => push(&mut found, made(kind, &id, None, from, &at)),
                None if depth < DEEP => look.push((at, depth + 1)),
                None => {}
            }
        }
    }
    found.sort_by(|a, b| a.id.cmp(&b.id));
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
    // **The patch states its version, not a name**, and the version is what
    // anybody installing one wants to see. Read by the module that owns the
    // patch, so an archive and an installed copy are read the same way.
    if one.kind == Kind::Csp {
        return super::csp::stated_in(&text);
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
            // Each part found whatever its case, as the game would.
            let at = relative.split('/').fold(root.clone(), |at, part| {
                fs::read_dir(&at)
                    .ok()
                    .and_then(|entries| {
                        entries.flatten().find_map(|e| {
                            e.file_name()
                                .to_str()
                                .is_some_and(|n| n.eq_ignore_ascii_case(part))
                                .then(|| e.path())
                        })
                    })
                    .unwrap_or_else(|| at.join(part))
            });
            if fs::metadata(&at).ok()?.len() > SMALL {
                return None;
            }
            fs::read(at).ok()
        }
        Source::InArchive { archive, under } => {
            let file = fs::File::open(archive).ok()?;
            let mut zip = zip::ZipArchive::new(std::io::BufReader::new(file)).ok()?;
            let wanted = format!("{under}{relative}");
            let at = zip.file_names().position(|name| {
                name.is_ok_and(|name| name.replace('\\', "/").to_lowercase() == wanted)
            })?;
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
    // **The same order on every system**: a folder's files before its
    // folders, each by name. The order the disk hands them over is ext4's
    // on Linux and alphabetical on NTFS, so a pack installed — and a test
    // that read it — in one order on one machine and another on the next.
    walkdir::WalkDir::new(root)
        .max_depth(DEEP)
        .sort_by(|a, b| {
            (a.file_type().is_dir(), a.file_name()).cmp(&(b.file_type().is_dir(), b.file_name()))
        })
        .into_iter()
        .filter_map(Result::ok)
        .take(MOST)
        .filter_map(|entry| {
            let rest = entry.path().strip_prefix(root).ok()?;
            // **As written, not lowercased**: these are joined back onto
            // the folder to copy from, and on Linux `MTRacing_Giulia` and
            // `mtracing_giulia` are two different folders. Every comparison
            // below ignores case on its own.
            let mut said = rest
                .components()
                .map(|part| part.as_os_str().to_string_lossy().into_owned())
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
            // A name the archive cannot decode is a name nothing here asked
            // for, and is left out rather than failing the whole listing.
            .filter_map(Result::ok)
            .map(|name| name.replace('\\', "/").to_lowercase())
            .collect(),
    )
}

/// Whether a path is worth opening as an archive.
fn is_zip(path: &Path) -> bool {
    path.extension()
        .is_some_and(|e| e.to_string_lossy().eq_ignore_ascii_case("zip"))
}

/// A 7z or rar archive — read through a tool on the machine.
pub fn is_other_archive(path: &Path) -> bool {
    path.extension().is_some_and(|e| {
        let e = e.to_string_lossy().to_ascii_lowercase();
        e == "7z" || e == "rar"
    })
}

/// The tools tried for a 7z or rar archive, in order, each as the program
/// and its arguments with `{archive}` and `{into}` filled in. 7-Zip first
/// (`7z`, `7zz`, its usual place on Windows), then libarchive's `bsdtar`
/// (most Linux machines, and Windows' own `tar`), then `unrar` for rar.
fn tools() -> Vec<(String, Vec<&'static str>)> {
    let mut list: Vec<(String, Vec<&'static str>)> = vec![
        ("7z".into(), vec!["x", "-y", "-o{into}", "{archive}"]),
        ("7zz".into(), vec!["x", "-y", "-o{into}", "{archive}"]),
        ("bsdtar".into(), vec!["-xf", "{archive}", "-C", "{into}"]),
        ("unrar".into(), vec!["x", "-o+", "{archive}", "{into}/"]),
    ];
    if cfg!(target_os = "windows") {
        list.insert(
            0,
            (
                r"C:\Program Files\7-Zip\7z.exe".into(),
                vec!["x", "-y", "-o{into}", "{archive}"],
            ),
        );
        list.push(("tar".into(), vec!["-xf", "{archive}", "-C", "{into}"]));
    }
    list
}

/// Unpack a 7z or rar archive into a folder of its own under the system's
/// temporary folder, through the first tool that works. The folder is named
/// for the archive and its size, so dropping the same file twice reuses it.
pub fn unpack_elsewhere(archive: &Path) -> Result<PathBuf, String> {
    let size = fs::metadata(archive).map(|m| m.len()).unwrap_or_default();
    let into = std::env::temp_dir()
        .join("pro-unpacked")
        .join(format!("{}-{size}", name_of(archive)));
    if into.is_dir() && fs::read_dir(&into).is_ok_and(|mut d| d.next().is_some()) {
        return Ok(into);
    }
    fs::create_dir_all(&into).map_err(|e| format!("{}: {e}", into.display()))?;
    let (archive_s, into_s) = (archive.display().to_string(), into.display().to_string());
    for (program, arguments) in tools() {
        let arguments: Vec<String> = arguments
            .iter()
            .map(|a| {
                a.replace("{archive}", &archive_s)
                    .replace("{into}", &into_s)
            })
            .collect();
        let ran = std::process::Command::new(&program)
            .args(&arguments)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status();
        if ran.is_ok_and(|status| status.success())
            && fs::read_dir(&into).is_ok_and(|mut d| d.next().is_some())
        {
            return Ok(into);
        }
    }
    let _ = fs::remove_dir_all(&into);
    Err(
        "no tool here opens 7z or rar — install 7-Zip (Windows) or p7zip / libarchive (Linux)"
            .to_string(),
    )
}

// ---------------------------------------------------------------- set aside

/// Where content switched off is kept: beside `content`, in the game's own
/// folder, so turning a mod off and on is a rename on the same disk and costs
/// nothing however big the mod is. The game never looks in it.
pub const OFF: &str = "content-off";

/// Where the copy an install replaced is kept, so the install can be undone.
pub const REPLACED: &str = "content-replaced";

/// The same place under another folder: `root/content/cars/x` under `OFF`
/// is `root/content-off/cars/x`.
fn mirrored(root: &Path, under: &str, at: &Path) -> Option<PathBuf> {
    let rest = at.strip_prefix(root.join("content")).ok()?;
    (!rest.as_os_str().is_empty()).then(|| root.join(under).join(rest))
}

/// Move a folder, by renaming where it can and copying where it must.
fn move_tree(from: &Path, to: &Path) -> Result<(), String> {
    if let Some(above) = to.parent() {
        fs::create_dir_all(above).map_err(|e| format!("{}: {e}", above.display()))?;
    }
    if fs::rename(from, to).is_ok() {
        return Ok(());
    }
    copy_tree(from, to).map_err(|e| e.to_string())?;
    fs::remove_dir_all(from).map_err(|e| format!("{}: {e}", from.display()))
}

/// Switch off a car, a circuit or a livery without deleting it: its folder
/// goes from `content/…` to `content-off/…`. Returns where it went.
pub fn switch_off(root: &Path, folder: &Path) -> Result<PathBuf, String> {
    let to =
        mirrored(root, OFF, folder).ok_or("only something under content/ can be switched off")?;
    if to.exists() {
        return Err(format!("{} is already switched off", to.display()));
    }
    move_tree(folder, &to)?;
    Ok(to)
}

/// Put something switched off back where the game reads it.
pub fn switch_on(root: &Path, off: &Path) -> Result<PathBuf, String> {
    let rest = off
        .strip_prefix(root.join(OFF))
        .map_err(|_| "not something that was switched off".to_string())?;
    let to = root.join("content").join(rest);
    if to.exists() {
        return Err(format!("{} is installed again already", to.display()));
    }
    move_tree(off, &to)?;
    Ok(to)
}

/// What is switched off: the cars and circuits under `content-off`, by kind
/// folder (`cars`, `tracks`) and folder name.
pub fn switched_off(root: &Path) -> Vec<(String, String, PathBuf)> {
    let mut found = Vec::new();
    for kind in ["cars", "tracks"] {
        let Ok(entries) = fs::read_dir(root.join(OFF).join(kind)) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir()
                && let Some(name) = path.file_name().and_then(|n| n.to_str())
            {
                found.push((kind.to_string(), name.to_string(), path.clone()));
            }
        }
    }
    found.sort_by_key(|one| one.1.to_lowercase());
    found
}

/// Undo an install: take away what was placed at `placed`, and put back the
/// copy it replaced where there is one. Returns what was done, in words.
pub fn undo(root: &Path, placed: &Path) -> Result<String, String> {
    let aside =
        mirrored(root, REPLACED, placed).ok_or("only something under content/ can be undone")?;
    if placed.exists() {
        fs::remove_dir_all(placed).map_err(|e| format!("{}: {e}", placed.display()))?;
    }
    if aside.exists() {
        move_tree(&aside, placed)?;
        return Ok("the copy it replaced is back".to_string());
    }
    Ok("taken out again".to_string())
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

    /// **Off and on again, nothing lost**, and an install that replaced
    /// something undone puts the old copy back.
    #[test]
    fn a_mod_is_switched_off_and_on_and_an_install_is_undone() {
        let root = std::env::temp_dir().join(format!("ac-switch-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        let car = root.join("content/cars/gt3");
        fs::create_dir_all(car.join("data")).expect("the test's own files");
        fs::write(car.join("data/car.ini"), "old").expect("the test's own files");

        let off = switch_off(&root, &car).expect("the test's own files");
        assert!(!car.exists() && off.join("data/car.ini").is_file());
        assert_eq!(switched_off(&root).len(), 1);
        assert!(switch_off(&root, &root.join("elsewhere")).is_err());
        switch_on(&root, &off).expect("the test's own files");
        assert!(car.join("data/car.ini").is_file());
        assert!(switched_off(&root).is_empty());

        // An install over it sets the old copy aside; undoing brings it back.
        let new = root.join("dropped/gt3");
        fs::create_dir_all(new.join("data")).expect("the test's own files");
        fs::write(new.join("data/car.ini"), "new").expect("the test's own files");
        let addition = Addition {
            kind: Kind::Car,
            id: "gt3".into(),
            name: None,
            car: None,
            from: Source::Folder(new.clone()),
        };
        let placed = place(&root, &addition, None, true).expect("the test's own files");
        assert_eq!(
            fs::read_to_string(placed.join("data/car.ini")).expect("the test's own files"),
            "new"
        );
        assert!(root.join(REPLACED).join("cars/gt3/data/car.ini").is_file());
        undo(&root, &placed).expect("the test's own files");
        assert_eq!(
            fs::read_to_string(car.join("data/car.ini")).expect("the test's own files"),
            "old"
        );

        // An install of something new is simply taken out again.
        let addition = Addition {
            id: "other".into(),
            ..addition
        };
        let placed = place(&root, &addition, None, false).expect("the test's own files");
        assert_eq!(
            undo(&root, &placed).expect("the test's own files"),
            "taken out again"
        );
        assert!(!placed.exists());
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn seven_zip_and_rar_are_recognised_as_archives() {
        assert!(is_other_archive(Path::new("a/car.7z")));
        assert!(is_other_archive(Path::new("a/CAR.RAR")));
        assert!(!is_other_archive(Path::new("a/car.zip")));
        assert!(!is_other_archive(Path::new("a/car")));
    }
    use std::io::Write;

    pub(super) fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("acpe-adding-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("a scratch folder");
        dir
    }

    /// Write a file and every folder above it.
    pub(super) fn file(at: &Path, relative: &str, said: &str) {
        let full = relative
            .split('/')
            .fold(at.to_path_buf(), |p, part| p.join(part));
        if let Some(above) = full.parent() {
            fs::create_dir_all(above).expect("a fixture");
        }
        fs::write(full, said).expect("a fixture");
    }

    /// A zip whose entries are exactly these names.
    pub(super) fn archive(at: &Path, named: &str, entries: &[(&str, &str)]) -> PathBuf {
        let path = at.join(named);
        let file = fs::File::create(&path).expect("a fixture");
        let mut zip = zip::ZipWriter::new(file);
        let plain = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Stored);
        for (name, said) in entries {
            zip.start_file(*name, plain).expect("a fixture");
            zip.write_all(said.as_bytes()).expect("a fixture");
        }
        zip.finish().expect("a fixture");
        path
    }

    /// A zip of these entries, as bytes, for putting inside another.
    fn zipped(entries: &[(String, Vec<u8>)]) -> Vec<u8> {
        let mut out = std::io::Cursor::new(Vec::new());
        {
            let mut zip = zip::ZipWriter::new(&mut out);
            let plain = zip::write::SimpleFileOptions::default()
                .compression_method(zip::CompressionMethod::Stored);
            for (name, said) in entries {
                zip.start_file(name.as_str(), plain).expect("a fixture");
                zip.write_all(said).expect("a fixture");
            }
            zip.finish().expect("a fixture");
        }
        out.into_inner()
    }

    /// A car's files under a prefix, as zip entries.
    fn car_entries(under: &str, id: &str) -> Vec<(String, Vec<u8>)> {
        vec![
            (format!("{under}{id}/data.acd"), b"x".to_vec()),
            (format!("{under}{id}/ui/ui_car.json"), b"{}".to_vec()),
            (format!("{under}{id}/{id}.kn5"), b"m".to_vec()),
        ]
    }

    /// **Everything, however deep it was put**: the worst drops the owner
    /// asked to be read in full — wrappers far past the eight levels an
    /// author uses, a pack of more cars as zips than the old bound of
    /// twenty-four, an archive five times inside an archive, and a pack
    /// folder six wrappers down. Every car in each is found.
    #[test]
    fn everything_is_found_however_deep_it_was_put() {
        let dir = scratch("deepest");

        // A car under ten wrapping folders, dropped as a folder.
        let wrappers = "Mods/Downloads/2026/Unpacked/MTRacing v0.99/MTRacing/Install/\
            Copy to game/assettocorsa/content/cars/";
        file(
            &dir.join("deep"),
            &format!("{wrappers}deep_car/data.acd"),
            "x",
        );
        file(
            &dir.join("deep"),
            &format!("{wrappers}deep_car/ui/ui_car.json"),
            "{}",
        );
        let ids: Vec<String> = whats_in(&dir.join("deep"))
            .into_iter()
            .map(|a| a.id)
            .collect();
        assert_eq!(ids, vec!["deep_car".to_string()]);

        // Thirty cars, each its own zip, in one zip.
        let pack: Vec<(String, Vec<u8>)> = (0..30)
            .map(|n| {
                let id = format!("pack_car_{n:02}");
                (format!("cars/{id}.zip"), zipped(&car_entries("", &id)))
            })
            .collect();
        let pack_path = dir.join("Thirty cars.zip");
        fs::write(&pack_path, zipped(&pack)).expect("a fixture");
        let found = whats_in(&pack_path);
        assert_eq!(
            found.len(),
            30,
            "{:?}",
            found.iter().map(|a| &a.id).collect::<Vec<_>>()
        );

        // A car in a zip in a zip in a zip in a zip in a zip.
        let mut bytes = zipped(&car_entries("Car v1/", "nested_car"));
        for level in 0..5 {
            bytes = zipped(&[
                (format!("Read me {level}.txt"), b"hi".to_vec()),
                (format!("level {level}/inner.zip"), bytes),
            ]);
        }
        let nested_path = dir.join("Russian doll.zip");
        fs::write(&nested_path, bytes).expect("a fixture");
        let ids: Vec<String> = whats_in(&nested_path).into_iter().map(|a| a.id).collect();
        assert_eq!(ids, vec!["nested_car".to_string()]);

        // Two cars side by side, six folders down, with no `content/cars`.
        let mut bag = car_entries("A/B/C/D/E/F/", "bag_one");
        bag.extend(car_entries("A/B/C/D/E/F/", "bag_two"));
        let bag_path = dir.join("bag.zip");
        fs::write(&bag_path, zipped(&bag)).expect("a fixture");
        let ids: Vec<String> = whats_in(&bag_path).into_iter().map(|a| a.id).collect();
        assert_eq!(ids, vec!["bag_one".to_string(), "bag_two".to_string()]);

        let _ = fs::remove_dir_all(&dir);
    }

    /// **An archive inside an archive**, as the owner's download was: a zip
    /// holding a readme, a livery template and the car's own archive — and
    /// a pack folder holding cars as zips. Each car inside is found, named
    /// and installed.
    #[test]
    fn a_car_inside_an_archive_inside_an_archive_is_found() {
        let dir = scratch("nested");
        let inner = archive(
            &dir,
            "mt_alfa_giulia_class_1.zip",
            &[
                (
                    "mt_alfa_giulia_class_1/ui/ui_car.json",
                    r#"{"name":"Alfa Romeo Giulia Class 1"}"#,
                ),
                ("mt_alfa_giulia_class_1/data.acd", "x"),
                ("mt_alfa_giulia_class_1/giulia.kn5", "m"),
                ("mt_alfa_giulia_class_1/skins/red/preview.jpg", "p"),
            ],
        );
        let bytes = fs::read(&inner).expect("a fixture");
        let outer_path = dir.join("MTRacing_Alfa_Romeo_Giulia_Class_1_v0.99.zip");
        {
            let file = fs::File::create(&outer_path).expect("a fixture");
            let mut zip = zip::ZipWriter::new(file);
            let plain = zip::write::SimpleFileOptions::default()
                .compression_method(zip::CompressionMethod::Stored);
            zip.start_file("Read me.txt", plain).expect("a fixture");
            zip.write_all(b"Install it.").expect("a fixture");
            zip.start_file("Template/livery.psd", plain)
                .expect("a fixture");
            zip.write_all(b"psd").expect("a fixture");
            zip.start_file("mt_alfa_giulia_class_1.zip", plain)
                .expect("a fixture");
            zip.write_all(&bytes).expect("a fixture");
            zip.finish().expect("a fixture");
        }
        fs::remove_file(&inner).expect("a fixture");
        let found = whats_in(&outer_path);
        assert_eq!(found.len(), 1, "{found:?}");
        assert_eq!(found[0].kind, Kind::Car);
        assert_eq!(found[0].id, "mt_alfa_giulia_class_1");
        assert_eq!(found[0].name.as_deref(), Some("Alfa Romeo Giulia Class 1"));
        let root = dir.join("game");
        fs::create_dir_all(root.join("content/cars")).expect("a fixture");
        assert!(place(&root, &found[0], None, false).is_ok());
        assert!(
            root.join("content/cars/mt_alfa_giulia_class_1/data.acd")
                .is_file()
        );

        // A pack folder of zips, one of them inside a subfolder.
        let pack = dir.join("GT pack");
        fs::create_dir_all(pack.join("extra")).expect("a fixture");
        archive(
            &pack,
            "one.zip",
            &[("car_one/data.acd", "x"), ("car_one/ui/ui_car.json", "{}")],
        );
        archive(
            &pack.join("extra"),
            "two.zip",
            &[("car_two/data.acd", "x"), ("car_two/ui/ui_car.json", "{}")],
        );
        let ids: Vec<String> = whats_in(&pack).into_iter().map(|a| a.id).collect();
        assert_eq!(ids, vec!["car_one".to_string(), "car_two".to_string()]);
    }

    /// **A download as Overtake hands it over**: the mod wrapped in its own
    /// name twice, seven levels down to the car's description, the car's
    /// parts in capitals and its data loose — found, named, and installed
    /// from the folder as it is written.
    #[test]
    fn a_car_wrapped_twice_with_capitals_is_found_and_installed() {
        let dir = scratch("wrapped");
        let drop = dir.join("MTRacing_Alfa_Romeo_Giulia_Class_1_v0.99-102696325");
        let car = drop.join("MTRacing Giulia").join("MTR_Alfa_Giulia_C1");
        file(
            &car,
            "UI/ui_car.json",
            r#"{"name":"Alfa Romeo Giulia Class 1"}"#,
        );
        file(&car, "data/car.ini", "[INFO]");
        file(&car, "Giulia.KN5", "m");
        file(&car, "Skins/00_red/preview.jpg", "p");
        let found = whats_in(&drop);
        assert_eq!(found.len(), 1, "{found:?}");
        assert_eq!(found[0].kind, Kind::Car);
        assert_eq!(found[0].id, "MTR_Alfa_Giulia_C1");
        assert_eq!(found[0].name.as_deref(), Some("Alfa Romeo Giulia Class 1"));
        let root = dir.join("game");
        fs::create_dir_all(root.join("content/cars")).unwrap_or_default();
        let landed = place(&root, &found[0], None, false);
        assert!(landed.is_ok(), "{landed:?}");
        assert!(
            root.join("content/cars/MTR_Alfa_Giulia_C1/UI/ui_car.json")
                .is_file()
        );

        // Deeper still: `content/cars/` inside two wrappers.
        let packed = dir.join("Pack v1/Pack/content/cars/mtr_giulia");
        file(&packed, "ui/ui_car.json", r#"{"name":"G"}"#);
        file(&packed, "data.acd", "x");
        let found = whats_in(&dir.join("Pack v1"));
        assert_eq!(found.first().map(|f| f.id.as_str()), Some("mtr_giulia"));
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

    /// **A circuit with billboards is not a car.** Black Cat County ships a
    /// `skins/` folder and its models at the top, which is also what a car
    /// without its ui looks like; it was installed into the cars until the
    /// circuit's own signs were asked first.
    #[test]
    fn a_circuit_with_skins_and_models_is_still_a_circuit() {
        let dir = scratch("billboards");
        let track = dir.join("ks_black_cat_county");
        file(&track, "ui/layout_long/ui_track.json", r#"{"name":"Long"}"#);
        file(&track, "layout_long/ai/fast_lane.ai", "");
        file(&track, "skins/default/billboards.dds", "");
        file(&track, "ks_black_cat_county.kn5", "");

        let found = whats_in(&track);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].kind, Kind::Track);
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

#[cfg(test)]
mod csp_tests {
    #![allow(clippy::expect_used)]

    use super::tests::{archive, file, scratch};
    use super::*;

    /// A patch archive as they are actually distributed: the loader at the top
    /// and everything else under `extension`.
    fn a_patch(at: &Path, named: &str) -> PathBuf {
        archive(
            at,
            named,
            &[
                ("dwrite.dll", "MZ the loader"),
                (
                    "extension/config/data_manifest.ini",
                    "[VERSION]\nSHADERS_PATCH=0.2.7\n",
                ),
                ("extension/lua/new-modes/something.lua", "-- a mode"),
                ("extension/textures/some_texture.dds", "DDS"),
            ],
        )
    }

    /// **The patch is recognised by both halves.** `dwrite.dll` is what
    /// Windows loads and `extension` is everything it is; either alone is not
    /// a patch.
    #[test]
    fn a_shaders_patch_archive_is_recognised_and_says_its_version() {
        let dir = scratch("csp-zip");
        let zipped = a_patch(&dir, "lights-patch-v0.2.7.zip");

        let found = whats_in(&zipped);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].kind, Kind::Csp);
        assert_eq!(found[0].name.as_deref(), Some("0.2.7"));
        assert_eq!(
            found[0].where_it_goes(Path::new("/game"), None),
            Some(PathBuf::from("/game"))
        );
    }

    /// **The patch carries hundreds of textures and a `weather.ini` of its
    /// own.** Asked in the wrong order it reads as a livery or a weather
    /// preset, and a livery goes inside a car — which would put the patch at
    /// the wrong end of the game entirely.
    #[test]
    fn a_patch_is_not_mistaken_for_a_livery_or_the_weather() {
        let dir = scratch("csp-order");
        let zipped = archive(
            &dir,
            "patch.zip",
            &[
                ("dwrite.dll", "MZ"),
                ("weather.ini", "[LAUNCHER]\nNAME=not a preset\n"),
                ("body_d.dds", "DDS"),
                (
                    "extension/config/data_manifest.ini",
                    "SHADERS_PATCH=0.2.7\n",
                ),
            ],
        );

        assert_eq!(
            whats_in(&zipped).first().map(|one| one.kind),
            Some(Kind::Csp)
        );
    }

    /// **The one destination that must never be cleared first.**
    ///
    /// Every other kind lands in a folder of its own, and replacing one means
    /// deleting that folder before writing. The patch lands in the game's own
    /// folder — clearing that first would delete the game, with every car and
    /// circuit in it. This is a test rather than a comment because the cost of
    /// it being wrong is somebody's whole install.
    #[test]
    fn installing_the_patch_does_not_clear_the_game() {
        let dir = scratch("csp-place");
        let zipped = a_patch(&dir, "patch.zip");
        let root = dir.join("game");
        file(&root, "acs.exe", "MZ the game");
        file(
            &root,
            "content/cars/ks_mazda_miata/ui/ui_car.json",
            r#"{"name":"MX-5"}"#,
        );

        let found = whats_in(&zipped);
        let landed = place(&root, &found[0], None, false).expect("it should go in");
        assert_eq!(landed, root);

        assert!(root.join("acs.exe").is_file(), "it removed the game");
        assert!(
            root.join("content/cars/ks_mazda_miata").is_dir(),
            "it removed the cars"
        );
        assert!(root.join("dwrite.dll").is_file(), "the loader is not there");
        assert!(
            root.join("extension/lua/new-modes/something.lua").is_file(),
            "the patch's own files are not there"
        );

        // And the module that owns the patch agrees it is installed.
        let seen = super::super::csp::look(&root);
        assert!(seen.is_working());
        assert_eq!(seen.version.as_deref(), Some("0.2.7"));
    }

    /// A patch over a patch is a decision, like every other replacement.
    #[test]
    fn installing_over_a_patch_has_to_be_asked_for() {
        let dir = scratch("csp-over");
        let zipped = a_patch(&dir, "patch.zip");
        let root = dir.join("game");
        file(&root, "acs.exe", "MZ");
        file(
            &root,
            "extension/config/data_manifest.ini",
            "SHADERS_PATCH=0.2.0\n",
        );

        let found = whats_in(&zipped);
        assert!(matches!(
            place(&root, &found[0], None, false),
            Err(Trouble::AlreadyThere(_))
        ));

        place(&root, &found[0], None, true).expect("replacing was asked for");
        assert_eq!(
            super::super::csp::look(&root).version.as_deref(),
            Some("0.2.7")
        );
        assert!(root.join("acs.exe").is_file(), "it removed the game");
    }
}
