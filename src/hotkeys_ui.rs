//! Hotkeys UI: capture widget and assignment dialog.
//!
//! Extracted from `ui.rs` to keep hotkey-related UI isolated.

use super::hotkeys::{Hotkey, HotkeyName};
use super::LivePlotApp;
use eframe::egui;

impl LivePlotApp {
    /// Render the Hotkeys settings dialog when requested.
    pub(super) fn show_hotkeys_dialog(&mut self, ctx: &egui::Context) {
        if !self.hotkeys_dialog_open {
            return;
        }
        egui::Window::new("Hotkeys")
            .collapsible(false)
            .resizable(true)
            .default_width(420.0)
            .show(ctx, |ui| {
                ui.label("Configure keyboard shortcuts for common actions.");
                ui.separator();
                ui.vertical(|ui| {
                    // Capture helper: map egui::Modifiers -> our Modifier
                    let mods_to_modifier = |m: egui::Modifiers| -> super::hotkeys::Modifier {
                        use super::hotkeys::Modifier as M;
                        match (m.ctrl, m.alt, m.shift) {
                            (false, false, false) => M::None,
                            (true, false, false) => M::Ctrl,
                            (false, true, false) => M::Alt,
                            (false, false, true) => M::Shift,
                            (true, true, false) => M::CtrlAlt,
                            (true, false, true) => M::CtrlShift,
                            (false, true, true) => M::AltShift,
                            (true, true, true) => M::CtrlAltShift,
                        }
                    };

                    // Try to convert an egui::Event into a Hotkey (using current modifiers).
                    let event_to_hotkey =
                        |ev: &egui::Event, mods: egui::Modifiers| -> Option<Hotkey> {
                            match ev {
                                egui::Event::Text(text) => {
                                    if let Some(ch) = text.chars().next() {
                                        return Some(Hotkey::new(
                                            mods_to_modifier(mods),
                                            ch.to_ascii_uppercase(),
                                        ));
                                    }
                                    None
                                }
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
                                    if let Some(ch) = ch_opt {
                                        return Some(Hotkey::new(mods_to_modifier(mods), ch));
                                    }
                                    None
                                }
                                _ => None,
                            }
                        };

                    // Snapshot current values to avoid borrow conflicts while the closure mutates state
                    let current = self.hotkeys.clone();

                    // Small helper to render a row for a single hotkey and wire assign/capture logic
                    let mut render_row =
                        |label: &str,
                         name: HotkeyName,
                         current: Option<Hotkey>,
                         ui: &mut egui::Ui| {
                            ui.horizontal(|ui| {
                                // Label with inline help tooltip (avoid rendering a separate symbol which
                                // looked broken on some platforms)
                                let tip = match name {
                                    HotkeyName::Fft => "Show / hide FFT panel",
                                    HotkeyName::Math => "Show / hide Math panel",
                                    HotkeyName::FitView => "Fit the current view to visible data",
                                    HotkeyName::FitViewCont => {
                                        "Toggle continuous fitting of the view"
                                    }
                                    HotkeyName::Traces => "Show / hide the Traces panel",
                                    HotkeyName::Thresholds => "Show / hide the Thresholds panel",
                                    HotkeyName::SavePng => "Save a PNG screenshot of the window",
                                    HotkeyName::ExportData => {
                                        "Export traces or threshold events to CSV/Parquet"
                                    }
                                    HotkeyName::ResetMarkers => "Clear/reset selected markers",
                                };
                                ui.label(label).on_hover_text(tip);

                                let capturing_this = self.capturing_hotkey == Some(name);
                                // Button shows current assignment; when clicked we start capturing.
                                let btn_text = if capturing_this {
                                    "⏺ Press keys...".to_owned()
                                } else {
                                    // Show the current keybinding right on the button
                                    match current {
                                        Some(h) => h.to_string(),
                                        None => "None".to_string(),
                                    }
                                };

                                if ui
                                    .button(btn_text)
                                    .on_hover_text(
                                        "Click to assign; press desired keys; Esc to cancel",
                                    )
                                    .clicked()
                                {
                                    // Toggle capture for this entry
                                    if capturing_this {
                                        // clicking again keeps capturing – no-op
                                    } else {
                                        self.capturing_hotkey = Some(name);
                                    }
                                }

                                if capturing_this && ui.button("Cancel").clicked() {
                                    self.capturing_hotkey = None;
                                }

                                // Provide a clear button to unset the hotkey
                                if !capturing_this {
                                    if ui
                                        .button("Clear")
                                        .on_hover_text("Disable this hotkey")
                                        .clicked()
                                    {
                                        match name {
                                            HotkeyName::Fft => self.hotkeys.fft = None,
                                            HotkeyName::Math => self.hotkeys.math = None,
                                            HotkeyName::FitView => self.hotkeys.fit_view = None,
                                            HotkeyName::FitViewCont => {
                                                self.hotkeys.fit_view_cont = None
                                            }
                                            HotkeyName::Traces => self.hotkeys.traces = None,
                                            HotkeyName::Thresholds => {
                                                self.hotkeys.thresholds = None
                                            }
                                            HotkeyName::SavePng => self.hotkeys.save_png = None,
                                            HotkeyName::ExportData => {
                                                self.hotkeys.export_data = None
                                            }
                                            HotkeyName::ResetMarkers => {
                                                self.hotkeys.reset_markers = None
                                            }
                                        }
                                        if let Err(e) = self.hotkeys.save_to_default_path() {
                                            eprintln!("Failed to save hotkeys after clear: {}", e);
                                        }
                                    }
                                }
                            });
                        };

                    // Render all rows
                    render_row("FFT:", HotkeyName::Fft, current.fft, ui);
                    render_row("Math:", HotkeyName::Math, current.math, ui);
                    render_row("Fit view:", HotkeyName::FitView, current.fit_view, ui);
                    render_row(
                        "Fit view continously:",
                        HotkeyName::FitViewCont,
                        current.fit_view_cont,
                        ui,
                    );
                    render_row("Traces:", HotkeyName::Traces, current.traces, ui);
                    render_row(
                        "Reset markers:",
                        HotkeyName::ResetMarkers,
                        current.reset_markers,
                        ui,
                    );
                    render_row(
                        "Thresholds:",
                        HotkeyName::Thresholds,
                        current.thresholds,
                        ui,
                    );
                    render_row("Save PNG:", HotkeyName::SavePng, current.save_png, ui);
                    render_row(
                        "Export data:",
                        HotkeyName::ExportData,
                        current.export_data,
                        ui,
                    );

                    // If we're capturing, inspect input events to commit assignment.
                    if let Some(target) = self.capturing_hotkey {
                        let input = ctx.input(|i| i.clone());
                        for ev in input.events.iter().rev() {
                            if let Some(hk) = event_to_hotkey(ev, input.modifiers) {
                                // assign
                                match target {
                                    HotkeyName::Fft => self.hotkeys.fft = Some(hk),
                                    HotkeyName::Math => self.hotkeys.math = Some(hk),
                                    HotkeyName::FitView => self.hotkeys.fit_view = Some(hk),
                                    HotkeyName::FitViewCont => {
                                        self.hotkeys.fit_view_cont = Some(hk)
                                    }
                                    HotkeyName::Traces => self.hotkeys.traces = Some(hk),
                                    HotkeyName::Thresholds => self.hotkeys.thresholds = Some(hk),
                                    HotkeyName::SavePng => self.hotkeys.save_png = Some(hk),
                                    HotkeyName::ExportData => self.hotkeys.export_data = Some(hk),
                                    HotkeyName::ResetMarkers => {
                                        self.hotkeys.reset_markers = Some(hk)
                                    }
                                }
                                self.capturing_hotkey = None;
                                // Persist change
                                if let Err(e) = self.hotkeys.save_to_default_path() {
                                    eprintln!("Failed to save hotkeys after assignment: {}", e);
                                }
                                break;
                            }
                            // Allow Esc to cancel
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
                });
                ui.separator();
                ui.horizontal(|ui| {
                    if ui.button("Reset to defaults").clicked() {
                        self.hotkeys.reset_defaults();
                        if let Err(e) = self.hotkeys.save_to_default_path() {
                            eprintln!("Failed to save hotkeys after reset: {}", e);
                        }
                    }
                    if ui.button("Close").clicked() {
                        self.hotkeys_dialog_open = false;
                        if let Err(e) = self.hotkeys.save_to_default_path() {
                            eprintln!("Failed to save hotkeys on close: {}", e);
                        }
                    }
                });
            });
    }
}
