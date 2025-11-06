use std::collections::HashMap;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::app::MainPanel;
use crate::data::scope::{AxisSettings, ScopeType};
use crate::data::traces::{TracesCollection, TraceRef};
use crate::panels::{math_ui::MathPanel, thresholds_ui::ThresholdsPanel, triggers_ui::TriggersPanel};
use crate::panels::panel_trait::Panel;
use crate::data::scope::ScopeData;

// ---------- Serializable mirror types ----------

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
    fn apply_to(self, a: &mut AxisSettings) {
        a.unit = self.unit;
        a.log_scale = self.log_scale;
        a.format = self.format;
        a.name = self.name;
        a.bounds = (self.bounds[0], self.bounds[1]);
        a.auto_fit = self.auto_fit;
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SerLineStyle {
    Solid,
    Dashed { length: f32 },
    Dotted { spacing: f32 },
}

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

impl From<&crate::data::trace_look::TraceLook> for TraceLookSerde {
    fn from(l: &crate::data::trace_look::TraceLook) -> Self {
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
    fn into_look(self) -> crate::data::trace_look::TraceLook {
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
        crate::data::trace_look::TraceLook {
            color: egui::Color32::from_rgba_unmultiplied(
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TraceStyleSerde {
    pub name: TraceRef,
    pub look: TraceLookSerde,
    pub offset: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SerTriggerSlope {
    Rising,
    Falling,
    Any,
}

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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThresholdSerde {
    pub name: String,
    pub target: String,
    pub kind: crate::data::thresholds::ThresholdKind,
    pub min_duration_s: f64,
    pub max_events: usize,
    pub look: TraceLookSerde,
    pub start_look: TraceLookSerde,
    pub stop_look: TraceLookSerde,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScopeStateSerde {
    pub x_axis: AxisSettingsSerde,
    pub y_axis: AxisSettingsSerde,
    pub time_window: f64,
    pub scope_is_xy: bool,
    pub show_legend: bool,
    pub show_info_in_legend: bool,
    pub selection_trace: Option<TraceRef>,
}

impl From<&ScopeData> for ScopeStateSerde {
    fn from(data: &ScopeData) -> Self {
        let s: &ScopeData = &data;
        Self {
            x_axis: AxisSettingsSerde::from(&s.x_axis),
            y_axis: AxisSettingsSerde::from(&s.y_axis),
            time_window: s.time_window,
            scope_is_xy: matches!(s.scope_type, ScopeType::XYScope),
            show_legend: s.show_legend,
            show_info_in_legend: s.show_info_in_legend,
            selection_trace: s.selection_trace.clone(),
        }
    }
}

impl ScopeStateSerde {
    fn apply_to(self, scope: &mut ScopeData) {
        self.x_axis.apply_to(&mut scope.x_axis);
        self.y_axis.apply_to(&mut scope.y_axis);
        scope.time_window = self.time_window;
        scope.scope_type = if self.scope_is_xy { ScopeType::XYScope } else { ScopeType::TimeScope };
        scope.show_legend = self.show_legend;
        scope.show_info_in_legend = self.show_info_in_legend;
        scope.selection_trace = self.selection_trace;
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PanelVisSerde {
    pub title: String,
    pub visible: bool,
    pub detached: bool,
    pub window_pos: Option<[f32; 2]>,
    pub window_size: Option<[f32; 2]>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppStateSerde {
    pub window_size: Option<[f32; 2]>,
    pub window_pos: Option<[f32; 2]>,
    pub scope: ScopeStateSerde,
    pub panels: Vec<PanelVisSerde>,
    pub traces_style: Vec<TraceStyleSerde>,
    pub math_traces: Vec<crate::data::math::MathTrace>,
    pub thresholds: Vec<ThresholdSerde>,
    pub triggers: Vec<TriggerSerde>,
}

impl AppStateSerde {
    fn capture(panel: &mut MainPanel, window_size: Option<[f32; 2]>) -> Self {
    let scope_data = panel.scope_panel.get_data_mut();
    let traces_data = &panel.traces_data;
        // traces styles
        let mut traces_style = Vec::new();
        for name in scope_data.trace_order.iter() {
            if let Some(tr) = traces_data.get_trace(name) {
                traces_style.push(TraceStyleSerde {
                    name: name.clone(),
                    look: TraceLookSerde::from(&tr.look),
                    offset: tr.offset,
                });
            }
        }
        // panels visibility
        let mut panels = Vec::new();
        let collect = |v: &Vec<Box<dyn Panel>>, out: &mut Vec<PanelVisSerde>| {
            for p in v.iter() {
                let st = p.state();
                out.push(PanelVisSerde {
                    title: st.title.to_string(),
                    visible: st.visible,
                    detached: st.detached,
                    window_pos: st.window_pos,
                    window_size: st.window_size,
                });
            }
        };
    collect(&panel.left_side_panels, &mut panels);
    collect(&panel.right_side_panels, &mut panels);
    collect(&panel.bottom_panels, &mut panels);
    collect(&panel.detached_panels, &mut panels);
    collect(&panel.empty_panels, &mut panels);

        Self {
            window_size,
            window_pos: None,
            scope: ScopeStateSerde::from(&*panel.scope_panel.get_data_mut()),
            panels,
            traces_style,
            math_traces: Vec::new(),
            thresholds: Vec::new(),
            triggers: Vec::new(),
        }
    }
}

// ---------- Public API ----------

/// Capture the current application state into a serializable struct.
pub fn save_mainpanel_to_struct(ctx: &egui::Context, panel: &mut MainPanel) -> AppStateSerde {
    // Capture last known window size and position (best-effort)
    let rect = ctx.input(|i| i.content_rect());
    let win_size = Some([rect.width(), rect.height()]);
    let win_pos = Some([rect.left(), rect.top()]);

    // Capture math/thresholds/triggers by temporarily taking mutable refs to panel lists to downcast
    let mut left = std::mem::take(&mut panel.left_side_panels);
    let mut right = std::mem::take(&mut panel.right_side_panels);
    let mut bottom = std::mem::take(&mut panel.bottom_panels);
    let mut detached = std::mem::take(&mut panel.detached_panels);
    let mut empty = std::mem::take(&mut panel.empty_panels);

    let mut state = AppStateSerde::capture(panel, win_size);
    state.window_pos = win_pos;

    let mut extract_panels = |list: &mut Vec<Box<dyn Panel>>| {
        for p in list.iter_mut() {
            // MathPanel
            let any = p.as_mut() as &mut dyn std::any::Any;
            if let Some(mp) = any.downcast_mut::<MathPanel>() {
                state.math_traces = mp.get_math_traces().clone();
            }
            let any = p.as_mut() as &mut dyn std::any::Any;
            if let Some(tp) = any.downcast_mut::<TriggersPanel>() {
                // Convert triggers map to serializable
                for (_k, t) in tp.triggers.iter() {
                    let slope = match t.slope {
                        crate::data::triggers::TriggerSlope::Rising => SerTriggerSlope::Rising,
                        crate::data::triggers::TriggerSlope::Falling => SerTriggerSlope::Falling,
                        crate::data::triggers::TriggerSlope::Any => SerTriggerSlope::Any,
                    };
                    state.triggers.push(TriggerSerde {
                        name: t.name.clone(),
                        target: t.target.0.clone(),
                        enabled: t.enabled,
                        level: t.level,
                        slope,
                        single_shot: t.single_shot,
                        trigger_position: t.trigger_position,
                        look: TraceLookSerde::from(&t.look),
                    });
                }
            }
            let any = p.as_mut() as &mut dyn std::any::Any;
            if let Some(thp) = any.downcast_mut::<ThresholdsPanel>() {
                for (_k, d) in thp.thresholds.iter() {
                    state.thresholds.push(ThresholdSerde {
                        name: d.name.clone(),
                        target: d.target.0.clone(),
                        kind: d.kind.clone(),
                        min_duration_s: d.min_duration_s,
                        max_events: d.max_events,
                        look: TraceLookSerde::from(&d.look),
                        start_look: TraceLookSerde::from(&d.start_look),
                        stop_look: TraceLookSerde::from(&d.stop_look),
                    });
                }
            }
        }
    };
    extract_panels(&mut left);
    extract_panels(&mut right);
    extract_panels(&mut bottom);
    extract_panels(&mut detached);
    extract_panels(&mut empty);

    // Return panels back
    panel.left_side_panels = left;
    panel.right_side_panels = right;
    panel.bottom_panels = bottom;
    panel.detached_panels = detached;
    panel.empty_panels = empty;

    state
}

/// Serialize the application state as pretty JSON.
pub fn save_mainpanel_to_json(ctx: &egui::Context, panel: &mut MainPanel) -> Result<String, String> {
    let state = save_mainpanel_to_struct(ctx, panel);
    serde_json::to_string_pretty(&state).map_err(|e| e.to_string())
}

/// Save the application state to a JSON file at the given path.
pub fn save_mainpanel_to_path(ctx: &egui::Context, panel: &mut MainPanel, path: &Path) -> Result<(), String> {
    let txt = save_mainpanel_to_json(ctx, panel)?;
    std::fs::write(path, txt).map_err(|e| e.to_string())
}

/// Load the application state from a JSON file at the given path and apply it.
pub fn load_mainpanel_from_path(ctx: &egui::Context, panel: &mut MainPanel, path: &Path) -> Result<(), String> {
    let txt = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
    load_mainpanel_from_json(ctx, panel, &txt)
}

/// Load the application state from a JSON string and apply it.
pub fn load_mainpanel_from_json(ctx: &egui::Context, panel: &mut MainPanel, json: &str) -> Result<(), String> {
    let state: AppStateSerde = serde_json::from_str(json).map_err(|e| e.to_string())?;
    load_mainpanel_from_struct(ctx, panel, state)
}

/// Apply a previously captured application state.
pub fn load_mainpanel_from_struct(ctx: &egui::Context, panel: &mut MainPanel, state: AppStateSerde) -> Result<(), String> {

    // Apply window size (best-effort)
    if let Some([w, h]) = state.window_size {
        ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(egui::Vec2::new(w, h)));
    }
    // Apply window position (not supported on all platforms/versions, skipped here)

    // Apply scope settings
    state.scope.apply_to(panel.scope_panel.get_data_mut());

    // Apply panel visibility and stored positions/sizes
    let panel_info: HashMap<String, (bool, bool, Option<[f32; 2]>, Option<[f32; 2]>)> = state
        .panels
        .iter()
        .map(|p| (p.title.clone(), (p.visible, p.detached, p.window_pos, p.window_size)))
        .collect();
    let set_vis = |list: &mut Vec<Box<dyn Panel>>, infos: &HashMap<String, (bool, bool, Option<[f32; 2]>, Option<[f32; 2]>)>| {
        for p in list.iter_mut() {
            if let Some((vis, det, pos, sz)) = infos.get(p.title()) {
                let st = p.state_mut();
                st.visible = *vis;
                st.detached = *det;
                st.window_pos = *pos;
                st.window_size = *sz;
            }
        }
    };
    set_vis(&mut panel.left_side_panels, &panel_info);
    set_vis(&mut panel.right_side_panels, &panel_info);
    set_vis(&mut panel.bottom_panels, &panel_info);
    set_vis(&mut panel.detached_panels, &panel_info);
    set_vis(&mut panel.empty_panels, &panel_info);

    // Apply trace styles/offsets to existing traces
    {
        let traces = &mut panel.traces_data;
        for s in state.traces_style.iter() {
            if let Some(tr) = traces.get_trace_mut(&s.name) {
                tr.look = s.look.clone().into_look();
                tr.offset = s.offset;
            }
        }
    }

    // Apply math/thresholds/triggers by downcasting to specific panels
    let mut left = std::mem::take(&mut panel.left_side_panels);
    let mut right = std::mem::take(&mut panel.right_side_panels);
    let mut bottom = std::mem::take(&mut panel.bottom_panels);
    let mut detached = std::mem::take(&mut panel.detached_panels);
    let mut empty = std::mem::take(&mut panel.empty_panels);

    let apply_panels = |list: &mut Vec<Box<dyn Panel>>| {
        for p in list.iter_mut() {
            let any = p.as_mut() as &mut dyn std::any::Any;
            if let Some(mp) = any.downcast_mut::<MathPanel>() {
                mp.set_math_traces(state.math_traces.clone());
            }
            let any = p.as_mut() as &mut dyn std::any::Any;
            if let Some(tp) = any.downcast_mut::<TriggersPanel>() {
                tp.triggers.clear();
                for t in state.triggers.iter() {
                    let slope = match t.slope {
                        SerTriggerSlope::Rising => crate::data::triggers::TriggerSlope::Rising,
                        SerTriggerSlope::Falling => crate::data::triggers::TriggerSlope::Falling,
                        SerTriggerSlope::Any => crate::data::triggers::TriggerSlope::Any,
                    };
                    let mut trig = crate::data::triggers::Trigger::default();
                    trig.name = t.name.clone();
                    trig.target = TraceRef(t.target.clone());
                    trig.enabled = t.enabled;
                    trig.level = t.level;
                    trig.slope = slope;
                    trig.single_shot = t.single_shot;
                    trig.trigger_position = t.trigger_position;
                    trig.look = t.look.clone().into_look();
                    tp.triggers.insert(trig.name.clone(), trig);
                }
            }
            let any = p.as_mut() as &mut dyn std::any::Any;
            if let Some(thp) = any.downcast_mut::<ThresholdsPanel>() {
                thp.thresholds.clear();
                for d in state.thresholds.iter() {
                    let mut def = crate::data::thresholds::ThresholdDef::default();
                    def.name = d.name.clone();
                    def.target = TraceRef(d.target.clone());
                    def.kind = d.kind.clone();
                    def.min_duration_s = d.min_duration_s;
                    def.max_events = d.max_events;
                    def.look = d.look.clone().into_look();
                    def.start_look = d.start_look.clone().into_look();
                    def.stop_look = d.stop_look.clone().into_look();
                    thp.thresholds.insert(def.name.clone(), def);
                }
            }
        }
    };
    apply_panels(&mut left);
    apply_panels(&mut right);
    apply_panels(&mut bottom);
    apply_panels(&mut detached);
    apply_panels(&mut empty);

    panel.left_side_panels = left;
    panel.right_side_panels = right;
    panel.bottom_panels = bottom;
    panel.detached_panels = detached;
    panel.empty_panels = empty;

    Ok(())
}
