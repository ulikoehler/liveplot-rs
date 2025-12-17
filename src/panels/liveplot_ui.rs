use super::scope_ui::ScopePanel;
use crate::data::scope::ScopeData;
use crate::data::traces::TracesCollection;
use egui::{Ui, WidgetText};
use egui_phosphor::regular::{BROOM, PLUS};
use egui_tiles::{Behavior, Container, Tabs, Tile, TileId, Tiles, Tree, UiResponse};

pub struct LiveplotPanel {
    tree: Tree<ScopePanel>,
    next_scope_idx: usize,
}

impl Default for LiveplotPanel {
    fn default() -> Self {
        let mut tiles = Tiles::default();
        let root_pane = tiles.insert_pane(ScopePanel::new(0));
        let root = tiles.insert_tab_tile(vec![root_pane]);
        Self {
            tree: Tree::new("liveplot_scopes", root, tiles),
            next_scope_idx: 1,
        }
    }
}

impl LiveplotPanel {
    pub fn get_data_mut(&mut self) -> Vec<&mut ScopeData> {
        let scopes_data: Vec<&mut ScopeData> = self
            .tree
            .tiles
            .tiles_mut()
            .filter_map(|tile| match tile {
                Tile::Pane(pane) => Some(pane.get_data_mut()),
                _ => None,
            })
            .collect();

        // Ensure a stable, predictable ordering by scope id (ascending).
        // New scopes are allocated with increasing ids, so this keeps newly
        // added scopes at the end of the returned list.
        let mut scopes_data = scopes_data;
        scopes_data.sort_by_key(|s| s.id);
        scopes_data
    }

    pub fn update_data(&mut self, traces: &TracesCollection) {
        for tile in self.tree.tiles.tiles_mut() {
            if let Tile::Pane(pane) = tile {
                pane.update_data(traces);
            }
        }
    }

    pub fn render_menu(&mut self, ui: &mut Ui, traces: &mut TracesCollection) {
        let mut remove_target: Option<TileId> = None;
        let can_remove = self
            .tree
            .tiles
            .tiles()
            .filter(|t| matches!(t, Tile::Pane(_)))
            .count()
            > 1;

        // Collect pane ids and sort them by panel id (scope id) so menus show
        // scopes in ascending id order (newer scopes last).
        let mut pane_ids: Vec<TileId> = self
            .tree
            .tiles
            .iter()
            .filter_map(|(id, tile)| match tile {
                Tile::Pane(_) => Some(*id),
                _ => None,
            })
            .collect();
        pane_ids.sort_by_key(|tid| {
            self.tree
                .tiles
                .get(*tid)
                .and_then(|t| match t {
                    Tile::Pane(p) => Some(p.id()),
                    _ => None,
                })
                .unwrap_or(0usize)
        });

        // Add an icon to the Scopes menu for easier recognition
        ui.menu_button("🔭 Scopes", |ui| {
            if ui.button(format!("{PLUS} Add scope")).clicked() {
                self.add_scope();
            }

            if ui
                .button(format!("{BROOM} Clear all"))
                .on_hover_text("Clear all trace data")
                .clicked()
            {
                traces.clear_all();
                for tile in self.tree.tiles.tiles_mut() {
                    if let Tile::Pane(pane) = tile {
                        pane.get_data_mut().clicked_point = None;
                    }
                }
                ui.close();
            }

            ui.separator();

            for tile_id in pane_ids.iter().copied() {
                if let Some(Tile::Pane(panel)) = self.tree.tiles.get_mut(tile_id) {
                    let name = panel.name().to_string();
                    ui.menu_button(name, |ui| {
                        panel.render_menu(ui, traces);
                        if can_remove {
                            ui.separator();
                            if ui
                                .button("Remove scope")
                                .on_hover_text("Remove this scope from layout")
                                .clicked()
                            {
                                remove_target = Some(tile_id);
                                ui.close();
                            }
                        }
                    });
                }
            }
        });

        if let Some(id) = remove_target {
            self.remove_scope(id);
        }
    }

