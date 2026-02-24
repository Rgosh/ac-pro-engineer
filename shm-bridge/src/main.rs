// Copyright (c) 2014 Jared Stafford (jspenguin@jspenguin.org)
// Copyright (c) 2024 Damir Jelić
// Copyright (c) 2026 Maxim Vasilchuk
//
// Permission is hereby granted, free of charge, to any person obtaining a copy
// of this software and associated documentation files (the "Software"), to deal
// in the Software without restriction, including without limitation the rights
// to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
// copies of the Software, and to permit persons to whom the Software is
// furnished to do so, subject to the following conditions:
//
// The above copyright notice and this permission notice shall be included in
// all copies or substantial portions of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
// IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
// FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
// AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
// LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
// OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
// SOFTWARE.

use anyhow::{Context, Result};
use clap::Parser;
use std::io::stdin;
use std::{
    fs::{File, remove_file},
    os::windows::fs::OpenOptionsExt,
    path::{Path, PathBuf},
};
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

// TODO: Support something besides Assetto Corsa.
/// The list of shared memory mappings AC/ACC create.
const ACC_FILES: &[&str] = &[
    "acpmf_crewchief",
    "acpmf_static",
    "acpmf_physics",
    "acpmf_graphics",
];

#[derive(Parser)]
#[command(author, version, about, long_about = LONG_ABOUT)]
struct Cli {}

// TODO: Should we use the real structs from simetry for this? Seems a bit
// overkill.
fn file_size(name: &str) -> usize {
    match name {
        "acpmf_crewchief" => 15660,
        _ => 2048,
    }
}

fn find_shm_dir() -> PathBuf {
    // TODO: Support non-standard tmpfs mount points. This can be achieved by
    // parsing `/proc/mounts`, or if that's not available, by parsing `/etc/fstab`.

    /// The default path for our tmpfs.
    const TMPFS_PATH: &str = "/dev/shm/";

    // TODO: We should also check that /dev/shm, or any other filesystem we found
    // using `/proc/mounts` is actually a `tmpfs`. This is sadly problematic, I
    // tried to use `GetVolumeInformationW` but, as the name suggest, it expects
    // a volume, so `C:\\`, or as Wine exposes `/`, `Z:\\`. We can't check the
    // file system name of `Z:\\dev\shm` for example. Even if we do check the
    // filesystem name of `Z:\\` we get `NTFS` back.

    PathBuf::from(TMPFS_PATH)
}

fn create_file_mapping(dir: &Path, file_name: &str, size: usize) -> Result<FileMapping> {
    let path = dir.join(file_name);

    // First we create a /dev/shm backed file.
    //
    // Now hear me out, usually we should use `shm_open(3)` here, but on Linux
    // `shm_open()` just calls `open()`. It does have some logic to find the
    // tmpfs location if it's mounted in a non-standard location. Since we can't
    // call `shm_open(3)` from inside the Wine environment
    let file = File::options()
        .read(true)
        .write(true)
        .attributes(FILE_ATTRIBUTE_TEMPORARY.0)
        .create(true)
        .open(&path)
        .context(format!("Could not open the tmpfs file: {path:?}"))?;

    // Now we create a mapping that is backed by the previously created /dev/shm`
    // file.
    let mapping = FileMapping::new(
        // We're going to use the same names the Simulator uses. This ensures that the
        // simulator will reuse this `/dev/shm` backed mapping instead of creating a new anonymous
        // one. Making the simulator reuse the mapping in turn means that the telemetry data will
        // be available in `/dev/shm` as well, making it accessible to Linux.
        file_name,
        // Pass in the handle of the `/dev/shm` file, this ensures that the file mapping is a file
        // backed one and is using our tmpfs file created on the Linux side.
        &file,
        // The documentation[1] for CreateFileMapping states that the sizes are only necessary if
        // we're using a `INVALID_HANDLE_VALUE` for the file handle.
        //
        // It also states the following:
        // > If this parameter and dwMaximumSizeHigh are 0 (zero), the maximum size of the
        // > file mapping object is equal to the current size of the file that hFile identifies.
        //
        // This sadly doesn't seem to work with our `/dev/shm` file and makes the Simulator crash,
        // so we're passing the sizes manually.
        //
        // [1]: https://learn.microsoft.com/en-us/windows/win32/api/winbase/nf-winbase-createfilemappinga#parameters
        size,
    )?;

    // Return the mapping, the caller needs to ensure that the mapping object stays
    // alive. On the other hand, the `/dev/shm` backed file can be closed.
    Ok(mapping)
}

fn main() -> Result<()> {
    let _ = Cli::parse();

    let mut mappings = Vec::new();

    // Find a suitable tmpfs based mountpoint, this is usually `/dev/shm`.
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

    // The CTRL-C handler has unparked us, somebody wants us to stop running so
    // let's unlink the `/dev/shm` files.
    for file_name in ACC_FILES {
        println!("Removing mapping {file_name}");
        let path = shm_dir.join(file_name);

        remove_file(&path)
            .with_context(|| format!("Could not unlink the /dev/shm backed file {file_name}"))?;
    }

    Ok(())
}
