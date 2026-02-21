//! Trigger system for oscilloscope-style edge detection and single-shot capture.
//!
//! This module provides functionality to trigger data capture on signal crossings,
//! similar to how a hardware oscilloscope triggers on voltage thresholds.

use crate::data::data::LivePlotData;
use crate::data::scope::AxisSettings;
use crate::data::trace_look::TraceLook;
use crate::data::traces::TraceRef;
use egui_plot::LineStyle;

/// A trigger configuration for oscilloscope-style capture.
///
/// Triggers detect when a signal crosses a specified level in a given direction
/// (rising, falling, or any). They can operate in single-shot mode (capture once
/// then stop) or auto mode (continuously re-arm after each trigger).
pub struct Trigger {
    /// User-facing name for this trigger
    pub name: String,
    /// The trace to monitor for trigger crossings
    pub target: TraceRef,
    /// Whether this trigger is enabled
    pub enabled: bool,
    /// The threshold level to detect crossings at
    pub level: f64,
    /// Which edge direction(s) to trigger on
    pub slope: TriggerSlope,
    /// If true, trigger once and stop; if false, continuously re-arm
    pub single_shot: bool,
    /// Position in buffer where trigger should appear (0.0 = left edge, 1.0 = right edge)
    pub trigger_position: f64,
    /// Visual appearance for the trigger level line
    pub look: TraceLook,

    /// Holdoff time in seconds. After a trigger fires, the next trigger
    /// will not fire until at least this much time has passed. Default: `0.0`.
    pub holdoff_secs: f64,

    // Runtime state (not serialized)
    start_trigger: bool,
    last_triggered: Option<f64>,
    trigger_pending: Option<f64>,
}

impl Default for Trigger {
    fn default() -> Self {
        Self {
            name: String::new(),
            target: TraceRef(String::new()),
            enabled: true,
            level: 0.0,
            slope: TriggerSlope::Rising,
            single_shot: true,
            trigger_position: 0.5,
            look: TraceLook {
                style: LineStyle::Dotted { spacing: 4.0 },
                ..TraceLook::default()
            },
            holdoff_secs: 0.0,
            start_trigger: false,
            last_triggered: None,
            trigger_pending: None,
        }
    }
}

/// Direction of signal change that triggers capture.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TriggerSlope {
    /// Trigger on rising edge (signal crosses level going up)
    Rising,
    /// Trigger on falling edge (signal crosses level going down)
    Falling,
    /// Trigger on any edge (rising or falling)
    Any,
}

impl Trigger {
    /// Reset the trigger state, clearing any pending or completed trigger.
    pub fn reset(&mut self) {
        self.last_triggered = None;
        self.trigger_pending = None;
    }

    /// Full reset including the start flag.
    pub fn reset_runtime_state(&mut self) {
        self.last_triggered = None;
        self.trigger_pending = None;
        self.start_trigger = false;
    }

    /// Arm the trigger to start looking for crossings.
    pub fn start(&mut self) {
        self.last_triggered = None;
        self.trigger_pending = None;
        if self.enabled {
            self.start_trigger = true;
        }
    }

    /// Stop the trigger from looking for crossings.
    pub fn stop(&mut self) {
        self.start_trigger = false;
        self.trigger_pending = None;
    }

    /// Short, user-facing description used in UI and legend labels.
    /// Example: "trace1: rising @ 1.23 V, pos 0.50, single"
    pub fn get_info(&self, axis: &AxisSettings) -> String {
        let slope_txt = match self.slope {
            TriggerSlope::Rising => "rising",
            TriggerSlope::Falling => "falling",
            TriggerSlope::Any => "any",
        };
        let dec_pl = 4usize;
        let step = if self.level.abs() > 0.0 {
            self.level.abs()
        } else {
            1.0
        };
        let lvl_fmt = axis.format_value(self.level, dec_pl, step);
        let mut s = format!("{}: {} @ {}", self.target.0, slope_txt, lvl_fmt);
        if self.single_shot {
            s.push_str(" • Single");
        } else {
            s.push_str(" • Auto");
        }
        s
    }

    /// Last trigger timestamp (seconds) if a crossing was detected since reset.
    pub fn last_trigger_time(&self) -> Option<f64> {
        self.last_triggered
    }

