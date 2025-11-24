use crate::LivePlotApp;
use eframe::egui;
use egui_tiles::{Behavior, Container, ContainerKind, TileId, Tiles, Tree, UiResponse};

/// Lightweight wrapper that renders a [`LivePlotApp`] inside an `egui_tiles` pane.
pub struct LivePlotTile {
    label: String,
    plot: LivePlotApp,
}

impl LivePlotTile {
    /// Create a new tile with the provided label and plot instance.
    pub fn new(label: impl Into<String>, plot: LivePlotApp) -> Self {
        Self {
            label: label.into(),
            plot,
        }
    }

    /// Pane title shown in the tab bar / window chrome.
    pub fn label(&self) -> &str {
        &self.label
    }

    /// Mutable access to the embedded plot for configuration after construction.
    pub fn plot_mut(&mut self) -> &mut LivePlotApp {
        &mut self.plot
    }

    /// Render the plot inside a framed container sized to the enclosing tile.
    pub fn ui(&mut self, ui: &mut egui::Ui, tile_id: TileId, id_prefix: &str) {
        let available = ui.available_size();
        let margin_x = 16.0;
        let margin_y = 12.0;
        egui::Frame::group(ui.style())
            .inner_margin(egui::Margin::symmetric(8, 6))
            .show(ui, |panel_ui| {
                let inner = egui::vec2(
                    (available.x - margin_x).max(0.0),
                    (available.y - margin_y).max(0.0),
                );
                panel_ui.set_min_size(inner);
                panel_ui.horizontal(|ui| {
                    ui.strong(self.label());
                });
                panel_ui.add_space(4.0);
                let plot_area = panel_ui.available_size();
                panel_ui.allocate_ui(plot_area, |plot_ui| {
                    let plot_id = plot_ui.id().with((id_prefix, tile_id));
                    self.plot.ui_embed_with_id(plot_ui, plot_id);
                });
            });
    }
}

/// Identifier stored inside an `egui_tiles::Tree`, referencing a tile by index.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct LivePlotPaneRef {
    pub index: usize,
}

/// Helper that lays out `pane_count` plots across `columns` columns (rows filled top-down).
pub fn build_grid_tree(
    tree_id: &'static str,
    pane_count: usize,
    columns: usize,
) -> Tree<LivePlotPaneRef> {
    let columns = columns.max(1);
    if pane_count == 0 {
        return Tree::empty(tree_id);
    }

    let mut tiles: Tiles<LivePlotPaneRef> = Tiles::default();
    let pane_ids: Vec<_> = (0..pane_count)
        .map(|index| tiles.insert_pane(LivePlotPaneRef { index }))
        .collect();

    let mut rows = Vec::new();
    for chunk in pane_ids.chunks(columns) {
        rows.push(
            tiles.insert_container(Container::new(ContainerKind::Horizontal, chunk.to_vec())),
        );
    }

    let root = if rows.len() == 1 {
        rows[0]
    } else {
        tiles.insert_container(Container::new(ContainerKind::Vertical, rows))
    };

    Tree::new(tree_id, root, tiles)
}

/// Render helper that wires the `Tree` and automatically sizes it to the available region.
pub fn render_tile_grid(
    ui: &mut egui::Ui,
    tree: &mut Tree<LivePlotPaneRef>,
    tiles: &mut [LivePlotTile],
    plot_id_prefix: &str,
) {
    let desired = ui.available_size();
    if desired.min_elem() <= 0.0 {
        ui.label("Expand the window to see the plots.");
        return;
    }

    ui.allocate_ui(desired, |dashboard_ui| {
        dashboard_ui.set_min_size(desired);
        dashboard_ui.set_clip_rect(dashboard_ui.max_rect());
        tree.set_width(desired.x);
        tree.set_height(desired.y);
        let mut behavior = LivePlotTilesBehavior {
            tiles,
            plot_id_prefix,
        };
        tree.ui(&mut behavior, dashboard_ui);
    });
}

struct LivePlotTilesBehavior<'a> {
    tiles: &'a mut [LivePlotTile],
    plot_id_prefix: &'a str,
}

impl<'a> Behavior<LivePlotPaneRef> for LivePlotTilesBehavior<'a> {
    fn tab_title_for_pane(&mut self, pane: &LivePlotPaneRef) -> egui::WidgetText {
        if let Some(tile) = self.tiles.get(pane.index) {
            tile.label().into()
        } else {
            format!("Plot {}", pane.index + 1).into()
        }
    }

    fn pane_ui(
        &mut self,
        ui: &mut egui::Ui,
        tile_id: TileId,
        pane: &mut LivePlotPaneRef,
    ) -> UiResponse {
        if let Some(tile) = self.tiles.get_mut(pane.index) {
            tile.ui(ui, tile_id, self.plot_id_prefix);
        } else {
            ui.colored_label(egui::Color32::LIGHT_RED, "Missing plot tile");
        }
        UiResponse::None
    }
}
