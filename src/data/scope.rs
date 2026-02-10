//! Scope data: axis settings, display state, and coordinate management.

use crate::data::trace_look::TraceLook;
use crate::data::traces::{TraceData, TraceRef, TracesCollection};
use std::collections::{HashMap, VecDeque};

/// Formatting options for the x-value (time) shown in point labels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum XDateFormat {
    /// Local time with date, ISO8601-like: YYYY-MM-DD HH:MM:SS
    Iso8601WithDate,
    /// Local time, time-of-day only: HH:MM:SS
    Iso8601Time,
}

impl Default for XDateFormat {
    fn default() -> Self {
        XDateFormat::Iso8601Time
    }
}

impl XDateFormat {
    /// Format an `x` value (seconds since UNIX epoch as f64) according to the selected format.
    pub fn format_value(&self, x_seconds: f64) -> String {
        let secs = x_seconds as i64;
        let nsecs = ((x_seconds - secs as f64) * 1e9) as u32;
        let dt_utc = chrono::DateTime::from_timestamp(secs, nsecs)
            .unwrap_or_else(|| chrono::DateTime::from_timestamp(0, 0).unwrap());
        match self {
            XDateFormat::Iso8601WithDate => dt_utc
                .with_timezone(&chrono::Local)
                .format("%Y-%m-%d %H:%M:%S")
                .to_string(),
            XDateFormat::Iso8601Time => dt_utc
                .with_timezone(&chrono::Local)
                .format("%H:%M:%S")
                .to_string(),
        }
    }
}

/// Axis type enum: Time or Value (Value holds optional unit).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AxisType {
    Time(XDateFormat),
    Value(Option<String>),
}

/// Settings for a single axis (X or Y).
#[derive(Clone, Debug)]
pub struct AxisSettings {
    pub log_scale: bool,
    pub name: Option<String>,
    pub bounds: (f64, f64),
    pub auto_fit: bool,
    pub axis_type: AxisType,
}

impl Default for AxisSettings {
    fn default() -> Self {
        Self {
            log_scale: false,
            name: None,
            bounds: (0.0, 1.0),
            auto_fit: false,
            axis_type: AxisType::Value(None),
        }
    }
}

impl AxisSettings {
    pub fn new_time_axis() -> Self {
        Self {
            name: Some("Time".to_string()),
            axis_type: AxisType::Time(XDateFormat::default()),
            ..Default::default()
        }
    }

    /// Get the unit for this axis. Returns "s" for time axes and the configured unit for value axes.
    pub fn get_unit(&self) -> Option<String> {
        match &self.axis_type {
            AxisType::Time(_) => Some("s".to_string()),
            AxisType::Value(unit) => unit.clone(),
        }
    }

    /// Set the unit for value axes. For time axes this is a no-op.
    pub fn set_unit(&mut self, unit: Option<String>) {
        match &mut self.axis_type {
            AxisType::Time(_) => {}
            AxisType::Value(existing) => *existing = unit,
        }
    }

    /// Format a numeric value with unit, using scientific notation when appropriate.
    fn format_value_numeric(&self, v: f64, dec_pl: usize, step: f64) -> String {
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

        if let Some(unit) = self.get_unit() {
            format!("{} {}", formatted, unit)
        } else {
            formatted
        }
    }

    fn format_time_with_precision(fmt: XDateFormat, v: f64, step: f64) -> String {
        // Choose base format (date vs time-of-day) using the same threshold as before
        let use_date = step.is_finite() && step >= 86400.0;

        // Compute total nanoseconds rounded to nearest ns to handle fractional seconds correctly
        let total_ns = (v * 1e9).round() as i128;
        let secs = (total_ns / 1_000_000_000) as i64;
        let nsecs = (total_ns % 1_000_000_000) as u32;

        let dt_utc = chrono::DateTime::from_timestamp(secs, nsecs)
            .unwrap_or_else(|| chrono::DateTime::from_timestamp(0, 0).unwrap());
        let local = dt_utc.with_timezone(&chrono::Local);

        // Decide fractional precision based on sampling step (bounds). Show more precision for
        // smaller steps: seconds, ms, us, ns.
        let frac_digits = if !step.is_finite() || step >= 1.0 {
            0
        } else if step >= 1e-3 {
            3
        } else if step >= 1e-6 {
            6
        } else {
            9
        };

        // Base formatting
        let base = if use_date {
            // include date portion
            local.format("%Y-%m-%d %H:%M:%S").to_string()
        } else {
            match fmt {
                XDateFormat::Iso8601WithDate => local.format("%Y-%m-%d %H:%M:%S").to_string(),
                XDateFormat::Iso8601Time => local.format("%H:%M:%S").to_string(),
            }
        };

        if frac_digits == 0 {
            base
        } else {
            // Extract fractional digits from nsecs rounded above
            let frac_value = match frac_digits {
                3 => (nsecs / 1_000_000) as u32,
                6 => (nsecs / 1_000) as u32,
                9 => nsecs,
                _ => 0,
            };
            format!("{}.{:0width$}", base, frac_value, width = frac_digits)
        }
    }

