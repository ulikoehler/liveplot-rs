use liveplot::panels::ThresholdsPanel;

#[test]
fn thresholds_menu_labels_include_icons() {
    assert_eq!(ThresholdsPanel::SHOW_THRESHOLDS_LABEL, "👁 Show Thresholds");
    assert_eq!(ThresholdsPanel::NEW_LABEL, "⊞ New");
}
