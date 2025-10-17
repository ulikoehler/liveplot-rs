use egui::Ui;
use super::panel_trait::{Panel, PanelState};

pub struct TracesPanel { pub state: PanelState }
impl Default for TracesPanel { fn default() -> Self { Self { state: PanelState { visible: true, detached: false } } } }
impl Panel for TracesPanel {
    fn name(&self) -> &'static str { "Traces" }
    fn state(&self) -> &PanelState { &self.state }
    fn state_mut(&mut self) -> &mut PanelState { &mut self.state }
    fn render_panel(&mut self, ui: &mut Ui) { ui.label("Traces settings placeholder"); }
}
