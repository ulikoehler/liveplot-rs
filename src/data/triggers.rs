use crate::data::scope::{AxisSettings, ScopeData};
use crate::data::trace_look::TraceLook;
use crate::data::traces::TraceRef;
use egui_plot::LineStyle;

pub struct Trigger {
    pub name: String,
    pub target: TraceRef,
    pub enabled: bool,
    pub level: f64,
    pub slope: TriggerSlope,
    pub single_shot: bool,
    pub trigger_position: f64,
    pub look: TraceLook,

    start_trigger: bool,
    last_triggered: Option<f64>,
    trigger_pending: bool,
}

impl Default for Trigger {
    fn default() -> Self {
        Self {
            name: "".to_string(),
            target: TraceRef("".to_string()),
            enabled: true,
            level: 0.0,
            slope: TriggerSlope::Rising,
            single_shot: true,
            trigger_position: 0.5,
            look: TraceLook { style: LineStyle::Dotted { spacing: 4.0 }, ..TraceLook::default() },
            start_trigger: false,
            last_triggered: None,
            trigger_pending: false,
        }
    }
}

pub enum TriggerSlope {
    Rising,
    Falling,
    Any,
}

impl Trigger {
    pub fn reset(&mut self) {
        self.last_triggered = None;
        if self.enabled && !self.single_shot {
            self.start_trigger = true;
        }
    }

    pub fn start(&mut self) {
        self.last_triggered = None;
        if self.enabled {
            self.start_trigger = true;
        }
    }

    pub fn stop(&mut self) {
        self.start_trigger = false;
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
        //s.push_str(&format!(", pos {:.2}", self.trigger_position));
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
    pub fn is_triggered(&self) -> bool {
        self.last_triggered.is_some() && self.enabled && !self.trigger_pending
    }
    pub fn is_active(&self) -> bool {
        self.enabled && self.start_trigger && !self.trigger_pending
    }
    pub fn is_trigger_pending(&self) -> bool {
        self.trigger_pending
    }

    /// Check the target trace for a trigger crossing and optionally pause after a configurable
    /// number of subsequent samples. `trigger_position` is in [0,1]:
    /// - 0.0 => pause immediately when the trigger occurs
    /// - 1.0 => pause after `data.max_points` new samples on the target trace
    /// Values in between scale linearly.
    pub fn check_trigger(&mut self, data: &mut ScopeData) -> bool {
        if !self.enabled {
            self.start_trigger = false;
            self.last_triggered = None;
            return false;
        }
        if !self.single_shot {
            self.start_trigger = true;
        }
        if !self.start_trigger && !self.trigger_pending {
            return self.last_triggered.is_some();
        }

        let target_name = &self.target.0;

        // Step 1: detect a new trigger crossing and compute a new trigger time (if any)
        if self.start_trigger && !self.trigger_pending{
            let new_trigger_time: Option<f64> = {
                if let Some(trace) = data.traces.get(target_name) {
                    let live = &trace.live;
                    let len = live.len();
                    if len < 2 {
                        None
                    } else {
                        // Start detection at the index determined by trigger_position within the last max_points window
                        let window_start = len.saturating_sub(data.max_points);
                        let pos = self.trigger_position.clamp(0.0, 1.0);
                        let offset = (pos * (data.max_points as f64)).round() as usize;
                        let mut i0 = window_start.saturating_add(offset);
                        if i0 >= len {
                            i0 = len - 1;
                        }
                        if i0 < 1 {
                            i0 = 1;
                        }

                        let mut found: Option<f64> = None;
                        for i in i0..len {
                            let p0 = live.get(i - 1).unwrap();
                            let p1 = live.get(i).unwrap();
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
                                if let Some(last_t) = self.last_triggered {
                                    if (t1 - last_t).abs() < std::f64::EPSILON {
                                        continue;
                                    }
                                }
                                found = Some(t1);
                                break;
                            }
                        }
                        found
                    }
                } else {
                    None
                }
            };

            // Step 2: apply effects of a new trigger (update last_triggered; maybe immediate pause)
            if let Some(t_trig) = new_trigger_time {
                self.last_triggered = Some(t_trig);
                self.trigger_pending = true;
                if self.single_shot {
                    self.start_trigger = false;
                }
            }

            println!("Trigger check: new_trigger_time = {:?}", new_trigger_time);
        }

        if self.trigger_pending {
            if let Some(t_trig) = self.last_triggered {
                let pos = self.trigger_position.clamp(0.0, 1.0);
                let needed: usize = (data.max_points as f64 * pos).round() as usize;
                println!(
                    "Trigger check: trigger_position = {}, needed samples after trigger = {}",
                    pos, needed
                );
                if needed == 0 {
                    data.pause();
                    self.trigger_pending = false;
                } else {
                    // Short immutable borrow to count samples after the trigger
                    let have: usize = if let Some(trace) = data.traces.get(target_name) {
                        trace.live.iter().filter(|p| p[0] > t_trig).count()
                    } else {
                        0
                    };
                    let have_other: usize = if let Some(trace) = data.traces.get(target_name) {
                        trace.live.iter().filter(|p| p[0] < t_trig).count()
                    } else {
                        0
                    };
                    println!(
                    "Trigger check: have {} samples after trigger time {:.6} with have {} samples before",
                    have, t_trig, have_other
                );
                    if have >= needed {
                        println!("Pausing due to trigger at time {:.6}", t_trig);
                        data.pause();
                        self.trigger_pending = false;
                    }
                }
                return true;
            } else {
                self.trigger_pending = false;
            }
        }

        return false;
    }
}
