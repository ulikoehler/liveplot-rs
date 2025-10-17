use egui::Ui;
use super::panel_trait::{Panel, PanelState};

pub struct ExportPanel { pub state: PanelState }
impl Default for ExportPanel { fn default() -> Self { Self { state: PanelState { visible: false, detached: false } } } }
impl Panel for ExportPanel {
    fn name(&self) -> &'static str { "Export" }
    fn state(&self) -> &PanelState { &self.state }
    fn state_mut(&mut self) -> &mut PanelState { &mut self.state }
    fn render_panel(&mut self, ui: &mut Ui) { ui.label("Export options placeholder"); }
}
