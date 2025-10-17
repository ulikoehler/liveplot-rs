use egui::{Ui, Context};

use crate::config::XDateFormat;
use crate::data::DataContext;

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
    fn render_menu(&mut self, _ui: &mut Ui, _data: &mut DataContext) {}
    fn render_panel(&mut self, _ui: &mut Ui, _data: &mut DataContext) {}
    fn draw(&mut self, _ui: &mut Ui, _ctx: &Context, _x_fmt: XDateFormat, _data: &DataContext) {}
    fn calculate(&mut self, _data: &mut DataContext) {}
}
