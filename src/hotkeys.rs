//! Hotkeys representation and parsing for LivePlot UI.

use serde::{Deserialize, Serialize};
use std::fmt;
use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::str::FromStr;

/// Modifier keys (combinations) used for hotkeys.
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

/// A single hotkey consisting of optional modifier(s) and a character key.
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
        // Accept formats like "Ctrl+F" or "F" or "Ctrl+Alt+X"
        let parts: Vec<&str> = s.split('+').map(|p| p.trim()).collect();
        if parts.is_empty() {
            return Err("invalid hotkey".to_string());
        }
        let last = parts.last().unwrap();
        let ch = last
            .chars()
            .next()
            .ok_or_else(|| "no key char".to_string())?;
        // modifiers are all but last
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

/// Container for all configurable hotkeys.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Hotkeys {
    pub fft: Hotkey,
    pub math: Hotkey,
    pub fit_view: Hotkey,
    pub fit_view_cont: Hotkey,
    pub traces: Hotkey,
    pub thresholds: Hotkey,
    pub save_png: Hotkey,
    pub export_data: Hotkey,
}

impl Default for Hotkeys {
    fn default() -> Self {
        Self {
            fft: Hotkey::new(Modifier::Ctrl, 'F'),
            math: Hotkey::new(Modifier::None, 'M'),
            fit_view: Hotkey::new(Modifier::None, 'F'),
            fit_view_cont: Hotkey::new(Modifier::None, 'C'),
            traces: Hotkey::new(Modifier::None, 'T'),
            thresholds: Hotkey::new(Modifier::Ctrl, 'T'),
            save_png: Hotkey::new(Modifier::None, 'S'),
            export_data: Hotkey::new(Modifier::None, 'E'),
        }
    }
}

impl Hotkeys {
    pub fn reset_defaults(&mut self) {
        *self = Hotkeys::default();
    }

    /// Save hotkeys to the default path `~/.liveplot/hotkeys.yaml`.
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

    /// Load hotkeys from `~/.liveplot/hotkeys.yaml` if present.
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

/// Name of a hotkey entry to identify capture targets
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum HotkeyName {
    Fft,
    Math,
    FitView,
    FitViewCont,
    Traces,
    Thresholds,
    SavePng,
    ExportData,
}
