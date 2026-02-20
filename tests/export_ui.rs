use liveplot::panels::ExportPanel;

#[test]
fn export_menu_labels_include_icons() {
    assert_eq!(ExportPanel::SNAPSHOT_CSV_LABEL, "🖹 Snapshot as CSV");
    assert_eq!(ExportPanel::SAVE_STATE_LABEL, "📂 Save state...");
    assert_eq!(ExportPanel::LOAD_STATE_LABEL, "📂 Load state...");
}
