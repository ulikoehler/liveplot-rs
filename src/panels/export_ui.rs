use egui::Ui;
use super::panel_trait::{Panel, PanelState};
use crate::data::scope::ScopeData;
use crate::data::export;
use std::collections::HashMap;

pub struct ExportPanel { pub state: PanelState }
impl Default for ExportPanel { fn default() -> Self { Self { state: PanelState { title: "Export", visible: false, detached: false, request_docket: false } } } }
impl Panel for ExportPanel {
    fn state(&self) -> &PanelState { &self.state }
    fn state_mut(&mut self) -> &mut PanelState { &mut self.state }

    fn render_menu(&mut self, ui: &mut Ui, data: &mut ScopeData) {
        ui.menu_button("🗁 Export", |ui| {
            if ui
                .button("🖼 Save Screenshot")
                .on_hover_text("Take a screenshot of the entire window")
                .clicked()
            {
                // Choose a path and request a screenshot; Scope panel will handle saving.
                if let Some(path) = rfd::FileDialog::new()
                    .set_file_name("screenshot.png")
                    .add_filter("PNG", &["png"])
                    .save_file()
                {
                    std::env::set_var("LIVEPLOT_SAVE_SCREENSHOT_TO", path.to_string_lossy().to_string());
                }
                ui.ctx().send_viewport_cmd(egui::ViewportCommand::Screenshot(Default::default()));
                ui.close();
            }
            if ui.button("Snapshot as CSV").clicked() {
                if let Some(path) = rfd::FileDialog::new()
                    .set_file_name("snapshot.csv")
                    .add_filter("CSV", &["csv"])
                    .save_file()
                {
                    // Build series map based on paused/snapshot state
                    let mut series: HashMap<String, Vec<[f64; 2]>> = HashMap::new();
                    for name in data.trace_order.iter() {
                        if let Some(tr) = data.traces.get(name) {
                            let iter: Box<dyn Iterator<Item = &[f64; 2]> + '_> = if data.is_paused() {
                                if let Some(snap) = &tr.snap { Box::new(snap.iter()) } else { Box::new(tr.live.iter()) }
                            } else { Box::new(tr.live.iter()) };
                            let vec: Vec<[f64; 2]> = iter.cloned().collect();
                            series.insert(name.clone(), vec);
                        }
                    }
                    if let Err(e) = export::write_csv_aligned_path(&path, &data.trace_order, &series, 1e-9) {
                        eprintln!("Failed to export snapshot CSV: {e}");
                    }
                }
                ui.close();
            }
            #[cfg(feature = "parquet")]
            {
                if ui.button("Snapshot as Parquet").clicked() {
                    if let Some(path) = rfd::FileDialog::new()
                        .set_file_name("snapshot.parquet")
                        .add_filter("Parquet", &["parquet"])
                        .save_file()
                    {
                        // Build series map like for CSV
                        let mut series: HashMap<String, Vec<[f64; 2]>> = HashMap::new();
                        for name in data.trace_order.iter() {
                            if let Some(tr) = data.traces.get(name) {
                                let iter: Box<dyn Iterator<Item = &[f64; 2]> + '_> = if data.paused {
                                    if let Some(snap) = &tr.snap { Box::new(snap.iter()) } else { Box::new(tr.live.iter()) }
                                } else { Box::new(tr.live.iter()) };
                                let vec: Vec<[f64; 2]> = iter.cloned().collect();
                                series.insert(name.clone(), vec);
                            }
                        }
                        if let Err(e) = export::write_parquet_aligned_path(&path, &data.trace_order, &series, 1e-9) {
                            eprintln!("Failed to export snapshot Parquet: {e}");
                        }
                    }
                    ui.close();
                }
            }
        });
    }
}
