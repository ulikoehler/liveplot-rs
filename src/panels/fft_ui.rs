use super::panel_trait::{Panel, PanelState};
use crate::data::data::LivePlotData;
use crate::data::fft::{FFTWindow, FftData};
use crate::data::scope::ScopeType;
use crate::data::traces::{TraceData, TracesCollection};
use crate::panels::scope_ui::ScopePanel;
use egui::Ui;

pub struct FftPanel {
    pub state: PanelState,
    pub fft_data: FftData,
    pub scope_ui: ScopePanel,
    pub fft_db: bool,
}

impl Default for FftPanel {
    fn default() -> Self {
        Self {
            state: PanelState::new("FFT"),
            fft_data: FftData::default(),
            scope_ui: ScopePanel::default(),
            fft_db: false,
        }
    }
}

impl Panel for FftPanel {
    fn state(&self) -> &PanelState {
        &self.state
    }

    fn state_mut(&mut self) -> &mut PanelState {
        &mut self.state
    }

    fn update_data(&mut self, data: &mut LivePlotData<'_>) {
        let paused = data.is_paused();
        // Retain only FFT traces that still exist in source data
        self.fft_data
            .fft_traces
            .retain(|name, _| data.traces.contains_key(name));

        for (name, tr) in data.traces.traces_iter() {
            if let Some(spec) = FftData::compute_fft(
                &tr.live,
                paused,
                &tr.snap,
                self.fft_data.fft_size,
                self.fft_data.fft_window,
            ) {
                let entry = self
                    .fft_data
                    .fft_traces
                    .entry(name.clone())
                    .or_insert_with(TraceData::default);
                entry.look = tr.look.clone();
                entry.offset = 0.0;
                entry.live.clear();
                entry.live.extend(spec.into_iter());
                entry.snap = None;
                entry.info = format!(
                    "FFT N={} {}",
                    self.fft_data.fft_size,
                    self.fft_data.fft_window.label()
                );
            }
        }
    }

    fn render_panel(&mut self, ui: &mut Ui, _data: &mut LivePlotData<'_>) {
        // Flat settings bar (no collapsing header) with auto-fit trigger on change
        let mut changed_settings = false;
        ui.horizontal(|ui| {
            ui.label("FFT size:");
            let mut size_log2 = (self.fft_data.fft_size as f32).log2() as u32;
            let slider = egui::Slider::new(&mut size_log2, 8..=15).text("2^N");
            if ui.add(slider).changed() {
                self.fft_data.fft_size = 1usize << size_log2;
                changed_settings = true;
            }
            ui.separator();
            ui.label("Window:");
            let mut w_idx = FFTWindow::ALL
                .iter()
                .position(|w| *w == self.fft_data.fft_window)
                .unwrap_or(1);
            let combo = egui::ComboBox::from_id_salt("fft_window_multi")
                .selected_text(self.fft_data.fft_window.label())
                .show_ui(ui, |ui| {
                    for (i, w) in FFTWindow::ALL.iter().enumerate() {
                        ui.selectable_value(&mut w_idx, i, w.label());
                    }
                });
            if combo.response.changed() {
                self.fft_data.fft_window = FFTWindow::ALL[w_idx];
                changed_settings = true;
            } else {
                self.fft_data.fft_window = FFTWindow::ALL[w_idx];
            }
            ui.separator();
            if ui
                .button(if self.fft_db { "Linear" } else { "dB" })
                .on_hover_text("Toggle FFT magnitude scale")
                .clicked()
            {
                self.fft_db = !self.fft_db;
                changed_settings = true;
            }
            ui.separator();
            if changed_settings {
                ui.label("(auto-fit)");
            }
        });
        ui.separator();

        // Build temporary traces collection for spectra
        let mut tmp_traces = TracesCollection::default();
        for (name, td) in self.fft_data.fft_traces.iter() {
            let out_td = tmp_traces.get_trace_or_new(name);
            out_td.look = td.look.clone();
            out_td.offset = 0.0;
            if self.fft_db {
                let mut v = td.live.clone();
                for p in v.iter_mut() {
                    let mag = p[1].max(1e-12);
                    p[1] = 20.0 * mag.log10();
                }
                out_td.live = v;
            } else {
                out_td.live = td.live.clone();
            }
            out_td.snap = None;
            out_td.info = td.info.clone();
        }

    // Configure scope for frequency domain
        let scope_data = self.scope_ui.get_data_mut();
        scope_data.scope_type = ScopeType::XYScope;
        scope_data.x_axis.name = Some("Frequency".to_string());
        scope_data.x_axis.unit = Some("Hz".to_string());
        scope_data.x_axis.format = None; // plain numeric
        scope_data.y_axis.name = Some(if self.fft_db {
            "Magnitude (dB)".to_string()
        } else {
            "Magnitude".to_string()
        });
        scope_data.y_axis.unit = None;
        scope_data.y_axis.log_scale = false;

        // Update scope ordering
        self.scope_ui.update_data(&tmp_traces);

        

        // Render using scope panel (no overlays)
        self.scope_ui
            .render_panel(ui, |_plot_ui, _scope, _traces| {}, &mut tmp_traces);
    }
}
