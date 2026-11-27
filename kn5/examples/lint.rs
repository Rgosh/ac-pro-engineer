//! What the game would trip on in a model, said line by line.
//!
//!     cargo run -p kn5 --example lint -- path/to/car.kn5 [more.kn5 …]
//!
//! Nothing printed under a file's name is a file the game reads as it is.
//! Exits non-zero when any file has a problem, so it can stand in a script.

fn main() {
    let mut bad = false;
    for path in std::env::args().skip(1) {
        println!("{path}");
        let bytes = match std::fs::read(&path) {
            Ok(bytes) => bytes,
            Err(why) => {
                println!("  cannot read it: {why}");
                bad = true;
                continue;
            }
        };
        match kn5::tree::read(&bytes) {
            Ok(model) => {
                for problem in kn5::lint::check(&model) {
                    println!("  {problem}");
                    bad = true;
                }
            }
            Err(why) => {
                println!("  not a model this reads: {why}");
                bad = true;
            }
        }
    }
    std::process::exit(i32::from(bad));
}
