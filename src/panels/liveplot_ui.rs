use egui::Ui;

use super::scope_ui::ScopePanel;
use crate::data::scope::ScopeData;
use crate::data::traces::TracesCollection;

pub struct LiveplotPanel {
    scope_ui: ScopePanel,
}

impl Default for LiveplotPanel {
    fn default() -> Self {
        Self {
            scope_ui: ScopePanel::default(),
        }
    }
}

impl LiveplotPanel {
    pub fn get_data_mut(&mut self) -> &mut ScopeData {
        self.scope_ui.get_data_mut()
    }

    pub fn update_data(&mut self, traces: &TracesCollection) {
        self.scope_ui.update_data(traces);
    }

    pub fn render_menu(&mut self, _ui: &mut Ui) {}

    pub fn render_panel<F>(&mut self, ui: &mut Ui, draw_overlays: F, traces: &mut TracesCollection)
    where
        F: FnMut(&mut egui_plot::PlotUi, &ScopeData, &TracesCollection),
    {
        self.scope_ui.render_panel(ui, draw_overlays, traces);
    }

    // Old specialized prefix/suffix helpers removed; functionality handled via closures.
}