    /// Format a value, with special handling for time axes.
    pub fn format_value(&self, v: f64, dec_pl: usize, step: f64) -> String {
        match self.axis_type {
            AxisType::Time(fmt) => Self::format_time_with_precision(fmt, v, step),
            AxisType::Value(_) => self.format_value_numeric(v, dec_pl, step),
        }
    }
}

/// Scope type: time-based or XY mode.
#[derive(PartialEq, Eq, Clone, Copy, Debug)]
pub enum ScopeType {
    TimeScope,
    XYScope,
}

/// Central state for the scope display.
pub struct ScopeData {
    pub id: usize,
    pub name: String,
    pub y_axis: AxisSettings,
    pub x_axis: AxisSettings,
    pub time_window: f64,
    pub scope_type: ScopeType,
    pub xy_pairs: Vec<(Option<TraceRef>, Option<TraceRef>, TraceLook)>,
    pub paused: bool,
    pub show_legend: bool,
    pub show_info_in_legend: bool,

    pub trace_order: Vec<TraceRef>,
    pub clicked_point: Option<[f64; 2]>,
}

impl Default for ScopeData {
    fn default() -> Self {
        Self {
            id: 0,
            name: "Scope".to_string(),
            y_axis: AxisSettings::default(),
            x_axis: AxisSettings::new_time_axis(),
            time_window: 10.0,
            scope_type: ScopeType::TimeScope,
            xy_pairs: Vec::new(),
            paused: false,
            show_legend: true,
            show_info_in_legend: false,
            trace_order: Vec::new(),
            clicked_point: None,
        }
    }
}

impl ScopeData {
    pub fn remove_trace(&mut self, trace: &TraceRef) {
        self.trace_order.retain(|t| t != trace);
        for (x, y, _look) in self.xy_pairs.iter_mut() {
            if x.as_ref() == Some(trace) {
                *x = None;
            }
            if y.as_ref() == Some(trace) {
                *y = None;
            }
        }
        self.xy_pairs.retain(|(x, y, _)| x.is_some() || y.is_some());
    }

