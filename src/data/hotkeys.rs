// Re-create clean content (entire file replaced)
#![allow(clippy::match_same_arms)]

use serde::{Deserialize, Serialize};
use std::fmt;
use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::str::FromStr;

use eframe::egui;

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
        if self.modifier == Modifier::None {
            write!(f, "{}", self.key)
        } else {
            write!(f, "{}+{}", self.modifier, self.key)
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
        let ch = last
            .chars()
            .next()
            .ok_or_else(|| "no key char".to_string())?;
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
    pub traces: Option<Hotkey>,
    pub thresholds: Option<Hotkey>,
    pub pause: Option<Hotkey>,
    pub save_png: Option<Hotkey>,
    pub export_data: Option<Hotkey>,
    pub reset_markers: Option<Hotkey>,
}

impl Default for Hotkeys {
    fn default() -> Self {
        Self {
            fft: Some(Hotkey::new(Modifier::Ctrl, 'F')),
            math: Some(Hotkey::new(Modifier::None, 'M')),
            fit_view: Some(Hotkey::new(Modifier::None, 'F')),
            fit_view_cont: Some(Hotkey::new(Modifier::None, 'C')),
            traces: Some(Hotkey::new(Modifier::None, 'T')),
            thresholds: Some(Hotkey::new(Modifier::Ctrl, 'T')),
            pause: Some(Hotkey::new(Modifier::None, 'P')),
            save_png: Some(Hotkey::new(Modifier::None, 'S')),
            export_data: Some(Hotkey::new(Modifier::None, 'E')),
            reset_markers: Some(Hotkey::new(Modifier::None, 'R')),
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
        let s = fs::read_to_string(&path).map_err(|e| format!("Failed to read {:?}: {}", path, e))?;
        let hk: Hotkeys = serde_yaml::from_str(&s).map_err(|e| format!("Deserialization error: {}", e))?;
        Ok(hk)
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum HotkeyName {
    Fft,
    Math,
    FitView,
    FitViewCont,
    Pause,
    Traces,
    Thresholds,
    SavePng,
    ExportData,
    ResetMarkers,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum HotkeyAction {
    Pause,
    FitView,
    FitViewCont,
    ResetMarkers,
    ToggleTraces,
    ToggleMath,
    ToggleThresholds,
    ToggleExport,
    ToggleFft,
    SavePng,
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
    let Some(key) = key_from_char(hk.key) else { return false };
    if !modifiers_match(&input.modifiers, hk.modifier) {
        return false;
    }
    input.key_pressed(key)
}

pub fn detect_hotkey_actions(cfg: &Hotkeys, ctx: &egui::Context) -> Vec<HotkeyAction> {
    let mut actions: Vec<HotkeyAction> = Vec::new();
    let input = ctx.input(|i| i.clone());
    if ctx.wants_keyboard_input() {
        return actions;
    }

    if is_hotkey_pressed(cfg.pause.as_ref(), &input) {
        actions.push(HotkeyAction::Pause);
    }
    if is_hotkey_pressed(cfg.fit_view.as_ref(), &input) {
        actions.push(HotkeyAction::FitView);
    }
    if is_hotkey_pressed(cfg.fit_view_cont.as_ref(), &input) {
        actions.push(HotkeyAction::FitViewCont);
    }
    if is_hotkey_pressed(cfg.reset_markers.as_ref(), &input) {
        actions.push(HotkeyAction::ResetMarkers);
    }
    if is_hotkey_pressed(cfg.traces.as_ref(), &input) {
        actions.push(HotkeyAction::ToggleTraces);
    }
    if is_hotkey_pressed(cfg.math.as_ref(), &input) {
        actions.push(HotkeyAction::ToggleMath);
    }
    if is_hotkey_pressed(cfg.thresholds.as_ref(), &input) {
        actions.push(HotkeyAction::ToggleThresholds);
    }
    if is_hotkey_pressed(cfg.export_data.as_ref(), &input) {
        actions.push(HotkeyAction::ToggleExport);
    }
    if is_hotkey_pressed(cfg.save_png.as_ref(), &input) {
        actions.push(HotkeyAction::SavePng);
    }
    if is_hotkey_pressed(cfg.fft.as_ref(), &input) {
        actions.push(HotkeyAction::ToggleFft);
    }

    actions
}

