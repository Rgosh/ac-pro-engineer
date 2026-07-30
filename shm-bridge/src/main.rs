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
];

#[derive(Parser)]
#[command(author, version, about, long_about = LONG_ABOUT)]
struct Cli {}

fn file_size(name: &str) -> usize {
    match name {
        "acpmf_crewchief" => 15660,
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
    while stdin().read_line(&mut input).is_ok() {
        match input.trim() {
            "exit" => break,
            _ => {
                println!("Incorrect command '{input}'");
            }
        }
    }

    println!("\nShutting down.");

    for file_name in ACC_FILES {
        println!("Removing mapping {file_name}");
        let path = shm_dir.join(file_name);

        remove_file(&path)
            .with_context(|| format!("Could not unlink the /dev/shm backed file {file_name}"))?;
    }

    Ok(())
}
