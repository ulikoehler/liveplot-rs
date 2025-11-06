use crate::data::scope::{ScopeData, ScopeType};
use std::collections::{HashMap, VecDeque};
use crate::data::traces::{TraceData, TraceRef, TracesCollection};

pub struct LivePlotData {
    pub scope_data: ScopeData,
    pub traces: TracesCollection,
}

impl LivePlotData {
    pub fn pause(&mut self) {
        self.scope_data.paused = true;
        self.traces.take_snapshot();
    }

    pub fn resume(&mut self) {
        self.scope_data.paused = false;
    }

    pub fn is_paused(&self) -> bool {
        self.scope_data.paused && self.traces.has_snapshot()
    }

    pub fn get_trace_or_new(&mut self, name: &TraceRef) -> &mut TraceData {
        if !self.scope_data.trace_order.iter().any(|n| n == name) {
            self.scope_data.trace_order.push(name.clone());
        }
        self.traces.get_trace_or_new(name)
    }

    pub fn remove_trace(&mut self, name: &TraceRef) {
        self.traces.remove_trace(name);
        self.scope_data.trace_order.retain(|n| n != name);
    }

    pub fn get_drawn_points(&self, name: &TraceRef) -> Option<VecDeque<[f64; 2]>> {
        if let Some(trace) = self.traces.get_points(name, self.scope_data.paused) {
            if self.scope_data.scope_type == ScopeType::XYScope {
                Some(trace.clone())
            } else {
                Some(TraceData::cap_by_x_bounds(&trace, self.scope_data.x_axis.bounds))
            }
        } else {
            None
        }
    }

    pub fn get_all_drawn_points(&self) -> HashMap<TraceRef, VecDeque<[f64; 2]>> {
        let mut result = HashMap::new();
        for name in self.scope_data.trace_order.iter() {
            if let Some(pts) = self.get_drawn_points(name) {
                result.insert(name.clone(), pts);
            }
        }
        result
    }
}
