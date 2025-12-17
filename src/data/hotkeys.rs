// Re-create clean content (entire file replaced)
#![allow(clippy::match_same_arms)]

use serde::{Deserialize, Serialize};
use std::fmt;
use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::str::FromStr;

use eframe::egui;

use crate::app::MainPanel;
use crate::data::data::LivePlotData;
#[cfg(feature = "fft")]
use crate::panels::fft_ui::FftPanel;
use crate::panels::{
    ExportPanel, HotkeysPanel, MathPanel, MeasurementPanel, ThresholdsPanel, TracesPanel,
    TriggersPanel,
};

// Types
#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Modifier {
    None,
    Ctrl,
    Alt,
    Shift,
    CtrlAlt,
    CtrlShift,
    AltShift,
    CtrlAltShift,
}

impl fmt::Display for Modifier {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            Modifier::None => "",
            Modifier::Ctrl => "Ctrl",
            Modifier::Alt => "Alt",
            Modifier::Shift => "Shift",
            Modifier::CtrlAlt => "Ctrl+Alt",
            Modifier::CtrlShift => "Ctrl+Shift",
            Modifier::AltShift => "Alt+Shift",
            Modifier::CtrlAltShift => "Ctrl+Alt+Shift",
        };
        write!(f, "{}", s)
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Hotkey {
    pub modifier: Modifier,
    pub key: char,
}

impl fmt::Display for Hotkey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let key = match self.key {
            ' ' => "Space".to_string(),
            other => other.to_string(),
        };

        if self.modifier == Modifier::None {
            write!(f, "{}", key)
        } else {
            write!(f, "{}+{}", self.modifier, key)
        }
    }
}

impl FromStr for Hotkey {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let s = s.trim();
        if s.is_empty() {
            return Err("empty hotkey".to_string());
        }
        let parts: Vec<&str> = s.split('+').map(|p| p.trim()).collect();
        if parts.is_empty() {
            return Err("invalid hotkey".to_string());
        }
        let last = parts.last().unwrap();
        let last_lower = last.to_lowercase();
        let ch = match last_lower.as_str() {
            "space" => ' ',
            _ => last
                .chars()
                .next()
                .ok_or_else(|| "no key char".to_string())?,
        };
        let mods = &parts[..parts.len().saturating_sub(1)];
        let modifier = match mods.len() {
            0 => Modifier::None,
            1 => match mods[0].to_lowercase().as_str() {
                "ctrl" | "control" => Modifier::Ctrl,
                "alt" => Modifier::Alt,
                "shift" => Modifier::Shift,
                other => return Err(format!("unknown modifier '{}'", other)),
            },
            2 => {
                let a = mods[0].to_lowercase();
                let b = mods[1].to_lowercase();
                if (a == "ctrl" && b == "alt") || (a == "alt" && b == "ctrl") {
                    Modifier::CtrlAlt
                } else if (a == "ctrl" && b == "shift") || (a == "shift" && b == "ctrl") {
                    Modifier::CtrlShift
                } else if (a == "alt" && b == "shift") || (a == "shift" && b == "alt") {
                    Modifier::AltShift
                } else {
                    return Err(format!("unknown modifier combo '{:?}'", mods));
                }
            }
            3 => {
                let mut lowers: Vec<String> = mods.iter().map(|m| m.to_lowercase()).collect();
                lowers.sort();
                if lowers == ["alt".to_string(), "ctrl".to_string(), "shift".to_string()] {
                    Modifier::CtrlAltShift
                } else {
                    return Err(format!("unknown modifier combo '{:?}'", mods));
                }
            }
            _ => return Err(format!("too many modifiers: {:?}", mods)),
        };
        Ok(Hotkey { modifier, key: ch })
    }
}

