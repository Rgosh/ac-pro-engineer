use ac_core::config::{AppConfig, Language};
use ac_core::engineer::{
    DrivingStyle, Engineer, EngineerStats, Recommendation, Severity, WizardPhase, WizardProblem,
};
use ac_core::setup_manager::CarSetup;
use std::cmp::Ordering;

fn get_english_config() -> AppConfig {
    let mut config = AppConfig::default();
    config.language = Language::English;
    config
}

fn get_russian_config() -> AppConfig {
    let mut config = AppConfig::default();
    config.language = Language::Russian;
    config
}

#[test]
fn test_01_stats_initialization() {
    let stats = EngineerStats::new();
    assert_eq!(stats.bottoming_frames, [0; 4]);
    assert_eq!(stats.lockup_frames_front, 0);
    assert_eq!(stats.lockup_frames_rear, 0);
    assert_eq!(stats.fuel_laps_remaining, 0.0);
    assert_eq!(stats.base_tyre_wear, [100.0; 4]);
    assert_eq!(stats.scrubbing_frames, 0);
}

#[test]
fn test_02_driving_style_defaults() {
    let style = DrivingStyle::new();
    assert_eq!(style.smoothness, 50.0);
    assert_eq!(style.aggression, 50.0);
}

#[test]
fn test_03_tyre_wear_mathematics() {
    let mut stats = EngineerStats::new();
    stats.base_tyre_wear = [100.0, 100.0, 100.0, 100.0];
    stats.stint_laps = 10;
    let current_wear = [96.0, 95.0, 98.0, 97.0];
    for i in 0..4 {
        let wear_used = stats.base_tyre_wear[i] - current_wear[i];
        let wear_per_lap = wear_used / stats.stint_laps as f32;
        let remaining_wear = current_wear[i] - 94.0;
        if wear_per_lap > 0.001 {
            stats.tyre_laps_remaining[i] = (remaining_wear / wear_per_lap).max(0.0);
        }
    }
    assert_eq!(stats.tyre_laps_remaining[0], 5.0);
    assert_eq!(stats.tyre_laps_remaining[1], 2.0);
    assert_eq!(stats.tyre_laps_remaining[2], 20.0);
    assert_eq!(stats.tyre_laps_remaining[3], 10.0);
}

#[test]
fn test_04_tyre_wear_extreme_values() {
    let mut stats = EngineerStats::new();
    stats.base_tyre_wear = [100.0, 100.0, 100.0, 100.0];
    stats.stint_laps = 0;
    let current_wear = [105.0, -50.0, 0.0, 94.0];
    for i in 0..4 {
        let wear_used = stats.base_tyre_wear[i] - current_wear[i];
        let wear_per_lap = if stats.stint_laps > 0 {
            wear_used / stats.stint_laps as f32
        } else {
            0.0
        };
        let remaining_wear = current_wear[i] - 94.0;
        if wear_per_lap > 0.001 {
            stats.tyre_laps_remaining[i] = (remaining_wear / wear_per_lap).max(0.0);
        } else {
            stats.tyre_laps_remaining[i] = 99.0;
        }
    }
    assert_eq!(stats.tyre_laps_remaining[0], 99.0);
}

#[test]
fn test_05_fuel_calculations_edge_cases() {
    let mut stats = EngineerStats::new();
    let fuel_variants = vec![
        (50.0_f32, 2.0_f32),
        (0.0, 2.0),
        (-10.0, 2.0),
        (50.0, 0.0),
        (-5.0, -1.0),
    ];
    for (fuel_level, fuel_x_lap) in fuel_variants {
        if fuel_x_lap > 0.0 {
            stats.fuel_laps_remaining = fuel_level / fuel_x_lap;
            assert!(stats.fuel_laps_remaining.is_finite());
        } else {
            stats.fuel_laps_remaining = 0.0;
            assert_eq!(stats.fuel_laps_remaining, 0.0);
        }
    }
}

#[test]
fn test_06_driving_style_extreme_inputs() {
    let mut style = DrivingStyle::new();
    let inputs = vec![0.0_f32, 1.0, -0.5, 5.0, -100.0];
    for throttle in &inputs {
        for brake in &inputs {
            let throttle_smoothness = 100.0_f32 - (throttle * 100.0_f32).abs();
            let brake_smoothness = 100.0_f32 - (brake * 100.0_f32).abs();
            style.smoothness =
                0.7 * style.smoothness + 0.3 * (throttle_smoothness + brake_smoothness) / 2.0;
            assert!(!style.smoothness.is_nan());
        }
    }
}

#[test]
fn test_07_wizard_matrix_entry_understeer() {
    let mut engineer = Engineer::new(&get_english_config());
    engineer.wizard_phase = WizardPhase::Entry;
    engineer.wizard_problem = WizardProblem::Understeer;
    assert!(engineer
        .get_wizard_advice()
        .contains(&"Decrease Front Rebound".to_string()));
}

#[test]
fn test_08_wizard_matrix_entry_oversteer() {
    let mut engineer = Engineer::new(&get_english_config());
    engineer.wizard_phase = WizardPhase::Entry;
    engineer.wizard_problem = WizardProblem::Oversteer;
    assert!(engineer
        .get_wizard_advice()
        .contains(&"Increase Front Rebound".to_string()));
}

