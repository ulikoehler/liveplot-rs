use egui_phosphor_icons::icons::{EYE, PLUS_SQUARE};
use liveplot::panels::thresholds_ui::{NEW_LABEL, SHOW_THRESHOLDS_LABEL};

#[test]
fn thresholds_menu_labels_include_icons() {
    assert_eq!(
        &*SHOW_THRESHOLDS_LABEL,
        &format!("{} Show Thresholds", EYE.as_str())
    );
    assert_eq!(&*NEW_LABEL, &format!("{} New", PLUS_SQUARE.as_str()));
}
