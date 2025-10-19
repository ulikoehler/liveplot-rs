use egui::{Context, Ui};
use egui_plot::PlotUi;

use crate::data::scope::ScopeData;

#[derive(Debug, Clone, Copy, Default)]
pub struct PanelState {
    pub title: &'static str,
    pub visible: bool,
    pub detached: bool,
    pub request_docket: bool,
}

pub trait Panel {
    fn title(&self) -> &'static str {
        self.state().title
    }

    fn state(&self) -> &PanelState;
    fn state_mut(&mut self) -> &mut PanelState;

    // Optional hooks with default empty impls
    fn render_menu(&mut self, _ui: &mut Ui, _data: &mut ScopeData) {}
    fn render_panel(&mut self, _ui: &mut Ui, _data: &mut ScopeData) {}
    fn draw(&mut self, _ui: &mut PlotUi, _data: &ScopeData) {}

    fn update_data(&mut self, _data: &mut ScopeData) {}

    // fn panel_contents(&mut self, _ui: &mut Ui, _data: &mut ScopeData) {}

    fn show_detached_dialog(&mut self, ctx: &Context, data: &mut ScopeData) {
        // Read minimal window state in a short borrow scope to avoid conflicts
        let (title, mut show_flag) = {
            let dock: &mut PanelState = self.state_mut();
            (dock.title, dock.visible)
        };

        let mut dock_clicked = false;
        egui::Window::new(title)
            .open(&mut show_flag)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.strong(title);
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button("Dock").clicked() {
                            dock_clicked = true;
                        }
                    });
                });
                ui.separator();
                // Render contents (may mutate app extensively)
                self.render_panel(ui, data);
            });

        // Write back state changes without overlapping borrows
        let state = self.state_mut();
        if dock_clicked {
            state.detached = false;
            // Closing the detached window after docking back to sidebar
            state.visible = true;
            state.request_docket = true;
        } else {
            if !show_flag {
                // If window was closed externally, clear detached flag
                state.detached = false;
            }
            state.visible = show_flag;
        }
    }
}
