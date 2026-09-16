use liveplot::data::hotkeys::*;

#[test]
fn collapse_when_width_is_strictly_less() {
    assert!(should_collapse_topbar(99.9, 100.0));
}

#[test]
fn no_collapse_when_width_equals_required() {
    // Boundary is at available == required + 80px buffer
    assert!(!should_collapse_topbar(180.0, 100.0));
}

#[test]
fn tooltip_with_no_hotkey_returns_description_only() {
    let text = format_button_tooltip("Clear All", None);
    assert_eq!(text, "Clear All");
}

#[test]
fn tooltip_with_ctrl_hotkey() {
    let hk = Hotkey::new(Modifier::Ctrl, 'M');
    let text = format_button_tooltip("Math", Some(&hk));
    assert_eq!(text, "Math [Ctrl+M]");
}

#[test]
fn get_hotkey_traces_default() {
    let hk = Hotkeys::default();
    let result = get_hotkey_for_name(&hk, HotkeyName::Traces);
    assert!(result.is_some());
    let result = result.unwrap();
    assert_eq!(result.key, 'T');
    assert_eq!(result.modifier, Modifier::None);
}

#[test]
fn get_hotkey_pause_default() {
    let hk = Hotkeys::default();
    let result = get_hotkey_for_name(&hk, HotkeyName::Pause);
    assert!(result.is_some());
    let result = result.unwrap();
    assert_eq!(result.key, 'P');
    assert_eq!(result.modifier, Modifier::None);
}

#[test]
fn get_hotkey_thresholds_default() {
    let hk = Hotkeys::default();
    let result = get_hotkey_for_name(&hk, HotkeyName::Thresholds);
    assert!(result.is_some());
    let result = result.unwrap();
    assert_eq!(result.key, 'T');
    assert_eq!(result.modifier, Modifier::Ctrl);
}

#[test]
fn get_hotkey_measurements_default() {
    let hk = Hotkeys::default();
    let result = get_hotkey_for_name(&hk, HotkeyName::Measurements);
    assert!(result.is_some());
    let result = result.unwrap();
    assert_eq!(result.key, 'P');
    assert_eq!(result.modifier, Modifier::None);
}

#[test]
fn get_hotkey_triggers_default() {
    let hk = Hotkeys::default();
    let result = get_hotkey_for_name(&hk, HotkeyName::Triggers);
    assert!(result.is_some());
    let result = result.unwrap();
    assert_eq!(result.key, 'G');
    assert_eq!(result.modifier, Modifier::Alt);
}

#[test]
fn get_hotkey_fft_default() {
    let hk = Hotkeys::default();
    let result = get_hotkey_for_name(&hk, HotkeyName::Fft);
    assert!(result.is_some());
    let result = result.unwrap();
    assert_eq!(result.key, 'F');
    assert_eq!(result.modifier, Modifier::Ctrl);
}

#[test]
fn get_hotkey_hotkeys_panel_default() {
    let hk = Hotkeys::default();
    let result = get_hotkey_for_name(&hk, HotkeyName::HotkeysPanel);
    assert!(result.is_some());
    let result = result.unwrap();
    assert_eq!(result.key, 'H');
    assert_eq!(result.modifier, Modifier::Ctrl);
}

#[test]
fn get_hotkey_returns_none_when_unset() {
    let hk = Hotkeys {
        traces: None,
        ..Default::default()
    };
    let result = get_hotkey_for_name(&hk, HotkeyName::Traces);
    assert!(result.is_none(), "Should return None when hotkey is unset");
}

#[test]
fn get_hotkey_save_png_default() {
    let hk = Hotkeys::default();
    let result = get_hotkey_for_name(&hk, HotkeyName::SavePng);
    assert!(result.is_some());
    let result = result.unwrap();
    assert_eq!(result.key, 'S');
    assert_eq!(result.modifier, Modifier::None);
}

#[test]
fn get_hotkey_export_data_default() {
    let hk = Hotkeys::default();
    let result = get_hotkey_for_name(&hk, HotkeyName::ExportData);
    assert!(result.is_some());
    let result = result.unwrap();
    assert_eq!(result.key, 'E');
    assert_eq!(result.modifier, Modifier::None);
}

// ── tooltip round-trip integration ──────────────────────────────────────

#[test]
fn tooltip_round_trip_traces() {
    let hk = Hotkeys::default();
    let hotkey = get_hotkey_for_name(&hk, HotkeyName::Traces);
    let tooltip = format_button_tooltip("Traces", hotkey);
    assert_eq!(tooltip, "Traces [T]");
}

#[test]
fn tooltip_round_trip_clear_all() {
    let hk = Hotkeys::default();
    let hotkey = get_hotkey_for_name(&hk, HotkeyName::ClearAll);
    let tooltip = format_button_tooltip("Clear All", hotkey);
    assert_eq!(tooltip, "Clear All [Ctrl+X]");
}

#[test]
fn tooltip_round_trip_math() {
    let hk = Hotkeys::default();
    let hotkey = get_hotkey_for_name(&hk, HotkeyName::Math);
    let tooltip = format_button_tooltip("Math", hotkey);
    assert_eq!(tooltip, "Math [Ctrl+M]");
}

#[test]
fn tooltip_round_trip_unset_hotkey() {
    let hk = Hotkeys {
        math: None,
        ..Default::default()
    };
    let hotkey = get_hotkey_for_name(&hk, HotkeyName::Math);
    let tooltip = format_button_tooltip("Math", hotkey);
    assert_eq!(
        tooltip, "Math",
        "When hotkey is unset, tooltip should be description only"
    );
}

// ── collapse + tooltip combined decision ────────────────────────────────

#[test]
fn collapse_decision_wide_window() {
    // 1920-pixel-wide window with a 100px rightmost-button requirement
    assert!(!should_collapse_topbar(1920.0, 100.0));
}

#[test]
fn collapse_decision_very_narrow_window() {
    // Phone-sized or small embedded widget: always collapse
    assert!(should_collapse_topbar(50.0, 100.0));
}

#[test]
fn collapse_decision_exactly_at_boundary() {
    // Exactly at the boundary (available == required + 80px buffer): should NOT collapse
    assert!(!should_collapse_topbar(180.0, 100.0));
}

#[test]
fn collapse_decision_one_pixel_short() {
    // One floating-point unit below the threshold: should collapse
    assert!(should_collapse_topbar(179.999_98, 100.0));
}
