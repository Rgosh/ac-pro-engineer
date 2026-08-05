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
const OVERLAY_FILE_SIZE: usize = 712;

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
struct Cli {}

fn file_size(name: &str) -> usize {
    match name {
        "acpmf_crewchief" => 15660,
        OVERLAY_FILE => OVERLAY_FILE_SIZE,
        _ => 2048,
    }
}

fn find_shm_dir() -> PathBuf {
    const TMPFS_PATH: &str = "/dev/shm/";
    PathBuf::from(TMPFS_PATH)
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

fn main() -> Result<()> {
    let _ = Cli::parse();

    let mut mappings = Vec::new();

    let shm_dir = find_shm_dir();

    println!(
        "shm-bridge {} (bridge protocol {BRIDGE_PROTOCOL}, overlay frame {OVERLAY_FILE_SIZE} bytes)",
        env!("CARGO_PKG_VERSION")
    );
    println!("Found a tmpfs filesystem at {}", shm_dir.to_string_lossy());

    for file_name in ACC_FILES {
        let size = file_size(file_name);
        let mapping = create_file_mapping(&shm_dir, file_name, size)
            .with_context(|| format!("Error creating a file mapping for {file_name}"))?;

        println!("Created a tmpfs backed mapping for {file_name} with size {size}");
        mappings.push(mapping);
    }

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
