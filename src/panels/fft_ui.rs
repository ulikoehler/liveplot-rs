use egui::Ui;
use super::panel_trait::{Panel, PanelState};

pub struct FftPanel { pub state: PanelState }
impl Default for FftPanel { fn default() -> Self { Self { state: PanelState { visible: false, detached: false } } } }
impl Panel for FftPanel {
    fn name(&self) -> &'static str { "FFT" }
    fn state(&self) -> &PanelState { &self.state }
    fn state_mut(&mut self) -> &mut PanelState { &mut self.state }
    fn render_panel(&mut self, ui: &mut Ui) { ui.label("FFT results placeholder"); }
}
