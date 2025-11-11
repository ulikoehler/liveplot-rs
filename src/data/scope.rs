use std::collections::{HashMap, VecDeque};
use crate::data::traces::{TraceData, TraceRef, TracesCollection};

pub struct AxisSettings {
    pub unit: Option<String>,
    pub log_scale: bool,
    pub format: Option<String>,
    pub name: Option<String>,
    pub bounds: (f64, f64),
    pub auto_fit: bool,
}

impl Default for AxisSettings {
    fn default() -> Self {
        Self {
            unit: None,
            log_scale: false,
            format: None,
            name: None,
            bounds: (0.0, 1.0),
            auto_fit: false,
        }
    }
}

impl AxisSettings {
    pub fn format_value_with_unit(&self, v: f64, dec_pl: usize, step: f64) -> String {
        // Decide scientific formatting based on step magnitude vs precision:
        // - Use scientific if step < 10^-dec_pl (too fine to show with dec_pl)
        // - Or if step >= 10^dec_pl (too large; many digits before decimal)
        let sci = if step.is_finite() && step != 0.0 {
            let exp = step.abs().log10().floor() as i32;
            exp < -(dec_pl as i32) || exp >= dec_pl as i32
        } else {
            false
        };

        let formatted = if sci {
            if v == 0.0 || !v.is_finite() {
                // Just show the value as-is with requested precision if zero/NaN/inf
                format!("{:.*}", dec_pl, v)
            } else {
                // Create a compact scientific representation like 1.23e5 (no +00 padding)
                let sign = if v.is_sign_negative() { -1.0 } else { 1.0 };
                let av = v.abs();
                let exp = av.log10().floor() as i32;
                let pow = 10f64.powi(exp);
                let mant = sign * (av / pow);
                if exp == 0 {
                    format!("{:.*}", dec_pl, mant)
                } else {
                    format!("{:.*}e{}", dec_pl, mant, exp)
                }
            }
        } else {
            format!("{:.*}", dec_pl, v)
        };

        if let Some(unit) = &self.unit {
            format!("{} {}", formatted, unit)
        } else {
            formatted
        }
    }

    pub fn format_value(&self, v: f64, dec_pl: usize, step: f64) -> String {
        // If a format string is provided, interpret it as a chrono DateTime format for
        // Unix timestamp seconds (used for time axes) and ignore dec_pl/sci.
        if let Some(fmt) = &self.format {
            let secs = v.floor() as i64;
            let nsecs = ((v - secs as f64) * 1e9) as u32;
            let dt_utc = chrono::DateTime::from_timestamp(secs, nsecs)
                .unwrap_or_else(|| chrono::DateTime::from_timestamp(0, 0).unwrap());
            return dt_utc
                .with_timezone(&chrono::Local)
                .format(fmt.as_str())
                .to_string();
        }

        return self.format_value_with_unit(v, dec_pl, step);
    }
}

#[derive(PartialEq, Eq)]
pub enum ScopeType {
    TimeScope,
    XYScope,
}

pub struct ScopeData {
    // Y Settings
    pub y_axis: AxisSettings,
    pub x_axis: AxisSettings,
    //pub max_points: usize,
    pub time_window: f64,
    pub scope_type: ScopeType,
    pub paused: bool,
    pub show_legend: bool,
    pub show_info_in_legend: bool,

    //pub traces: HashMap<String, TraceData>,
    pub trace_order: Vec<TraceRef>,
    pub hover_trace: Option<TraceRef>,
    pub selection_trace: Option<TraceRef>,
    pub clicked_point: Option<[f64; 2]>,
}

impl Default for ScopeData {
    fn default() -> Self {
        let mut x_axis = AxisSettings::default();
        x_axis.name = Some("Time".to_string());
        x_axis.format = Some("%H:%M:%S".to_string());
        x_axis.unit = Some("s".to_string());
        Self {
            y_axis: AxisSettings::default(),
            x_axis,
            //max_points: 10_000,
            time_window: 10.0,
            scope_type: ScopeType::TimeScope,
            paused: false,
            show_legend: true,
            show_info_in_legend: false,
            // rx: None,
            //traces: HashMap::new(),
            trace_order: Vec::new(),
            hover_trace: None,
            selection_trace: None,
            clicked_point: None,
        }
    }
}

