//! Hotkeys configuration panel built on the shared Panel trait.

use std::cell::RefCell;
use std::rc::Rc;

use eframe::egui;

use crate::data::hotkeys::{Hotkey, HotkeyName, Hotkeys, Modifier};

use super::panel_trait::{Panel, PanelState};
use crate::data::data::LivePlotData;

pub struct HotkeysPanel {
    state: PanelState,
    capturing_hotkey: Option<HotkeyName>,
    hotkeys: Rc<RefCell<Hotkeys>>, // shared with the app so edits take effect immediately
}

impl HotkeysPanel {
    pub fn new(shared_hotkeys: Rc<RefCell<Hotkeys>>) -> Self {
        Self {
            // Use the basic keyboard glyph (no FE0F variation selector) to avoid double-char rendering
            state: PanelState::new("Hotkeys", "⌨"),
            capturing_hotkey: None,
            hotkeys: shared_hotkeys,
        }
    }

    fn set_hotkey(&mut self, name: HotkeyName, value: Option<Hotkey>) {
        {
            let mut hk = self.hotkeys.borrow_mut();
            match name {
                HotkeyName::Fft => hk.fft = value,
                HotkeyName::Math => hk.math = value,
                HotkeyName::FitView => hk.fit_view = value,
                HotkeyName::FitY => hk.fit_y = value,
                HotkeyName::FitViewCont => hk.fit_view_cont = value,
                HotkeyName::Pause => hk.pause = value,
                HotkeyName::Traces => hk.traces = value,
                HotkeyName::Thresholds => hk.thresholds = value,
                HotkeyName::Measurements => hk.measurements = value,
                HotkeyName::Triggers => hk.triggers = value,
                HotkeyName::HotkeysPanel => hk.hotkeys_panel = value,
                HotkeyName::SavePng => hk.save_png = value,
                HotkeyName::ExportData => hk.export_data = value,
                HotkeyName::ClearAll => hk.clear_all = value,
                HotkeyName::ResetMeasurements => hk.reset_measurements = value,
            }
            let _ = hk.save_to_default_path();
        }
        self.capturing_hotkey = None;
    }

    fn reset_defaults(&mut self) {
        let mut hk = self.hotkeys.borrow_mut();
        hk.reset_defaults();
        let _ = hk.save_to_default_path();
    }

    fn mods_to_modifier(m: egui::Modifiers) -> Modifier {
        match (m.ctrl, m.alt, m.shift) {
            (false, false, false) => Modifier::None,
            (true, false, false) => Modifier::Ctrl,
            (false, true, false) => Modifier::Alt,
            (false, false, true) => Modifier::Shift,
            (true, true, false) => Modifier::CtrlAlt,
            (true, false, true) => Modifier::CtrlShift,
            (false, true, true) => Modifier::AltShift,
            (true, true, true) => Modifier::CtrlAltShift,
        }
    }

    fn event_to_hotkey(ev: &egui::Event, mods: egui::Modifiers) -> Option<Hotkey> {
        match ev {
            egui::Event::Text(text) => text
                .chars()
                .next()
                .map(|ch| Hotkey::new(Self::mods_to_modifier(mods), ch.to_ascii_uppercase())),
            egui::Event::Key {
                key, pressed: true, ..
            } => {
                use egui::Key;
                let ch_opt = match key {
                    Key::A => Some('A'),
                    Key::B => Some('B'),
                    Key::C => Some('C'),
                    Key::D => Some('D'),
                    Key::E => Some('E'),
                    Key::F => Some('F'),
                    Key::G => Some('G'),
                    Key::H => Some('H'),
                    Key::I => Some('I'),
                    Key::J => Some('J'),
                    Key::K => Some('K'),
                    Key::L => Some('L'),
                    Key::M => Some('M'),
                    Key::N => Some('N'),
                    Key::O => Some('O'),
                    Key::P => Some('P'),
                    Key::Q => Some('Q'),
                    Key::R => Some('R'),
                    Key::S => Some('S'),
                    Key::T => Some('T'),
                    Key::U => Some('U'),
                    Key::V => Some('V'),
                    Key::W => Some('W'),
                    Key::X => Some('X'),
                    Key::Y => Some('Y'),
                    Key::Z => Some('Z'),
                    Key::Num0 => Some('0'),
                    Key::Num1 => Some('1'),
                    Key::Num2 => Some('2'),
                    Key::Num3 => Some('3'),
                    Key::Num4 => Some('4'),
                    Key::Num5 => Some('5'),
                    Key::Num6 => Some('6'),
                    Key::Num7 => Some('7'),
                    Key::Num8 => Some('8'),
                    Key::Num9 => Some('9'),
                    Key::Space => Some(' '),
                    _ => None,
                };
                ch_opt.map(|ch| Hotkey::new(Self::mods_to_modifier(mods), ch))
            }
            _ => None,
        }
    }
}

