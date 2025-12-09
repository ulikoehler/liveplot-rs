//! State persistence: save and load application state to/from JSON files.
//!
//! This module provides serializable mirror types for UI state that cannot directly
//! derive serde traits (e.g., egui types like Color32, LineStyle).

use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::data::scope::{AxisSettings, ScopeData, ScopeType};
use crate::data::thresholds::{ThresholdDef, ThresholdKind};
use crate::data::trace_look::TraceLook;
use crate::data::traces::TraceRef;
use crate::data::triggers::{Trigger, TriggerSlope};

// ---------- Serializable mirror types ----------

/// Serializable version of AxisSettings.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AxisSettingsSerde {
    pub unit: Option<String>,
    pub log_scale: bool,
    pub format: Option<String>,
    pub name: Option<String>,
    pub bounds: [f64; 2],
    pub auto_fit: bool,
}

impl From<&AxisSettings> for AxisSettingsSerde {
    fn from(a: &AxisSettings) -> Self {
        Self {
            unit: a.unit.clone(),
            log_scale: a.log_scale,
            format: a.format.clone(),
            name: a.name.clone(),
            bounds: [a.bounds.0, a.bounds.1],
            auto_fit: a.auto_fit,
        }
    }
}

impl AxisSettingsSerde {
    /// Apply stored settings to an AxisSettings instance.
    pub fn apply_to(self, a: &mut AxisSettings) {
        a.unit = self.unit;
        a.log_scale = self.log_scale;
        a.format = self.format;
        a.name = self.name;
        a.bounds = (self.bounds[0], self.bounds[1]);
        a.auto_fit = self.auto_fit;
    }
}

/// Serializable version of egui_plot::LineStyle.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SerLineStyle {
    Solid,
    Dashed { length: f32 },
    Dotted { spacing: f32 },
}

/// Serializable version of egui_plot::MarkerShape.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SerMarkerShape {
    Circle,
    Square,
    Diamond,
    Cross,
    Plus,
    Asterisk,
    Up,
    Down,
    Left,
    Right,
}

/// Serializable version of TraceLook.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TraceLookSerde {
    pub color_rgba: [u8; 4],
    pub visible: bool,
    pub width: f32,
    pub show_points: bool,
    pub style: SerLineStyle,
    pub point_size: f32,
    pub marker: SerMarkerShape,
}

impl From<&TraceLook> for TraceLookSerde {
    fn from(l: &TraceLook) -> Self {
        use egui_plot::LineStyle;
        use egui_plot::MarkerShape;
        let style = match l.style {
            LineStyle::Solid => SerLineStyle::Solid,
            LineStyle::Dashed { length } => SerLineStyle::Dashed { length },
            LineStyle::Dotted { spacing } => SerLineStyle::Dotted { spacing },
        };
        let marker = match l.marker {
            MarkerShape::Circle => SerMarkerShape::Circle,
            MarkerShape::Square => SerMarkerShape::Square,
            MarkerShape::Diamond => SerMarkerShape::Diamond,
            MarkerShape::Cross => SerMarkerShape::Cross,
            MarkerShape::Plus => SerMarkerShape::Plus,
            MarkerShape::Asterisk => SerMarkerShape::Asterisk,
            MarkerShape::Up => SerMarkerShape::Up,
            MarkerShape::Down => SerMarkerShape::Down,
            MarkerShape::Left => SerMarkerShape::Left,
            MarkerShape::Right => SerMarkerShape::Right,
        };
        Self {
            color_rgba: [l.color.r(), l.color.g(), l.color.b(), l.color.a()],
            visible: l.visible,
            width: l.width,
            show_points: l.show_points,
            style,
            point_size: l.point_size,
            marker,
        }
    }
}

