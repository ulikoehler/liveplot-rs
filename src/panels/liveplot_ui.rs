use super::scope_ui::ScopePanel;
use crate::data::scope::ScopeData;
use crate::data::traces::TracesCollection;
use egui::{Stroke, Ui, Visuals, WidgetText};
use egui_phosphor::regular::{BROOM, MINUS, PLUS, SIDEBAR};
use egui_tiles::{Behavior, Container, TabState, Tabs, Tile, TileId, Tiles, Tree, UiResponse};

pub struct LiveplotPanel {
    tree: Tree<ScopePanel>,
    next_scope_idx: usize,
    /// Cached event controller to propagate to newly added scopes.
    event_ctrl_cache: Option<crate::events::EventController>,
}

impl Default for LiveplotPanel {
    fn default() -> Self {
        Self::new_with_id("liveplot_scopes", 0)
    }
}

impl LiveplotPanel {
    /// Create a new `LiveplotPanel` with a unique tree ID and scope name suffix.
    ///
    /// Use `new_with_id` when embedding multiple `LiveplotPanel` instances in the
    /// same egui context to avoid widget ID collisions.
    ///
    /// - `tree_key`: used as a prefix in the egui tree ID so each instance gets
    ///   its own pan/zoom state.
    /// - `scope_id_offset`: added to the default scope index so the scope's plot
    ///   ID (`scope_plot_<name>`) is unique across instances.
    pub fn new_with_id(tree_key: impl std::hash::Hash, scope_id_offset: usize) -> Self {
        let mut tiles = Tiles::default();
        let mut scope = ScopePanel::new(0);
        // Give the scope a name that includes the offset so the egui Plot ID is unique.
        if scope_id_offset != 0 {
            scope.set_name(format!("Scope 1 ({})", scope_id_offset));
        }
        let root_pane = tiles.insert_pane(scope);
        let root = tiles.insert_tab_tile(vec![root_pane]);
        Self {
            tree: Tree::new(egui::Id::new(("liveplot_scopes", tree_key)), root, tiles),
            next_scope_idx: 1,
            event_ctrl_cache: None,
        }
    }
    /// Get immutable references to all scope data, sorted by id.
    pub fn get_data(&self) -> Vec<&ScopeData> {
        let mut scopes_data: Vec<&ScopeData> = self
            .tree
            .tiles
            .tiles()
            .filter_map(|tile| match tile {
                Tile::Pane(pane) => Some(pane.get_data()),
                _ => None,
            })
            .collect();
        scopes_data.sort_by_key(|s| s.id);
        scopes_data
    }

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

