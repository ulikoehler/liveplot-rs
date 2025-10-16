use eframe::egui;
use std::collections::HashMap;

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
    pub fn dock_button_row<F: FnOnce()>(&mut self, ui: &mut egui::Ui, on_dock: F) {
        ui.horizontal(|ui| {
            ui.strong(self.title);
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button("Dock").on_hover_text("Attach this panel to the right sidebar").clicked() {
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
