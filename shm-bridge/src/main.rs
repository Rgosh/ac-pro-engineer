// Copyright (c) 2014 Jared Stafford (jspenguin@jspenguin.org)
// Copyright (c) 2024 Damir Jelić
// Copyright (c) 2026 Maxim Vasilchuk

use anyhow::{Context, Result};
use clap::Parser;
use std::fs::{File, remove_file};
use std::io::stdin;
use std::path::{Path, PathBuf};

#[cfg(target_os = "windows")]
use std::os::windows::fs::OpenOptionsExt;
#[cfg(target_os = "windows")]
use windows::Win32::Storage::FileSystem::FILE_ATTRIBUTE_TEMPORARY;

use crate::file_mapping::FileMapping;

mod file_mapping;

const LONG_ABOUT: &str = "Shared Memory Bridge facilitates sharing memory between Windows\n\
                          applications running under Wine/Proton and Linux, offering a seamless\n\
                          way to access and manipulate named shared memory spaces across these\n\
                          platforms. It's particularly useful in gaming and simulations, allowing\n\
                          Linux applications to directly read data from Windows applications.\n\n\
                          Example Usage:\n\n\
                          To launch the bridge and view command line options, use the following \
                          command:\n    \
                              protontricks-launch --appid APPID shm-bridge.exe\n\n\
                          This will display help output and available options for `shm-bridge`,\n\
                          guiding you through the necessary steps to set up and run the bridge\n\
                          within your specific environment.";

const ACC_FILES: &[&str] = &[
    "acpmf_crewchief",
    "acpmf_static",
    "acpmf_physics",
    "acpmf_graphics",
    // Runs the other way to the rest: the desktop application writes this one
    // and the in-game Lua overlay reads it. The mechanism is identical — a
    // file under /dev/shm wrapped in a Win32 named mapping — only the
    // direction differs, so it costs one more entry rather than a second
    // bridge.
    OVERLAY_FILE,
];

/// Shared block the Lua overlay reads. Must match
/// `ac_core::overlay::frame::OVERLAY_MMF_NAME`; the `AcTools.CSP.Limited.`
/// prefix is what lets a CSP script without IO permission open it.
const OVERLAY_FILE: &str = "AcTools.CSP.Limited.ACPE.v1";

/// Size of `ac_core::overlay::frame::OverlayFrame`.
///
/// Hardcoded because shm-bridge deliberately does not depend on ac_core — it
/// is a tiny Windows binary that runs under Wine, and pulling in the whole
/// telemetry crate to learn one number would be a poor trade. The mapping only
/// has to be at least struct-sized, so this is checked against the real value
/// by a test in ac_core rather than kept in step by hand.
const OVERLAY_FILE_SIZE: usize = 2484;

/// Shape of the note this bridge leaves behind — see [`BRIDGE_INFO_FILE`].
///
/// Bumped when the note gains or loses a key, not when the bridge changes.
/// Must match `ac_core::overlay::bridge::BRIDGE_PROTOCOL`.
const BRIDGE_PROTOCOL: u32 = 1;

/// Where the bridge says who it is.
///
/// The application cannot ask a running bridge anything: it is a Windows
/// process inside a Wine prefix, started by hand through protontricks, with no
/// channel back. But it can write a file, and `/dev/shm` is the one directory
/// both sides already agree on. Every failure that cost an evening was a
/// bridge older than the frame it was mapping, and this is what makes that
/// visible instead of leaving the panel saying "waiting for AC Pro Engineer"
/// with the mapping sitting right there.
///
/// Must match `ac_core::overlay::bridge::BRIDGE_INFO_FILE`.
const BRIDGE_INFO_FILE: &str = "acpe-bridge.info";

/// The version, findable in the built `.exe` without running it.
///
/// The application has to be able to tell how old a `shm-bridge.exe` sitting
/// next to it is, and the honest answers are all unavailable: it cannot run a
/// Windows binary to ask, and a bridge that is not running has left no note.
/// So the version travels in the file itself, behind a prefix distinctive
/// enough that scanning for it cannot match anything else.
///
/// `#[used]` keeps it through dead-code elimination — nothing reads this
/// static, which is the point.
#[used]
static VERSION_MARKER: &[u8] =
    concat!("ACPE-SHM-BRIDGE-VERSION=", env!("CARGO_PKG_VERSION"), ";").as_bytes();

