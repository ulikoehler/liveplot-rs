use egui::{Ui, Context};
use egui_plot::{Plot, Line, Points};

use crate::config::XDateFormat;

use super::panel_trait::{Panel, PanelState};

pub struct ScopePanel {
    pub state: PanelState,
}
impl Default for ScopePanel { fn default() -> Self { Self { state: PanelState { visible: true, detached: false } } } }

impl Panel for ScopePanel {
    fn name(&self) -> &'static str { "Scope" }
    fn state(&self) -> &PanelState { &self.state }
    fn state_mut(&mut self) -> &mut PanelState { &mut self.state }
    fn render_panel(&mut self, ui: &mut Ui) {
        ui.label("Scope plot placeholder");
    }
    fn draw(&mut self, ui: &mut Ui, _ctx: &Context, _x_fmt: XDateFormat) {
        ui.label("draw(): would render traces here");
    }
}