impl TraceLookSerde {
    /// Convert back to a TraceLook.
    pub fn into_look(self) -> TraceLook {
        use egui::Color32;
        use egui_plot::LineStyle;
        use egui_plot::MarkerShape;
        let style = match self.style {
            SerLineStyle::Solid => LineStyle::Solid,
            SerLineStyle::Dashed { length } => LineStyle::Dashed { length },
            SerLineStyle::Dotted { spacing } => LineStyle::Dotted { spacing },
        };
        let marker = match self.marker {
            SerMarkerShape::Circle => MarkerShape::Circle,
            SerMarkerShape::Square => MarkerShape::Square,
            SerMarkerShape::Diamond => MarkerShape::Diamond,
            SerMarkerShape::Cross => MarkerShape::Cross,
            SerMarkerShape::Plus => MarkerShape::Plus,
            SerMarkerShape::Asterisk => MarkerShape::Asterisk,
            SerMarkerShape::Up => MarkerShape::Up,
            SerMarkerShape::Down => MarkerShape::Down,
            SerMarkerShape::Left => MarkerShape::Left,
            SerMarkerShape::Right => MarkerShape::Right,
        };
        TraceLook {
            color: Color32::from_rgba_unmultiplied(
                self.color_rgba[0],
                self.color_rgba[1],
                self.color_rgba[2],
                self.color_rgba[3],
            ),
            visible: self.visible,
            width: self.width,
            show_points: self.show_points,
            style,
            point_size: self.point_size,
            marker,
        }
    }
}

/// Serializable trace style entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TraceStyleSerde {
    pub name: String,
    pub look: TraceLookSerde,
    pub offset: f64,
}

/// Serializable trigger slope.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SerTriggerSlope {
    Rising,
    Falling,
    Any,
}

impl From<TriggerSlope> for SerTriggerSlope {
    fn from(s: TriggerSlope) -> Self {
        match s {
            TriggerSlope::Rising => SerTriggerSlope::Rising,
            TriggerSlope::Falling => SerTriggerSlope::Falling,
            TriggerSlope::Any => SerTriggerSlope::Any,
        }
    }
}

impl From<SerTriggerSlope> for TriggerSlope {
    fn from(s: SerTriggerSlope) -> Self {
        match s {
            SerTriggerSlope::Rising => TriggerSlope::Rising,
            SerTriggerSlope::Falling => TriggerSlope::Falling,
            SerTriggerSlope::Any => TriggerSlope::Any,
        }
    }
}

/// Serializable trigger definition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TriggerSerde {
    pub name: String,
    pub target: String,
    pub enabled: bool,
    pub level: f64,
    pub slope: SerTriggerSlope,
    pub single_shot: bool,
    pub trigger_position: f64,
    pub look: TraceLookSerde,
}

impl TriggerSerde {
    /// Create from a Trigger.
    pub fn from_trigger(t: &Trigger) -> Self {
        Self {
            name: t.name.clone(),
            target: t.target.0.clone(),
            enabled: t.enabled,
            level: t.level,
            slope: SerTriggerSlope::from(t.slope),
            single_shot: t.single_shot,
            trigger_position: t.trigger_position,
            look: TraceLookSerde::from(&t.look),
        }
    }

    /// Convert back to a Trigger.
    pub fn into_trigger(self) -> Trigger {
        let mut t = Trigger::default();
        t.name = self.name;
        t.target = TraceRef(self.target);
        t.enabled = self.enabled;
        t.level = self.level;
        t.slope = TriggerSlope::from(self.slope);
        t.single_shot = self.single_shot;
        t.trigger_position = self.trigger_position;
        t.look = self.look.into_look();
        t
    }
}

/// Serializable threshold definition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThresholdSerde {
    pub name: String,
    pub target: String,
    pub kind: ThresholdKind,
    pub min_duration_s: f64,
    pub max_events: usize,
    pub look: TraceLookSerde,
    pub start_look: TraceLookSerde,
    pub stop_look: TraceLookSerde,
}

impl ThresholdSerde {
    /// Create from a ThresholdDef.
    pub fn from_threshold(d: &ThresholdDef) -> Self {
        Self {
            name: d.name.clone(),
            target: d.target.0.clone(),
            kind: d.kind.clone(),
            min_duration_s: d.min_duration_s,
            max_events: d.max_events,
            look: TraceLookSerde::from(&d.look),
            start_look: TraceLookSerde::from(&d.start_look),
            stop_look: TraceLookSerde::from(&d.stop_look),
        }
    }

    /// Convert back to a ThresholdDef.
    pub fn into_threshold(self) -> ThresholdDef {
        let mut d = ThresholdDef::default();
        d.name = self.name;
        d.target = TraceRef(self.target);
        d.kind = self.kind;
        d.min_duration_s = self.min_duration_s;
        d.max_events = self.max_events;
        d.look = self.look.into_look();
        d.start_look = self.start_look.into_look();
        d.stop_look = self.stop_look.into_look();
        d
    }
}

