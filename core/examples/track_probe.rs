//! What this machine's Assetto Corsa knows about its own circuits.
//!
//! Run it to check the reader against a real installation rather than against
//! a fixture: `cargo run -p ac_core --example track_probe`.

fn main() {
    let Some(install) = ac_core::games::assetto_corsa::paths::ac_install_root(None) else {
        println!("Assetto Corsa is not installed here.");
        return;
    };
    println!("install: {}\n", install.display());

    let tracks = install.join("content").join("tracks");
    let Ok(entries) = std::fs::read_dir(&tracks) else {
        println!("no content/tracks");
        return;
    };
    let mut names: Vec<String> = entries
        .flatten()
        .filter(|entry| entry.path().is_dir())
        .map(|entry| entry.file_name().to_string_lossy().to_string())
        .collect();
    names.sort();

    for name in names {
        let data = ac_core::games::assetto_corsa::tracks::read(&install, &name, "");
        if data.is_empty() {
            println!("{name:<26} —");
            continue;
        }
        println!(
            "{name:<26} outline {}  align {}  sections {:<3} drs {:<2} ai {} points",
            if data.outline.is_some() { "yes" } else { "no " },
            if data.alignment.is_some() { "yes" } else { "no " },
            data.sections.len(),
            data.drs.len(),
            data.ai_line.len(),
        );
        for section in data.sections.iter().take(3) {
            println!("      {:.3}–{:.3}  {}", section.from, section.to, section.name);
        }
    }
}