impl ScopeData {
    pub fn update(&mut self, traces: &TracesCollection) {
        // Keep trace_order in sync with current traces: drop missing, append new
        self.trace_order.retain(|n| traces.contains_key(n));
        for name in traces.keys() {
            if !self.trace_order.iter().any(|n| n == name) {
                self.trace_order.push(name.clone());
            }
        }

        if self.x_axis.auto_fit {
            self.fit_x_bounds(traces);
        }

        self.live_update(traces);

        if self.y_axis.auto_fit {
            self.fit_y_bounds(traces);
        }
    }

    fn live_update(&mut self, traces: &TracesCollection) {
        if self.scope_type == ScopeType::TimeScope {
            if !self.paused {
                let now = if let Some((_name, trace)) = traces.traces_iter().next() {
                    if let Some(last) = trace.live.back() {
                        last[0]
                    } else {
                        self.time_window
                    }
                } else {
                    self.time_window
                };
                let time_lower = now - self.time_window;
                self.x_axis.bounds = (time_lower, now);
            } else {
                let diff = ((self.x_axis.bounds.1 - self.x_axis.bounds.0) - self.time_window) / 2.0;
                self.x_axis.bounds = (self.x_axis.bounds.0 + diff, self.x_axis.bounds.1 - diff);
            }
        }
    }

    pub fn fit_x_bounds(&mut self, traces: &TracesCollection) {
        let mut min_x = f64::MAX;
        let mut max_x = f64::MIN;
        for (_name, trace) in traces.traces_iter() {
            let points = if self.paused {
                if let Some(snap) = &trace.snap {
                    snap
                } else {
                    &trace.live
                }
            } else {
                &trace.live
            };
            for p in points.iter() {
                if p[0] < min_x {
                    min_x = p[0];
                }
                if p[0] > max_x {
                    max_x = p[0];
                }
            }
        }
        if min_x < max_x {
            self.x_axis.bounds = (min_x, max_x);
            self.time_window = max_x - min_x;
        }
    }

    pub fn fit_y_bounds(&mut self, traces: &TracesCollection) {
        let mut min_y = f64::MAX;
        let mut max_y = f64::MIN;
        let x_bounds = self.x_axis.bounds;
        for (_name, trace) in traces.traces_iter() {
            let points = if self.paused {
                if let Some(snap) = &trace.snap {
                    snap
                } else {
                    &trace.live
                }
            } else {
                &trace.live
            };
            for p in points.iter() {
                if p[0] < x_bounds.0 {
                    continue;
                }
                if p[0] > x_bounds.1 {
                    break;
                }
                if p[1] + trace.offset < min_y {
                    min_y = p[1] + trace.offset;
                }
                if p[1] + trace.offset > max_y {
                    max_y = p[1] + trace.offset;
                }
            }
        }
        if min_y < max_y {
            self.y_axis.bounds = (min_y, max_y);
        }
    }
    
    pub fn fit_bounds(&mut self, traces: &TracesCollection) {
        self.fit_x_bounds(traces);
        self.fit_y_bounds(traces);
    }

    pub fn get_drawn_points(&self, name: &TraceRef, traces: &TracesCollection) -> Option<VecDeque<[f64; 2]>> {
        if let Some(trace) = traces.get_points(name, self.paused) {
            if self.scope_type == ScopeType::XYScope {
                Some(trace.clone())
            } else {
                Some(TraceData::cap_by_x_bounds(&trace, self.x_axis.bounds))
            }
        } else {
            None
        }
    }

    pub fn get_all_drawn_points(&self, traces: &TracesCollection) -> HashMap<TraceRef, VecDeque<[f64; 2]>> {
        let mut result = HashMap::new();
        for name in self.trace_order.iter() {
            if let Some(pts) = self.get_drawn_points(name, traces) {
                result.insert(name.clone(), pts);
            }
        }
        result
    }
    
}
