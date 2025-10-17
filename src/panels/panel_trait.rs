use egui::{Ui, Context};

use crate::config::XDateFormat;

#[derive(Debug, Clone, Copy, Default)]
pub struct PanelState {
    pub visible: bool,
    pub detached: bool,
}

pub trait Panel {
    fn name(&self) -> &'static str;
    fn state(&self) -> &PanelState;
    fn state_mut(&mut self) -> &mut PanelState;

    // Optional hooks with default empty impls
    fn render_menu(&mut self, _ui: &mut Ui) {}
    fn render_panel(&mut self, _ui: &mut Ui) {}
    fn draw(&mut self, _ui: &mut Ui, _ctx: &Context, _x_fmt: XDateFormat) {}
    fn calculate(&mut self) {}
}
