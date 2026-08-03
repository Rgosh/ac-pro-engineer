use ac_core::ac_structs::{AcGraphics, AcPhysics};
use ac_core::session_info::SessionInfo;
use ac_tui::AppState;

pub struct TelemetrySampleFixture {
    pub physics: AcPhysics,
    pub graphics: AcGraphics,
    pub session: SessionInfo,
}

pub struct TelemetrySessionFixture {
    pub samples: Vec<TelemetrySampleFixture>,
}

impl TelemetrySessionFixture {
    /// Generates a realistic 3-lap telemetry session (Lap 0: Outlap, Lap 1: Fast Lap 1:45.000, Lap 2: Cool Lap)
    pub fn generate_3_lap_session() -> Self {
        let mut samples = Vec::new();

        let lap_time_sec = 105.0; // 1:45.000
        let steps_per_lap = 210; // 0.5s per step
        let dt = lap_time_sec / steps_per_lap as f32;

        let mut total_time = 0.0f32;

        // `lap` doubles as the completed-lap count: it is incremented only
        // between iterations, so inside the body it is always the number of
        // laps finished so far.
        for lap in 0..3 {
            let base_speed = match lap {
                0 => 120.0, // Outlap
                1 => 180.0, // Push lap
                _ => 110.0, // Cool lap
            };

            for step in 0..steps_per_lap {
                total_time += dt;
                let progress = step as f32 / steps_per_lap as f32;

                let speed_kmh = base_speed + (progress * std::f32::consts::TAU).sin() * 40.0;
                // Dynamic tyre state
                let wear_factor = 1.0 - (total_time / 1000.0) * 0.05;

                let phys = AcPhysics {
                    speed_kmh,
                    rpms: (5000.0 + (speed_kmh / 220.0) * 3500.0) as i32,
                    gas: if step % 20 < 15 { 0.9 } else { 0.0 },
                    brake: if step % 20 >= 15 { 0.7 } else { 0.0 },
                    steer_angle: (progress * std::f32::consts::TAU * 4.0).sin() * 0.3,
                    tyre_wear: [wear_factor; 4],
                    wheels_pressure: [
                        27.2 + (lap as f32 * 0.3),
                        27.5 + (lap as f32 * 0.3),
                        26.8,
                        26.9,
                    ],
                    tyre_temp_i: [88.0 + lap as f32 * 2.0; 4],
                    // Fuel consumption
                    fuel: (50.0 - total_time * 0.02).max(1.0),
                    ..Default::default()
                };

                let gfx = AcGraphics {
                    status: 1,
                    completed_laps: lap,
                    normalized_car_position: progress,
                    i_current_time: (progress * lap_time_sec * 1000.0) as i32,
                    i_last_time: if lap > 0 {
                        (lap_time_sec * 1000.0) as i32
                    } else {
                        0
                    },
                    i_best_time: if lap > 1 {
                        (lap_time_sec * 1000.0) as i32
                    } else {
                        0
                    },
                    session_time_left: (1800.0 - total_time) * 1000.0,
                    fuel_x_lap: 2.1,
                    car_coordinates: [
                        400.0 * (progress * std::f32::consts::TAU).cos(),
                        0.0,
                        250.0 * (progress * std::f32::consts::TAU).sin(),
                    ],
                    ..Default::default()
                };

                let session = SessionInfo {
                    car_name: "ks_ferrari_488_gt3".to_string(),
                    track_name: "monza".to_string(),
                    max_rpm: 8500,
                    ..Default::default()
                };

                samples.push(TelemetrySampleFixture {
                    physics: phys,
                    graphics: gfx,
                    session,
                });
            }
        }

        Self { samples }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_multi_lap_full_pipeline_integration() {
        let fixture = TelemetrySessionFixture::generate_3_lap_session();
        assert_eq!(fixture.samples.len(), 630);

        let mut app = AppState::new(ac_core::overlay::OverlayMode::External);
        app.config.auto_save = false;

        // Process all 630 telemetry samples through the full application pipeline
        for sample in &fixture.samples {
            app.session_info = sample.session.clone();
            app.process_tick_logic(
                sample.physics,
                sample.graphics,
                ac_core::ac_structs::AcStatic::default(),
            );
        }

        // 1. Verify telemetry history buffers aggregated correctly
        assert_eq!(app.physics_history.len(), app.config.history_size);

        // 2. Verify SessionInfo updated correctly from stream
        assert_eq!(app.session_info.car_name, "ks_ferrari_488_gt3");
        assert_eq!(app.session_info.track_name, "monza");
        assert_eq!(app.session_info.max_rpm, 8500);

        // 3. Verify Analyzer recorded laps correctly
        assert!(app.analyzer.laps.len() >= 2);

        // 4. Verify Engineer engine generated advice for session
        let last_sample = fixture.samples.last().expect("the fixture is not empty");
        let recs = app
            .engineer
            .analyze_live(&last_sample.physics, &last_sample.graphics, None);
        assert!(!recs.is_empty());
    }
}
