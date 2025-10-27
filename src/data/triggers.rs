

use crate::data::traces::TraceRef;
use crate::data::trace_look::TraceLook;
use crate::data::scope::{AxisSettings, ScopeData};


pub struct Trigger {
    pub name: String,
    pub target: TraceRef,
    pub enabled: bool,
    pub level: f64,
    pub slope: TriggerSlope,
    pub single_shot: bool,
    pub trigger_position: f64,
    pub look: TraceLook,

    last_triggered: Option<f64>,
}

impl Default for Trigger {
    fn default() -> Self {
        Self {
            name: "Trigger".to_string(),
            target: TraceRef("".to_string()),
            enabled: false,
            level: 0.0,
            slope: TriggerSlope::Rising,
            single_shot: false,
            trigger_position: 0.5,
            look: TraceLook::default(),
            last_triggered: None,
        }
    }
}

pub enum TriggerSlope {
    Rising,
    Falling,
    Any,
}

impl Trigger{
    pub fn reset(&mut self) {
        self.last_triggered = None;
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
        let step = if self.level.abs() > 0.0 { self.level.abs() } else { 1.0 };
        let lvl_fmt = axis.format_value(self.level, dec_pl, step);
        let mut s = format!("{}: {} @ {}", self.target.0, slope_txt, lvl_fmt);
        s.push_str(&format!(", pos {:.2}", self.trigger_position));
        if self.single_shot {
            s.push_str(", single");
        }
        s
    }

    /// Last trigger timestamp (seconds) if a crossing was detected since reset.
    pub fn last_trigger_time(&self) -> Option<f64> { self.last_triggered }

    /// Check the target trace for a trigger crossing and optionally pause after a configurable
    /// number of subsequent samples. `trigger_position` is in [0,1]:
    /// - 0.0 => pause immediately when the trigger occurs
    /// - 1.0 => pause after `data.max_points` new samples on the target trace
    /// Values in between scale linearly.
    pub fn check_trigger(&mut self, data: &mut ScopeData) {
        if !self.enabled {
            return;
        }

        let target_name = &self.target.0;

        // Step 1: detect a new trigger crossing and compute a new trigger time (if any)
        let new_trigger_time: Option<f64> = {
            if let Some(trace) = data.traces.get(target_name) {
                let live = &trace.live;
                if live.len() < 2 { None } else {
                    let mut found: Option<f64> = None;
                    for i in 1..live.len() {
                        let p0 = live.get(i - 1).unwrap();
                        let p1 = live.get(i).unwrap();
                        let (v0, v1) = (p0[1], p1[1]);
                        let t1 = p1[0];
                        let crossed = match self.slope {
                            TriggerSlope::Rising => v0 < self.level && v1 >= self.level,
                            TriggerSlope::Falling => v0 > self.level && v1 <= self.level,
                            TriggerSlope::Any => (v0 < self.level && v1 >= self.level)
                                || (v0 > self.level && v1 <= self.level),
                        };
                        if crossed {
                            if let Some(last_t) = self.last_triggered {
                                if (t1 - last_t).abs() < std::f64::EPSILON { continue; }
                            }
                            found = Some(t1);
                            break;
                        }
                    }
                    found
                }
            } else { None }
        };

        // Step 2: apply effects of a new trigger (update last_triggered; maybe immediate pause)
        let mut should_pause = false;
        if let Some(t_trig) = new_trigger_time {
            self.last_triggered = Some(t_trig);
            if self.single_shot { self.enabled = false; }
            if self.trigger_position <= 0.0 { should_pause = true; }
        }

        // Step 3: if we already have a trigger, count samples after it and decide whether to pause
        if !should_pause {
            if let Some(t_trig) = self.last_triggered {
                let pos = self.trigger_position.clamp(0.0, 1.0);
                let needed: usize = (data.max_points as f64 * pos).round() as usize;
                if needed == 0 {
                    should_pause = true;
                } else {
                    // Short immutable borrow to count samples after the trigger
                    let have: usize = if let Some(trace) = data.traces.get(target_name) {
                        trace.live.iter().filter(|p| p[0] > t_trig).count()
                    } else { 0 };
                    if have >= needed { should_pause = true; }
                }
            }
        }

        // Step 4: perform pause if needed (no active borrows of `data` at this point)
        if should_pause {
            data.pause();
            self.last_triggered = None;
        }
    }
}