    pub fn update(&mut self, traces: &TracesCollection) {
        // Keep trace_order in sync with current traces: drop missing, append new
        self.trace_order.retain(|n| traces.contains_key(n));

        // Keep XY pairs in sync with current traces.
        // Incomplete pairs (None) are allowed, but are not rendered/used until complete.
        self.xy_pairs.retain(|(x, y, _)| {
            let x_ok = x.as_ref().is_none_or(|t| traces.contains_key(t));
            let y_ok = y.as_ref().is_none_or(|t| traces.contains_key(t));
            x_ok && y_ok && !(x.is_none() && y.is_none())
        });

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
                // Use only traces assigned to this scope to determine the current time
                let now = self
                    .trace_order
                    .iter()
                    .filter_map(|name| traces.get_trace(name))
                    .filter_map(|trace| trace.live.back().map(|last| last[0]))
                    .fold(None, |acc: Option<f64>, val| {
                        Some(acc.map_or(val, |a: f64| a.max(val)))
                    })
                    .unwrap_or(self.time_window);
                let time_lower = now - self.time_window;
                self.x_axis.bounds = (time_lower, now);
            } else {
                let diff = ((self.x_axis.bounds.1 - self.x_axis.bounds.0) - self.time_window) / 2.0;
                self.x_axis.bounds = (self.x_axis.bounds.0 + diff, self.x_axis.bounds.1 - diff);
            }
        }
    }

    pub fn fit_x_bounds(&mut self, traces: &TracesCollection) {
        if self.scope_type == ScopeType::XYScope && !self.xy_pairs.is_empty() {
            let mut min_x = f64::MAX;
            let mut max_x = f64::MIN;
            let tol = 1e-9_f64;

            for (x_name, y_name, _pair_look) in self.xy_pairs.iter() {
                let (Some(x_name), Some(y_name)) = (x_name.as_ref(), y_name.as_ref()) else {
                    continue;
                };

                let (Some(x_tr), Some(y_tr)) = (traces.get_trace(x_name), traces.get_trace(y_name))
                else {
                    continue;
                };
                if !x_tr.look.visible || !y_tr.look.visible {
                    continue;
                }

                let x_pts = traces.get_points(x_name, self.paused);
                let y_pts = traces.get_points(y_name, self.paused);
                let (Some(x_pts), Some(y_pts)) = (x_pts, y_pts) else {
                    continue;
                };

                let mut i = 0usize;
                let mut j = 0usize;
                while i < x_pts.len() && j < y_pts.len() {
                    let tx = x_pts[i][0];
                    let ty = y_pts[j][0];
                    let dt = tx - ty;
                    if dt.abs() <= tol {
                        let x = x_pts[i][1] + x_tr.offset;
                        if x < min_x {
                            min_x = x;
                        }
                        if x > max_x {
                            max_x = x;
                        }
                        i += 1;
                        j += 1;
                    } else if dt < 0.0 {
                        i += 1;
                    } else {
                        j += 1;
                    }
                }
            }

            if min_x < max_x {
                self.x_axis.bounds = (min_x, max_x);
                self.time_window = max_x - min_x;
            }
            return;
        }

        let mut min_x = f64::MAX;
        let mut max_x = f64::MIN;
        for name in self.trace_order.iter() {
            let Some(trace) = traces.get_trace(name) else {
                continue;
            };
            if !trace.look.visible {
                continue;
            }
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
        } else if min_x == min_x {
            if min_x < 0.0 {
                self.y_axis.bounds = (min_x, 0.0);
                self.time_window = -min_x;
            } else if min_x > 0.0 {
                self.y_axis.bounds = (0.0, min_x);
                self.time_window = min_x;
            } else {
                // Both min and max are zero; set to -1.0 to 1.0
                self.y_axis.bounds = (-1.0, 1.0);
                self.time_window = 2.0;
            }
        }
    }

    pub fn fit_y_bounds(&mut self, traces: &TracesCollection) {
        if self.scope_type == ScopeType::XYScope && !self.xy_pairs.is_empty() {
            let mut min_y = f64::MAX;
            let mut max_y = f64::MIN;
            let tol = 1e-9_f64;

            for (x_name, y_name, _pair_look) in self.xy_pairs.iter() {
                let (Some(x_name), Some(y_name)) = (x_name.as_ref(), y_name.as_ref()) else {
                    continue;
                };

                let (Some(x_tr), Some(y_tr)) = (traces.get_trace(x_name), traces.get_trace(y_name))
                else {
                    continue;
                };
                if !x_tr.look.visible || !y_tr.look.visible {
                    continue;
                }

                let x_pts = traces.get_points(x_name, self.paused);
                let y_pts = traces.get_points(y_name, self.paused);
                let (Some(x_pts), Some(y_pts)) = (x_pts, y_pts) else {
                    continue;
                };

                let mut i = 0usize;
                let mut j = 0usize;
                while i < x_pts.len() && j < y_pts.len() {
                    let tx = x_pts[i][0];
                    let ty = y_pts[j][0];
                    let dt = tx - ty;
                    if dt.abs() <= tol {
                        let y = y_pts[j][1] + y_tr.offset;
                        if y < min_y {
                            min_y = y;
                        }
                        if y > max_y {
                            max_y = y;
                        }
                        i += 1;
                        j += 1;
                    } else if dt < 0.0 {
                        i += 1;
                    } else {
                        j += 1;
                    }
                }
            }

            if min_y < max_y {
                self.y_axis.bounds = (min_y, max_y);
            }
            return;
        }

        let mut min_y = f64::MAX;
        let mut max_y = f64::MIN;
        let x_bounds = self.x_axis.bounds;
        for name in self.trace_order.iter() {
            let Some(trace) = traces.get_trace(name) else {
                continue;
            };
            if !trace.look.visible {
                continue;
            }
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
        } else if min_y == max_y {
            if min_y < 0.0 {
                self.y_axis.bounds = (min_y, 0.0);
            } else if min_y > 0.0 {
                self.y_axis.bounds = (0.0, max_y);
            } else {
                // Both min and max are zero; set to -1.0 to 1.0
                self.y_axis.bounds = (-1.0, 1.0);
            }
        }
    }

    pub fn fit_bounds(&mut self, traces: &TracesCollection) {
        self.fit_x_bounds(traces);
        self.fit_y_bounds(traces);
    }

    pub fn get_drawn_points(
        &self,
        name: &TraceRef,
        traces: &TracesCollection,
    ) -> Option<VecDeque<[f64; 2]>> {
        if let Some(trace) = traces.get_points(name, self.paused) {
            if self.scope_type == ScopeType::XYScope {
                Some(trace)
            } else {
                Some(TraceData::cap_by_x_bounds(&trace, self.x_axis.bounds))
            }
        } else {
            None
        }
    }

    pub fn get_all_drawn_points(
        &self,
        traces: &TracesCollection,
    ) -> HashMap<TraceRef, VecDeque<[f64; 2]>> {
        let mut result = HashMap::new();
        for name in self.trace_order.iter() {
            if let Some(pts) = self.get_drawn_points(name, traces) {
                result.insert(name.clone(), pts);
            }
        }
        result
    }
}
