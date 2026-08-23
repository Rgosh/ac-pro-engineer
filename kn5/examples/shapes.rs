//! Every car installed on this machine, and the shape read out of it.
//!
//! `cargo run -p kn5 --example shapes`
//!
//! The parser is checked against real files the way the rest of this project
//! pins a memory layout to a recording: a format read from memory is a format
//! nobody has verified.

fn main() {
    let Some(install) = ac_install() else {
        println!("Assetto Corsa is not installed here.");
        return;
    };
    let cars = install.join("content").join("cars");
    let Ok(entries) = std::fs::read_dir(&cars) else {
        println!("no content/cars under {}", install.display());
        return;
    };
    let mut ids: Vec<std::path::PathBuf> = entries
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.is_dir())
        .collect();
    ids.sort();

    let (mut read, mut none) = (0, 0);
    for folder in &ids {
        let name = folder.file_name().unwrap_or_default().to_string_lossy();
        match kn5::car_shape(folder).ok_or(()) {
            Ok(shape) => {
                read += 1;
                if read <= 10 {
                    println!(
                        "{name:<30} {:>5.2} × {:>4.2} × {:>4.2} m   outline {:>3}   wheels {}",
                        shape.length_m,
                        shape.width_m,
                        shape.height_m,
                        shape.outline.len(),
                        match (shape.wheelbase_m(), shape.track_front_m()) {
                            (Some(base), Some(track)) => format!("base {base:.2} track {track:.2}"),
                            _ => "not named".to_string(),
                        }
                    );
                }
            }
            Err(()) => {
                none += 1;
            }
        }
    }
    println!("\n{read} read, {none} with no usable model");
}

/// Where Steam put the game, without depending on the rest of the workspace.
fn ac_install() -> Option<std::path::PathBuf> {
    let home = std::env::var_os("HOME")?;
    for under in [
        ".local/share/Steam/steamapps/common/assettocorsa",
        ".steam/steam/steamapps/common/assettocorsa",
    ] {
        let path = std::path::Path::new(&home).join(under);
        if path.is_dir() {
            return Some(path);
        }
    }
    None
}