#[derive(Parser)]
#[command(author, version, about, long_about = LONG_ABOUT)]
struct Cli {
    /// Open the overlay mapping the way CSP does and report what is in it,
    /// then exit.
    ///
    /// The one question nobody could answer from inside the Proton prefix:
    /// *can a Windows process in here see the frame at all*. The desktop
    /// application checks the file, the bridge's note and the versions, and
    /// all three can be right while CSP still refuses the mapping — that is
    /// the failure this whole subsystem exists to make visible, and until now
    /// the only way to see it was to launch the game.
    ///
    /// Run it in the same prefix, with the bridge already running:
    ///
    ///     protontricks-launch --appid 244210 shm-bridge.exe --verify
    #[arg(long)]
    verify: bool,
}

fn file_size(name: &str) -> usize {
    match name {
        "acpmf_crewchief" => 15660,
        OVERLAY_FILE => OVERLAY_FILE_SIZE,
        _ => 2048,
    }
}

/// A page the game already owns, copied out to the tmpfs file.
///
/// **Why this exists at all.** `CreateFileMappingW` with a name that is
/// already taken hands back the *existing* section and ignores the file it was
/// given — so a bridge that starts after the game creates nothing: the game
/// keeps writing into its own anonymous section and the tmpfs file sits there
/// holding whatever was in it, frozen, for anyone reading it on the Linux
/// side to mistake for telemetry.
///
/// That made the order of startup a rule nobody could keep: the bridge had to
/// be in the prefix before the game, and while it is there Steam cannot launch
/// the game into that prefix. Mirroring is what removes the rule. If the
/// section is already there, open it and copy — the same call CSP makes, at a
/// rate faster than the reader ticks.
#[cfg(target_os = "windows")]
struct Mirror {
    view: *const u8,
    handle: windows::Win32::Foundation::HANDLE,
    file: File,
    size: usize,
    name: &'static str,
}

// SAFETY: the view is a read-only mapping of a section that outlives the
// thread, and nothing else writes through this pointer. Wine hands out one
// view per handle and it is not moved.
#[cfg(target_os = "windows")]
unsafe impl Send for Mirror {}

#[cfg(target_os = "windows")]
impl Mirror {
    /// Open a section somebody else already created, or `None` if nobody has.
    fn open(name: &'static str, file: File, size: usize) -> Option<Self> {
        use windows::Win32::System::Memory::{FILE_MAP_READ, MapViewOfFile, OpenFileMappingW};
        use windows::core::HSTRING;

        let wide = HSTRING::from(name);
        // SAFETY: the name outlives the call; the handle and the view are
        // checked before use and released in `Drop`.
        unsafe {
            let handle = OpenFileMappingW(FILE_MAP_READ.0, false, &wide).ok()?;
            let view = MapViewOfFile(handle, FILE_MAP_READ, 0, 0, 0);
            if view.Value.is_null() {
                use windows::Win32::Foundation::CloseHandle;
                let _ = CloseHandle(handle);
                return None;
            }
            Some(Self {
                view: view.Value as *const u8,
                handle,
                file,
                size,
                name,
            })
        }
    }

    /// One copy, section to file.
    fn pump(&mut self) -> std::io::Result<()> {
        use std::io::{Seek, SeekFrom, Write};

        // SAFETY: the view was mapped for at least `size` bytes — the section
        // is the game's own page and is never smaller than the size this
        // build maps — and it is read only.
        let bytes = unsafe { std::slice::from_raw_parts(self.view, self.size) };
        self.file.seek(SeekFrom::Start(0))?;
        self.file.write_all(bytes)
    }
}

