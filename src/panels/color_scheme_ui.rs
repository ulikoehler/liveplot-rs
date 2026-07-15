//! Color scheme panel for managing trace color palettes.

use eframe::egui;
use eframe::egui::Color32;
use egui_phosphor::regular::{ARROW_DOWN, ARROW_UP, CHECK, FLOPPY_DISK, MINUS, PALETTE, PLUS, TRASH, WARNING};
use serde::{Deserialize, Serialize};

use super::panel_trait::{Panel, PanelState};
use crate::color_scheme::{set_global_palette, ColorScheme, CustomColorScheme};
use crate::data::data::LivePlotData;

/// A user-defined custom color scheme that can be serialized.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NamedCustomScheme {
    pub name: String,
    /// Colors stored as `[r, g, b]` triples.
    pub colors: Vec<[u8; 3]>,
}

impl NamedCustomScheme {
    pub fn to_color32_vec(&self) -> Vec<Color32> {
        self.colors
            .iter()
            .map(|c| Color32::from_rgb(c[0], c[1], c[2]))
            .collect()
    }

    pub fn from_palette(name: &str, palette: &[Color32]) -> Self {
        Self {
            name: name.to_string(),
            colors: palette
                .iter()
                .map(|c| [c.r(), c.g(), c.b()])
                .collect(),
        }
    }
}

pub struct ColorSchemePanel {
    state: PanelState,
    /// Index of the currently selected scheme in the dropdown.
    /// 0..N = built-in schemes, N.. = custom schemes.
    selected_index: usize,
    /// Working copy of the palette being edited.
    editing_palette: Vec<Color32>,
    /// Text input for the custom scheme name.
    editing_name: String,
    /// User-saved custom schemes.
    pub custom_schemes: Vec<NamedCustomScheme>,
    /// Whether the editing palette has been modified since loading.
    dirty: bool,
}

impl Default for ColorSchemePanel {
    fn default() -> Self {
        let default_scheme = ColorScheme::Dark;
        let palette = default_scheme.trace_colors();
        Self {
            state: PanelState::new("Color Scheme", PALETTE),
            selected_index: 0,
            editing_palette: palette,
            editing_name: String::new(),
            custom_schemes: Vec::new(),
            dirty: false,
        }
    }
}

impl ColorSchemePanel {
    /// Get the list of all scheme names (built-in + custom).
    fn scheme_labels(&self) -> Vec<String> {
        let mut labels: Vec<String> = ColorScheme::all()
            .iter()
            .map(|s| s.label())
            .collect();
        for cs in &self.custom_schemes {
            labels.push(cs.name.clone());
        }
        labels
    }

    /// Get the palette for a given scheme index.
    fn palette_for_index(&self, index: usize) -> Vec<Color32> {
        let builtins = ColorScheme::all();
        if index < builtins.len() {
            builtins[index].trace_colors()
        } else {
            let ci = index - builtins.len();
            if ci < self.custom_schemes.len() {
                self.custom_schemes[ci].to_color32_vec()
            } else {
                ColorScheme::Dark.trace_colors()
            }
        }
    }

    /// Apply the current editing palette to the global state.
    fn apply_palette(&self, ui: &mut egui::Ui, palette: &[Color32]) {
        let ctx = ui.ctx().clone();
        let custom = CustomColorScheme {
            visuals: None,
            palette: palette.to_vec(),
            label: Some("Custom".to_string()),
        };
        let scheme = ColorScheme::Custom(custom);
        scheme.apply(&ctx);
        // Also recolor existing traces via the global palette.
        set_global_palette(palette.to_vec());
    }

    /// Apply a built-in or custom scheme by index.
    fn apply_scheme_by_index(&self, ui: &mut egui::Ui, index: usize) {
        let builtins = ColorScheme::all();
        if index < builtins.len() {
            let ctx = ui.ctx().clone();
            builtins[index].apply(&ctx);
        } else {
            let palette = self.palette_for_index(index);
            self.apply_palette(ui, &palette);
        }
    }

