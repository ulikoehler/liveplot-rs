use super::panel_trait::{Panel, PanelState};
use crate::data::data::{LivePlotData, ScreenshotRequest, ScreenshotTarget};
use crate::data::fft::{FFTWindow, FftData};
use crate::data::scope::{AxisType, LegendPosition, ScopeType, ValueFormat};
use crate::data::traces::TraceRef;
use crate::data::traces::{TraceData, TracesCollection};
use crate::panels::scope_ui::{ScopePanel, ZoomMode};
use egui::Ui;
use egui_phosphor_icons::icons::{CHART_BAR, WARNING};
use egui_plot::PlotMemory;
use std::collections::HashSet;

pub struct FftPanel {
    pub state: PanelState,
    pub fft_data: FftData,
    pub scope_ui: ScopePanel,
    pub fft_db: bool,
    /// Trace names hidden via the plot legend (clicked to hide).
    /// These traces are neither computed nor rendered.
    hidden_in_legend: HashSet<TraceRef>,
    // Tracked widths for toolbar control groups so they wrap as units
    last_fft_size_width: f32,
    last_pad_width: f32,
    last_window_width: f32,
    last_throttle_width: f32,
    last_db_width: f32,
    /// Whether any visible trace had fewer points than fft_size in the last
    /// `update_data` pass.  Used to show a warning in the toolbar without
    /// re-iterating all traces in `render_panel`.
    insufficient_data: bool,
}

impl Default for FftPanel {
    fn default() -> Self {
        let mut scope_ui = ScopePanel::default();
        scope_ui.set_zoom_mode(ZoomMode::Both);
        let scope_data = scope_ui.get_data_mut();
        scope_data.x_axis.axis_type = AxisType::Value(ValueFormat::default());
        scope_data.x_axis.auto_fit = true;
        scope_data.x_axis.name = Some("Frequency".to_string());
        scope_data.x_axis.set_unit(Some("Hz".to_string()));
        scope_data.x_axis.show_label = true;
        scope_data.x_axis.value_decimals = 0;
        scope_data.y_axis.auto_fit = true;
        scope_data.y_axis.name = Some("Magnitude".to_string());
        scope_data.y_axis.set_unit(None);
        scope_data.y_axis.show_label = true;
        scope_data.scope_type = ScopeType::XYScope;
        scope_data.show_legend = true;
        scope_data.legend_position = LegendPosition::RightTop;

        Self {
            state: PanelState::new("FFT", CHART_BAR.as_str()),
            fft_data: FftData::default(),
            scope_ui,
            fft_db: false,
            hidden_in_legend: HashSet::default(),
            last_fft_size_width: 200.0,
            last_pad_width: 100.0,
            last_window_width: 120.0,
            last_throttle_width: 120.0,
            last_db_width: 60.0,
            insufficient_data: false,
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

    fn hotkey_name(&self) -> Option<crate::data::hotkeys::HotkeyName> {
        Some(crate::data::hotkeys::HotkeyName::Fft)
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
                if ui.button("Show FFT").clicked() {
                    let st = self.state_mut();
                    st.visible = true;
                    st.request_focus = true;
                    ui.close();
                }

                ui.separator();

                let prev = self.fft_db;
                if ui
                    .button(if self.fft_db { "Linear" } else { "dB" })
                    .clicked()
                {
                    self.fft_db = !self.fft_db;
                    if self.fft_db {
                        self.scope_ui.get_data_mut().y_axis.name =
                            Some("Magnitude (dB)".to_string());
                        self.scope_ui
                            .get_data_mut()
                            .y_axis
                            .set_unit(Some("dB".to_string()));
                    } else {
                        self.scope_ui.get_data_mut().y_axis.name = Some("Magnitude".to_string());
                        self.scope_ui.get_data_mut().y_axis.set_unit(None);
                    }
                }
                ui.menu_button("Window", |ui| {
                    // Select FFT window function
                    let mut changed = false;
                    for w in FFTWindow::ALL.iter().copied() {
                        let sel = w == self.fft_data.fft_window;
                        if ui.selectable_label(sel, w.label()).clicked() {
                            if self.fft_data.fft_window != w {
                                self.fft_data.fft_window = w;
                            }
                            changed = true;
                        }
                    }
                    if changed {
                        ui.close();
                    }
                });
                if self.fft_db != prev {
                    ui.close();
                }

                ui.separator();
                // Reuse scope controls (fit, axes, pause) from Scope panel for FFT view
                self.scope_ui.render_menu(ui, data.traces);
            });
        if !tooltip.is_empty() {
            mr.0.on_hover_text(tooltip);
        }
    }

