use ac_core::overlay::{OverlayManager, OverlayMode};

#[test]
fn test_01_overlay_state_defaults() {
    let manager = OverlayManager::new(OverlayMode::External);
    let state = manager.get_state();

    assert_eq!(state.speed_kmh, 0);
    assert_eq!(state.gear, 0);
    assert_eq!(state.rpm, 0);
    assert_eq!(state.max_rpm, 8500);
    assert!(!state.is_pit_limiter_on);
    assert!(state.show_telemetry);
    assert!(state.show_engineer);
    assert!(state.engineer_messages.is_empty());
}

#[test]
fn test_02_overlay_modes_initialization() {
    let manager_ext = OverlayManager::new(OverlayMode::External);
    assert_eq!(manager_ext.mode, OverlayMode::External);

    let manager_vr = OverlayManager::new(OverlayMode::VR);
    assert_eq!(manager_vr.mode, OverlayMode::VR);

    let manager_test = OverlayManager::new(OverlayMode::StandaloneTest);
    assert_eq!(manager_test.mode, OverlayMode::StandaloneTest);
}

#[test]
#[cfg(target_os = "windows")]
fn test_03_overlay_native_desktop_windows_init() {
    let manager = OverlayManager::new(OverlayMode::NativeDesktop);
    assert_eq!(manager.mode, OverlayMode::NativeDesktop);
}

#[test]
fn test_04_overlay_manual_state_mutation() {
    let mut manager = OverlayManager::new(OverlayMode::External);

    manager.state.rpm = 7500;
    manager.state.max_rpm = 8500;
    manager.state.gear = 4;
    manager.state.speed_kmh = 210;
    manager.state.is_pit_limiter_on = true;
    manager.state.show_telemetry = false;
    manager.state.show_engineer = false;
    manager.state.engineer_messages = vec!["Test Msg".to_string()];

    let state = manager.get_state();

    assert_eq!(state.rpm, 7500);
    assert_eq!(state.max_rpm, 8500);
    assert_eq!(state.gear, 4);
    assert_eq!(state.speed_kmh, 210);
    assert!(state.is_pit_limiter_on);
    assert!(!state.show_telemetry);
    assert!(!state.show_engineer);
    assert_eq!(state.engineer_messages[0], "Test Msg");
}

#[test]
fn test_05_overlay_redline_logic_bounds() {
    let mut manager = OverlayManager::new(OverlayMode::External);
    manager.state.max_rpm = 8500;
    manager.state.rpm = 8200;

    let is_redline = manager.state.rpm > (manager.state.max_rpm - 500);
    assert!(is_redline);

    manager.state.rpm = 7000;
    let is_redline_safe = manager.state.rpm > (manager.state.max_rpm - 500);
    assert!(!is_redline_safe);
}