    /// Set custom schemes from loaded state (used by session restore).
    pub fn set_custom_schemes(&mut self, schemes: Vec<NamedCustomScheme>) {
        self.custom_schemes = schemes;
    }

    /// Get custom schemes for saving (used by session save).
    pub fn get_custom_schemes(&self) -> &[NamedCustomScheme] {
        &self.custom_schemes
    }

    /// Get the current editing palette as RGB triples for persistence.
    pub fn get_active_palette(&self) -> Vec<[u8; 3]> {
        self.editing_palette
            .iter()
            .map(|c| [c.r(), c.g(), c.b()])
            .collect()
    }

    /// Restore the active palette from loaded state and apply it.
    pub fn restore_active_palette(&mut self, colors: &[[u8; 3]]) {
        self.editing_palette = colors
            .iter()
            .map(|c| Color32::from_rgb(c[0], c[1], c[2]))
            .collect();
        self.dirty = false;
    }

    /// Set the initial scheme from config (called after construction).
    pub fn set_initial_scheme(&mut self, scheme: &ColorScheme) {
        let builtins = ColorScheme::all();
        for (i, s) in builtins.iter().enumerate() {
            if s == scheme {
                self.selected_index = i;
                self.editing_palette = s.trace_colors();
                return;
            }
        }
        // If it's a custom scheme, try to match by palette.
        if let ColorScheme::Custom(custom) = scheme {
            self.editing_palette = custom.palette.clone();
        }
    }
}

impl Panel for ColorSchemePanel {
    fn state(&self) -> &PanelState {
        &self.state
    }

    fn state_mut(&mut self) -> &mut PanelState {
        &mut self.state
    }