#[cfg(target_os = "windows")]
impl Drop for Mirror {
    fn drop(&mut self) {
        use windows::Win32::Foundation::CloseHandle;
        use windows::Win32::System::Memory::{MEMORY_MAPPED_VIEW_ADDRESS, UnmapViewOfFile};
        // SAFETY: both were produced by `open` and are released once.
        unsafe {
            let _ = UnmapViewOfFile(MEMORY_MAPPED_VIEW_ADDRESS {
                Value: self.view as *mut _,
            });
            let _ = CloseHandle(self.handle);
        }
    }
}

/// How often a mirrored page is copied.
///
/// Assetto Corsa rewrites its physics page at 333 Hz and the desktop side
/// reads at sixty. Four milliseconds is comfortably inside both, and copying
/// two kilobytes at that rate is not measurable next to a game.
#[cfg(target_os = "windows")]
const MIRROR_INTERVAL: std::time::Duration = std::time::Duration::from_millis(4);

fn find_shm_dir() -> PathBuf {
    const TMPFS_PATH: &str = "/dev/shm/";
    PathBuf::from(TMPFS_PATH)
}

/// The tmpfs file behind a page: opened, and sized to what this build maps.
///
/// Split out of `create_file_mapping` because a mirrored page needs the file
/// and *not* the Win32 section — the section it would create is the one the
/// game already owns, which is the whole problem being fixed.
///
/// Windows only, because mirroring is: on Linux this binary exists to be
/// cross-compiled and to run under Wine, and its non-Windows build is a stub
/// that reports it cannot map anything.
#[cfg(target_os = "windows")]
fn open_tmpfs_file(dir: &Path, file_name: &str, size: usize) -> Result<File> {
    let path = dir.join(file_name);

    let mut options = File::options();
    options.read(true).write(true).create(true);
    #[cfg(target_os = "windows")]
    options.attributes(FILE_ATTRIBUTE_TEMPORARY.0);

    let file = options
        .open(&path)
        .context(format!("Could not open the tmpfs file: {path:?}"))?;

    // Every time, not only on creation — see the note in `create_file_mapping`
    // about a 712-byte overlay file surviving into a build that maps 2484.
    file.set_len(size as u64)
        .context(format!("Could not size the tmpfs file: {path:?}"))?;
    Ok(file)
}

fn create_file_mapping(dir: &Path, file_name: &str, size: usize) -> Result<FileMapping> {
    let path = dir.join(file_name);

    let mut options = File::options();
    options.read(true).write(true).create(true);
    #[cfg(target_os = "windows")]
    options.attributes(FILE_ATTRIBUTE_TEMPORARY.0);

    let file = options
        .open(&path)
        .context(format!("Could not open the tmpfs file: {path:?}"))?;

    // Set the length explicitly, every time.
    //
    // `create(true)` creates the file when it is missing and *leaves an
    // existing one exactly as it was* — including its length. The overlay
    // mapping grew from 712 bytes to 2484 between releases, so a machine that
    // ran the older bridge has a 712-byte file sitting in /dev/shm, and the new
    // bridge opened it and carried on at the old size. CSP then refuses the
    // mapping, the panel waits forever, and every version number in sight says
    // the bridge is current — because it is. It is the file that is old.
    //
    // Truncating is safe here: this is a mapping of live telemetry, rewritten
    // sixty times a second, and there is nothing in it worth keeping across a
    // restart.
    file.set_len(size as u64)
        .context(format!("Could not size the tmpfs file: {path:?}"))?;

    let mapping = FileMapping::new(file_name, &file, size)?;

    Ok(mapping)
}

/// Leave the note the application reads to learn which bridge is running.
///
/// Written after the mappings, so its presence means they exist. Best effort:
/// a bridge that cannot write this still maps everything, and the application
/// then reports an unknown bridge rather than a broken one.
fn write_bridge_info(dir: &Path) -> Result<PathBuf> {
    let path = dir.join(BRIDGE_INFO_FILE);
    let body = format!(
        "protocol={BRIDGE_PROTOCOL}\n\
         version={}\n\
         frame_bytes={OVERLAY_FILE_SIZE}\n\
         mmf={OVERLAY_FILE}\n\
         pid={}\n",
        env!("CARGO_PKG_VERSION"),
        std::process::id(),
    );
    std::fs::write(&path, body).context(format!("Could not write {path:?}"))?;
    Ok(path)
}