    fn update_data(&mut self, data: &mut LivePlotData<'_>) {
        if !self.state().visible {
            return;
        }
        let paused = data.are_all_paused();

        // Clamp fft_size to the current max_points so we never request an
        // FFT larger than the buffer can hold.
        let max_pts = data.traces.max_points;
        if self.fft_data.fft_size > max_pts {
            // Round down to the nearest power of two <= max_pts
            let mut p = 1usize;
            while p * 2 <= max_pts {
                p *= 2;
            }
            self.fft_data.fft_size = p.max(256);
        }

        // Reset the insufficient-data flag; it will be set if any trace
        // has fewer points than fft_size during the dispatch loop below.
        self.insufficient_data = false;

        // Detect window/pause/size changes and invalidate cache if needed
        self.fft_data.check_window_pause_changed(paused);

        // Retain only FFT traces that still exist in source data
        self.fft_data
            .fft_traces
            .retain(|name, _| data.traces.contains_key(name));
        // Clean up hidden set for traces that no longer exist
        self.hidden_in_legend
            .retain(|name| data.traces.contains_key(name));

        // Poll for completed FFT results from the background worker
        let results = self.fft_data.poll_fft_results();
        for (trace_ref, spectrum, info) in results {
            if let Some(entry) = self.fft_data.fft_traces.get_mut(&trace_ref) {
                entry.live.clear();
                entry.live.extend(spectrum.into_iter());
                entry.snap = None;
                entry.info = info;
            }
        }

        // Dispatch new FFT jobs for traces that need recomputation
        for (name, tr) in data.traces.traces_iter() {
            // Skip traces hidden in the legend — no computation needed
            if self.hidden_in_legend.contains(name) {
                continue;
            }

            // Ensure a placeholder entry exists so the trace shows up in the
            // legend immediately, even before the first result arrives.
            let entry = self
                .fft_data
                .fft_traces
                .entry(name.clone())
                .or_insert_with(TraceData::default);
            entry.look = tr.look.clone();
            entry.offset = 0.0;
            if entry.info.is_empty() {
                entry.info = "Computing...".to_string();
            }

            // Determine which buffer we'd use, to compute cache key.
            let buf = if paused {
                match &tr.snap {
                    Some(s) => s,
                    None => continue,
                }
            } else {
                &tr.live
            };
            let buf_len = buf.len();
            let last_ts = buf.back().map(|p| p[0]);

            // Check for insufficient data before the throttle gate so the
            // warning doesn't flicker on/off at the recompute interval.
            if buf_len < self.fft_data.fft_size {
                self.insufficient_data = true;
            }

            if !self
                .fft_data
                .needs_recompute(name, buf_len, last_ts, paused)
            {
                continue;
            }

            // Dispatch to background worker; mark as computed regardless of
            // success to prevent retrying every frame (throttle handles retry).
            if self.fft_data.dispatch_fft(name, &tr.live, paused, &tr.snap) {
                self.fft_data.mark_computed(name, buf_len, last_ts);
            } else if buf_len < self.fft_data.fft_size {
                // Not enough data for the requested FFT size — update info
                // and mark computed so we don't spin every frame.
                self.insufficient_data = true;
                if let Some(entry) = self.fft_data.fft_traces.get_mut(name) {
                    entry.info =
                        format!("Need {} samples (have {})", self.fft_data.fft_size, buf_len);
                }
                self.fft_data.mark_computed(name, buf_len, last_ts);
            }
        }
    }

