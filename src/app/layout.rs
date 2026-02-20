//! Responsive layout computation and UI rendering for [`LivePlotPanel`].
//!
//! This module implements the visual layout of the LivePlot widget:
//!
//! * **[`compute_effective_layout`](LivePlotPanel::compute_effective_layout)** –
//!   decides which buttons appear in the top menu bar vs. the sidebar icon strip,
//!   depending on the available viewport size and user configuration.
//! * **[`render_menu`](LivePlotPanel::render_menu)** – draws the top menu bar with
//!   panel toggle buttons, pause/resume, clear-all, and state save/load.
//! * **[`render_panels`](LivePlotPanel::render_panels)** – draws the sidebar icon
//!   strip, left/right/bottom sidebars, and detached panel windows.
//! * **[`render_tabs`](LivePlotPanel::render_tabs)** – shared tab-strip + panel-body
//!   renderer used by the left, right, and bottom sidebars.

use eframe::egui;
use eframe::egui::scroll_area::{ScrollBarVisibility, ScrollSource};
use egui_phosphor::regular::BROOM;

use crate::config::ScopeButton;
use crate::data::data::LivePlotData;
use crate::data::hotkeys::{format_button_tooltip, get_hotkey_for_name, should_collapse_topbar};
use crate::panels::panel_trait::Panel;

use super::{EffectiveLayout, LivePlotPanel};