    pub fn render_menu(&mut self, ui: &mut Ui, traces: &mut TracesCollection, collapsed: bool) {
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

        // Add an icon to the Scopes menu for easier recognition; collapse to icon when narrow
        let scopes_label = if collapsed { "🔭" } else { "🔭 Scopes" };
        ui.menu_button(scopes_label, |ui| {
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

        // Check if any scope currently has controls visible (for the shared toggle button).
        let any_controls_on = self
            .tree
            .tiles
            .tiles()
            .any(|t| matches!(t, Tile::Pane(p) if p.controls_in_toolbar()));

        let mut behavior = ScopeBehavior {
            draw_overlays,
            traces,
            controls_currently_on: any_controls_on,
            controls_toggle_pressed: false,
        };
        self.tree.ui(&mut behavior, ui);

        // Apply the shared controls toggle if the button was pressed.
        if behavior.controls_toggle_pressed {
            let new_state = !behavior.controls_currently_on;
            for tile in self.tree.tiles.tiles_mut() {
                if let Tile::Pane(pane) = tile {
                    pane.set_controls_in_toolbar(new_state);
                }
            }
        }
    }

    pub fn add_scope(&mut self) -> usize {
        let new_scope_id = self.next_scope_idx;
        let mut scope = ScopePanel::new(self.next_scope_idx);
        scope.event_ctrl = self.event_ctrl_cache.clone();
        let id = self.tree.tiles.insert_pane(scope);
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

        // Emit SCOPE_ADDED event
        if let Some(ctrl) = &self.event_ctrl_cache {
            let mut evt = crate::events::PlotEvent::new(crate::events::EventKind::SCOPE_ADDED);
            evt.scope_manage = Some(crate::events::ScopeManageMeta {
                scope_id: new_scope_id,
                scope_name: None,
            });
            ctrl.emit_filtered(evt);
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

        // Emit SCOPE_REMOVED event
        if let (Some(ctrl), Some(sid)) = (&self.event_ctrl_cache, removed_id) {
            let mut evt = crate::events::PlotEvent::new(crate::events::EventKind::SCOPE_REMOVED);
            evt.scope_manage = Some(crate::events::ScopeManageMeta {
                scope_id: sid,
                scope_name: None,
            });
            ctrl.emit_filtered(evt);
        }
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

    /// Return the current next_scope_idx counter.
    pub fn next_scope_idx(&self) -> usize {
        self.next_scope_idx
    }

    /// Propagate the total widget size to every scope panel.
    pub fn set_total_widget_size(&mut self, size: egui::Vec2) {
        for tile in self.tree.tiles.tiles_mut() {
            if let Tile::Pane(pane) = tile {
                pane.total_widget_size = size;
            }
        }
    }

    /// Set the tick-label-hiding thresholds on every scope panel.
    pub fn set_tick_label_thresholds(
        &mut self,
        min_width_for_y_ticklabels: f32,
        min_height_for_x_ticklabels: f32,
    ) {
        for tile in self.tree.tiles.tiles_mut() {
            if let Tile::Pane(pane) = tile {
                pane.min_width_for_y_ticklabels = min_width_for_y_ticklabels;
                pane.min_height_for_x_ticklabels = min_height_for_x_ticklabels;
            }
        }
    }

    /// Set the legend-hiding thresholds on every scope panel.
    pub fn set_legend_thresholds(&mut self, min_width_for_legend: f32, min_height_for_legend: f32) {
        for tile in self.tree.tiles.tiles_mut() {
            if let Tile::Pane(pane) = tile {
                pane.min_width_for_legend = min_width_for_legend;
                pane.min_height_for_legend = min_height_for_legend;
            }
        }
    }

    /// Propagate the event controller to every scope panel.
    pub fn set_event_controller(&mut self, ctrl: Option<crate::events::EventController>) {
        self.event_ctrl_cache = ctrl.clone();
        for tile in self.tree.tiles.tiles_mut() {
            if let Tile::Pane(pane) = tile {
                pane.event_ctrl = ctrl.clone();
            }
        }
    }

    /// Replace all scope panels with the given scope data states.
    ///
    /// Existing scopes are cleared and new ScopePanels are created with the
    /// provided data. `next_idx` restores the counter so new scopes get
    /// non-colliding ids.
    pub fn restore_scopes(
        &mut self,
        scope_states: Vec<crate::persistence::ScopeStateSerde>,
        next_idx: Option<usize>,
    ) {
        if scope_states.is_empty() {
            return;
        }

        // Remove all existing panes
        let _pane_ids: Vec<TileId> = self
            .tree
            .tiles
            .iter()
            .filter_map(|(id, tile)| match tile {
                Tile::Pane(_) => Some(*id),
                _ => None,
            })
            .collect();

        // Clear tree
        self.tree = Tree::new(
            egui::Id::new("liveplot_scopes_restore"),
            TileId::from_u64(0), // dummy, replaced below
            Tiles::default(),
        );

        // Create scope panels from the saved states
        let mut tile_ids = Vec::new();
        let mut max_id: usize = 0;
        for ss in scope_states {
            let scope_id = ss.id.unwrap_or(max_id);
            max_id = max_id.max(scope_id + 1);
            let mut panel = ScopePanel::new(scope_id);
            panel.event_ctrl = self.event_ctrl_cache.clone();
            ss.apply_to(panel.get_data_mut());
            let tid = self.tree.tiles.insert_pane(panel);
            tile_ids.push(tid);
        }

        let root = self.tree.tiles.insert_tab_tile(tile_ids);
        self.tree.root = Some(root);

        self.next_scope_idx = next_idx.unwrap_or(max_id);
    }
}

struct ScopeBehavior<'a, F> {
    draw_overlays: F,
    traces: &'a mut TracesCollection,
    /// Whether any scope currently has controls visible (used for the shared toggle button label).
    controls_currently_on: bool,
    /// Set to `true` when the shared controls toggle button is clicked.
    controls_toggle_pressed: bool,
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

    /// Slightly taller tab bar for better readability.
    fn tab_bar_height(&self, _style: &egui::Style) -> f32 {
        28.0
    }

    /// Make active tab outline more prominent; show a subtle outline for inactive tabs too.
    fn tab_outline_stroke(
        &self,
        visuals: &Visuals,
        _tiles: &Tiles<ScopePanel>,
        _tile_id: TileId,
        state: &TabState,
    ) -> Stroke {
        if state.active {
            Stroke::new(2.0, visuals.widgets.active.bg_fill)
        } else {
            Stroke::new(1.0, visuals.widgets.noninteractive.bg_stroke.color)
        }
    }

    /// Thicker separator line between tab bar and content.
    fn tab_bar_hline_stroke(&self, visuals: &Visuals) -> Stroke {
        Stroke::new(2.0, visuals.widgets.noninteractive.bg_stroke.color)
    }

    /// Add a shared "Controls" toggle button on the right side of the tab bar.
    fn top_bar_right_ui(
        &mut self,
        _tiles: &Tiles<ScopePanel>,
        ui: &mut Ui,
        _tile_id: TileId,
        _tabs: &Tabs,
        _scroll_offset: &mut f32,
    ) {
        let icon = if self.controls_currently_on {
            MINUS
        } else {
            SIDEBAR
        };
        let label = format!("{icon} Controls");
        if ui
            .small_button(label)
            .on_hover_text("Toggle plot controls for all scopes")
            .clicked()
        {
            self.controls_toggle_pressed = true;
        }
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