    fn render_panel(&mut self, ui: &mut Ui, data: &mut LivePlotData<'_>) {
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

        // Sync the internal scope's trace_order with whatever FFT traces are present.
        // `scope_data.update()` only *retains* existing entries – it never adds new ones –
        // so we must explicitly insert any trace names that are in tmp_traces but not yet
        // in trace_order.  We also prune stale entries (traces that disappeared).
        scope_data
            .trace_order
            .retain(|n| tmp_traces.contains_key(n));
        for name in self.fft_data.fft_traces.keys() {
            if !scope_data.trace_order.iter().any(|n| n == name) {
                scope_data.trace_order.push(name.clone());
            }
        }

        // Update scope ordering and auto-fit bounds
        self.scope_ui.update_data(&tmp_traces);

        // FFT-specific controls above the plot (wraps to multiple lines like scope toolbar)
        ui.horizontal_wrapped(|ui| {
            // FFT size group
            let desired = egui::vec2(self.last_fft_size_width, ui.spacing().interact_size.y);
            let (rect, _) = ui.allocate_exact_size(desired, egui::Sense::hover());
            ui.scope_builder(egui::UiBuilder::new().max_rect(rect), |ui| {
                let resp = ui.horizontal(|ui| {
                    ui.label("FFT size:");
                    let max_log2 = (data.traces.max_points as f32).log2().floor() as u32;
                    let max_log2 = max_log2.max(8).min(20);
                    let mut size_log2 = (self.fft_data.fft_size as f32).log2() as u32;
                    size_log2 = size_log2.min(max_log2);
                    let slider = egui::Slider::new(&mut size_log2, 8..=max_log2).text("2^N");
                    if ui.add(slider).changed() {
                        self.fft_data.fft_size = 1usize << size_log2;
                    }
                });
                self.last_fft_size_width = resp.response.rect.width();
            });

            // Warning when not enough datapoints for the current FFT size
            if self.insufficient_data {
                ui.label(
                    egui::RichText::new(format!(
                        "{} Not enough data for FFT size",
                        WARNING.as_str()
                    ))
                    .color(egui::Color32::from_rgb(220, 160, 40)),
                );
            }

            ui.separator();

            // Pad group
            let desired = egui::vec2(self.last_pad_width, ui.spacing().interact_size.y);
            let (rect, _) = ui.allocate_exact_size(desired, egui::Sense::hover());
            ui.scope_builder(egui::UiBuilder::new().max_rect(rect), |ui| {
                let resp = ui.horizontal(|ui| {
                    ui.label("Pad:");
                    let pad_options: [(usize, &str); 5] =
                        [(1, "1×"), (2, "2×"), (4, "4×"), (8, "8×"), (16, "16×")];
                    let pad_label = pad_options
                        .iter()
                        .find(|(v, _)| *v == self.fft_data.zero_pad_factor)
                        .map(|(_, l)| *l)
                        .unwrap_or("1×");
                    let _ = egui::ComboBox::from_id_salt("fft_zero_pad")
                        .selected_text(pad_label)
                        .show_ui(ui, |ui| {
                            for (v, label) in pad_options.iter() {
                                ui.selectable_value(&mut self.fft_data.zero_pad_factor, *v, *label);
                            }
                        });
                });
                self.last_pad_width = resp.response.rect.width();
            });

            ui.separator();

            // Window group
            let desired = egui::vec2(self.last_window_width, ui.spacing().interact_size.y);
            let (rect, _) = ui.allocate_exact_size(desired, egui::Sense::hover());
            ui.scope_builder(egui::UiBuilder::new().max_rect(rect), |ui| {
                let resp = ui.horizontal(|ui| {
                    ui.label("Window:");
                    let mut w_idx = FFTWindow::ALL
                        .iter()
                        .position(|w| *w == self.fft_data.fft_window)
                        .unwrap_or(1);
                    let _ = egui::ComboBox::from_id_salt("fft_window_multi")
                        .selected_text(self.fft_data.fft_window.label())
                        .show_ui(ui, |ui| {
                            for (i, w) in FFTWindow::ALL.iter().enumerate() {
                                ui.selectable_value(&mut w_idx, i, w.label());
                            }
                        });
                    self.fft_data.fft_window = FFTWindow::ALL[w_idx];
                });
                self.last_window_width = resp.response.rect.width();
            });

            ui.separator();

            // Update throttle group
            let desired = egui::vec2(self.last_throttle_width, ui.spacing().interact_size.y);
            let (rect, _) = ui.allocate_exact_size(desired, egui::Sense::hover());
            ui.scope_builder(egui::UiBuilder::new().max_rect(rect), |ui| {
                let resp = ui.horizontal(|ui| {
                    ui.label("Update:");
                    let throttle_options: [(u64, &str); 5] = [
                        (50, "50ms"),
                        (100, "100ms"),
                        (200, "200ms"),
                        (500, "500ms"),
                        (1000, "1s"),
                    ];
                    let throttle_label = throttle_options
                        .iter()
                        .find(|(v, _)| *v == self.fft_data.recompute_interval_ms)
                        .map(|(_, l)| *l)
                        .unwrap_or("100ms");
                    let _ = egui::ComboBox::from_id_salt("fft_throttle")
                        .selected_text(throttle_label)
                        .show_ui(ui, |ui| {
                            for (v, label) in throttle_options.iter() {
                                ui.selectable_value(
                                    &mut self.fft_data.recompute_interval_ms,
                                    *v,
                                    *label,
                                );
                            }
                        });
                });
                self.last_throttle_width = resp.response.rect.width();
            });

            ui.separator();

            // dB toggle group
            let desired = egui::vec2(self.last_db_width, ui.spacing().interact_size.y);
            let (rect, _) = ui.allocate_exact_size(desired, egui::Sense::hover());
            ui.scope_builder(egui::UiBuilder::new().max_rect(rect), |ui| {
                let resp = ui.horizontal(|ui| {
                    if ui
                        .button(if self.fft_db { "Linear" } else { "dB" })
                        .on_hover_text("Toggle FFT magnitude scale")
                        .clicked()
                    {
                        self.fft_db = !self.fft_db;
                    }
                });
                self.last_db_width = resp.response.rect.width();
            });

            ui.separator();

            let controlls_in_toolbar = self.scope_ui.controls_in_toolbar();
            if ui
                .selectable_label(controlls_in_toolbar, "Controls in Toolbar")
                .clicked()
            {
                self.scope_ui.set_controls_in_toolbar(!controlls_in_toolbar);
            }
        });

        ui.separator();

        // Render using scope panel (legend is enabled via scope_data settings)
        self.scope_ui.render_panel(
            ui,
            |_plot_ui, _scope_unused, _traces_unused| {},
            &mut tmp_traces,
        );

        // Read back which traces the user toggled in the legend
        let plot_id = ui.make_persistent_id(egui::Id::new(format!(
            "scope_plot_{}",
            self.scope_ui.get_data().name
        )));
        if let Some(mem) = PlotMemory::load(ui.ctx(), plot_id) {
            self.hidden_in_legend.clear();
            for name in self.fft_data.fft_traces.keys() {
                let item_id = egui::Id::new(&name.0);
                if mem.hidden_items.contains(&item_id) {
                    self.hidden_in_legend.insert(name.clone());
                }
            }
        }

        if self.scope_ui.take_screenshot_request() {
            let scope = self.scope_ui.get_data();
            if let Some(rect) = scope.last_plot_screen_rect {
                data.pending_requests.screenshot = Some(ScreenshotRequest {
                    target: ScreenshotTarget::ScopeRect {
                        scope_id: scope.id,
                        scope_name: scope.name.clone(),
                        rect,
                        show_x_axis_label: scope.x_axis.show_label,
                        show_y_axis_label: scope.y_axis.show_label,
                    },
                    path: None,
                });
            }
        }
    }

    fn settings_snapshot(&self, _data: &LivePlotData<'_>) -> Option<String> {
        let snap = crate::persistence::FftPanelStateSerde::from_panel(self);
        serde_json::to_string(&snap).ok()
    }
}
