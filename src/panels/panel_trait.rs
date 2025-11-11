use egui::{Context, Ui};
use egui_plot::PlotUi;

use crate::data::data::LivePlotData;
use crate::data::scope::ScopeData;
use crate::data::traces::TracesCollection;
use downcast_rs::{impl_downcast, Downcast};

#[derive(Debug, Clone, Copy, Default)]
pub struct PanelState {
    pub title: &'static str,
    pub visible: bool,
    pub detached: bool,
    pub request_docket: bool,
    pub window_pos: Option<[f32; 2]>,
    pub window_size: Option<[f32; 2]>,
    // If set, the panel is shown in an external OS window with this ViewportId
    pub viewport_id: Option<egui::ViewportId>,
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
            viewport_id: None,
        }
    }
}

pub trait Panel: Downcast {
    fn title(&self) -> &'static str {
        self.state().title
    }

    fn state(&self) -> &PanelState;
    fn state_mut(&mut self) -> &mut PanelState;

    // Optional hooks with default empty impls
    fn render_menu(&mut self, _ui: &mut Ui, _data: &mut LivePlotData<'_>) {}
    fn render_panel(&mut self, _ui: &mut Ui, _data: &mut LivePlotData<'_>) {}
    fn draw(&mut self, _plot_ui: &mut PlotUi, _scope: &ScopeData, _traces: &TracesCollection) {}

    fn update_data(&mut self, _data: &mut LivePlotData<'_>) {}

    // Clear all internal runtime state / events / buffers specific to the panel.
    // Default: no-op. Panels with internal collections override this.
    fn clear_all(&mut self) {}

    // fn panel_contents(&mut self, _ui: &mut Ui, _data: &mut ScopeData) {}

    fn show_detached_dialog(&mut self, ctx: &Context, data: &mut LivePlotData<'_>) {
        // Read minimal window state in a short borrow scope to avoid conflicts
        let (title, vis, pos, size, vid_opt) = {
            let st = self.state();
            (st.title, st.visible, st.window_pos, st.window_size, st.viewport_id)
        };

        // Ensure a stable viewport id for this panel
        let vid = vid_opt.unwrap_or_else(|| egui::ViewportId::from_hash_of(&(title, "panel")));

        // Persist the id back to state
        {
            let st = self.state_mut();
            st.viewport_id = Some(vid);
        }

        // Build viewport with persisted geometry if present
        let mut builder = egui::ViewportBuilder::default().with_title(title);
        if let Some(sz) = size { builder = builder.with_inner_size([sz[0], sz[1]]); }
        if let Some(p) = pos { builder = builder.with_position([p[0], p[1]]); }

        // Show new viewport (external if supported, embedded otherwise)
        ctx.show_viewport_immediate(vid, builder, |vctx, class| {
            // If the OS window was closed, hide and re-dock the panel
            let close = vctx.input(|i| i.viewport().close_requested());
            if close {
                let st = self.state_mut();
                st.detached = false;
                st.visible = false;
                return;
            }

            let mut dock_clicked = false;

            let mut draw_ui = |ui: &mut Ui| {
                ui.horizontal(|ui| {
                    ui.strong(title);
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button("Dock").clicked() {
                            dock_clicked = true;
                        }
                    });
                });
                ui.separator();
                self.render_panel(ui, data);
            };

            match class {
                egui::ViewportClass::Embedded => {
                    // In backends without multi-viewport support, embed as a normal egui window
                    let mut show_flag = vis;
                    let mut win = egui::Window::new(title).open(&mut show_flag);
                    // Apply persisted position/size if available
                    if let Some(p) = pos { win = win.default_pos(egui::pos2(p[0], p[1])); }
                    if let Some(sz) = size { win = win.default_size(egui::vec2(sz[0], sz[1])); }
                    let resp = win.show(vctx, |ui| draw_ui(ui));

                    // Write back state changes without overlapping borrows
                    let st = self.state_mut();
                    if let Some(ir) = &resp {
                        let rect = ir.response.rect;
                        st.window_pos = Some([rect.min.x, rect.min.y]);
                        st.window_size = Some([rect.size().x, rect.size().y]);
                    }
                    if dock_clicked {
                        st.detached = false;
                        st.visible = true;
                        st.request_docket = true;
                    } else {
                        if !show_flag {
                            st.detached = false;
                        }
                        st.visible = show_flag;
                    }
                }
                _ => {
                    // External OS window: render content in the child viewport
                    egui::CentralPanel::default().show(vctx, |ui| draw_ui(ui));
                    if dock_clicked {
                        let st = self.state_mut();
                        st.detached = false;
                        st.visible = true;
                        st.request_docket = true;
                    }
                }
            }
        });
    }
}

impl_downcast!(Panel);
