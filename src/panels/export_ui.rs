use egui::Ui;
use super::panel_trait::{Panel, PanelState};
use crate::data::DataContext;

pub struct ExportPanel { pub state: PanelState }
impl Default for ExportPanel { fn default() -> Self { Self { state: PanelState { visible: false, detached: false } } } }
impl Panel for ExportPanel {
    fn name(&self) -> &'static str { "Export" }
    fn state(&self) -> &PanelState { &self.state }
    fn state_mut(&mut self) -> &mut PanelState { &mut self.state }

    // As requested, provide menu-only actions; no render_panel body needed
    fn render_menu(&mut self, ui: &mut Ui, data: &mut DataContext) {
        ui.menu_button("Export", |ui| {
            if ui.button("Snapshot as CSV").clicked() {
                if let Some(path) = rfd::FileDialog::new().set_file_name("snapshot.csv").add_filter("CSV", &["csv"]).save_file() {
                    if let Err(e) = data.export.save_snapshot_csv(&path, &data.traces) { eprintln!("Failed to export snapshot CSV: {e}"); }
                }
                ui.close();
            }
            #[cfg(feature = "parquet")]
            {
                if ui.button("Snapshot as Parquet").clicked() {
                    if let Some(path) = rfd::FileDialog::new().set_file_name("snapshot.parquet").add_filter("Parquet", &["parquet"]).save_file() {
                        if let Err(e) = data.export.save_snapshot_parquet(&path, &data.traces) { eprintln!("Failed to export snapshot Parquet: {e}"); }
                    }
                    ui.close();
                }
            }
        });
    }

    fn render_panel(&mut self, _ui: &mut Ui, _data: &mut DataContext) {}
}
