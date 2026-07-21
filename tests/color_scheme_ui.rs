use liveplot::panels::color_scheme_ui::{ColorSchemePanel, NamedCustomScheme};
use liveplot::ColorScheme;
use egui::Color32;

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
    panel.custom_schemes.push(NamedCustomScheme::from_palette(
        &name,
        &panel.editing_palette,
    ));

    assert_eq!(panel.custom_schemes.len(), 1);
    assert_eq!(panel.custom_schemes[0].name, "Test Scheme");
    assert_eq!(panel.custom_schemes[0].colors.len(), 2);
}

#[test]
fn save_replaces_existing_name() {
    let mut panel = ColorSchemePanel::default();
    panel.editing_palette = vec![Color32::from_rgb(1, 2, 3)];

    // First save.
    panel.custom_schemes.push(NamedCustomScheme::from_palette(
        "MyName",
        &panel.editing_palette,
    ));

    // Second save with same name but different colors.
    panel.editing_palette = vec![Color32::from_rgb(7, 8, 9)];
    let name = "MyName".to_string();
    if let Some(pos) = panel.custom_schemes.iter().position(|s| s.name == name) {
        panel.custom_schemes[pos] =
            NamedCustomScheme::from_palette(&name, &panel.editing_palette);
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

    panel.custom_schemes.push(NamedCustomScheme::from_palette(
        "Custom1",
        &[Color32::from_rgb(1, 2, 3)],
    ));
    let labels = panel.scheme_labels();
    assert_eq!(labels.len(), builtin_count + 1);
    assert_eq!(labels[builtin_count], "Custom1");
}
