use eframe::egui;
use super::app::ScopeAppMulti;

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

}

// Per-panel state structs are defined in their respective modules (math_ui, traces_ui, thresholds_ui)

/// Trait that abstracts a dockable panel (Traces, Math, Thresholds).
pub trait DockPanel {
    /// Retrieve a mutable reference to this panel from the app
    fn get_mut(app: &mut ScopeAppMulti) -> &mut Self where Self: Sized;
    /// Access this panel's DockState through self
    fn dock_mut(&mut self) -> &mut DockState;
    /// Called when rendering the panel's content
    fn panel_contents(app: &mut ScopeAppMulti, ui: &mut egui::Ui);
    /// Called when the Dock button is pressed (to reattach to the sidebar)
    fn on_dock(app: &mut ScopeAppMulti);

    /// Generic renderer for a DockPanel's detached dialog.
    fn show_detached_dialog(app: &mut ScopeAppMulti, ctx: &egui::Context) where Self: Sized {
        // Read minimal window state in a short borrow scope to avoid conflicts
        let (title, mut show_flag) = {
            let dock: &mut DockState = Self::get_mut(app).dock_mut();
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
                Self::panel_contents(app, ui);
            });

        // Write back state changes without overlapping borrows
        if dock_clicked {
            Self::on_dock(app);
            let dock = Self::get_mut(app).dock_mut();
            dock.detached = false;
            dock.show_dialog = false;
        } else {
            let dock = Self::get_mut(app).dock_mut();
            if !show_flag {
                // If window was closed externally, clear detached flag
                dock.detached = false;
            }
            dock.show_dialog = show_flag;
        }
    }
}

// DockPanel implementors (MathDockPanel, TracesDockPanel, ThresholdsDockPanel) live next to each panel

// Implementors (MathPanel, TracesPanel, ThresholdsPanel) provide get_mut and dock_mut; the default
// show_detached_dialog implementation above handles the dialog lifecycle.