impl Default for HotkeysPanel {
    fn default() -> Self {
        Self::new(Rc::new(RefCell::new(Hotkeys::default())))
    }
}

impl Panel for HotkeysPanel {
    fn state(&self) -> &PanelState {
        &self.state
    }

    fn state_mut(&mut self) -> &mut PanelState {
        &mut self.state
    }

    fn hotkey_name(&self) -> Option<crate::data::hotkeys::HotkeyName> {
        Some(crate::data::hotkeys::HotkeyName::HotkeysPanel)
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
        // Panel menu item: show panel, reset defaults, save config
        let mr = ui.menu_button(label, |ui| {
            if ui.button("Show Hotkeys").clicked() {
                let st = self.state_mut();
                st.visible = true;
                st.request_focus = true;
                ui.close();
            }
            if ui.button("Reset to defaults").clicked() {
                self.reset_defaults();
                ui.close();
            }
            if ui.button("Save").clicked() {
                let _ = self.hotkeys.borrow().save_to_default_path();
                ui.close();
            }
        });
        if !tooltip.is_empty() {
            mr.response.on_hover_text(tooltip);
        }
    }

    fn render_panel(&mut self, ui: &mut egui::Ui, _data: &mut LivePlotData<'_>) {
        ui.label("Configure keyboard shortcuts for common actions.");
        ui.separator();

        // Snapshot current to avoid borrow conflicts while mutating later.
        let current = self.hotkeys.borrow().clone();

        let mut render_row =
            |ui: &mut egui::Ui, label: &str, name: HotkeyName, current: Option<Hotkey>| {
                ui.horizontal(|ui| {
                    let tip = match name {
                        HotkeyName::Fft => "Show / Hide FFT panel",
                        HotkeyName::Math => "Show / Hide Math panel",
                        HotkeyName::FitView => "Fit the current view to visible data",
                        HotkeyName::FitY => "Fit the Y axis to visible data",
                        HotkeyName::FitViewCont => "Toggle continuous fitting of the view",
                        HotkeyName::Pause => "Pause / resume plotting (Space also toggles)",
                        HotkeyName::Traces => "Show / Hide the Traces panel",
                        HotkeyName::Thresholds => "Show / Hide the Thresholds panel",
                        HotkeyName::Measurements => "Show / Hide the Measurements panel",
                        HotkeyName::Triggers => "Show / Hide the Triggers panel",
                        HotkeyName::HotkeysPanel => "Show / Hide the Hotkeys panel",
                        HotkeyName::SavePng => "Save a PNG screenshot of the window",
                        HotkeyName::ExportData => "Show / Hide the Export panel",
                        HotkeyName::ClearAll => "Clear all trace data",
                        HotkeyName::ResetMeasurements => "Clear all measurement points",
                    };
                    ui.label(label).on_hover_text(tip);

                    let capturing_this = self.capturing_hotkey == Some(name);
                    let btn_text = if capturing_this {
                        "⏺ Press keys...".to_owned()
                    } else {
                        match current {
                            Some(h) => h.to_string(),
                            None => "None".to_string(),
                        }
                    };

                    if ui
                        .button(btn_text)
                        .on_hover_text("Click to assign; press desired keys; Esc to cancel")
                        .clicked()
                    {
                        if !capturing_this {
                            self.capturing_hotkey = Some(name);
                        }
                    }

                    if capturing_this && ui.button("Cancel").clicked() {
                        self.capturing_hotkey = None;
                    }

                    if !capturing_this {
                        if ui
                            .button("Clear")
                            .on_hover_text("Disable this hotkey")
                            .clicked()
                        {
                            self.set_hotkey(name, None);
                        }
                    }
                });
            };

        let panel_rows = vec![
            #[cfg(feature = "fft")]
            ("FFT:", HotkeyName::Fft, current.fft.clone()),
            ("Traces:", HotkeyName::Traces, current.traces.clone()),
            (
                "Thresholds:",
                HotkeyName::Thresholds,
                current.thresholds.clone(),
            ),
            ("Math:", HotkeyName::Math, current.math.clone()),
            (
                "Measurements:",
                HotkeyName::Measurements,
                current.measurements.clone(),
            ),
            ("Triggers:", HotkeyName::Triggers, current.triggers.clone()),
            (
                "Hotkeys:",
                HotkeyName::HotkeysPanel,
                current.hotkeys_panel.clone(),
            ),
        ];

        let view_rows = vec![
            ("Fit view:", HotkeyName::FitView, current.fit_view.clone()),
            ("Fit Y:", HotkeyName::FitY, current.fit_y.clone()),
            (
                "Fit view continuously:",
                HotkeyName::FitViewCont,
                current.fit_view_cont.clone(),
            ),
        ];

        let control_rows = vec![
            ("Pause:", HotkeyName::Pause, current.pause.clone()),
            (
                "Export:",
                HotkeyName::ExportData,
                current.export_data.clone(),
            ),
            (
                "Reset measurements:",
                HotkeyName::ResetMeasurements,
                current.reset_measurements.clone(),
            ),
        ];

        let data_rows = vec![
            (
                "Clear all data:",
                HotkeyName::ClearAll,
                current.clear_all.clone(),
            ),
            ("Save PNG:", HotkeyName::SavePng, current.save_png.clone()),
        ];

        let sections: Vec<(&str, Vec<(&str, HotkeyName, Option<Hotkey>)>)> = vec![
            ("Panels", panel_rows),
            ("View", view_rows),
            ("Controls", control_rows),
            ("Data", data_rows),
        ];
        let total_sections = sections.len();

        for (idx, (heading, rows)) in sections.into_iter().enumerate() {
            ui.heading(heading);
            ui.add_space(4.0);
            for (label, name, value) in rows {
                render_row(ui, label, name, value);
            }
            if heading == "Controls" {
                ui.label(egui::RichText::new("Space always toggles pause.").weak());
            }
            if idx + 1 < total_sections {
                ui.separator();
            }
        }

        // Capture input events if waiting for assignment.
        if let Some(target) = self.capturing_hotkey {
            let input = ui.ctx().input(|i| i.clone());
            for ev in input.events.iter().rev() {
                if let Some(hk) = Self::event_to_hotkey(ev, input.modifiers) {
                    self.set_hotkey(target, Some(hk));
                    break;
                }
                if let egui::Event::Key {
                    key: egui::Key::Escape,
                    pressed: true,
                    ..
                } = ev
                {
                    self.capturing_hotkey = None;
                    break;
                }
            }
        }

        ui.separator();
        ui.horizontal(|ui| {
            if ui.button("Reset to defaults").clicked() {
                self.reset_defaults();
            }
            if ui.button("Save").clicked() {
                let _ = self.hotkeys.borrow().save_to_default_path();
            }
        });
    }
}
