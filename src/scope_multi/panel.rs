use eframe::egui;
use super::app::LivePlotApp;

/// Shared docking state for dockable panels (Traces, Math, Thresholds).
#[derive(Debug, Clone)]
pub struct DockState {
    /// Whether this panel is currently shown as a detached window
    pub detached: bool,
    /// Whether to show the dialog/window (only relevant when detached)
    pub show_dialog: bool,
    /// To signal docking back to sidebar
    pub focus_dock: bool,
    /// Window title for the detached panel
    pub title: &'static str,
}

impl DockState {
    pub fn new(title: &'static str) -> Self {
        Self { detached: false, show_dialog: false, focus_dock: false, title }
    }

}

// Per-panel state structs are defined in their respective modules (math_ui, traces_ui, thresholds_ui)

/// Trait that abstracts a dockable panel (Traces, Math, Thresholds).
pub trait DockPanel {
    /// Access this panel's DockState through self
    fn dock_mut(&mut self) -> &mut DockState;
    /// Called when rendering the panel's content
    fn panel_contents(&mut self, app: &mut LivePlotApp, ui: &mut egui::Ui);

    /// Generic renderer for a DockPanel's detached dialog.
    fn show_detached_dialog(&mut self, app: &mut LivePlotApp, ctx: &egui::Context) {
        // Read minimal window state in a short borrow scope to avoid conflicts
        let (title, mut show_flag) = {
            let dock: &mut DockState = self.dock_mut();
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
                // Render contents (may mutate app extensively)
                self.panel_contents(app, ui);
            });

        // Write back state changes without overlapping borrows
        if dock_clicked {
            let dock = self.dock_mut();
            dock.detached = false;
            // Closing the detached window after docking back to sidebar
            dock.show_dialog = true;
            dock.focus_dock = true;
        } else {
            let dock = self.dock_mut();
            if !show_flag {
                // If window was closed externally, clear detached flag
                dock.detached = false;
            }
            dock.show_dialog = show_flag;
        }
    }
}