/// Open the overlay mapping by name and say what is in it.
///
/// Deliberately the same call a CSP script makes — `OpenFileMappingW` on the
/// bare name, no `Local\\` prefix — so a success here means the panel can open
/// it too, and a failure here is the failure the panel would hit.
#[cfg(target_os = "windows")]
fn verify() -> Result<()> {
    use windows::Win32::Foundation::CloseHandle;
    use windows::Win32::System::Memory::{
        FILE_MAP_READ, MEMORY_MAPPED_VIEW_ADDRESS, MapViewOfFile, OpenFileMappingW, UnmapViewOfFile,
    };
    use windows::core::HSTRING;

    println!(
        "shm-bridge {} — verifying the overlay mapping",
        env!("CARGO_PKG_VERSION")
    );
    println!("opening {OVERLAY_FILE} the way CSP does");

    let name = HSTRING::from(OVERLAY_FILE);
    // SAFETY: every pointer below is either checked for null or derived from a
    // view this function mapped and unmaps before returning.
    unsafe {
        let handle = OpenFileMappingW(FILE_MAP_READ.0, false, &name).map_err(|error| {
            anyhow::anyhow!(
                "could not open {OVERLAY_FILE}: {error}\n\n\
                 Nothing has created it in this prefix. Start shm-bridge.exe here \n\
                 first, and make sure the desktop application is running on the \n\
                 Linux side — it is what fills the file the bridge wraps."
            )
        })?;

        let view = MapViewOfFile(handle, FILE_MAP_READ, 0, 0, 0);
        if view.Value.is_null() {
            let _ = CloseHandle(handle);
            return Err(anyhow::anyhow!(
                "opened {OVERLAY_FILE} and could not map a view of it"
            ));
        }

        let base = view.Value as *const u8;
        let version = std::ptr::read_unaligned(base as *const u32);
        let sequence = std::ptr::read_unaligned(base.add(4) as *const u32);
        // Last field in the struct, so its offset is the size minus its own.
        let app_version = std::slice::from_raw_parts(base.add(OVERLAY_FILE_SIZE - 16), 16);
        let end = app_version
            .iter()
            .position(|byte| *byte == 0)
            .unwrap_or(app_version.len());
        let app_version = String::from_utf8_lossy(&app_version[..end]);

        println!("  opened          yes");
        println!("  frame version   {version}");
        println!("  sequence        {sequence}");
        println!(
            "  application     {}",
            if app_version.is_empty() {
                "— (nothing has published yet)"
            } else {
                &app_version
            }
        );

        let _ = UnmapViewOfFile(MEMORY_MAPPED_VIEW_ADDRESS { Value: view.Value });
        let _ = CloseHandle(handle);

        if sequence == 0 {
            println!(
                "\nThe mapping is there and empty. The bridge is doing its job; the\n\
                 desktop application is not publishing. Start it on the Linux side."
            );
        } else {
            println!("\nThe overlay can be read from inside this prefix.");
        }
    }

    Ok(())
}