    /// Returns true if a trigger has fired and is enabled.
    pub fn is_triggered(&self) -> bool {
        self.last_triggered.is_some() && self.enabled
    }

    /// Returns true if the trigger is armed and actively looking for crossings.
    pub fn is_active(&self) -> bool {
        self.enabled && self.start_trigger && (self.trigger_pending.is_none() || !self.single_shot)
    }

    /// Returns true if a trigger crossing was detected but we're waiting for more samples.
    pub fn is_trigger_pending(&self) -> bool {
        self.trigger_pending.is_some()
    }

    /// Check the target trace for a trigger crossing and optionally pause after a configurable
    /// number of subsequent samples. `trigger_position` is in [0,1]:
    /// - 0.0 => pause immediately when the trigger occurs
    /// - 1.0 => pause after `data.max_points` new samples on the target trace
    /// Values in between scale linearly.
    ///
    /// Returns `true` if there's an active trigger pending or just fired.
    pub fn check_trigger(&mut self, data: &mut LivePlotData<'_>) -> bool {
        let livedata = if let Some(data) = data.traces.get_points(&self.target, false) {
            data
        } else {
            self.enabled = false;
            if let Some(first) = data.traces.all_trace_names().first() {
                self.target = first.clone();
            }
            return false;
        };

        if !self.enabled {
            self.start_trigger = false;
            return false;
        }
        if !self.start_trigger && self.trigger_pending.is_none() {
            return false;
        }

        // Step 1: detect a new trigger crossing and compute a new trigger time (if any)
        if self.start_trigger && self.trigger_pending.is_none() {
            let new_trigger_time: Option<f64> = {
                let len = livedata.len();
                if len < 2 {
                    None
                } else {
                    // Start detection at the index determined by trigger_position within the last max_points window
                    let window_start = len.saturating_sub(data.traces.max_points);
                    let pos = self.trigger_position.clamp(0.0, 1.0);
                    let offset = (pos * (data.traces.max_points as f64)).round() as usize;
                    let mut i0 = window_start.saturating_add(offset);
                    if i0 >= len {
                        i0 = len - 1;
                    }
                    if i0 < 1 {
                        i0 = 1;
                    }

                    let mut found: Option<f64> = None;
                    for i in i0..len {
                        let p0 = livedata.get(i - 1).unwrap();
                        let p1 = livedata.get(i).unwrap();
                        let (v0, v1) = (p0[1], p1[1]);
                        let t1 = p1[0];
                        let crossed = match self.slope {
                            TriggerSlope::Rising => v0 < self.level && v1 >= self.level,
                            TriggerSlope::Falling => v0 > self.level && v1 <= self.level,
                            TriggerSlope::Any => {
                                (v0 < self.level && v1 >= self.level)
                                    || (v0 > self.level && v1 <= self.level)
                            }
                        };
                        if crossed {
                            // Holdoff: skip crossings too close to the previous trigger
                            if let Some(last_t) = self.last_triggered {
                                let holdoff = if self.holdoff_secs > 0.0 {
                                    self.holdoff_secs
                                } else {
                                    f64::EPSILON
                                };
                                if (t1 - last_t) < holdoff {
                                    continue;
                                }
                            }
                            found = Some(t1);
                            break;
                        }
                    }
                    found
                }
            };

            // Step 2: apply effects of a new trigger (update last_triggered; maybe immediate pause)
            if let Some(_t_trig) = new_trigger_time {
                self.trigger_pending = new_trigger_time;
                if self.single_shot {
                    self.start_trigger = false;
                }
            }
        }

        if let Some(t_trig) = self.trigger_pending {
            let pos = self.trigger_position.clamp(0.0, 1.0);
            let needed: usize = (data.traces.max_points as f64 * pos).round() as usize;

            if needed == 0 {
                data.pause_all();
                self.last_triggered = self.trigger_pending;
                self.trigger_pending = None;
            } else {
                // Short immutable borrow to count samples after the trigger
                let have = livedata.iter().filter(|p| p[0] > t_trig).count();

                if have >= needed {
                    data.pause_all();
                    self.last_triggered = self.trigger_pending;
                    self.trigger_pending = None;
                }
            }
            return true;
        }

        false
    }
}
