use liveplot::data::hotkeys::*;

#[test]
fn collapse_when_width_is_strictly_less() {
    assert!(should_collapse_topbar(99.9, 100.0));
}

#[test]
fn no_collapse_when_width_equals_required() {
    assert!(!should_collapse_topbar(100.0, 100.0));
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