/// Same flag on the Linux build, so `--help` does not describe something that
/// is not there. It cannot do the check: the mapping is a Win32 object and
/// only a process inside the prefix can open it.
#[cfg(not(target_os = "windows"))]
fn verify() -> Result<()> {
    Err(anyhow::anyhow!(
        "--verify has to run inside the Proton prefix, where the Win32 mapping \n\
         exists. Run the Windows build there:\n\n    \
         protontricks-launch --appid 244210 shm-bridge.exe --verify"
    ))
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    if cli.verify {
        return verify();
    }

    let mut mappings = Vec::new();

    let shm_dir = find_shm_dir();

    println!(
        "shm-bridge {} (bridge protocol {BRIDGE_PROTOCOL}, overlay frame {OVERLAY_FILE_SIZE} bytes)",
        env!("CARGO_PKG_VERSION")
    );
    println!("Found a tmpfs filesystem at {}", shm_dir.to_string_lossy());

    // Pages somebody else already owns are copied rather than created; see
    // `Mirror`. This is what makes the bridge work whichever order the game
    // and this were started in.
    #[cfg(target_os = "windows")]
    let mut mirrors: Vec<Mirror> = Vec::new();

    for file_name in ACC_FILES {
        let size = file_size(file_name);

        #[cfg(target_os = "windows")]
        if let Some(mirror) = open_tmpfs_file(&shm_dir, file_name, size)
            .ok()
            .and_then(|file| Mirror::open(file_name, file, size))
        {
            println!(
                "{file_name} already exists in this prefix — mirroring it into the tmpfs \
                 file ({size} bytes)"
            );
            mirrors.push(mirror);
            continue;
        }

        let mapping = create_file_mapping(&shm_dir, file_name, size)
            .with_context(|| format!("Error creating a file mapping for {file_name}"))?;

        println!("Created a tmpfs backed mapping for {file_name} with size {size}");
        mappings.push(mapping);
    }

    // One thread for all of them, because they are copied together and a
    // thread each would be five wakeups where one does.
    #[cfg(target_os = "windows")]
    let mirror_stop = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    #[cfg(target_os = "windows")]
    let mirror_thread = (!mirrors.is_empty()).then(|| {
        let stop = std::sync::Arc::clone(&mirror_stop);
        std::thread::spawn(move || {
            while !stop.load(std::sync::atomic::Ordering::Relaxed) {
                for mirror in mirrors.iter_mut() {
                    if let Err(error) = mirror.pump() {
                        // Said once per page and then carried on: a write that
                        // fails every four milliseconds would fill a log with
                        // one fault.
                        eprintln!("Could not mirror {}: {error}", mirror.name);
                        stop.store(true, std::sync::atomic::Ordering::Relaxed);
                        break;
                    }
                }
                std::thread::sleep(MIRROR_INTERVAL);
            }
        })
    });

    match write_bridge_info(&shm_dir) {
        Ok(path) => println!("Announced this bridge in {}", path.display()),
        // Not fatal. The mappings are what the game needs; the note is only
        // how the application names the version it is talking to.
        Err(error) => eprintln!("Could not announce this bridge: {error:#}"),
    }

    println!("All mappings were successfully created, enter 'exit' to close the app");

    let mut input = String::new();
    while let Ok(bytes) = stdin().read_line(&mut input) {
        if bytes == 0 {
            break;
        }
        match input.trim() {
            "exit" => break,
            _ => {
                println!("Incorrect command '{}'", input.trim());
            }
        }
        input.clear();
    }

    println!("\nShutting down.");

    #[cfg(target_os = "windows")]
    {
        mirror_stop.store(true, std::sync::atomic::Ordering::Relaxed);
        if let Some(handle) = mirror_thread {
            // Before the files below are unlinked: a copy landing after the
            // unlink would recreate one of them, and a page nobody owns is
            // exactly what this bridge exists to prevent.
            let _ = handle.join();
        }
    }

    // Best effort, and deliberately so. `?` here meant one already-removed
    // file — or one owned by another user — returned early and left the rest
    // of the mappings in place. They persist as zero-filled pages that the TUI
    // maps without complaint, so it reports a healthy connection to a feed
    // that is all zeroes: the state that produces NaN telemetry downstream.
    // Before the mappings, not after: while this file is there the application
    // takes it as a promise that they are too, and the window where that is
    // untrue should be as short as possible.
    let info_path = shm_dir.join(BRIDGE_INFO_FILE);
    if let Err(error) = remove_file(&info_path)
        && error.kind() != std::io::ErrorKind::NotFound
    {
        eprintln!("Could not unlink {}: {error}", info_path.display());
    }

    let mut failures = 0;
    for file_name in ACC_FILES {
        println!("Removing mapping {file_name}");
        let path = shm_dir.join(file_name);

        if let Err(error) = remove_file(&path) {
            eprintln!("Could not unlink {}: {error}", path.display());
            failures += 1;
        }
    }

    if failures > 0 {
        eprintln!(
            "{failures} of {} mappings could not be removed; stale pages may remain in {}",
            ACC_FILES.len(),
            shm_dir.display()
        );
    }

    Ok(())
}
