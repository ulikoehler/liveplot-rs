use egui_phosphor_icons::icons::{FILE_TEXT, FOLDER_OPEN};
use liveplot::panels::export_ui::{LOAD_STATE_LABEL, SAVE_STATE_LABEL, SNAPSHOT_CSV_LABEL};

#[test]
fn export_menu_labels_include_icons() {
    assert_eq!(
        &*SNAPSHOT_CSV_LABEL,
        &format!("{} Snapshot as CSV", FILE_TEXT.as_str())
    );
    assert_eq!(
        &*SAVE_STATE_LABEL,
        &format!("{} Save state...", FOLDER_OPEN.as_str())
    );
    assert_eq!(
        &*LOAD_STATE_LABEL,
        &format!("{} Load state...", FOLDER_OPEN.as_str())
    );
}
