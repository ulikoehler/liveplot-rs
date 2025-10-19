use egui::{Ui, Context};

use crate::config::XDateFormat;
use crate::data::scope::ScopeData;

#[derive(Debug, Clone, Copy, Default)]
pub struct PanelState {
    pub title: &'static str,
    pub visible: bool,
    pub detached: bool,
    pub request_docket: bool,
}

pub trait Panel {
    fn title(&self) -> &'static str{
        self.state().title
    }

    fn state(&self) -> &PanelState;
    fn state_mut(&mut self) -> &mut PanelState;

    // Optional hooks with default empty impls
    fn render_menu(&mut self, _ui: &mut Ui, _data: &mut ScopeData) {}
    fn render_panel(&mut self, _ui: &mut Ui, _data: &mut ScopeData) {}
    fn draw(&mut self, _ui: &mut Ui, _ctx: &Context, _x_fmt: XDateFormat, _data: &ScopeData) {}
    fn update_data(&mut self, _data: &mut ScopeData) {}

    fn panel_contents(&mut self, _ui: &mut Ui, _data: &mut ScopeData) {}

    fn show_detached_dialog(&mut self,  _ui: &mut Ui, _data: &mut ScopeData) {
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
                        if ui
                            .button("Dock")
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