    pub fn render_panel<F>(&mut self, ui: &mut Ui, draw_overlays: F, traces: &mut TracesCollection)
    where
        F: FnMut(&mut egui_plot::PlotUi, &ScopeData, &TracesCollection),
    {
        if self.tree.is_empty() {
            ui.label("No scope panels. Add one from the menu.");
            return;
        }

        let mut behavior = ScopeBehavior {
            draw_overlays,
            traces,
        };
        self.tree.ui(&mut behavior, ui);
    }

    pub fn add_scope(&mut self) -> usize {
        let new_scope_id = self.next_scope_idx;
        let id = self
            .tree
            .tiles
            .insert_pane(ScopePanel::new(self.next_scope_idx));
        self.next_scope_idx += 1;

        if let Some(root_id) = self.tree.root {
            match self.tree.tiles.get_mut(root_id) {
                Some(Tile::Container(Container::Tabs(tabs))) => {
                    tabs.add_child(id);
                    tabs.set_active(id);
                }
                Some(Tile::Container(Container::Linear(linear))) => {
                    linear.children.push(id);
                }
                Some(Tile::Pane(_)) => {
                    let previous = self.tree.root.unwrap();
                    let tabs = Tabs::new(vec![previous, id]);
                    let new_root = self.tree.tiles.insert_container(tabs);
                    self.tree.root = Some(new_root);
                }
                _ => {}
            }
        } else {
            let root = self.tree.tiles.insert_tab_tile(vec![id]);
            self.tree.root = Some(root);
        }

        new_scope_id
    }

    fn remove_scope(&mut self, tile_id: TileId) {
        let removed_id = match self.tree.tiles.get(tile_id) {
            Some(Tile::Pane(pane)) => Some(pane.id()),
            _ => None,
        };

        if self
            .tree
            .tiles
            .tiles()
            .filter(|t| matches!(t, Tile::Pane(_)))
            .count()
            <= 1
        {
            return;
        }

        if let Some(root_id) = self.tree.root {
            if let Some(Tile::Container(Container::Tabs(tabs))) = self.tree.tiles.get_mut(root_id) {
                tabs.children.retain(|c| *c != tile_id);
                if tabs.active == Some(tile_id) {
                    tabs.active = tabs.children.first().copied();
                }
                if tabs.children.is_empty() {
                    self.tree.root = None;
                }
            }
        }

        self.tree.remove_recursively(tile_id);
        let mut noop = NoopBehavior;
        self.tree.gc(&mut noop);

        let _ = removed_id;
    }

    pub fn remove_scope_by_id(&mut self, scope_id: usize) -> bool {
        let pane_ids: Vec<(TileId, usize)> = self
            .tree
            .tiles
            .iter()
            .filter_map(|(id, tile)| match tile {
                Tile::Pane(pane) => Some((*id, pane.id())),
                _ => None,
            })
            .collect();

        let pane_count = pane_ids.len();
        if pane_count <= 1 {
            return false;
        }

        if let Some((tile_id, _)) = pane_ids.into_iter().find(|(_tid, sid)| *sid == scope_id) {
            self.remove_scope(tile_id);
            return true;
        }
        false
    }
}

struct ScopeBehavior<'a, F> {
    draw_overlays: F,
    traces: &'a mut TracesCollection,
}

impl<'a, F> Behavior<ScopePanel> for ScopeBehavior<'a, F>
where
    F: FnMut(&mut egui_plot::PlotUi, &ScopeData, &TracesCollection),
{
    fn pane_ui(&mut self, ui: &mut Ui, _tile_id: TileId, pane: &mut ScopePanel) -> UiResponse {
        pane.render_panel(ui, &mut self.draw_overlays, self.traces);
        UiResponse::None
    }

    fn tab_title_for_pane(&mut self, pane: &ScopePanel) -> WidgetText {
        pane.name().to_string().into()
    }
}

struct NoopBehavior;

impl Behavior<ScopePanel> for NoopBehavior {
    fn pane_ui(&mut self, _ui: &mut Ui, _tile_id: TileId, _pane: &mut ScopePanel) -> UiResponse {
        UiResponse::None
    }

    fn tab_title_for_pane(&mut self, _pane: &ScopePanel) -> WidgetText {
        WidgetText::from("Scope")
    }
}
