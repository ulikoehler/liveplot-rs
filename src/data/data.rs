//! LivePlotData: a view struct combining scope data and traces.

use crate::data::scope::ScopeData;
use crate::data::traces::{TraceData, TraceRef, TracesCollection};
use std::collections::{HashMap, VecDeque};

/// A view struct that combines scope data and traces for panel rendering.
pub struct LivePlotData<'a> {
    pub scope_data: Vec<&'a mut ScopeData>,
    pub traces: &'a mut TracesCollection,
    // Optional requests set by panel UI to trigger app-level persistence actions.
    pub request_save_state: Option<std::path::PathBuf>,
    pub request_load_state: Option<std::path::PathBuf>,
    // Scope management requests (consumed by the app after panel rendering)
    pub request_add_scope: bool,
    pub request_remove_scope: Option<usize>,
}

impl<'a> LivePlotData<'a> {
    pub fn pause_all(&mut self) {
        for scope in self.scope_data.iter_mut() {
            let scope = &mut **scope;
            scope.paused = true;
        }
        self.traces.take_snapshot();
    }

    pub fn resume_all(&mut self) {
        for scope in self.scope_data.iter_mut() {
            let scope = &mut **scope;
            scope.paused = false;
        }
        self.traces.clear_snapshot();
    }

    pub fn toggle_pause(&mut self) {
        if self.are_all_paused() {
            self.resume_all();
        } else {
            self.pause_all();
        }
    }

    pub fn pause(&mut self, scope_id: usize) {
        for scope in self.scope_data.iter_mut() {
            let scope = &mut **scope;
            if scope.id == scope_id {
                scope.paused = true;
                break;
            }
        }
        if !self.traces.has_snapshot() {
            self.traces.take_snapshot();
        }
    }

    pub fn resume(&mut self, scope_id: usize) {
        for scope in self.scope_data.iter_mut() {
            let scope = &mut **scope;
            if scope.id == scope_id {
                scope.paused = false;
                break;
            }
        }
        if self.scope_data.iter().all(|scope| !(**scope).paused) {
            self.traces.clear_snapshot();
        }
    }

    pub fn are_all_paused(&self) -> bool {
        self.scope_data.iter().all(|scope| (**scope).paused) && self.traces.has_snapshot()
    }

    pub fn get_trace_or_new(&mut self, name: &TraceRef) -> &mut TraceData {
        let is_new = !self.traces.contains_key(name);

        // Create trace if missing. Don't keep a borrow of the returned trace
        // across the mutation of scope structures (primary_scope_mut).
        let traces = if is_new {
            if let Some(primary) = self.primary_scope_mut() {
                if !primary.trace_order.iter().any(|n| n == name) {
                    primary.trace_order.push(name.clone());
                }
            }
            self.traces.get_trace_or_new(name)
        } else {
            self.traces.get_trace_or_new(name)
        };
        traces
    }

    pub fn remove_trace(&mut self, name: &TraceRef) {
        self.traces.remove_trace(name);
        for scope in self.scope_data.iter_mut() {
            let scope = &mut **scope;
            scope.trace_order.retain(|n| n != name);
        }
    }

    pub fn get_drawn_points(&self, name: &TraceRef, scope_id: usize) -> Option<VecDeque<[f64; 2]>> {
        self.scope_data.iter().find_map(|scope| {
            let scope = &**scope;
            if scope.id == scope_id {
                scope.get_drawn_points(name, &*self.traces)
            } else {
                None
            }
        })
    }

    pub fn get_all_drawn_points(&self, scope_id: usize) -> HashMap<TraceRef, VecDeque<[f64; 2]>> {
        self.scope_data
            .iter()
            .find_map(|scope| {
                let scope = &**scope;
                if scope.id == scope_id {
                    Some(scope.get_all_drawn_points(&*self.traces))
                } else {
                    None
                }
            })
            .unwrap_or_default()
    }

    #[inline]
    pub fn scope_by_id(&self, scope_id: usize) -> Option<&ScopeData> {
        self.scope_data.iter().find_map(|scope| {
            let scope = &**scope;
            if scope.id == scope_id {
                Some(scope)
            } else {
                None
            }
        })
    }

    pub fn scope_by_id_mut(&mut self, scope_id: usize) -> Option<&mut ScopeData> {
        self.scope_data.iter_mut().find_map(|scope| {
            if (**scope).id == scope_id {
                Some(&mut **scope)
            } else {
                None
            }
        })
    }

    pub fn scope_containing_trace(&self, name: &TraceRef) -> Option<&ScopeData> {
        self.scope_data.iter().find_map(|scope| {
            let scope = &**scope;
            if scope.trace_order.iter().any(|n| n == name) {
                Some(scope)
            } else {
                None
            }
        })
    }

    pub fn fit_all_bounds(&mut self) {
        for scope in self.scope_data.iter_mut() {
            let scope = &mut **scope;
            scope.fit_bounds(&*self.traces);
        }
    }

    pub fn primary_scope(&self) -> Option<&ScopeData> {
        self.scope_data.first().map(|scope| &**scope)
    }

    pub fn primary_scope_mut(&mut self) -> Option<&mut ScopeData> {
        self.scope_data.first_mut().map(|scope| &mut **scope)
    }
}
