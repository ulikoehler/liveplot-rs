use std::collections::{HashMap, VecDeque};

use crate::data::trace_look::TraceLook;
use crate::data::traces::TraceData;
use crate::sink::MultiSample;

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

#[derive(PartialEq, Eq)]
pub enum ScopeType {
    TimeScope,
    XYScope,
}

pub struct ScopeData {
    // Y Settings
    pub y_axis: AxisSettings,
    pub x_axis: AxisSettings,
    pub max_points: usize,
    pub time_window: f64,
    pub scope_type: ScopeType,
    pub paused: bool,
    pub show_legend: bool,
    pub show_info_in_legend: bool,
    pub rx: Option<std::sync::mpsc::Receiver<MultiSample>>,
    pub traces: HashMap<String, TraceData>,
    pub trace_order: Vec<String>,
    pub hover_trace: Option<String>,
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
            max_points: 10_000,
            time_window: 10.0,
            scope_type: ScopeType::TimeScope,
            paused: false,
            show_legend: true,
            show_info_in_legend: false,
            rx: None,
            traces: HashMap::new(),
            trace_order: Vec::new(),
            hover_trace: None,
        }
    }
}

impl ScopeData {
    pub fn set_rx(&mut self, rx: std::sync::mpsc::Receiver<MultiSample>) {
        self.rx = Some(rx);
    }

    fn update_rx(&mut self) {
        if let Some(rx) = &self.rx {
            while let Ok(s) = rx.try_recv() {
                let entry = self.traces.entry(s.trace.clone()).or_insert_with(|| {
                    self.trace_order.push(s.trace.clone());
                    TraceData {
                        name: s.trace.clone(),
                        look: TraceLook::new(self.trace_order.len() - 1),
                        offset: 0.0,
                        live: VecDeque::new(),
                        snap: None,
                        info: String::new(),
                    }
                });
                let t = s.timestamp_micros as f64 * 1e-6;
                entry.live.push_back([t, s.value]);
                if entry.live.len() > self.max_points {
                    entry.live.pop_front();
                }
                if let Some(inf) = s.info {
                    entry.info = inf;
                }
            }
        }
    }

    fn drain(&mut self) {
        for (_name, trace) in self.traces.iter_mut() {
            trace.prune_by_points(self.max_points);
        }
    }

    pub fn update(&mut self) {
        self.update_rx();
        self.drain();

        if self.x_axis.auto_fit {
            self.fit_x_bounds();
        }

        self.live_update();

        if self.y_axis.auto_fit {
            self.fit_y_bounds();
        }
    }

    fn live_update(&mut self) {
        if self.scope_type == ScopeType::TimeScope {
            if !self.paused {
                let now = if let Some((_name, trace)) = self.traces.iter().next() {
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

    pub fn fit_x_bounds(&mut self) {
        let mut min_x = f64::MAX;
        let mut max_x = f64::MIN;
        for (_name, trace) in self.traces.iter() {
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

    pub fn fit_y_bounds(&mut self) {
        let mut min_y = f64::MAX;
        let mut max_y = f64::MIN;
        let x_bounds = self.x_axis.bounds;
        for (_name, trace) in self.traces.iter() {
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
                if p[1] < min_y {
                    min_y = p[1];
                }
                if p[1] > max_y {
                    max_y = p[1];
                }
            }
        }
        if min_y < max_y {
            self.y_axis.bounds = (min_y, max_y);
        }
    }
    pub fn fit_bounds(&mut self) {
        self.fit_x_bounds();
        self.fit_y_bounds();
    }

    pub fn pause(&mut self) {
        self.paused = true;
        for (_name, trace) in self.traces.iter_mut() {
            trace.take_snapshot();
        }
    }

    pub fn resume(&mut self) {
        self.paused = false;
        for (_name, trace) in self.traces.iter_mut() {
            trace.clear_snapshot();
        }
    }

    pub fn clear_all(&mut self) {
        for (_name, trace) in self.traces.iter_mut() {
            trace.clear_all();
        }
    }
}