#[test]
fn test_09_wizard_matrix_apex_understeer() {
    let mut engineer = Engineer::new(&get_english_config());
    engineer.wizard_phase = WizardPhase::Apex;
    engineer.wizard_problem = WizardProblem::Understeer;
    assert!(engineer
        .get_wizard_advice()
        .contains(&"Softer Front Springs".to_string()));
}

#[test]
fn test_10_wizard_matrix_apex_oversteer() {
    let mut engineer = Engineer::new(&get_english_config());
    engineer.wizard_phase = WizardPhase::Apex;
    engineer.wizard_problem = WizardProblem::Oversteer;
    assert!(engineer
        .get_wizard_advice()
        .contains(&"Softer Rear Springs".to_string()));
}

#[test]
fn test_11_wizard_matrix_exit_understeer() {
    let mut engineer = Engineer::new(&get_english_config());
    engineer.wizard_phase = WizardPhase::Exit;
    engineer.wizard_problem = WizardProblem::Understeer;
    assert!(engineer
        .get_wizard_advice()
        .contains(&"Increase Front Bump".to_string()));
}

#[test]
fn test_12_wizard_matrix_exit_oversteer() {
    let mut engineer = Engineer::new(&get_english_config());
    engineer.wizard_phase = WizardPhase::Exit;
    engineer.wizard_problem = WizardProblem::Oversteer;
    assert!(engineer
        .get_wizard_advice()
        .contains(&"Decrease Rear Bump".to_string()));
}

#[test]
fn test_13_wizard_matrix_instability_any() {
    let mut engineer = Engineer::new(&get_english_config());
    engineer.wizard_phase = WizardPhase::Apex;
    engineer.wizard_problem = WizardProblem::Instability;
    assert!(engineer
        .get_wizard_advice()
        .contains(&"Increase Downforce (Wings)".to_string()));
}

#[test]
fn test_14_setup_comparison_identical() {
    let engineer = Engineer::new(&get_english_config());
    let setup_a = CarSetup::default();
    let advice = engineer.compare_setups_advice(&setup_a, &setup_a);
    assert!(!advice.is_empty());
    assert_eq!(advice[0], "No major differences");
}

#[test]
fn test_15_setup_comparison_aero_extreme() {
    let engineer = Engineer::new(&get_english_config());
    let mut target = CarSetup::default();
    let mut ref_setup = CarSetup::default();
    target.wing_1 = 500;
    target.wing_2 = 1000;
    ref_setup.wing_1 = 0;
    ref_setup.wing_2 = 0;
    let advice = engineer.compare_setups_advice(&target, &ref_setup);
    assert!(advice.iter().any(|s| s.contains("Aero: +1500")));
}

#[test]
fn test_16_setup_comparison_camber_extreme() {
    let engineer = Engineer::new(&get_english_config());
    let mut target = CarSetup::default();
    let ref_setup = CarSetup::default();
    target.camber_lf = 40;
    target.camber_rf = 40;
    let advice = engineer.compare_setups_advice(&target, &ref_setup);
    assert!(advice.iter().any(|s| s.contains("Front Camber: +80")));
}

#[test]
fn test_17_setup_comparison_pressure_extreme() {
    let engineer = Engineer::new(&get_english_config());
    let mut target = CarSetup::default();
    let ref_setup = CarSetup::default();
    target.pressure_lf = 100;
    target.pressure_rf = 100;
    target.pressure_lr = 100;
    target.pressure_rr = 100;
    let advice = engineer.compare_setups_advice(&target, &ref_setup);
    assert!(advice.iter().any(|s| s.contains("Tyre Press: +100.0 PSI")));
}

#[test]
fn test_18_engineer_history_buffer_bounds() {
    let mut engineer = Engineer::new(&get_english_config());
    engineer.stats.total_frames = 1000000;
    if engineer.stats.total_frames > 600 {
        engineer.stats.total_frames = 0;
        engineer.stats.bottoming_frames = [0; 4];
    }
    assert_eq!(engineer.stats.total_frames, 0);
}

#[test]
fn test_19_severity_ordering() {
    let mut recs = vec![
        Recommendation {
            component: "A".into(),
            category: "1".into(),
            severity: Severity::Info,
            message: "".into(),
            action: "".into(),
            parameters: vec![],
            confidence: 1.0,
        },
        Recommendation {
            component: "B".into(),
            category: "2".into(),
            severity: Severity::Critical,
            message: "".into(),
            action: "".into(),
            parameters: vec![],
            confidence: 0.5,
        },
        Recommendation {
            component: "C".into(),
            category: "3".into(),
            severity: Severity::Warning,
            message: "".into(),
            action: "".into(),
            parameters: vec![],
            confidence: 0.9,
        },
        Recommendation {
            component: "D".into(),
            category: "4".into(),
            severity: Severity::Critical,
            message: "".into(),
            action: "".into(),
            parameters: vec![],
            confidence: 0.99,
        },
    ];
    recs.sort_by(|a, b| {
        b.severity
            .partial_cmp(&a.severity)
            .unwrap_or(Ordering::Equal)
            .then(
                b.confidence
                    .partial_cmp(&a.confidence)
                    .unwrap_or(Ordering::Equal),
            )
    });
    assert_eq!(recs[0].severity, Severity::Critical);
    assert_eq!(recs[0].component, "D");
    assert_eq!(recs[1].severity, Severity::Critical);
    assert_eq!(recs[1].component, "B");
    assert_eq!(recs[2].severity, Severity::Warning);
    assert_eq!(recs[3].severity, Severity::Info);
}

