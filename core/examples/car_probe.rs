//! What this machine's Assetto Corsa knows about one car.
//!
//! `cargo run -p ac_core --example car_probe -- bmw_z4_gt3`
//!
//! Reads through the same path the program does, so what it prints is what a
//! screen would draw — which is the point of having it rather than reading the
//! file by hand and believing the two agree.

fn main() {
    let id = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "bmw_z4_gt3".into());
    let Some(install) = ac_core::games::assetto_corsa::paths::ac_install_root(None) else {
        println!("Assetto Corsa is not installed here.");
        return;
    };
    let car = ac_core::games::assetto_corsa::tracks::read_car(&install, &id);
    if car.is_empty() {
        println!("{id}: nothing — not installed, or it ships no metadata.");
        return;
    }

    println!("{id}");
    println!("  name    {}", car.name);
    println!("  brand   {}", car.brand);
    println!("  class   {} (the game's word, not ours)", car.class);
    println!("  ours    {:?}", ac_core::games::CarClass::from_id(&id));
    println!("  tags    {}", car.tags.join(", "));
    for (key, value) in &car.specs {
        println!("  {key:<8}{value}");
    }
    if let Some((revs, value)) = car.power_peak() {
        println!("  power   {value:.0} bhp at {revs:.0} rpm");
    }
    if let Some((revs, value)) = car.torque_peak() {
        println!("  torque  {value:.0} Nm at {revs:.0} rpm");
    }
    println!("  curve   {} points", car.torque.len());
    match car.shape.as_ref() {
        Some(shape) => {
            println!(
                "  shape   {:.2} m long, {:.2} wide, {:.2} tall, {} points",
                shape.length_m,
                shape.width_m,
                shape.height_m,
                shape.outline.len()
            );
            match (
                shape.wheelbase_m(),
                shape.track_front_m(),
                shape.track_rear_m(),
            ) {
                (Some(base), Some(front), Some(rear)) => println!(
                    "  wheels  base {base:.2} m, track {front:.2} front and {rear:.2} rear"
                ),
                // The model did not name them, and four wheels in the wrong
                // place is worse than none.
                _ => println!("  wheels  the model does not name them"),
            }
        }
        None => println!("  shape   no model this can read"),
    }
    println!(
        "  badge   {}",
        car.badge
            .as_ref()
            .map(|path| path.display().to_string())
            .unwrap_or_else(|| "none".into())
    );

    // The band the engine is worth using, which is the thing the curve is read
    // for and the reason it is on screen at all.
    if let (Some((peak_revs, peak)), false) = (car.power_peak(), car.power.is_empty()) {
        let usable: Vec<f32> = car
            .power
            .iter()
            .filter(|(_, value)| *value >= peak * 0.95)
            .map(|(revs, _)| *revs)
            .collect();
        if let (Some(from), Some(to)) = (usable.first(), usable.last()) {
            println!("  within 5% of peak power: {from:.0} to {to:.0} rpm (peak {peak_revs:.0})");
        }
    }
}
