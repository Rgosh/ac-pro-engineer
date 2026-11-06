//! What a dropped file or folder holds, as the installer reads it.
//!
//!     cargo run -p ac_core --example peek_drop -- path/to/mod.7z
//!
//! Prints each car, circuit, livery or patch found, and where it would go —
//! the quickest way to see why a mod is or is not recognised, 7z and rar
//! included (read through 7-Zip, bsdtar or unrar on the machine).
use ac_core::games::assetto_corsa::adding;
use std::path::PathBuf;

fn main() {
    let Some(path) = std::env::args().nth(1).map(PathBuf::from) else {
        eprintln!("usage: peek_drop <file or folder>");
        return;
    };
    if adding::is_other_archive(&path)
        && let Err(why) = adding::unpack_elsewhere(&path)
    {
        eprintln!("{why}");
        return;
    }
    let found = adding::whats_in(&path);
    if found.is_empty() {
        println!("nothing recognised in {}", path.display());
    }
    for one in found {
        println!("{:?} {} — {}", one.kind, one.id, one.title());
    }
}
