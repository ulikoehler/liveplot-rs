use egui::{Context, Ui};
use egui_plot::PlotUi;

use crate::data::data::LivePlotData;
use crate::data::scope::ScopeData;
use crate::data::traces::TracesCollection;
use std::any::Any;

#[derive(Debug, Clone, Copy, Default)]
pub struct PanelState {
    pub title: &'static str,
    pub visible: bool,
    pub detached: bool,
    pub request_docket: bool,
    pub window_pos: Option<[f32; 2]>,
    pub window_size: Option<[f32; 2]>,
}

impl PanelState {
    pub fn new(title: &'static str) -> Self {
        Self {
            title,
            visible: false,
            detached: false,
            request_docket: false,
            window_pos: None,
            window_size: None,
        }
    }
}

pub trait Panel: Any {
    fn title(&self) -> &'static str {
        self.state().title
    }

    fn state(&self) -> &PanelState;
    fn state_mut(&mut self) -> &mut PanelState;

    // For downcasting to concrete panel types in persistence and other cross-cutting features
    fn as_any_mut(&mut self) -> &mut dyn Any
    where
        Self: 'static + Sized,
    {
        self as &mut dyn Any
    }

    // Optional hooks with default empty impls
    fn render_menu(&mut self, _ui: &mut Ui, _data: &mut LivePlotData<'_>) {}
    fn render_panel(&mut self, _ui: &mut Ui, _data: &mut LivePlotData<'_>) {}
    fn draw(&mut self, _plot_ui: &mut PlotUi, _scope: &ScopeData, _traces: &TracesCollection) {}

    fn update_data(&mut self, _data: &mut LivePlotData<'_>) {}

    // fn panel_contents(&mut self, _ui: &mut Ui, _data: &mut ScopeData) {}

    fn show_detached_dialog(&mut self, ctx: &Context, data: &mut LivePlotData<'_>) {
        // Read minimal window state in a short borrow scope to avoid conflicts
        let (title, mut show_flag) = {
            let dock: &mut PanelState = self.state_mut();
            (dock.title, dock.visible)
        };

        let mut dock_clicked = false;
        let mut win = egui::Window::new(title).open(&mut show_flag);
        // Apply persisted position/size if available
        {
            let st_ro = self.state();
            if let Some(pos) = st_ro.window_pos {
                win = win.default_pos(egui::pos2(pos[0], pos[1]));
            }
            if let Some(sz) = st_ro.window_size {
                win = win.default_size(egui::vec2(sz[0], sz[1]));
            }
        }
        let resp = win.show(ctx, |ui| {
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
        // Capture window position and size from the response if available
        if let Some(ir) = &resp {
            let rect = ir.response.rect;
            state.window_pos = Some([rect.min.x, rect.min.y]);
            state.window_size = Some([rect.size().x, rect.size().y]);
        }
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
