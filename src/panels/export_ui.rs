use super::panel_trait::{Panel, PanelState};
use crate::data::data::LivePlotData;
use crate::data::data::{ScreenshotRequest, ScreenshotTarget};
use crate::data::export; // main crate's export module
use crate::data::traces::TraceRef;
use egui::Ui;
use egui_phosphor_icons::icons::{EXPORT, FOLDER_OPEN, IMAGE, FILE_CSV};
#[cfg(feature = "parquet")]
use egui_phosphor_icons::icons::TABLE;
use std::collections::HashMap;

pub struct ExportPanel {
    pub state: PanelState,
}
impl Default for ExportPanel {
    fn default() -> Self {
        Self {
            state: PanelState::new("Export", EXPORT.as_str()),
        }
    }
}


impl Panel for ExportPanel {
    fn state(&self) -> &PanelState {
        &self.state
    }
    fn state_mut(&mut self) -> &mut PanelState {
        &mut self.state
    }

    fn hotkey_name(&self) -> Option<crate::data::hotkeys::HotkeyName> {
        Some(crate::data::hotkeys::HotkeyName::ExportData)
    }

    fn render_menu(
        &mut self,
        ui: &mut Ui,
        data: &mut LivePlotData<'_>,
        collapsed: bool,
        tooltip: &str,
    ) {
        let label = if collapsed {
            self.icon_only()
                .map(|s| s.to_string())
                .unwrap_or_else(|| self.title().to_string())
        } else {
            self.title_and_icon()
        };
        let menu_cfg = egui::containers::menu::MenuConfig::new()
            .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside);
        let mr = egui::containers::menu::MenuButton::new(label)
            .config(menu_cfg)
            .ui(ui, |ui| {
                if ui
                    .button(format!("{} Save Screenshot", IMAGE.as_str()))
                    .on_hover_text("Take one screenshot of the full center panel")
                    .clicked()
                {
                    data.pending_requests.screenshot = Some(ScreenshotRequest {
                        target: ScreenshotTarget::CenterPanel,
                        path: None,
                    });
                    ui.close();
                }
                if ui
                    .button(format!("{} Snapshot as CSV", FILE_CSV.as_str()))
                    .clicked()
                {
                    if let Some(path) = rfd::FileDialog::new()
                        .set_file_name("snapshot.csv")
                        .add_filter("CSV", &["csv"])
                        .save_file()
                    {
                        // Build series map based on paused/snapshot state (convert TraceRef to String)
                        let mut series: HashMap<TraceRef, Vec<[f64; 2]>> = HashMap::new();
                        for (name, tr) in data.traces.traces_iter() {
                            let iter: Box<dyn Iterator<Item = &[f64; 2]> + '_> =
                                if data.are_all_paused() {
                                    if let Some(snap) = &tr.snap {
                                        Box::new(snap.iter())
                                    } else {
                                        Box::new(tr.live.iter())
                                    }
                                } else {
                                    Box::new(tr.live.iter())
                                };
                            let vec: Vec<[f64; 2]> = iter.cloned().collect();
                            series.insert(name.clone(), vec);
                        }
                        if let Err(e) = export::write_csv_aligned_path(
                            &path,
                            &data.traces.all_trace_names(),
                            &series,
                            1e-9,
                        ) {
                            eprintln!("Failed to export snapshot CSV: {e}");
                        } else {
                            // Emit EXPORT event
                            if let Some(ctrl) = &data.event_ctrl {
                                let mut evt =
                                    crate::events::PlotEvent::new(crate::events::EventKind::EXPORT);
                                evt.export = Some(crate::events::ExportMeta {
                                    format: "csv".to_string(),
                                    path: Some(path.to_string_lossy().to_string()),
                                });
                                ctrl.emit_filtered(evt);
                            }
                        }
                    }
                    ui.close();
                }
                // Move Save/Load state into Export menu
                ui.separator();
                if ui.button(format!("{} Save state...", FOLDER_OPEN.as_str())).clicked() {
                    if let Some(path) = rfd::FileDialog::new()
                        .add_filter("JSON", &["json"])
                        .set_file_name("liveplot_state.json")
                        .save_file()
                    {
                        data.pending_requests.save_state = Some(path);
                    }
                    ui.close();
                }
                if ui.button(format!("{} Load state...", FOLDER_OPEN.as_str())).clicked() {
                    if let Some(path) = rfd::FileDialog::new()
                        .add_filter("JSON", &["json"])
                        .pick_file()
                    {
                        data.pending_requests.load_state = Some(path);
                    }
                    ui.close();
                }
                #[cfg(feature = "parquet")]
                {
                    if ui
                        .button(format!("{} Snapshot as Parquet", TABLE.as_str()))
                        .clicked()
                    {
                        if let Some(path) = rfd::FileDialog::new()
                            .set_file_name("snapshot.parquet")
                            .add_filter("Parquet", &["parquet"])
                            .save_file()
                        {
                            // Build series map like for CSV (convert TraceRef to String)
                            let mut series: HashMap<TraceRef, Vec<[f64; 2]>> = HashMap::new();
                            let names = data.traces.all_trace_names();
                            for name in names.iter() {
                                if let Some(tr) = data.traces.get_trace(name) {
                                    let iter: Box<dyn Iterator<Item = &[f64; 2]> + '_> =
                                        if data.are_all_paused() {
                                            if let Some(snap) = &tr.snap {
                                                Box::new(snap.iter())
                                            } else {
                                                Box::new(tr.live.iter())
                                            }
                                        } else {
                                            Box::new(tr.live.iter())
                                        };
                                    let vec: Vec<[f64; 2]> = iter.cloned().collect();
                                    series.insert(name.clone(), vec);
                                }
                            }
                            if let Err(e) =
                                export::write_parquet_aligned_path(&path, &names, &series, 1e-9)
                            {
                                eprintln!("Failed to export snapshot Parquet: {e}");
                            }
                        }
                        ui.close();
                    }
                }
            });
        if !tooltip.is_empty() {
            mr.0.on_hover_text(tooltip);
        }
    }
}

// tests moved to `tests/export_ui.rs`
