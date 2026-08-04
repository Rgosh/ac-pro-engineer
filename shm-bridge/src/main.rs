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
const OVERLAY_FILE_SIZE: usize = 416;

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

fn main() -> Result<()> {
    let _ = Cli::parse();

    let mut mappings = Vec::new();

    let shm_dir = find_shm_dir();

    println!("Found a tmpfs filesystem at {}", shm_dir.to_string_lossy());

    for file_name in ACC_FILES {
        let size = file_size(file_name);
        let mapping = create_file_mapping(&shm_dir, file_name, size)
            .with_context(|| format!("Error creating a file mapping for {file_name}"))?;

        println!("Created a tmpfs backed mapping for {file_name} with size {size}");
        mappings.push(mapping);
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