impl LivePlotPanel {
    /// Compute which buttons appear in the top bar vs. the sidebar for the current frame.
    ///
    /// The decision is based on the viewport dimensions captured in the previous
    /// frame ([`last_plot_size`](LivePlotPanel::last_plot_size)) and the minimum-size
    /// thresholds configured on the panel.  When the plot area is too small for
    /// the top bar, its buttons migrate to the sidebar (and vice versa).
    pub(crate) fn compute_effective_layout(&self) -> EffectiveLayout {
        let plot_h = self.last_plot_size.y;
        let plot_w = self.last_plot_size.x;
        let suppress_top = plot_h < self.min_height_for_top_bar;
        let suppress_sidebar =
            plot_w < self.min_width_for_sidebar || plot_h < self.min_height_for_sidebar;

        let user_top: Vec<ScopeButton> = self
            .top_bar_buttons
            .clone()
            .unwrap_or_else(ScopeButton::all_defaults);
        let user_sidebar: Vec<ScopeButton> = self.sidebar_buttons.clone().unwrap_or_default();

        if suppress_top && suppress_sidebar {
            // Both bars suppressed – hide everything.
            EffectiveLayout {
                top_bar_buttons: vec![],
                sidebar_buttons: vec![],
                show_top_bar: false,
                show_sidebar_panels: false,
            }
        } else if suppress_top {
            // Top bar hidden → its buttons move to the sidebar.
            let mut sidebar = user_sidebar;
            sidebar.extend(user_top);
            EffectiveLayout {
                top_bar_buttons: vec![],
                sidebar_buttons: sidebar,
                show_top_bar: false,
                show_sidebar_panels: true,
            }
        } else if suppress_sidebar {
            // Sidebar hidden → its icon-strip buttons move to the top bar.
            let mut top = user_top;
            top.extend(user_sidebar);
            EffectiveLayout {
                top_bar_buttons: top,
                sidebar_buttons: vec![],
                show_top_bar: true,
                show_sidebar_panels: false,
            }
        } else {
            EffectiveLayout {
                top_bar_buttons: user_top,
                sidebar_buttons: user_sidebar,
                show_top_bar: true,
                show_sidebar_panels: true,
            }
        }
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Top menu bar
    // ─────────────────────────────────────────────────────────────────────────

    /// Render the top menu bar (panel toggle buttons, pause/resume, clear-all,
    /// and state save/load).
    ///
    /// When the available width is too tight the buttons collapse from full
    /// labels (`"📈 Traces"`) to icon-only (`"📈"`).
    pub(crate) fn render_menu(&mut self, ui: &mut egui::Ui) {
        // ── Responsive layout: should we show the top bar at all? ───────────
        let layout = self.compute_effective_layout();
        if !layout.show_top_bar {
            return; // top bar suppressed – its buttons have been moved to the sidebar
        }
        let top_bar_btns = layout.top_bar_buttons;

        // ── When to collapse text labels to icon-only ────────────────────────
        let button_font = egui::TextStyle::Button.resolve(ui.style());
        let button_padding = ui.spacing().button_padding.x * 2.0;
        let item_spacing = ui.spacing().item_spacing.x;
        let mut required_width = 0.0;

        let calc_width = |text: &str| -> f32 {
            let w = ui.fonts_mut(|f| {
                f.layout_no_wrap(text.to_string(), button_font.clone(), egui::Color32::WHITE)
                    .rect
                    .width()
            });
            w + button_padding + item_spacing
        };

        // 1. Scopes button (only if in top_bar_btns)
        if top_bar_btns.contains(&ScopeButton::Scopes) {
            required_width += calc_width("🔭 Scopes");
        }

        // 2. All other panels (only those in top_bar_btns)
        let all_panels = self
            .left_side_panels
            .iter()
            .chain(self.right_side_panels.iter())
            .chain(self.bottom_panels.iter())
            .chain(self.detached_panels.iter())
            .chain(self.empty_panels.iter());

        for p in all_panels {
            if top_bar_btns
                .iter()
                .any(|b| b.matches_panel_title(p.title()))
            {
                required_width += calc_width(&p.title_and_icon());
            }
        }

        // 3. Separator (approximate width)
        required_width += item_spacing * 2.0;

        // 4. Pause / Resume (take the wider one, only if in top_bar_btns)
        if top_bar_btns.contains(&ScopeButton::PauseResume) {
            required_width += calc_width("⏸ Pause");
        }

        // 5. Clear All (only if in top_bar_btns)
        if top_bar_btns.contains(&ScopeButton::ClearAll) {
            required_width += calc_width(&format!("{BROOM} Clear All"));
        }

        // Remove trailing spacing
        required_width -= item_spacing;

        let topbar_collapsed = should_collapse_topbar(ui.available_width(), required_width);

        // Clone Rc so it can be borrowed independently inside the closure.
        let hk_rc = self.hotkeys.clone();

        egui::MenuBar::new().ui(ui, |ui| {
            // Render the Scopes button only if configured
            if top_bar_btns.contains(&ScopeButton::Scopes) {
                self.liveplot_panel
                    .render_menu(ui, &mut self.traces_data, topbar_collapsed);
            }

            let (save_req, load_req, add_scope_req, remove_scope_req) = {
                let scope_data = self.liveplot_panel.get_data_mut();
                let mut data = LivePlotData {
                    scope_data,
                    traces: &mut self.traces_data,
                    pending_requests: &mut self.pending_requests,
                    event_ctrl: self.event_ctrl.clone(),
                };

                {
                    let hk = hk_rc.borrow();

                    // Render toggle buttons for every panel that belongs in the top bar.
                    for p in &mut self.left_side_panels {
                        if !top_bar_btns
                            .iter()
                            .any(|b| b.matches_panel_title(p.title()))
                        {
                            continue;
                        }
                        let tt = p
                            .hotkey_name()
                            .and_then(|name| get_hotkey_for_name(&hk, name))
                            .map(|k| format_button_tooltip(p.title(), Some(k)))
                            .unwrap_or_else(|| p.title().to_string());
                        p.render_menu(ui, &mut data, topbar_collapsed, &tt);
                    }
                    for p in &mut self.right_side_panels {
                        if !top_bar_btns
                            .iter()
                            .any(|b| b.matches_panel_title(p.title()))
                        {
                            continue;
                        }
                        let tt = p
                            .hotkey_name()
                            .and_then(|name| get_hotkey_for_name(&hk, name))
                            .map(|k| format_button_tooltip(p.title(), Some(k)))
                            .unwrap_or_else(|| p.title().to_string());
                        p.render_menu(ui, &mut data, topbar_collapsed, &tt);
                    }
                    for p in &mut self.bottom_panels {
                        if !top_bar_btns
                            .iter()
                            .any(|b| b.matches_panel_title(p.title()))
                        {
                            continue;
                        }
                        let tt = p
                            .hotkey_name()
                            .and_then(|name| get_hotkey_for_name(&hk, name))
                            .map(|k| format_button_tooltip(p.title(), Some(k)))
                            .unwrap_or_else(|| p.title().to_string());
                        p.render_menu(ui, &mut data, topbar_collapsed, &tt);
                    }
                    for p in &mut self.detached_panels {
                        if !top_bar_btns
                            .iter()
                            .any(|b| b.matches_panel_title(p.title()))
                        {
                            continue;
                        }
                        let tt = p
                            .hotkey_name()
                            .and_then(|name| get_hotkey_for_name(&hk, name))
                            .map(|k| format_button_tooltip(p.title(), Some(k)))
                            .unwrap_or_else(|| p.title().to_string());
                        p.render_menu(ui, &mut data, topbar_collapsed, &tt);
                    }
                    for p in &mut self.empty_panels {
                        if !top_bar_btns
                            .iter()
                            .any(|b| b.matches_panel_title(p.title()))
                        {
                            continue;
                        }
                        let tt = p
                            .hotkey_name()
                            .and_then(|name| get_hotkey_for_name(&hk, name))
                            .map(|k| format_button_tooltip(p.title(), Some(k)))
                            .unwrap_or_else(|| p.title().to_string());
                        p.render_menu(ui, &mut data, topbar_collapsed, &tt);
                    }

                    // ── Pause / Resume / Clear All ───────────────────────────
                    if top_bar_btns.contains(&ScopeButton::PauseResume)
                        || top_bar_btns.contains(&ScopeButton::ClearAll)
                    {
                        ui.separator();
                    }
                    if top_bar_btns.contains(&ScopeButton::PauseResume) {
                        let pause_tt = format_button_tooltip("Pause / Resume", hk.pause.as_ref());
                        if !data.are_all_paused() {
                            let pause_label = if topbar_collapsed { "⏸" } else { "⏸ Pause" };
                            if ui.button(pause_label).on_hover_text(&pause_tt).clicked() {
                                data.pause_all();
                            }
                        } else {
                            let resume_label = if topbar_collapsed {
                                "▶"
                            } else {
                                "▶ Resume"
                            };
                            if ui.button(resume_label).on_hover_text(&pause_tt).clicked() {
                                data.resume_all();
                            }
                        }
                    }

                    if top_bar_btns.contains(&ScopeButton::ClearAll) {
                        let clear_all_label = if topbar_collapsed {
                            BROOM.to_string()
                        } else {
                            format!("{BROOM} Clear All")
                        };
                        let clear_tt = format_button_tooltip("Clear All", hk.clear_all.as_ref());
                        if ui.button(clear_all_label).on_hover_text(clear_tt).clicked() {
                            data.request_clear_all();
                        }
                    }
                }

                (
                    data.pending_requests.save_state.take(),
                    data.pending_requests.load_state.take(),
                    std::mem::take(&mut data.pending_requests.add_scope),
                    data.pending_requests.remove_scope.take(),
                )
            };

            // Apply scope add/remove requests produced by the menu.
            if add_scope_req {
                self.liveplot_panel.add_scope();
            }
            if let Some(scope_id) = remove_scope_req {
                let _ = self.liveplot_panel.remove_scope_by_id(scope_id);
            }

            // ── State persistence (save / load) ─────────────────────────────
            if let Some(path) = save_req {
                self.handle_save_state(ui, &path);
            }

            if let Some(path) = load_req {
                self.handle_load_state(ui, &path);
            }
        });
    }

    /// Serialize the current application state and write it to `path`.
    ///
    /// Called from [`render_menu`](Self::render_menu) when the user (or a
    /// controller) requests a state save.
    fn handle_save_state(&mut self, ui: &mut egui::Ui, path: &std::path::Path) {
        let ctx = ui.ctx();
        let rect = ctx.input(|i| i.content_rect());
        let win_size = Some([rect.width(), rect.height()]);
        let win_pos = Some([rect.left(), rect.top()]);
        let live_data = LivePlotData {
            scope_data: self.liveplot_panel.get_data_mut(),
            traces: &mut self.traces_data,
            pending_requests: &mut self.pending_requests,
            event_ctrl: self.event_ctrl.clone(),
        };

        // Save all scopes.
        let scope_states: Vec<crate::persistence::ScopeStateSerde> = live_data
            .scope_data
            .iter()
            .map(|s| crate::persistence::ScopeStateSerde::from(&**s))
            .collect();

        // Helper to convert Panel::state() to PanelVisSerde.
        let mut panels_state: Vec<crate::persistence::PanelVisSerde> = Vec::new();
        let mut push_panel = |p: &Box<dyn Panel>| {
            let st = p.state();
            panels_state.push(crate::persistence::PanelVisSerde {
                title: st.title.to_string(),
                visible: st.visible,
                detached: st.detached,
                window_pos: st.window_pos,
                window_size: st.window_size,
            });
        };
        for p in &self.left_side_panels {
            push_panel(p);
        }
        for p in &self.right_side_panels {
            push_panel(p);
        }
        for p in &self.bottom_panels {
            push_panel(p);
        }
        for p in &self.detached_panels {
            push_panel(p);
        }
        for p in &self.empty_panels {
            push_panel(p);
        }

        // Trace styles from all scopes.
        let trace_styles: Vec<crate::persistence::TraceStyleSerde> = {
            let mut seen = std::collections::HashSet::new();
            let mut snapshot: Vec<(String, crate::data::trace_look::TraceLook, f64)> = Vec::new();
            for scope in live_data.scope_data.iter() {
                for name in scope.trace_order.iter() {
                    if seen.insert(name.0.clone()) {
                        if let Some(tr) = live_data.traces.get_trace(name) {
                            snapshot.push((name.0.clone(), tr.look.clone(), tr.offset));
                        }
                    }
                }
            }
            snapshot
                .into_iter()
                .map(|(n, look, off)| crate::persistence::TraceStyleSerde {
                    name: n,
                    look: crate::persistence::TraceLookSerde::from(&look),
                    offset: off,
                })
                .collect()
        };

        // Math traces: extract from MathPanel.
        let math_traces_ser: Vec<crate::data::math::MathTrace> = {
            let mut out = Vec::new();
            for p in self
                .left_side_panels
                .iter()
                .chain(self.right_side_panels.iter())
                .chain(self.bottom_panels.iter())
                .chain(self.detached_panels.iter())
                .chain(self.empty_panels.iter())
            {
                let any: &dyn Panel = &**p;
                if let Some(mp) = any.downcast_ref::<crate::panels::math_ui::MathPanel>() {
                    out.extend(mp.get_math_traces().iter().cloned());
                }
            }
            out
        };

        // Thresholds & Triggers: extract from specialized panels, if present.
        let mut thresholds_ser: Vec<crate::persistence::ThresholdSerde> = Vec::new();
        let mut triggers_ser: Vec<crate::persistence::TriggerSerde> = Vec::new();
        for p in self
            .left_side_panels
            .iter()
            .chain(self.right_side_panels.iter())
            .chain(self.bottom_panels.iter())
            .chain(self.detached_panels.iter())
            .chain(self.empty_panels.iter())
        {
            let any: &dyn Panel = &**p;
            if let Some(tp) = any.downcast_ref::<crate::panels::thresholds_ui::ThresholdsPanel>() {
                for (_n, d) in tp.thresholds.iter() {
                    thresholds_ser.push(crate::persistence::ThresholdSerde::from_threshold(d));
                }
            }
            if let Some(trg) = any.downcast_ref::<crate::panels::triggers_ui::TriggersPanel>() {
                for (_n, t) in trg.triggers.iter() {
                    triggers_ser.push(crate::persistence::TriggerSerde::from_trigger(t));
                }
            }
        }

        let state = crate::persistence::AppStateSerde {
            window_size: win_size,
            window_pos: win_pos,
            scope: None,
            scopes: scope_states,
            panels: panels_state,
            traces_style: trace_styles,
            thresholds: thresholds_ser,
            triggers: triggers_ser,
            math_traces: math_traces_ser,
            next_scope_idx: Some(self.liveplot_panel.next_scope_idx()),
        };

        let _ = crate::persistence::save_state_to_path(&state, path);
    }

    /// Load application state from `path` and apply it to the panel.
    ///
    /// Called from [`render_menu`](Self::render_menu) when the user (or a
    /// controller) requests a state load.
    fn handle_load_state(&mut self, ui: &mut egui::Ui, path: &std::path::Path) {
        let Ok(loaded) = crate::persistence::load_state_from_path(path) else {
            return;
        };

        // Window: attempt to request size/pos via ctx.
        if let Some(sz) = loaded.window_size {
            ui.ctx()
                .send_viewport_cmd(egui::ViewportCommand::InnerSize(egui::Vec2::new(
                    sz[0], sz[1],
                )));
        }

        // Restore all scopes (or fall back to legacy single-scope).
        let scope_states = loaded.all_scopes();
        if !scope_states.is_empty() {
            self.liveplot_panel
                .restore_scopes(scope_states, loaded.next_scope_idx);
        }

        // Panels: match by title and set visible/detached/pos/size.
        let apply_panel_state = |p: &mut Box<dyn Panel>| {
            let st = p.state_mut();
            for pser in &loaded.panels {
                if pser.title == st.title {
                    st.visible = pser.visible;
                    st.detached = pser.detached;
                    st.window_pos = pser.window_pos;
                    st.window_size = pser.window_size;
                    break;
                }
            }
        };
        for p in &mut self.left_side_panels {
            apply_panel_state(p);
        }
        for p in &mut self.right_side_panels {
            apply_panel_state(p);
        }
        for p in &mut self.bottom_panels {
            apply_panel_state(p);
        }
        for p in &mut self.detached_panels {
            apply_panel_state(p);
        }
        for p in &mut self.empty_panels {
            apply_panel_state(p);
        }

        // Apply traces styles (uses pending_styles for traces not yet created).
        crate::persistence::apply_trace_styles(&loaded.traces_style, |name, look, off| {
            self.traces_data.set_pending_style(name, look, off);
        });

        // Apply math traces.
        if !loaded.math_traces.is_empty() {
            for p in self
                .left_side_panels
                .iter_mut()
                .chain(self.right_side_panels.iter_mut())
                .chain(self.bottom_panels.iter_mut())
                .chain(self.detached_panels.iter_mut())
                .chain(self.empty_panels.iter_mut())
            {
                let any: &mut dyn Panel = &mut **p;
                if let Some(mp) = any.downcast_mut::<crate::panels::math_ui::MathPanel>() {
                    mp.set_math_traces(loaded.math_traces.clone());
                }
            }
        }

        // Apply thresholds and triggers to specialized panels.
        for p in self
            .left_side_panels
            .iter_mut()
            .chain(self.right_side_panels.iter_mut())
            .chain(self.bottom_panels.iter_mut())
            .chain(self.detached_panels.iter_mut())
            .chain(self.empty_panels.iter_mut())
        {
            let any: &mut dyn Panel = &mut **p;
            if let Some(tp) = any.downcast_mut::<crate::panels::thresholds_ui::ThresholdsPanel>() {
                tp.thresholds.clear();
                for tser in &loaded.thresholds {
                    let def = tser.clone().into_threshold();
                    tp.thresholds.insert(def.name.clone(), def);
                }
            }
            if let Some(trg) = any.downcast_mut::<crate::panels::triggers_ui::TriggersPanel>() {
                trg.triggers.clear();
                for trser in &loaded.triggers {
                    let def = trser.clone().into_trigger();
                    trg.triggers.insert(def.name.clone(), def);
                }
            }
        }
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Side panels, bottom panel, icon strip
    // ─────────────────────────────────────────────────────────────────────────

    /// Render the sidebar icon strip, left/right/bottom panel docks, and
    /// detached panel windows.
    pub(crate) fn render_panels(&mut self, ui: &mut egui::Ui) {
        let layout = self.compute_effective_layout();
        let has_icon_strip = !layout.sidebar_buttons.is_empty();

        // ── Persistent sidebar icon strip (rightmost panel) ──────────────────
        if has_icon_strip {
            let sidebar_btns = layout.sidebar_buttons.clone();
            let hk_rc = self.hotkeys.clone();
            let all_paused = self.liveplot_panel.get_data().iter().all(|s| s.paused);
            let mut clicked_btns: Vec<ScopeButton> = Vec::new();

            egui::SidePanel::right(format!("right_icon_strip_{}", self.panel_id))
                .resizable(false)
                .exact_width(36.0)
                .show_inside(ui, |ui| {
                    let hk = hk_rc.borrow();
                    ui.with_layout(egui::Layout::top_down(egui::Align::Center), |ui| {
                        for btn in &sidebar_btns {
                            match btn {
                                ScopeButton::PauseResume => {
                                    let (icon, tooltip) = if all_paused {
                                        ("▶", "Resume")
                                    } else {
                                        ("⏸", "Pause")
                                    };
                                    if ui.button(icon).on_hover_text(tooltip).clicked() {
                                        clicked_btns.push(ScopeButton::PauseResume);
                                    }
                                }
                                ScopeButton::ClearAll => {
                                    let tt =
                                        format_button_tooltip("Clear All", hk.clear_all.as_ref());
                                    if ui.button(BROOM.to_string()).on_hover_text(tt).clicked() {
                                        clicked_btns.push(ScopeButton::ClearAll);
                                    }
                                }
                                ScopeButton::Scopes => {
                                    ui.button("🔭").on_hover_text("Scopes (use the top bar)");
                                }
                                other => {
                                    // Find panel info across all lists (immutable borrows only).
                                    let panel_info: Option<(bool, String, String)> = {
                                        let all = self
                                            .left_side_panels
                                            .iter()
                                            .chain(self.right_side_panels.iter())
                                            .chain(self.bottom_panels.iter())
                                            .chain(self.detached_panels.iter())
                                            .chain(self.empty_panels.iter());
                                        let mut found = None;
                                        for p in all {
                                            if other.matches_panel_title(p.title()) {
                                                let active =
                                                    p.state().visible && !p.state().detached;
                                                let icon =
                                                    p.icon_only().unwrap_or(p.title()).to_string();
                                                let hk_str = p
                                                    .hotkey_name()
                                                    .and_then(|n| get_hotkey_for_name(&hk, n));
                                                let tt = format_button_tooltip(p.title(), hk_str);
                                                found = Some((active, icon, tt));
                                                break;
                                            }
                                        }
                                        found
                                    };
                                    if let Some((active, icon, tt)) = panel_info {
                                        if ui
                                            .selectable_label(active, icon)
                                            .on_hover_text(tt)
                                            .clicked()
                                        {
                                            clicked_btns.push(other.clone());
                                        }
                                    }
                                }
                            }
                        }
                    });
                });

            // Apply icon-strip actions now that the closure (and its borrows) is done.
            for btn in clicked_btns {
                match btn {
                    ScopeButton::PauseResume => {
                        if all_paused {
                            for s in self.liveplot_panel.get_data_mut() {
                                s.paused = false;
                            }
                        } else {
                            for s in self.liveplot_panel.get_data_mut() {
                                s.paused = true;
                            }
                            self.traces_data.take_snapshot();
                        }
                    }
                    ScopeButton::ClearAll => {
                        self.traces_data.clear_all();
                        for s in self.liveplot_panel.get_data_mut() {
                            s.clicked_point = None;
                        }
                    }
                    other => {
                        // Toggle the matching panel.
                        for p in self
                            .left_side_panels
                            .iter_mut()
                            .chain(self.right_side_panels.iter_mut())
                            .chain(self.bottom_panels.iter_mut())
                            .chain(self.detached_panels.iter_mut())
                            .chain(self.empty_panels.iter_mut())
                        {
                            if other.matches_panel_title(p.title()) {
                                let st = p.state_mut();
                                let is_shown = st.visible && !st.detached;
                                st.visible = !is_shown;
                                st.detached = false;
                                if !is_shown {
                                    st.request_focus = true;
                                }
                                break;
                            }
                        }
                    }
                }
            }
        }

        // ── Sidebar panel content (left / right / bottom) ─────────────────────
        if layout.show_sidebar_panels {
            self.render_left_sidebar(ui);
            self.render_right_sidebar(ui, has_icon_strip);
            self.render_bottom_bar(ui);
        }

        // ── Detached windows (always shown regardless of responsive state) ────
        self.render_detached_windows(ui);
    }

    /// Render the left sidebar (panel dock or collapsed icon strip).
    fn render_left_sidebar(&mut self, ui: &mut egui::Ui) {
        let show_left = !self.left_side_panels.is_empty()
            && self
                .left_side_panels
                .iter()
                .any(|p| p.state().visible && !p.state().detached);

        if show_left {
            let mut list = std::mem::take(&mut self.left_side_panels);
            egui::SidePanel::left(format!("left_sidebar_{}", self.panel_id))
                .resizable(true)
                .default_width(280.0)
                .min_width(160.0)
                .show_inside(ui, |ui| {
                    egui::ScrollArea::vertical()
                        .scroll_bar_visibility(ScrollBarVisibility::AlwaysHidden)
                        .scroll_source(ScrollSource::NONE)
                        .show(ui, |ui| {
                            self.render_tabs(ui, &mut list);
                        });
                });
            self.left_side_panels = list;
        } else if !self.left_side_panels.is_empty() {
            let mut list = std::mem::take(&mut self.left_side_panels);
            let hk_rc_left = self.hotkeys.clone();
            egui::SidePanel::left(format!("left_sidebar_{}", self.panel_id))
                .resizable(true)
                .default_width(30.0)
                .min_width(30.0)
                .show_inside(ui, |ui| {
                    let hk = hk_rc_left.borrow();
                    egui::ScrollArea::vertical()
                        .scroll_bar_visibility(ScrollBarVisibility::AlwaysHidden)
                        .scroll_source(ScrollSource::NONE)
                        .show(ui, |ui| {
                            let mut clicked: Option<usize> = None;
                            ui.vertical(|ui| {
                                for (i, p) in list.iter_mut().enumerate() {
                                    let active = p.state().visible && !p.state().detached;
                                    let label = p.icon_only().unwrap_or(p.title()).to_string();
                                    let hotkey = p
                                        .hotkey_name()
                                        .and_then(|name| get_hotkey_for_name(&hk, name));
                                    let tooltip = format_button_tooltip(p.title(), hotkey);
                                    if ui
                                        .selectable_label(active, label)
                                        .on_hover_text(tooltip)
                                        .clicked()
                                    {
                                        clicked = Some(i);
                                    }
                                }
                            });
                            if let Some(ci) = clicked {
                                for (i, p) in list.iter_mut().enumerate() {
                                    if i == ci {
                                        p.state_mut().visible = true;
                                        p.state_mut().request_focus = true;
                                    } else if !p.state().detached {
                                        p.state_mut().visible = false;
                                    }
                                }
                            }
                        });
                });
            self.left_side_panels = list;
        }
    }

    /// Render the right sidebar (panel dock or collapsed icon strip).
    fn render_right_sidebar(&mut self, ui: &mut egui::Ui, has_icon_strip: bool) {
        let show_right = !self.right_side_panels.is_empty()
            && self
                .right_side_panels
                .iter()
                .any(|p| p.state().visible && !p.state().detached);

        if show_right {
            let mut list = std::mem::take(&mut self.right_side_panels);
            egui::SidePanel::right(format!("right_sidebar_{}", self.panel_id))
                .resizable(true)
                .default_width(320.0)
                .min_width(200.0)
                .show_inside(ui, |ui| {
                    self.render_tabs(ui, &mut list);
                });
            self.right_side_panels = list;
        } else if !self.right_side_panels.is_empty() && !has_icon_strip {
            // Only show the collapsed icon strip when there is no persistent
            // icon strip (which already provides this navigation).
            let mut list = std::mem::take(&mut self.right_side_panels);
            let hk_rc_right = self.hotkeys.clone();
            egui::SidePanel::right(format!("right_sidebar_{}", self.panel_id))
                .resizable(true)
                .default_width(30.0)
                .min_width(30.0)
                .show_inside(ui, |ui| {
                    let hk = hk_rc_right.borrow();
                    let mut clicked: Option<usize> = None;
                    ui.vertical(|ui| {
                        for (i, p) in list.iter_mut().enumerate() {
                            let active = p.state().visible && !p.state().detached;
                            let label = p.icon_only().unwrap_or(p.title()).to_string();
                            let hotkey = p
                                .hotkey_name()
                                .and_then(|name| get_hotkey_for_name(&hk, name));
                            let tooltip = format_button_tooltip(p.title(), hotkey);
                            if ui
                                .selectable_label(active, label)
                                .on_hover_text(tooltip)
                                .clicked()
                            {
                                clicked = Some(i);
                            }
                        }
                    });
                    if let Some(ci) = clicked {
                        for (i, p) in list.iter_mut().enumerate() {
                            if i == ci {
                                p.state_mut().visible = true;
                                p.state_mut().request_focus = true;
                            } else if !p.state().detached {
                                p.state_mut().visible = false;
                            }
                        }
                    }
                });
            self.right_side_panels = list;
        }
    }

    /// Render the bottom panel bar (expanded or collapsed).
    fn render_bottom_bar(&mut self, ui: &mut egui::Ui) {
        let show_bottom = !self.bottom_panels.is_empty()
            && self
                .bottom_panels
                .iter()
                .any(|p| p.state().visible && !p.state().detached);

        if show_bottom {
            let mut list = std::mem::take(&mut self.bottom_panels);
            egui::TopBottomPanel::bottom(format!("bottom_bar_{}", self.panel_id))
                .resizable(true)
                .default_height(220.0)
                .min_height(120.0)
                .show_inside(ui, |ui| {
                    self.render_tabs(ui, &mut list);
                });
            self.bottom_panels = list;
        } else if !self.bottom_panels.is_empty() {
            let mut list = std::mem::take(&mut self.bottom_panels);
            let hk_rc_bottom = self.hotkeys.clone();
            egui::TopBottomPanel::bottom(format!("bottom_bar_{}", self.panel_id))
                .resizable(false)
                .default_height(24.0)
                .min_height(24.0)
                .show_inside(ui, |ui| {
                    let hk = hk_rc_bottom.borrow();
                    let mut clicked: Option<usize> = None;
                    ui.add_space(2.0);
                    ui.horizontal(|ui| {
                        for (i, p) in list.iter_mut().enumerate() {
                            let label = p.title_and_icon();
                            let hotkey = p
                                .hotkey_name()
                                .and_then(|name| get_hotkey_for_name(&hk, name));
                            let tooltip = format_button_tooltip(p.title(), hotkey);
                            if ui.button(label).on_hover_text(tooltip).clicked() {
                                clicked = Some(i);
                            }
                        }
                    });
                    if let Some(ci) = clicked {
                        for (i, p) in list.iter_mut().enumerate() {
                            if i == ci {
                                p.state_mut().visible = true;
                                p.state_mut().request_focus = true;
                            } else if !p.state().detached {
                                p.state_mut().visible = false;
                            }
                        }
                    }
                });
            self.bottom_panels = list;
        }
    }

    /// Render detached (floating) windows for all panel lists.
    ///
    /// Detached windows are always shown regardless of the responsive layout state.
    fn render_detached_windows(&mut self, ui: &mut egui::Ui) {
        for p in &mut self.left_side_panels {
            if p.state().visible && p.state().detached {
                p.show_detached_dialog(
                    ui.ctx(),
                    &mut LivePlotData {
                        scope_data: self.liveplot_panel.get_data_mut(),
                        traces: &mut self.traces_data,
                        pending_requests: &mut self.pending_requests,
                        event_ctrl: self.event_ctrl.clone(),
                    },
                );
            }
        }

        for p in &mut self.right_side_panels {
            if p.state().visible && p.state().detached {
                p.show_detached_dialog(
                    ui.ctx(),
                    &mut LivePlotData {
                        scope_data: self.liveplot_panel.get_data_mut(),
                        traces: &mut self.traces_data,
                        pending_requests: &mut self.pending_requests,
                        event_ctrl: self.event_ctrl.clone(),
                    },
                );
            }
        }

        for p in &mut self.bottom_panels {
            if p.state().visible && p.state().detached {
                p.show_detached_dialog(
                    ui.ctx(),
                    &mut LivePlotData {
                        scope_data: self.liveplot_panel.get_data_mut(),
                        traces: &mut self.traces_data,
                        pending_requests: &mut self.pending_requests,
                        event_ctrl: self.event_ctrl.clone(),
                    },
                );
            }
        }

        for p in &mut self.detached_panels {
            if p.state().visible && p.state().detached {
                p.show_detached_dialog(
                    ui.ctx(),
                    &mut LivePlotData {
                        scope_data: self.liveplot_panel.get_data_mut(),
                        traces: &mut self.traces_data,
                        pending_requests: &mut self.pending_requests,
                        event_ctrl: self.event_ctrl.clone(),
                    },
                );
            }
        }
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Shared tab rendering
    // ─────────────────────────────────────────────────────────────────────────

    /// Render a tab strip followed by the active panel's body.
    ///
    /// Used by left, right, and bottom sidebars.  When there is only one panel
    /// in `list`, the tab header shows a simple label instead of selectable tabs.
    /// The header also contains "Pop out" and "Hide" action buttons.
    pub(crate) fn render_tabs(&mut self, ui: &mut egui::Ui, list: &mut Vec<Box<dyn Panel>>) {
        let count = list.len();

        let mut clicked: Option<usize> = None;

        let hk_rc_tabs = self.hotkeys.clone();

        let (add_scope_req, remove_scope_req) = {
            let scope_data = self.liveplot_panel.get_data_mut();
            let data = &mut LivePlotData {
                scope_data,
                traces: &mut self.traces_data,
                pending_requests: &mut self.pending_requests,
                event_ctrl: self.event_ctrl.clone(),
            };

            if count > 0 {
                // Honor focus requests from panels (request_docket): make that
                // panel the active attached tab.
                if let Some(req_idx) = list.iter().enumerate().find_map(|(i, p)| {
                    if p.state().request_docket {
                        Some(i)
                    } else {
                        None
                    }
                }) {
                    for (j, p) in list.iter_mut().enumerate() {
                        if j == req_idx {
                            let st = p.state_mut();
                            st.visible = true;
                            st.detached = false;
                            st.request_docket = false;
                        } else if !p.state().detached {
                            p.state_mut().visible = false;
                        }
                    }
                }

                // Compute whether tabs should collapse to icon-only.
                let available = ui.available_width();
                let button_font = egui::TextStyle::Button.resolve(ui.style());
                let txt_width = |text: &str, ui: &egui::Ui| -> f32 {
                    ui.fonts_mut(|f| {
                        f.layout_no_wrap(text.to_owned(), button_font.clone(), egui::Color32::WHITE)
                            .rect
                            .width()
                    })
                };
                let pad = ui.spacing().button_padding.x * 2.0 + ui.spacing().item_spacing.x;

                let actions_w = txt_width("Pop out", ui) + pad + txt_width("Hide", ui) + pad;

                let full_tabs_w: f32 = match count {
                    0 => 0.0,
                    1 => txt_width(&list[0].title_and_icon(), ui) + pad,
                    _ => list
                        .iter()
                        .map(|p| txt_width(&p.title_and_icon(), ui) + pad)
                        .sum(),
                };

                let icon_tabs_w: f32 = match count {
                    0 => 0.0,
                    1 => {
                        let label = list[0]
                            .icon_only()
                            .map(|s| s.to_string())
                            .unwrap_or_else(|| list[0].title_and_icon());
                        txt_width(&label, ui) + pad
                    }
                    _ => list
                        .iter()
                        .map(|p| {
                            let label = p
                                .icon_only()
                                .map(|s| s.to_string())
                                .unwrap_or_else(|| p.title_and_icon());
                            txt_width(&label, ui) + pad
                        })
                        .sum(),
                };

                let use_icon_only = full_tabs_w + actions_w > available;
                let wrap_tabs = use_icon_only && (icon_tabs_w + actions_w > available);

                // Pre-compute per-panel tooltips (title + hotkey hint) once.
                let hk = hk_rc_tabs.borrow();
                let tooltips: Vec<String> = list
                    .iter()
                    .map(|p| {
                        let hotkey = p
                            .hotkey_name()
                            .and_then(|name| get_hotkey_for_name(&hk, name));
                        format_button_tooltip(p.title(), hotkey)
                    })
                    .collect();
                drop(hk);

                // Render header: actions pinned right, tabs left.
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Min), |ui| {
                    // Right: actions
                    if ui.button("Hide").clicked() {
                        for p in list.iter_mut() {
                            if !p.state().detached {
                                p.state_mut().visible = false;
                            }
                        }
                    }
                    if ui.button("Pop out").clicked() {
                        for p in list.iter_mut() {
                            if p.state().visible && !p.state().detached {
                                p.state_mut().detached = true;
                                p.state_mut().request_docket = false;
                                p.state_mut().visible = true;
                                p.state_mut().request_focus = true;
                            }
                        }
                    }

                    // Left: tabs in remaining width
                    ui.with_layout(egui::Layout::left_to_right(egui::Align::Min), |ui| {
                        let render_tabs =
                            |ui: &mut egui::Ui,
                             list: &mut Vec<Box<dyn Panel>>,
                             clicked: &mut Option<usize>,
                             use_icon_only: bool,
                             count: usize,
                             tooltips: &[String]| {
                                if count > 1 {
                                    for (i, p) in list.iter_mut().enumerate() {
                                        let active = p.state().visible && !p.state().detached;
                                        let tooltip =
                                            tooltips.get(i).map(|s| s.as_str()).unwrap_or("");

                                        let label = if use_icon_only {
                                            p.icon_only()
                                                .map(|s| s.to_string())
                                                .unwrap_or_else(|| p.title_and_icon())
                                        } else {
                                            p.title_and_icon()
                                        };

                                        let mut resp = ui.selectable_label(active, label);
                                        if !tooltip.is_empty() {
                                            resp = resp.on_hover_text(tooltip);
                                        }
                                        if resp.clicked() {
                                            *clicked = Some(i);
                                        }
                                    }
                                } else {
                                    let p = &mut list[0];
                                    let tooltip =
                                        tooltips.first().map(|s| s.as_str()).unwrap_or("");
                                    let label = if use_icon_only {
                                        p.icon_only()
                                            .map(|s| s.to_string())
                                            .unwrap_or_else(|| p.title_and_icon())
                                    } else {
                                        p.title_and_icon()
                                    };
                                    if !tooltip.is_empty() {
                                        ui.label(label).on_hover_text(tooltip);
                                    } else {
                                        ui.label(label);
                                    }
                                    *clicked = Some(0);
                                }
                            };

                        if wrap_tabs {
                            ui.horizontal_wrapped(|ui| {
                                render_tabs(
                                    ui,
                                    list,
                                    &mut clicked,
                                    use_icon_only,
                                    count,
                                    &tooltips,
                                );
                            });
                        } else {
                            ui.horizontal(|ui| {
                                render_tabs(
                                    ui,
                                    list,
                                    &mut clicked,
                                    use_icon_only,
                                    count,
                                    &tooltips,
                                );
                            });
                        }
                    });
                });

                // Apply clicked selection when multiple tabs are present.
                if count > 1 {
                    if let Some(i) = clicked {
                        for (j, p) in list.iter_mut().enumerate() {
                            if j == i {
                                p.state_mut().visible = true;
                                p.state_mut().detached = false;
                            } else if !p.state().detached {
                                p.state_mut().visible = false;
                            }
                        }
                    }
                }
            }

            ui.separator();

            // Body: render the first attached+visible panel.
            if let Some((idx, _)) = list
                .iter()
                .enumerate()
                .find(|(_i, p)| p.state().visible && !p.state().detached)
            {
                let p = &mut list[idx];
                p.render_panel(ui, data);
            } else {
                ui.label("No panel active");
            }
            (
                std::mem::take(&mut data.pending_requests.add_scope),
                data.pending_requests.remove_scope.take(),
            )
        };

        // Apply any scope add/remove requests issued by the rendered panel(s).
        if add_scope_req {
            self.liveplot_panel.add_scope();
        }
        if let Some(scope_id) = remove_scope_req {
            let _ = self.liveplot_panel.remove_scope_by_id(scope_id);
        }
    }
}
