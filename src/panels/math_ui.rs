use egui::Ui;
use super::panel_trait::{Panel, PanelState};

pub struct MathPanel { pub state: PanelState }
impl Default for MathPanel { fn default() -> Self { Self { state: PanelState { visible: false, detached: false } } } }
impl Panel for MathPanel {
    fn name(&self) -> &'static str { "Math" }
    fn state(&self) -> &PanelState { &self.state }
    fn state_mut(&mut self) -> &mut PanelState { &mut self.state }
    fn render_panel(&mut self, ui: &mut Ui) { ui.label("Math builder placeholder"); }
}
