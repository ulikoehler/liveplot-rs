use eframe::egui;
use std::collections::HashMap;

use super::app::ScopeAppMulti;
use super::types::{MathBuilderState, ThresholdBuilderState, TraceLook};

/// Shared docking state for dockable panels (Traces, Math, Thresholds).
#[derive(Debug, Clone)]
pub struct DockState {
    /// Whether this panel is currently shown as a detached window
    pub detached: bool,
    /// Whether to show the dialog/window (only relevant when detached)
    pub show_dialog: bool,
    /// Window title for the detached panel
    pub title: &'static str,
}

impl DockState {
    pub fn new(title: &'static str) -> Self {
        Self { detached: false, show_dialog: false, title }
    }

    /// Show a simple header with a Dock button that calls the provided closure on click.
    #[allow(dead_code)]
    pub fn dock_button_row<F: FnOnce()>(&mut self, ui: &mut egui::Ui, on_dock: F) {
        ui.horizontal(|ui| {
            ui.strong(self.title);
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui
                    .button("Dock")
                    .on_hover_text("Attach this panel to the right sidebar")
                    .clicked()
                {
                    on_dock();
                }
            });
        });
    }
}

#[derive(Debug, Clone)]
pub struct MathPanelState {
    pub dock: DockState,
    pub builder: MathBuilderState,
    pub editing: Option<String>,
    pub error: Option<String>,
    pub creating: bool,
}

impl Default for MathPanelState {
    fn default() -> Self {
        Self {
            dock: DockState::new("Math traces"),
            builder: MathBuilderState::default(),
            editing: None,
            error: None,
            creating: false,
        }
    }
}

#[derive(Debug, Clone)]
pub struct TracesPanelState {
    pub dock: DockState,
    pub look_editor_trace: Option<String>,
}

impl Default for TracesPanelState {
    fn default() -> Self {
        Self { dock: DockState::new("Traces"), look_editor_trace: None }
    }
}

#[derive(Debug, Clone)]
pub struct ThresholdsPanelState {
    pub dock: DockState,
    pub builder: ThresholdBuilderState,
    pub editing: Option<String>,
    pub error: Option<String>,
    pub creating: bool,
    pub looks: HashMap<String, TraceLook>,
    pub start_looks: HashMap<String, TraceLook>,
    pub stop_looks: HashMap<String, TraceLook>,
    pub events_filter: Option<String>,
}

impl Default for ThresholdsPanelState {
    fn default() -> Self {
        Self {
            dock: DockState::new("Thresholds"),
            builder: ThresholdBuilderState::default(),
            editing: None,
            error: None,
            creating: false,
            looks: HashMap::new(),
            start_looks: HashMap::new(),
            stop_looks: HashMap::new(),
            events_filter: None,
        }
    }
}

/// Helper: Render a compact header for an in-sidebar panel with a Pop out button.
#[allow(dead_code)]
pub fn panel_header_with_popout(dock: &mut DockState, ui: &mut egui::Ui) {
    ui.horizontal(|ui| {
        ui.strong(dock.title);
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui
                .button("Pop out")
                .on_hover_text(format!("Open {} panel in a floating window", dock.title))
                .clicked()
            {
                dock.detached = true;
                dock.show_dialog = true;
            }
        });
    });
    ui.separator();
}

/// Helper: Show a detached window for a panel with standard Dock header and content.
/// The `on_dock` closure is called when the Dock button is pressed to let the caller
/// reattach the panel to the sidebar and adjust any app-level state.
#[allow(dead_code)]
pub fn show_detached_panel_window<F: FnOnce(&mut egui::Ui)>(
    dock: &mut DockState,
    ctx: &egui::Context,
    mut on_dock: impl FnMut(),
    content: F,
) {
    let mut show_flag = dock.show_dialog;
    egui::Window::new(dock.title)
        .open(&mut show_flag)
        .show(ctx, |ui| {
            let mut dock_clicked = false;
            dock.dock_button_row(ui, || {
                dock_clicked = true;
            });
            if dock_clicked {
                // Let caller handle reattaching state
                on_dock();
                dock.detached = false;
                dock.show_dialog = false;
            }
            ui.separator();
            content(ui);
        });
    // If window was closed externally, clear detached flag
    if !show_flag {
        dock.detached = false;
    }
    dock.show_dialog = show_flag;
}

/// Trait that abstracts a dockable panel (Traces, Math, Thresholds).
pub trait DockPanel {
    /// Access this panel's DockState inside the app
    fn dock_mut(app: &mut ScopeAppMulti) -> &mut DockState;
    /// Called when rendering the panel's content
    fn panel_contents(app: &mut ScopeAppMulti, ui: &mut egui::Ui);
    /// Called when the Dock button is pressed (to reattach to the sidebar)
    fn on_dock(app: &mut ScopeAppMulti);
}

pub struct MathDockPanel;
pub struct TracesDockPanel;
pub struct ThresholdsDockPanel;

impl DockPanel for MathDockPanel {
    fn dock_mut(app: &mut ScopeAppMulti) -> &mut DockState { &mut app.math_panel.dock }
    fn panel_contents(app: &mut ScopeAppMulti, ui: &mut egui::Ui) {
        super::math_ui::math_panel_contents(app, ui);
    }
    fn on_dock(app: &mut ScopeAppMulti) {
        app.right_panel_active_tab = super::app::RightTab::Math;
        app.right_panel_visible = true;
    }
}

impl DockPanel for TracesDockPanel {
    fn dock_mut(app: &mut ScopeAppMulti) -> &mut DockState { &mut app.traces_panel.dock }
    fn panel_contents(app: &mut ScopeAppMulti, ui: &mut egui::Ui) {
        super::traces_ui::traces_panel_contents(app, ui);
    }
    fn on_dock(app: &mut ScopeAppMulti) {
        app.right_panel_active_tab = super::app::RightTab::Traces;
        app.right_panel_visible = true;
    }
}

impl DockPanel for ThresholdsDockPanel {
    fn dock_mut(app: &mut ScopeAppMulti) -> &mut DockState { &mut app.thresholds_panel.dock }
    fn panel_contents(app: &mut ScopeAppMulti, ui: &mut egui::Ui) {
        super::thresholds_ui::thresholds_panel_contents(app, ui);
    }
    fn on_dock(app: &mut ScopeAppMulti) {
        app.right_panel_active_tab = super::app::RightTab::Thresholds;
        app.right_panel_visible = true;
    }
}

/// Generic renderer for a DockPanel's detached dialog.
pub fn show_detached_dialog<P: DockPanel>(app: &mut ScopeAppMulti, ctx: &egui::Context) {
    // Copy minimal state out to avoid borrowing conflicts while rendering
    let (title, mut show_flag) = {
        let dock = P::dock_mut(app);
        (dock.title, dock.show_dialog)
    };

    let mut dock_clicked = false;
    egui::Window::new(title)
        .open(&mut show_flag)
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.strong(title);
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui
                        .button("Dock")
                        .on_hover_text("Attach this panel to the right sidebar")
                        .clicked()
                    {
                        dock_clicked = true;
                    }
                });
            });
            ui.separator();
            P::panel_contents(app, ui);
        });

    // Write back state changes without overlapping borrows
    if dock_clicked {
        P::on_dock(app);
        let dock = P::dock_mut(app);
        dock.detached = false;
        dock.show_dialog = false;
    } else {
        let dock = P::dock_mut(app);
        if !show_flag {
            // If window was closed externally, clear detached flag
            dock.detached = false;
        }
        dock.show_dialog = show_flag;
    }
}