/// Serializable scope state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScopeStateSerde {
    pub x_axis: AxisSettingsSerde,
    pub y_axis: AxisSettingsSerde,
    pub time_window: f64,
    pub scope_is_xy: bool,
    pub show_legend: bool,
    pub show_info_in_legend: bool,
}

impl From<&ScopeData> for ScopeStateSerde {
    fn from(s: &ScopeData) -> Self {
        Self {
            x_axis: AxisSettingsSerde::from(&s.x_axis),
            y_axis: AxisSettingsSerde::from(&s.y_axis),
            time_window: s.time_window,
            scope_is_xy: matches!(s.scope_type, ScopeType::XYScope),
            show_legend: s.show_legend,
            show_info_in_legend: s.show_info_in_legend,
        }
    }
}

impl ScopeStateSerde {
    /// Apply stored settings to a ScopeData instance.
    pub fn apply_to(self, scope: &mut ScopeData) {
        self.x_axis.apply_to(&mut scope.x_axis);
        self.y_axis.apply_to(&mut scope.y_axis);
        scope.time_window = self.time_window;
        scope.scope_type = if self.scope_is_xy {
            ScopeType::XYScope
        } else {
            ScopeType::TimeScope
        };
        scope.show_legend = self.show_legend;
        scope.show_info_in_legend = self.show_info_in_legend;
    }
}

/// Panel visibility state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PanelVisSerde {
    pub title: String,
    pub visible: bool,
    pub detached: bool,
    pub window_pos: Option<[f32; 2]>,
    pub window_size: Option<[f32; 2]>,
}

/// Full application state (for save/load).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppStateSerde {
    pub window_size: Option<[f32; 2]>,
    pub window_pos: Option<[f32; 2]>,
    pub scope: ScopeStateSerde,
    pub panels: Vec<PanelVisSerde>,
    pub traces_style: Vec<TraceStyleSerde>,
    pub thresholds: Vec<ThresholdSerde>,
    pub triggers: Vec<TriggerSerde>,
}

impl Default for AppStateSerde {
    fn default() -> Self {
        Self {
            window_size: None,
            window_pos: None,
            scope: ScopeStateSerde {
                x_axis: AxisSettingsSerde {
                    unit: None,
                    log_scale: false,
                    format: None,
                    name: None,
                    bounds: [0.0, 1.0],
                    auto_fit: true,
                },
                y_axis: AxisSettingsSerde {
                    unit: None,
                    log_scale: false,
                    format: None,
                    name: None,
                    bounds: [0.0, 1.0],
                    auto_fit: true,
                },
                time_window: 10.0,
                scope_is_xy: false,
                show_legend: true,
                show_info_in_legend: false,
            },
            panels: Vec::new(),
            traces_style: Vec::new(),
            thresholds: Vec::new(),
            triggers: Vec::new(),
        }
    }
}

// ---------- Public API ----------

/// Serialize the application state as pretty JSON.
pub fn state_to_json(state: &AppStateSerde) -> Result<String, String> {
    serde_json::to_string_pretty(state).map_err(|e| e.to_string())
}

/// Deserialize application state from JSON.
pub fn state_from_json(json: &str) -> Result<AppStateSerde, String> {
    serde_json::from_str(json).map_err(|e| e.to_string())
}

/// Save the application state to a JSON file at the given path.
pub fn save_state_to_path(state: &AppStateSerde, path: &Path) -> Result<(), String> {
    let txt = state_to_json(state)?;
    std::fs::write(path, txt).map_err(|e| e.to_string())
}

/// Load the application state from a JSON file at the given path.
pub fn load_state_from_path(path: &Path) -> Result<AppStateSerde, String> {
    let txt = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
    state_from_json(&txt)
}

/// Helper to capture trace styles from a traces collection.
pub fn capture_trace_styles<'a>(
    trace_order: impl Iterator<Item = &'a TraceRef>,
    get_trace: impl Fn(&TraceRef) -> Option<(&TraceLook, f64)>,
) -> Vec<TraceStyleSerde> {
    trace_order
        .filter_map(|name| {
            get_trace(name).map(|(look, offset)| TraceStyleSerde {
                name: name.0.clone(),
                look: TraceLookSerde::from(look),
                offset,
            })
        })
        .collect()
}

/// Helper to apply trace styles to a traces collection.
pub fn apply_trace_styles(styles: &[TraceStyleSerde], mut apply: impl FnMut(&str, TraceLook, f64)) {
    for s in styles {
        apply(&s.name, s.look.clone().into_look(), s.offset);
    }
}