impl Hotkey {
    pub fn new(modifier: Modifier, key: char) -> Self {
        Self { modifier, key }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Hotkeys {
    pub fft: Option<Hotkey>,
    pub math: Option<Hotkey>,
    pub fit_view: Option<Hotkey>,
    pub fit_view_cont: Option<Hotkey>,
    pub fit_y: Option<Hotkey>,
    pub traces: Option<Hotkey>,
    pub thresholds: Option<Hotkey>,
    pub measurements: Option<Hotkey>,
    pub triggers: Option<Hotkey>,
    pub hotkeys_panel: Option<Hotkey>,
    pub pause: Option<Hotkey>,
    pub save_png: Option<Hotkey>,
    pub export_data: Option<Hotkey>,
    pub reset_markers: Option<Hotkey>,
    pub clear_all: Option<Hotkey>,
    pub reset_measurements: Option<Hotkey>,
}

impl Default for Hotkeys {
    fn default() -> Self {
        Self {
            fft: Some(Hotkey::new(Modifier::Ctrl, 'F')),
            math: Some(Hotkey::new(Modifier::Ctrl, 'M')),
            fit_view: Some(Hotkey::new(Modifier::None, 'F')),
            fit_view_cont: Some(Hotkey::new(Modifier::None, 'C')),
            fit_y: Some(Hotkey::new(Modifier::None, 'Y')),
            traces: Some(Hotkey::new(Modifier::None, 'T')),
            thresholds: Some(Hotkey::new(Modifier::Ctrl, 'T')),
            measurements: Some(Hotkey::new(Modifier::None, 'M')),
            triggers: Some(Hotkey::new(Modifier::Alt, 'G')),
            hotkeys_panel: Some(Hotkey::new(Modifier::Ctrl, 'H')),
            pause: Some(Hotkey::new(Modifier::None, 'P')),
            save_png: Some(Hotkey::new(Modifier::None, 'S')),
            export_data: Some(Hotkey::new(Modifier::None, 'E')),
            reset_markers: Some(Hotkey::new(Modifier::None, 'R')),
            clear_all: Some(Hotkey::new(Modifier::Ctrl, 'X')),
            reset_measurements: Some(Hotkey::new(Modifier::CtrlShift, 'M')),
        }
    }
}

impl Hotkeys {
    pub fn reset_defaults(&mut self) {
        *self = Hotkeys::default();
    }

    pub fn save_to_default_path(&self) -> Result<(), String> {
        let home = std::env::var("HOME").map_err(|e| format!("HOME env var not set: {}", e))?;
        let dir = PathBuf::from(home).join(".liveplot");
        if let Err(e) = fs::create_dir_all(&dir) {
            return Err(format!("Failed to create dir {:?}: {}", dir, e));
        }
        let path = dir.join("hotkeys.yaml");
        let s = serde_yaml::to_string(self).map_err(|e| format!("Serialization error: {}", e))?;
        let mut f = fs::File::create(&path)
            .map_err(|e| format!("Failed to create file {:?}: {}", path, e))?;
        f.write_all(s.as_bytes())
            .map_err(|e| format!("Failed to write file {:?}: {}", path, e))?;
        Ok(())
    }

    pub fn load_from_default_path() -> Result<Hotkeys, String> {
        let home = std::env::var("HOME").map_err(|e| format!("HOME env var not set: {}", e))?;
        let path = PathBuf::from(home).join(".liveplot").join("hotkeys.yaml");
        if !path.exists() {
            return Err(format!("Hotkeys file {:?} does not exist", path));
        }
        let s =
            fs::read_to_string(&path).map_err(|e| format!("Failed to read {:?}: {}", path, e))?;
        let hk: Hotkeys =
            serde_yaml::from_str(&s).map_err(|e| format!("Deserialization error: {}", e))?;
        Ok(hk)
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum HotkeyName {
    Fft,
    Math,
    FitView,
    FitY,
    FitViewCont,
    Pause,
    Traces,
    Thresholds,
    Measurements,
    Triggers,
    HotkeysPanel,
    SavePng,
    ExportData,
    ClearAll,
    ResetMeasurements,
}

fn key_from_char(c: char) -> Option<egui::Key> {
    match c.to_ascii_uppercase() {
        'A' => Some(egui::Key::A),
        'B' => Some(egui::Key::B),
        'C' => Some(egui::Key::C),
        'D' => Some(egui::Key::D),
        'E' => Some(egui::Key::E),
        'F' => Some(egui::Key::F),
        'G' => Some(egui::Key::G),
        'H' => Some(egui::Key::H),
        'I' => Some(egui::Key::I),
        'J' => Some(egui::Key::J),
        'K' => Some(egui::Key::K),
        'L' => Some(egui::Key::L),
        'M' => Some(egui::Key::M),
        'N' => Some(egui::Key::N),
        'O' => Some(egui::Key::O),
        'P' => Some(egui::Key::P),
        'Q' => Some(egui::Key::Q),
        'R' => Some(egui::Key::R),
        'S' => Some(egui::Key::S),
        'T' => Some(egui::Key::T),
        'U' => Some(egui::Key::U),
        'V' => Some(egui::Key::V),
        'W' => Some(egui::Key::W),
        'X' => Some(egui::Key::X),
        'Y' => Some(egui::Key::Y),
        'Z' => Some(egui::Key::Z),
        '0' => Some(egui::Key::Num0),
        '1' => Some(egui::Key::Num1),
        '2' => Some(egui::Key::Num2),
        '3' => Some(egui::Key::Num3),
        '4' => Some(egui::Key::Num4),
        '5' => Some(egui::Key::Num5),
        '6' => Some(egui::Key::Num6),
        '7' => Some(egui::Key::Num7),
        '8' => Some(egui::Key::Num8),
        '9' => Some(egui::Key::Num9),
        ' ' => Some(egui::Key::Space),
        _ => None,
    }
}

fn modifiers_match(mods: &egui::Modifiers, modifier: Modifier) -> bool {
    let ctrl = mods.ctrl || mods.command;
    let alt = mods.alt;
    let shift = mods.shift;
    match modifier {
        Modifier::None => !ctrl && !alt,
        Modifier::Ctrl => ctrl && !alt,
        Modifier::Alt => alt && !ctrl,
        Modifier::Shift => shift && !ctrl && !alt,
        Modifier::CtrlAlt => ctrl && alt,
        Modifier::CtrlShift => ctrl && shift && !alt,
        Modifier::AltShift => alt && shift && !ctrl,
        Modifier::CtrlAltShift => ctrl && alt && shift,
    }
}

fn is_hotkey_pressed(hk: Option<&Hotkey>, input: &egui::InputState) -> bool {
    let Some(hk) = hk else { return false };
    let Some(key) = key_from_char(hk.key) else {
        return false;
    };
    if !modifiers_match(&input.modifiers, hk.modifier) {
        return false;
    }
    input.key_pressed(key)
}
fn event_to_hotkey(ev: &egui::Event, mods: egui::Modifiers) -> Option<Hotkey> {
    match ev {
        egui::Event::Text(text) => text
            .chars()
            .next()
            .map(|ch| Hotkey::new(mods_to_modifier(mods), ch.to_ascii_uppercase())),
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
            ch_opt.map(|ch| Hotkey::new(mods_to_modifier(mods), ch))
        }
        _ => None,
    }
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

pub fn detect_hotkey_actions(cfg: &Hotkeys, ctx: &egui::Context) -> Vec<HotkeyName> {
    let mut actions: Vec<HotkeyName> = Vec::new();
    let input = ctx.input(|i| i.clone());
    if ctx.wants_keyboard_input() {
        return actions;
    }

    let push_action = |actions: &mut Vec<HotkeyName>, act: HotkeyName| {
        if !actions.contains(&act) {
            actions.push(act);
        }
    };

    let matches_cfg = |cfg_hk: Option<&Hotkey>, hk: &Hotkey| {
        cfg_hk
            .map(|cfg_hk| cfg_hk.modifier == hk.modifier && cfg_hk.key == hk.key)
            .unwrap_or(false)
    };

    let space_hotkey = Hotkey::new(Modifier::None, ' ');

    // First prefer event-based detection which preserves modifiers and works reliably
    // across platforms (including Ctrl/Command variants).
    for ev in input.events.iter().rev() {
        if let Some(hk) = event_to_hotkey(ev, input.modifiers) {
            // Compare to configured hotkeys and push matching actions once.
            if matches_cfg(cfg.pause.as_ref(), &hk) || matches_cfg(Some(&space_hotkey), &hk) {
                push_action(&mut actions, HotkeyName::Pause);
            }
            if matches_cfg(cfg.fit_view.as_ref(), &hk) {
                push_action(&mut actions, HotkeyName::FitView);
            }
            if matches_cfg(cfg.fit_y.as_ref(), &hk) {
                push_action(&mut actions, HotkeyName::FitY);
            }
            if matches_cfg(cfg.fit_view_cont.as_ref(), &hk) {
                push_action(&mut actions, HotkeyName::FitViewCont);
            }
            if matches_cfg(cfg.reset_measurements.as_ref(), &hk) {
                push_action(&mut actions, HotkeyName::ResetMeasurements);
            }
            if matches_cfg(cfg.traces.as_ref(), &hk) {
                push_action(&mut actions, HotkeyName::Traces);
            }
            if matches_cfg(cfg.math.as_ref(), &hk) {
                push_action(&mut actions, HotkeyName::Math);
            }
            if matches_cfg(cfg.thresholds.as_ref(), &hk) {
                push_action(&mut actions, HotkeyName::Thresholds);
            }
            if matches_cfg(cfg.measurements.as_ref(), &hk) {
                push_action(&mut actions, HotkeyName::Measurements);
            }
            if matches_cfg(cfg.triggers.as_ref(), &hk) {
                push_action(&mut actions, HotkeyName::Triggers);
            }
            if matches_cfg(cfg.hotkeys_panel.as_ref(), &hk) {
                push_action(&mut actions, HotkeyName::HotkeysPanel);
            }
            if matches_cfg(cfg.export_data.as_ref(), &hk) {
                push_action(&mut actions, HotkeyName::ExportData);
            }
            if matches_cfg(cfg.save_png.as_ref(), &hk) {
                push_action(&mut actions, HotkeyName::SavePng);
            }
            if matches_cfg(cfg.fft.as_ref(), &hk) {
                push_action(&mut actions, HotkeyName::Fft);
            }
            if matches_cfg(cfg.clear_all.as_ref(), &hk) {
                push_action(&mut actions, HotkeyName::ClearAll);
            }
        }
    }

    // Fall back to InputState.key_pressed detection if no event-based matches found
    if actions.is_empty() {
        if is_hotkey_pressed(cfg.pause.as_ref(), &input)
            || (matches_cfg(
                Some(&space_hotkey),
                &Hotkey::new(mods_to_modifier(input.modifiers), ' '),
            ) && input.key_pressed(egui::Key::Space))
        {
            push_action(&mut actions, HotkeyName::Pause);
        }
        if is_hotkey_pressed(cfg.fit_view.as_ref(), &input) {
            push_action(&mut actions, HotkeyName::FitView);
        }
        if is_hotkey_pressed(cfg.fit_y.as_ref(), &input) {
            push_action(&mut actions, HotkeyName::FitY);
        }
        if is_hotkey_pressed(cfg.fit_view_cont.as_ref(), &input) {
            push_action(&mut actions, HotkeyName::FitViewCont);
        }
        if is_hotkey_pressed(cfg.reset_measurements.as_ref(), &input) {
            push_action(&mut actions, HotkeyName::ResetMeasurements);
        }
        if is_hotkey_pressed(cfg.traces.as_ref(), &input) {
            push_action(&mut actions, HotkeyName::Traces);
        }
        if is_hotkey_pressed(cfg.math.as_ref(), &input) {
            push_action(&mut actions, HotkeyName::Math);
        }
        if is_hotkey_pressed(cfg.thresholds.as_ref(), &input) {
            push_action(&mut actions, HotkeyName::Thresholds);
        }
        if is_hotkey_pressed(cfg.measurements.as_ref(), &input) {
            push_action(&mut actions, HotkeyName::Measurements);
        }
        if is_hotkey_pressed(cfg.triggers.as_ref(), &input) {
            push_action(&mut actions, HotkeyName::Triggers);
        }
        if is_hotkey_pressed(cfg.hotkeys_panel.as_ref(), &input) {
            push_action(&mut actions, HotkeyName::HotkeysPanel);
        }
        if is_hotkey_pressed(cfg.export_data.as_ref(), &input) {
            push_action(&mut actions, HotkeyName::ExportData);
        }
        if is_hotkey_pressed(cfg.save_png.as_ref(), &input) {
            push_action(&mut actions, HotkeyName::SavePng);
        }
        if is_hotkey_pressed(cfg.fft.as_ref(), &input) {
            push_action(&mut actions, HotkeyName::Fft);
        }
        if is_hotkey_pressed(cfg.clear_all.as_ref(), &input) {
            push_action(&mut actions, HotkeyName::ClearAll);
        }
    }

    actions
}

pub fn handle_hotkeys(main_panel: &mut MainPanel, ctx: &egui::Context) {
    let hk = main_panel.hotkeys.borrow().clone();
    let actions = detect_hotkey_actions(&hk, ctx);
    for act in actions {
        let mut data = LivePlotData {
            scope_data: main_panel.liveplot_panel.get_data_mut(),
            traces: &mut main_panel.traces_data,
            pending_requests: &mut main_panel.pending_requests,
        };
        match act {
            HotkeyName::Pause => {
                data.toggle_pause();
            }
            HotkeyName::FitView => {
                data.fit_all_bounds();
            }
            HotkeyName::FitY => {
                data.fit_all_y_bounds();
            }
            HotkeyName::FitViewCont => {
                let mut scopes = main_panel.liveplot_panel.get_data_mut();
                let auto_fit = scopes
                    .first()
                    .map(|s| (**s).y_axis.auto_fit)
                    .unwrap_or(false);
                for scope in scopes.iter_mut() {
                    let scope = &mut **scope;
                    scope.y_axis.auto_fit = !auto_fit;
                }
            }
            HotkeyName::ResetMeasurements => {
                data.pending_requests.clear_measurements = true;
            }
            HotkeyName::Traces => {
                main_panel.toggle_panel_visibility::<TracesPanel>();
                main_panel.hide_hotkeys_panel();
            }
            HotkeyName::Math => {
                main_panel.toggle_panel_visibility::<MathPanel>();
                main_panel.hide_hotkeys_panel();
            }
            HotkeyName::Thresholds => {
                main_panel.toggle_panel_visibility::<ThresholdsPanel>();
                main_panel.hide_hotkeys_panel();
            }
            HotkeyName::Measurements => {
                main_panel.toggle_panel_visibility::<MeasurementPanel>();
                main_panel.hide_hotkeys_panel();
            }
            HotkeyName::Triggers => {
                main_panel.toggle_panel_visibility::<TriggersPanel>();
                main_panel.hide_hotkeys_panel();
            }
            HotkeyName::HotkeysPanel => {
                main_panel.toggle_panel_visibility::<HotkeysPanel>();
            }
            HotkeyName::ExportData => {
                main_panel.toggle_panel_visibility::<ExportPanel>();
                main_panel.hide_hotkeys_panel();
            }
            HotkeyName::Fft => {
                #[cfg(feature = "fft")]
                {
                    main_panel.toggle_panel_visibility::<FftPanel>();
                    main_panel.hide_hotkeys_panel();
                }
            }
            HotkeyName::SavePng => {
                ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(Default::default()));
            }
            HotkeyName::ClearAll => {
                data.request_clear_all();
            }
        }
    }
}
