use egui::Ui;
use super::panel_trait::{Panel, PanelState};

pub struct ThresholdsPanel { pub state: PanelState }
impl Default for ThresholdsPanel { fn default() -> Self { Self { state: PanelState { visible: false, detached: false } } } }
impl Panel for ThresholdsPanel {
    fn name(&self) -> &'static str { "Thresholds" }
    fn state(&self) -> &PanelState { &self.state }
    fn state_mut(&mut self) -> &mut PanelState { &mut self.state }
    fn render_panel(&mut self, ui: &mut Ui) { ui.label("Thresholds configuration placeholder"); }
}