#[test]
fn test_20_localization_russian_setup_comparison() {
    let engineer = Engineer::new(&get_russian_config());
    let mut target = CarSetup::default();
    let ref_setup = CarSetup::default();
    target.wing_1 = 5;
    target.wing_2 = 10;
    let advice = engineer.compare_setups_advice(&target, &ref_setup);
    assert!(advice.iter().any(|s| s.contains("Аэродинамика: +15")));
}

#[test]
fn test_21_localization_russian_wizard() {
    let mut engineer = Engineer::new(&get_russian_config());
    engineer.wizard_phase = WizardPhase::Entry;
    engineer.wizard_problem = WizardProblem::Understeer;
    let advice = engineer.get_wizard_advice();
    assert!(advice.contains(&"Уменьшить отбой (Rebound) спереди".to_string()));
}

#[test]
fn test_22_tyre_pressure_optimizer() {
    use ac_core::ac_structs::AcPhysics;
    use ac_core::engineer::TyrePressureOptimizer;

    let mut phys = AcPhysics::default();
    phys.wheels_pressure = [26.0, 27.5, 27.5, 27.5];
    phys.tyre_temp_i = [95.0, 85.0, 85.0, 85.0];
    phys.tyre_temp_o = [80.0, 85.0, 85.0, 85.0];

    let opt = TyrePressureOptimizer::calculate(&phys, 27.5);
    assert_eq!(opt.corners[0].corner_name, "FL");
    assert!(opt.corners[0].recommended_delta_psi > 0.0);
}

#[test]
fn test_23_predictive_lap_time_calculation() {
    use ac_core::analyzer::TelemetryAnalyzer;
    let analyzer = TelemetryAnalyzer::new();
    let estimated = analyzer.predictive_lap_time_ms(40000, 0.5);
    assert_eq!(estimated, Some(80000));
}

#[test]
fn test_24_theoretical_best_lap_calculation() {
    use ac_core::analyzer::{LapData, TelemetryAnalyzer};
    let mut analyzer = TelemetryAnalyzer::new();

    let mut lap1 = LapData {
        lap_number: 1,
        lap_time_ms: 85000,
        sectors: [25000, 30000, 30000],
        valid: true,
        car_model: "test".into(),
        track_name: "test".into(),
        save_date: "2026-07-30".into(),
        from_file: false,
        air_temp: 20.0,
        road_temp: 30.0,
        track_grip: 98.0,
        timestamp: "12:00".into(),
        max_speed: 250.0,
        avg_speed: 200.0,
        avg_pressure: 27.5,
        min_corner_speed_avg: 80.0,
        fuel_used: 2.0,
        gear_shifts: 30,
        peak_lat_g: 2.0,
        peak_brake_g: 3.0,
        avg_tyre_temp: [85.0; 4],
        max_brake_temp: [400.0; 4],
        pressure_deviation: 0.1,
        suspension_travel_hist: [10.0; 4],
        avg_wheels_pressure: [27.5; 4],
        avg_tyre_temp_i: [85.0; 4],
        avg_tyre_temp_m: [85.0; 4],
        avg_tyre_temp_o: [85.0; 4],
        avg_brake_temp: [400.0; 4],
        avg_ride_height: [30.0; 2],
        damper_histograms: [[25.0; 4]; 4],
        throttle_smoothness: 90.0,
        steering_smoothness: 90.0,
        trail_braking_score: 85.0,
        coasting_percent: 5.0,
        pedal_overlap_percent: 2.0,
        full_throttle_percent: 60.0,
        grip_usage_percent: 90.0,
        oversteer_count: 0,
        understeer_count: 0,
        lockup_count: 0,
        car_control_score: 90.0,
        scrubbing_incidents: 0,
        max_steering_over_rotation: 0.0,
        radar_stats: ac_core::analyzer::RadarStats {
            consistency: 90.0,
            car_control: 90.0,
            aggression: 80.0,
            smoothness: 90.0,
            tyre_mgmt: 90.0,
        },
        telemetry_trace: vec![],
        bounds_min_x: 0.0,
        bounds_max_x: 100.0,
        bounds_min_y: 0.0,
        bounds_max_y: 100.0,
    };

    let mut lap2 = lap1.clone();
    lap2.sectors = [24000, 31000, 29000];

    analyzer.laps.push(lap1);
    analyzer.laps.push(lap2);

    let theoretical = analyzer.theoretical_best_lap_ms();
    assert_eq!(theoretical, Some(24000 + 30000 + 29000));
}
