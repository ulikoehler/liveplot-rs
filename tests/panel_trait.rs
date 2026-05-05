use liveplot::data::hotkeys::HotkeyName;
use liveplot::panels::Panel;
use liveplot::panels::*;

fn collapsed_label_for(p: &impl Panel) -> String {
    p.icon_only()
        .map(|s| s.to_string())
        .unwrap_or_else(|| p.title().to_string())
}

#[test]
fn traces_panel_hotkey_name_is_traces() {
    let p = TracesPanel::default();
    assert_eq!(p.hotkey_name(), Some(HotkeyName::Traces));
}

#[test]
fn collapsed_label_uses_icon_when_available() {
    let p = TracesPanel::default();
    let label = collapsed_label_for(&p);
    assert_eq!(label, p.icon_only().unwrap().to_string());
    assert!(!label.contains(p.title()));
}

#[test]
fn full_label_contains_both_icon_and_title() {
    let p = TracesPanel::default();
    let label = p.title_and_icon();
    assert!(label.contains(p.title()));
    assert!(label.contains(p.icon_only().unwrap()));
}
