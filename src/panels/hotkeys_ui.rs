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
                HotkeyName::FitViewCont => hk.fit_view_cont = value,
                HotkeyName::Pause => hk.pause = value,
                HotkeyName::Traces => hk.traces = value,
                HotkeyName::Thresholds => hk.thresholds = value,
                HotkeyName::SavePng => hk.save_png = value,
                HotkeyName::ExportData => hk.export_data = value,
                HotkeyName::ResetMarkers => hk.reset_markers = value,
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

    fn render_panel(&mut self, ui: &mut egui::Ui, _data: &mut LivePlotData<'_>) {
        ui.label("Configure keyboard shortcuts for common actions.");
        ui.separator();

        // Snapshot current to avoid borrow conflicts while mutating later.
        let current = self.hotkeys.borrow().clone();

        let mut render_row = |ui: &mut egui::Ui,
                              label: &str,
                              name: HotkeyName,
                              current: Option<Hotkey>| {
            ui.horizontal(|ui| {
                let tip = match name {
                    HotkeyName::Fft => "Show / hide FFT panel",
                    HotkeyName::Math => "Show / hide Math panel",
                    HotkeyName::FitView => "Fit the current view to visible data",
                    HotkeyName::FitViewCont => "Toggle continuous fitting of the view",
                    HotkeyName::Pause => "Pause / resume plotting",
                    HotkeyName::Traces => "Show / hide the Traces panel",
                    HotkeyName::Thresholds => "Show / hide the Thresholds panel",
                    HotkeyName::SavePng => "Save a PNG screenshot of the window",
                    HotkeyName::ExportData => "Export traces or threshold events to CSV/Parquet",
                    HotkeyName::ResetMarkers => "Clear/reset selected markers",
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

        #[cfg(feature = "fft")]
        render_row(ui, "FFT:", HotkeyName::Fft, current.fft);
        render_row(ui, "Math:", HotkeyName::Math, current.math);
        render_row(ui, "Fit view:", HotkeyName::FitView, current.fit_view);
        render_row(
            ui,
            "Fit view continously:",
            HotkeyName::FitViewCont,
            current.fit_view_cont,
        );
        render_row(ui, "Traces:", HotkeyName::Traces, current.traces);
        render_row(ui, "Pause:", HotkeyName::Pause, current.pause);
        render_row(
            ui,
            "Reset markers:",
            HotkeyName::ResetMarkers,
            current.reset_markers,
        );
        render_row(
            ui,
            "Thresholds:",
            HotkeyName::Thresholds,
            current.thresholds,
        );
        render_row(ui, "Save PNG:", HotkeyName::SavePng, current.save_png);
        render_row(
            ui,
            "Export data:",
            HotkeyName::ExportData,
            current.export_data,
        );

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
