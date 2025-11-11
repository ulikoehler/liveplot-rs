use crate::data::scope::{ScopeData};
use std::collections::{HashMap, VecDeque};
use crate::data::traces::{TraceData, TraceRef, TracesCollection};

pub struct LivePlotData<'a> {
    pub scope_data: &'a mut ScopeData,
    pub traces: &'a mut TracesCollection,
}

impl<'a> LivePlotData<'a> {

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
        // Delegate to ScopeData helper to ensure consistent bounds handling
        self.scope_data.get_drawn_points(name, &*self.traces)
    }

    pub fn get_all_drawn_points(&self) -> HashMap<TraceRef, VecDeque<[f64; 2]>> {
        self.scope_data.get_all_drawn_points(&*self.traces)
    }
}