    fn render_menu(
        &mut self,
        ui: &mut egui::Ui,
        _data: &mut LivePlotData<'_>,
        collapsed: bool,
        tooltip: &str,
    ) {
        let label = if collapsed {
            self.icon_only()
                .map(|s| s.to_string())
                .unwrap_or_else(|| self.title().to_string())
        } else {
            self.title_and_icon()
        };
        let menu_cfg = egui::containers::menu::MenuConfig::new()
            .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside);
        let mr = egui::containers::menu::MenuButton::new(label)
            .config(menu_cfg)
            .ui(ui, |ui| {
                if ui.button("Show Color Scheme").clicked() {
                    let st = self.state_mut();
                    st.visible = true;
                    st.request_focus = true;
                    ui.close();
                }
            });
        if !tooltip.is_empty() {
            mr.0.on_hover_text(tooltip);
        }
    }

    fn render_panel(&mut self, ui: &mut egui::Ui, data: &mut LivePlotData<'_>) {
        let builtins = ColorScheme::all();
        let labels = self.scheme_labels();

        // ── Scheme selector ──────────────────────────────────────────────
        ui.horizontal(|ui| {
            ui.label("Scheme:");
            let prev_index = self.selected_index;
            egui::ComboBox::from_id_salt("color_scheme_selector")
                .selected_text(
                    labels
                        .get(self.selected_index)
                        .cloned()
                        .unwrap_or_else(|| "—".to_string()),
                )
                .show_ui(ui, |ui| {
                    for (i, label) in labels.iter().enumerate() {
                        let is_custom = i >= builtins.len();
                        let prefix = if is_custom { "★ " } else { "" };
                        ui.selectable_value(
                            &mut self.selected_index,
                            i,
                            format!("{}{}", prefix, label),
                        );
                    }
                });
            // If selection changed, load the palette.
            if self.selected_index != prev_index {
                self.editing_palette = self.palette_for_index(self.selected_index);
                self.dirty = false;
                self.apply_scheme_by_index(ui, self.selected_index);
                data.traces.recolor_using_palette();
            }
        });

        ui.separator();

        // ── Color list ───────────────────────────────────────────────────
        ui.label("Trace colors:");
        let mut to_remove: Option<usize> = None;
        let mut to_move_up: Option<usize> = None;
        let mut to_move_down: Option<usize> = None;
        let mut color_changed = false;

        for (i, color) in self.editing_palette.iter_mut().enumerate() {
            ui.horizontal(|ui| {
                let mut color_arr = [color.r(), color.g(), color.b(), color.a()];
                let resp = ui.color_edit_button_srgba_unmultiplied(&mut color_arr);
                *color = Color32::from_rgba_unmultiplied(
                    color_arr[0],
                    color_arr[1],
                    color_arr[2],
                    color_arr[3],
                );
                if resp.changed() {
                    color_changed = true;
                }

                ui.label(format!("#{}", i + 1));

                if ui.button(ARROW_UP).on_hover_text("Move up").clicked() {
                    to_move_up = Some(i);
                }
                if ui.button(ARROW_DOWN).on_hover_text("Move down").clicked() {
                    to_move_down = Some(i);
                }
                if ui.button(MINUS).on_hover_text("Remove color").clicked() {
                    to_remove = Some(i);
                }
            });
        }

        // Apply moves.
        if color_changed {
            self.dirty = true;
        }
        if let Some(i) = to_move_up {
            if i > 0 {
                self.editing_palette.swap(i, i - 1);
                self.dirty = true;
            }
        }
        if let Some(i) = to_move_down {
            if i + 1 < self.editing_palette.len() {
                self.editing_palette.swap(i, i + 1);
                self.dirty = true;
            }
        }
        // Apply removal.
        if let Some(i) = to_remove {
            if self.editing_palette.len() > 1 {
                self.editing_palette.remove(i);
                self.dirty = true;
            }
        }

        // Mark dirty if any color changed (we can't easily detect this per-widget,
        // so we set dirty on any interaction in the color list area).
        ui.horizontal(|ui| {
            if ui.button(format!("{} Add color", PLUS)).clicked() {
                // Add a contrasting color.
                let new_color = Color32::from_rgb(200, 200, 80);
                self.editing_palette.push(new_color);
                self.dirty = true;
            }
            if ui.button(format!("{} Apply", CHECK)).clicked() {
                self.apply_palette(ui, &self.editing_palette);
                data.traces.recolor_using_palette();
                self.dirty = false;
            }
        });

        ui.separator();

        // ── Save as custom scheme ────────────────────────────────────────
        ui.label("Save current palette as:");
        ui.horizontal(|ui| {
            ui.text_edit_singleline(&mut self.editing_name);
            ui.add_space(4.0);
            if ui.button(format!("{} Save", FLOPPY_DISK)).clicked() && !self.editing_name.trim().is_empty() {
                let name = self.editing_name.trim().to_string();
                // Check if name already exists — if so, replace.
                if let Some(pos) = self
                    .custom_schemes
                    .iter()
                    .position(|s| s.name == name)
                {
                    self.custom_schemes[pos] =
                        NamedCustomScheme::from_palette(&name, &self.editing_palette);
                } else {
                    self.custom_schemes.push(NamedCustomScheme::from_palette(
                        &name,
                        &self.editing_palette,
                    ));
                }
                // Select the newly saved scheme.
                self.selected_index = builtins.len() + self.custom_schemes.len() - 1;
                self.dirty = false;
                // Keep the name in the text field so the user can see what was saved.
            }
        });

        // ── Delete custom scheme ─────────────────────────────────────────
        let is_custom = self.selected_index >= builtins.len();
        if is_custom {
            ui.add_space(4.0);
            if ui.button(format!("{} Delete this custom scheme", TRASH)).clicked() {
                let ci = self.selected_index - builtins.len();
                if ci < self.custom_schemes.len() {
                    self.custom_schemes.remove(ci);
                    self.selected_index = 0;
                    self.editing_palette = self.palette_for_index(0);
                    self.apply_scheme_by_index(ui, 0);
                    data.traces.recolor_using_palette();
                }
            }
        }

        // ── Unsaved changes warning ───────────────────────────────────
        if self.dirty {
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.colored_label(
                    Color32::from_rgb(200, 160, 0),
                    format!("{} Unsaved changes — click Apply to preview, or save as a custom scheme", WARNING),
                );
            });
        }
    }

    fn settings_snapshot(&self, _data: &LivePlotData<'_>) -> Option<String> {
        let snap: (Vec<NamedCustomScheme>, Vec<[u8; 3]>) =
            (self.custom_schemes.clone(), self.get_active_palette());
        serde_json::to_string(&snap).ok()
    }
}

