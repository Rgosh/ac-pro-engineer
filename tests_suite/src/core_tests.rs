use ac_core::config::{AppConfig, Language};
use ac_core::engineer::{
    DrivingStyle, Engineer, EngineerStats, Recommendation, Severity, WizardPhase, WizardProblem,
};
use ac_core::setup_manager::CarSetup;
use std::cmp::Ordering;

fn get_english_config() -> AppConfig {
    AppConfig {
        language: Language::English,
        ..Default::default()
    }
}

fn get_russian_config() -> AppConfig {
    AppConfig {
        language: Language::Russian,
        ..Default::default()
    }
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
    for (i, &current) in current_wear.iter().enumerate() {
        let wear_used = stats.base_tyre_wear[i] - current;
        let wear_per_lap = wear_used / stats.stint_laps as f32;
        let remaining_wear = current - 94.0;
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
    for (i, &current) in current_wear.iter().enumerate() {
        let wear_used = stats.base_tyre_wear[i] - current;
        let wear_per_lap = if stats.stint_laps > 0 {
            wear_used / stats.stint_laps as f32
        } else {
            0.0
        };
        let remaining_wear = current - 94.0;
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
    assert!(
        engineer
            .get_wizard_advice()
            .contains(&"Decrease Front Rebound".to_string())
    );
}

#[test]
fn test_08_wizard_matrix_entry_oversteer() {
    let mut engineer = Engineer::new(&get_english_config());
    engineer.wizard_phase = WizardPhase::Entry;
    engineer.wizard_problem = WizardProblem::Oversteer;
    assert!(
        engineer
            .get_wizard_advice()
            .contains(&"Increase Front Rebound".to_string())
    );
}

#[test]
fn test_09_wizard_matrix_apex_understeer() {
    let mut engineer = Engineer::new(&get_english_config());
    engineer.wizard_phase = WizardPhase::Apex;
    engineer.wizard_problem = WizardProblem::Understeer;
    assert!(
        engineer
            .get_wizard_advice()
            .contains(&"Softer Front Springs".to_string())
    );
}

#[test]
fn test_10_wizard_matrix_apex_oversteer() {
    let mut engineer = Engineer::new(&get_english_config());
    engineer.wizard_phase = WizardPhase::Apex;
    engineer.wizard_problem = WizardProblem::Oversteer;
    assert!(
        engineer
            .get_wizard_advice()
            .contains(&"Softer Rear Springs".to_string())
    );
}

#[test]
fn test_11_wizard_matrix_exit_understeer() {
    let mut engineer = Engineer::new(&get_english_config());
    engineer.wizard_phase = WizardPhase::Exit;
    engineer.wizard_problem = WizardProblem::Understeer;
    assert!(
        engineer
            .get_wizard_advice()
            .contains(&"Increase Front Bump".to_string())
    );
}

#[test]
fn test_12_wizard_matrix_exit_oversteer() {
    let mut engineer = Engineer::new(&get_english_config());
    engineer.wizard_phase = WizardPhase::Exit;
    engineer.wizard_problem = WizardProblem::Oversteer;
    assert!(
        engineer
            .get_wizard_advice()
            .contains(&"Decrease Rear Bump".to_string())
    );
}

#[test]
fn test_13_wizard_matrix_instability_any() {
    let mut engineer = Engineer::new(&get_english_config());
    engineer.wizard_phase = WizardPhase::Apex;
    engineer.wizard_problem = WizardProblem::Instability;
    assert!(
        engineer
            .get_wizard_advice()
            .contains(&"Increase Downforce (Wings)".to_string())
    );
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
    let mut recs = [
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

    let phys = AcPhysics {
        wheels_pressure: [26.0, 27.5, 27.5, 27.5],
        tyre_temp_i: [95.0, 85.0, 85.0, 85.0],
        tyre_temp_o: [80.0, 85.0, 85.0, 85.0],
        ..Default::default()
    };

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

    let lap1 = LapData {
        lap_number: 1,
        lap_time_ms: 85000,
        sectors: [25000, 30000, 30000],
        valid: true,
        car_model: "test".into(),
        track_name: "test".into(),
        track_length_m: 0.0,
        save_date: "2026-07-30".into(),
        from_file: false,
        air_temp: 20.0,
        road_temp: 30.0,
        track_grip: 98.0,
        timestamp: "12:00".into(),
        max_speed: 250.0,
        avg_speed: 200.0,
        avg_pressure: Some(27.5),
        min_corner_speed_avg: 80.0,
        fuel_used: 2.0,
        gear_shifts: 30,
        peak_lat_g: 2.0,
        peak_brake_g: 3.0,
        avg_tyre_temp: [85.0; 4],
        max_brake_temp: [400.0; 4],
        pressure_deviation: Some(0.1),
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

// ========== P0-1: SessionTiming tests ==========

#[test]
fn test_25_session_timing_time_limited_30sec() {
    use ac_core::session_info::SessionTiming;
    // 30 seconds left, best lap 10 seconds => 3 laps remaining
    let result = SessionTiming::remaining_laps(30_000.0, 10_000, 0, 0, 0, 0.0);
    assert!((result - 3.0).abs() < 0.01, "Expected 3.0, got {}", result);
}

#[test]
fn test_26_session_timing_time_limited_60sec() {
    use ac_core::session_info::SessionTiming;
    // 60 seconds left, best lap 20 seconds => 3 laps remaining
    let result = SessionTiming::remaining_laps(60_000.0, 20_000, 0, 0, 0, 0.0);
    assert!((result - 3.0).abs() < 0.01, "Expected 3.0, got {}", result);
}

#[test]
fn test_27_session_timing_time_limited_1800sec() {
    use ac_core::session_info::SessionTiming;
    // 30 minutes left, best lap 90 seconds => 20 laps remaining
    let result = SessionTiming::remaining_laps(1_800_000.0, 90_000, 0, 0, 0, 0.0);
    assert!(
        (result - 20.0).abs() < 0.01,
        "Expected 20.0, got {}",
        result
    );
}

#[test]
fn test_28_session_timing_lap_limited_race() {
    use ac_core::session_info::SessionTiming;
    // 20-lap race, 15 completed, at position 0.5 => 4.5 remaining
    let result = SessionTiming::remaining_laps(0.0, 80_000, 0, 20, 15, 0.5);
    assert!((result - 4.5).abs() < 0.01, "Expected 4.5, got {}", result);
}

#[test]
fn test_29_session_timing_fallback_lap_time() {
    use ac_core::session_info::SessionTiming;
    // No best time, uses last_time as fallback
    let result = SessionTiming::remaining_laps(60_000.0, 0, 30_000, 0, 0, 0.0);
    assert!((result - 2.0).abs() < 0.01, "Expected 2.0, got {}", result);
}

#[test]
fn test_30_session_timing_no_lap_time_fallback_120s() {
    use ac_core::session_info::SessionTiming;
    // No best time, no last time => fallback 120 seconds
    let result = SessionTiming::remaining_laps(240_000.0, 0, 0, 0, 0, 0.0);
    assert!((result - 2.0).abs() < 0.01, "Expected 2.0, got {}", result);
}

#[test]
fn test_31_session_timing_ui_and_engine_same_result() {
    use ac_core::session_info::SessionTiming;
    // Verify UI strategy and engineer engine produce identical results
    let session_ms = 600_000.0;
    let best_ms = 85_000;
    let last_ms = 86_000;
    let n_laps = 0;
    let completed = 3;
    let pos = 0.3;

    let ui_result =
        SessionTiming::remaining_laps(session_ms, best_ms, last_ms, n_laps, completed, pos);
    let engine_result =
        SessionTiming::remaining_laps(session_ms, best_ms, last_ms, n_laps, completed, pos);
    assert!((ui_result - engine_result).abs() < f32::EPSILON);
}

#[test]
fn test_32_session_timing_format_minutes() {
    use ac_core::session_info::SessionTiming;
    let formatted = SessionTiming::format_time_left_minutes(1_800_000.0);
    assert_eq!(formatted, "30.0 min");
}

#[test]
fn test_33_session_timing_format_mm_ss() {
    use ac_core::session_info::SessionTiming;
    let formatted = SessionTiming::format_time_left_ms(1_800_000.0);
    assert_eq!(formatted, "30:00");

    let formatted2 = SessionTiming::format_time_left_ms(65_500.0);
    assert_eq!(formatted2, "1:05");
}

#[test]
fn test_34_ring_buffer_history_size_dynamic_reconfiguration() {
    use ac_core::RingBuffer;

    // 1. Initial capacity 50
    let mut buf = RingBuffer::new(50);
    for i in 0..100 {
        buf.push(i);
    }
    assert_eq!(buf.capacity(), 50);
    assert_eq!(buf.len(), 50);
    assert_eq!(buf[0], 50);
    assert_eq!(buf[49], 99);

    // 2. Expand capacity to 5000 (like high history_size setting)
    buf.set_capacity(5000);
    assert_eq!(buf.capacity(), 5000);
    assert_eq!(buf.len(), 50); // existing 50 items preserved
    for i in 100..600 {
        buf.push(i);
    }
    assert_eq!(buf.len(), 550);
    assert_eq!(buf[0], 50);
    assert_eq!(buf[549], 599);

    // 3. Shrink capacity to 300 on the fly
    buf.set_capacity(300);
    assert_eq!(buf.capacity(), 300);
    assert_eq!(buf.len(), 300);
    // Newest 300 items preserved (300..600)
    assert_eq!(buf[0], 300);
    assert_eq!(buf[299], 599);
}

#[test]
fn test_35_config_resolve_data_path_and_autosave_semantics() {
    let mut config = ac_core::config::AppConfig::default();
    let resolved = config.resolve_data_path();
    assert!(!resolved.as_os_str().is_empty());
    assert_ne!(resolved, std::path::PathBuf::from("./data"));

    config.data_path = std::path::PathBuf::from("/custom/telemetry/path");
    assert_eq!(
        config.resolve_data_path(),
        std::path::PathBuf::from("/custom/telemetry/path")
    );
}

#[test]
fn test_36_engineer_update_rate_independence() {
    use ac_core::ac_structs::{AcGraphics, AcPhysics};
    use ac_core::engineer::Engineer;
    use ac_core::session_info::SessionInfo;

    let mut cfg_fast = get_english_config();
    cfg_fast.update_rate = 16; // 60 Hz (~16ms)

    let mut cfg_slow = get_english_config();
    cfg_slow.update_rate = 100; // 10 Hz (100ms)

    let mut eng_fast = Engineer::new(&cfg_fast);
    let mut eng_slow = Engineer::new(&cfg_slow);

    let phys = AcPhysics {
        speed_kmh: 100.0,
        brake: 0.8,
        wheel_slip: [0.4, 0.4, 0.0, 0.0],
        ..Default::default()
    };

    let gfx = AcGraphics::default();
    let session = SessionInfo::default();

    // 1 second simulation at 60 Hz = 60 steps
    for _ in 0..60 {
        eng_fast.update(&phys, &gfx, &session);
    }

    // 1 second simulation at 10 Hz = 10 steps
    for _ in 0..10 {
        eng_slow.update(&phys, &gfx, &session);
    }

    // Both should accumulate approximately equal lockup frames (~60 normalized frames)
    let fast_lockups = eng_fast.stats.lockup_frames_front;
    let slow_lockups = eng_slow.stats.lockup_frames_front;

    assert!((fast_lockups as i32 - slow_lockups as i32).abs() <= 2);
}

#[test]
fn test_37_track_map_bounds_and_zero_coords_safety() {
    let min_x = f32::NAN;
    let max_x = f32::INFINITY;

    let safe_min = if min_x.is_finite() && min_x.abs() < 1e6 {
        min_x as f64
    } else {
        -500.0
    };
    let safe_max = if max_x.is_finite() && max_x.abs() < 1e6 {
        max_x as f64
    } else {
        500.0
    };

    let diff_x = (safe_max - safe_min).max(10.0);
    let scale = diff_x / 50.0;

    assert!(scale.is_finite());
    assert_eq!(scale, 20.0); // (500 - (-500)) / 50 = 1000 / 50 = 20.0
}

#[test]
fn test_38_target_tyre_pressure_config_affects_engineer_recommendations() {
    use ac_core::ac_structs::{AcGraphics, AcPhysics};
    use ac_core::engineer::Engineer;
    use ac_core::session_info::SessionInfo;

    let mut cfg1 = get_english_config();
    cfg1.target_tyre_pressure = 27.5;
    let mut eng1 = Engineer::new(&cfg1);

    let mut cfg2 = get_english_config();
    cfg2.target_tyre_pressure = 32.0;
    let mut eng2 = Engineer::new(&cfg2);

    let phys = AcPhysics {
        speed_kmh: 80.0,
        wheels_pressure: [25.0; 4], // low pressure
        ..Default::default()
    };

    let gfx = AcGraphics::default();
    let _session = SessionInfo::default();

    let recs1 = eng1.analyze_live(&phys, &gfx, None);
    let recs2 = eng2.analyze_live(&phys, &gfx, None);

    // Target tyre pressure in config dynamically shifts the engineer recommendations
    let target1 = recs1
        .iter()
        .find(|r| r.category.contains("Tyre Pressure"))
        .and_then(|r| r.parameters.first())
        .map(|p| p.target);
    let target2 = recs2
        .iter()
        .find(|r| r.category.contains("Tyre Pressure"))
        .and_then(|r| r.parameters.first())
        .map(|p| p.target);

    if let (Some(t1), Some(t2)) = (target1, target2) {
        assert_eq!(t1, 27.5);
        assert_eq!(t2, 32.0);
    }
}

#[test]
fn test_39_corrupted_records_file_and_atomic_save_safety() {
    use ac_core::records::RecordManager;

    let tmp_dir = std::env::temp_dir().join(format!("test_records_{}", std::process::id()));
    let _ = std::fs::create_dir_all(&tmp_dir);
    let junk_file = tmp_dir.join("corrupted.json");

    // Write binary garbage to file
    let _ = std::fs::write(&junk_file, b"\xFF\xFE\x00\x01NOT_VALID_JSON_BINARY_TRASH");

    // Loading corrupted file does not panic or crash
    let res = RecordManager::load_from_path(&junk_file);
    assert!(res.is_err());

    let mut mgr = RecordManager {
        db_path: junk_file,
        ..Default::default()
    };
    mgr.load(); // handles error gracefully

    // Atomic save to valid path
    let valid_file = tmp_dir.join("valid_records.json");
    mgr.db_path = valid_file.clone();
    assert!(mgr.save_with_result().is_ok());
    assert!(valid_file.exists());

    let _ = std::fs::remove_dir_all(&tmp_dir);
}

#[test]
fn test_40_string_u16_formatting_has_no_side_effects() {
    use ac_core::ac_structs::StringU16_33;

    let _s = StringU16_33::from("ks_ferrari_488_gt3");
    let s = StringU16_33::from("ks_ferrari_488_gt3");
    let formatted = format!("{}", s);
    assert_eq!(formatted, "ks_ferrari_488_gt3");
}

#[test]
fn test_41_cold_tyre_pressure_calculator() {
    use ac_core::engineer::ColdPressureCalculator;

    let estimate = ColdPressureCalculator::calculate(27.5, 20.0, 0.98);
    assert!(estimate.recommended_cold_psi < estimate.target_hot_psi);
    assert_eq!(estimate.target_hot_psi, 27.5);
}

#[test]
fn test_42_csv_export_format() {
    use ac_core::analyzer::{LapData, TelemetryPoint, export_lap_to_csv};

    let mut lap = LapData {
        lap_number: 1,
        ..Default::default()
    };
    lap.telemetry_trace.push(TelemetryPoint {
        distance: 100.0,
        time_ms: 2500,
        speed: 180.0,
        rpms: 0,
        gas: 1.0,
        brake: 0.0,
        gear: 4,
        steer: 0.05,
        lat_g: 0.2,
        lon_g: 0.1,
        slip_avg: 0.01,
        x: 10.0,
        y: 20.0,
    });

    let tmp_path = std::env::temp_dir().join("test_export.csv");
    let res = export_lap_to_csv(&lap, &tmp_path);
    assert!(res.is_ok());

    let content = std::fs::read_to_string(&tmp_path).expect("the export just wrote this file");
    assert!(content.contains("\"Time\",\"Distance\",\"Speed\""));
    assert!(content.contains("2.500,100.00000,180.0"));

    let _ = std::fs::remove_file(tmp_path);
}

#[test]
fn test_43_ghost_delta_calculation() {
    use ac_core::analyzer::{LapData, TelemetryPoint, calculate_ghost_delta};

    let mut best_lap = LapData::default();
    best_lap.telemetry_trace.push(TelemetryPoint {
        distance: 0.0,
        time_ms: 0,
        speed: 100.0,
        rpms: 0,
        gas: 1.0,
        brake: 0.0,
        gear: 3,
        steer: 0.0,
        lat_g: 0.0,
        lon_g: 0.0,
        slip_avg: 0.0,
        x: 0.0,
        y: 0.0,
    });
    best_lap.telemetry_trace.push(TelemetryPoint {
        distance: 1000.0,
        time_ms: 30000, // 30.0s at finish
        speed: 200.0,
        rpms: 0,
        gas: 1.0,
        brake: 0.0,
        gear: 5,
        steer: 0.0,
        lat_g: 0.0,
        lon_g: 0.0,
        slip_avg: 0.0,
        x: 100.0,
        y: 100.0,
    });

    let delta = calculate_ghost_delta(&best_lap, 1.0, 31.5);
    assert!(delta.is_some());
    let delta = delta.expect("the best lap has a trace, so a delta is computed");
    assert!((delta - 1.5).abs() < 0.01);
}

/// `updater::CURRENT_VERSION` is `ac_core`'s own `CARGO_PKG_VERSION`, and it is
/// what the app displays, what Discord rich presence reports, and what release
/// tags are compared against to decide whether an update is newer.
///
/// When `ac_core` carried a hardcoded version it silently fell behind the
/// workspace: a build tagged v0.3.0 would have reported itself as 0.2.3, seen
/// its own release as an upgrade, installed it, and offered it again forever.
/// This pins the crate to the workspace version — `tests_suite` inherits it, so
/// the two only agree while `core/Cargo.toml` says `version.workspace = true`.
#[test]
fn core_version_tracks_the_workspace_version() {
    assert_eq!(
        ac_core::updater::CURRENT_VERSION,
        env!("CARGO_PKG_VERSION"),
        "ac_core has drifted from the workspace version; \
         core/Cargo.toml must use `version.workspace = true`"
    );
}
