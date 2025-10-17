use egui::Ui;
use super::panel_trait::{Panel, PanelState};
use crate::data::DataContext;

pub struct TriggersPanel { pub state: PanelState }
impl Default for TriggersPanel { fn default() -> Self { Self { state: PanelState { visible: false, detached: false } } } }
impl Panel for TriggersPanel {
    fn name(&self) -> &'static str { "Triggers" }
    fn state(&self) -> &PanelState { &self.state }
    fn state_mut(&mut self) -> &mut PanelState { &mut self.state }
    fn render_panel(&mut self, ui: &mut Ui, _data: &mut DataContext) { ui.label("Triggers panel placeholder"); }
}