// --- tests -----------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn named_custom_scheme_roundtrip() {
        let palette = vec![
            Color32::from_rgb(31, 119, 180),
            Color32::from_rgb(255, 127, 14),
        ];
        let named = NamedCustomScheme::from_palette("My Scheme", &palette);
        assert_eq!(named.name, "My Scheme");
        assert_eq!(named.colors.len(), 2);
        assert_eq!(named.colors[0], [31, 119, 180]);
        assert_eq!(named.colors[1], [255, 127, 14]);

        let restored = named.to_color32_vec();
        assert_eq!(restored, palette);
    }

    #[test]
    fn panel_default_has_palette() {
        let panel = ColorSchemePanel::default();
        assert!(!panel.editing_palette.is_empty());
        assert_eq!(panel.selected_index, 0);
    }

    #[test]
    fn add_and_remove_colors() {
        let mut panel = ColorSchemePanel::default();
        let initial_len = panel.editing_palette.len();
        panel.editing_palette.push(Color32::from_rgb(200, 200, 80));
        assert_eq!(panel.editing_palette.len(), initial_len + 1);
        panel.editing_palette.remove(0);
        assert_eq!(panel.editing_palette.len(), initial_len);
    }

    #[test]
    fn save_custom_scheme() {
        let mut panel = ColorSchemePanel::default();
        panel.editing_name = "Test Scheme".to_string();
        panel.editing_palette = vec![Color32::from_rgb(1, 2, 3), Color32::from_rgb(4, 5, 6)];

        // Simulate save.
        let name = panel.editing_name.trim().to_string();
        panel.custom_schemes
            .push(NamedCustomScheme::from_palette(&name, &panel.editing_palette));

        assert_eq!(panel.custom_schemes.len(), 1);
        assert_eq!(panel.custom_schemes[0].name, "Test Scheme");
        assert_eq!(panel.custom_schemes[0].colors.len(), 2);
    }

    #[test]
    fn save_replaces_existing_name() {
        let mut panel = ColorSchemePanel::default();
        panel.editing_palette = vec![Color32::from_rgb(1, 2, 3)];

        // First save.
        panel
            .custom_schemes
            .push(NamedCustomScheme::from_palette("MyName", &panel.editing_palette));

        // Second save with same name but different colors.
        panel.editing_palette = vec![Color32::from_rgb(7, 8, 9)];
        let name = "MyName".to_string();
        if let Some(pos) = panel.custom_schemes.iter().position(|s| s.name == name) {
            panel.custom_schemes[pos] = NamedCustomScheme::from_palette(&name, &panel.editing_palette);
        }

        assert_eq!(panel.custom_schemes.len(), 1);
        assert_eq!(panel.custom_schemes[0].colors[0], [7, 8, 9]);
    }

    #[test]
    fn scheme_labels_include_builtins_and_custom() {
        let mut panel = ColorSchemePanel::default();
        let builtin_count = ColorScheme::all().len();
        let labels = panel.scheme_labels();
        assert_eq!(labels.len(), builtin_count);

        panel
            .custom_schemes
            .push(NamedCustomScheme::from_palette("Custom1", &[Color32::from_rgb(1, 2, 3)]));
        let labels = panel.scheme_labels();
        assert_eq!(labels.len(), builtin_count + 1);
        assert_eq!(labels[builtin_count], "Custom1");
    }
}
