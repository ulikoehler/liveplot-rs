//! Deterministic dummy-signal generators (see docs/e2e-test-plan.md §3).

use liveplot::PlotPoint;

/// xorshift64 — tiny deterministic PRNG so tests never depend on OS entropy.
pub struct TestRng(u64);

impl TestRng {
    pub fn new(seed: u64) -> Self {
        Self(seed.max(1))
    }
    pub fn next_u64(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }
    /// Uniform in [-1, 1).
    pub fn next_f64(&mut self) -> f64 {
        (self.next_u64() as f64 / u64::MAX as f64) * 2.0 - 1.0
    }
}

#[derive(Clone)]
#[allow(dead_code)] // full generator library — not every spec is used yet
pub enum SignalSpec {
    Sine {
        freq: f64,
        amp: f64,
        phase: f64,
    },
    Square {
        freq: f64,
        amp: f64,
        duty: f64,
    },
    Sawtooth {
        freq: f64,
        amp: f64,
    },
    Ramp {
        slope: f64,
        offset: f64,
    },
    Constant {
        v: f64,
    },
    Step {
        levels: Vec<f64>,
        per: usize,
    },
    Impulse {
        at_idx: usize,
        amp: f64,
    },
    WhiteNoise {
        seed: u64,
        amp: f64,
    },
    RandomWalk {
        seed: f64,
        step: f64,
    },
    Chirp {
        f0: f64,
        f1: f64,
        dur: f64,
    },
    SumOfSines(Vec<(f64, f64)>),
    /// Sample-wise custom fn(idx, t_seconds) -> y.
    Custom(std::sync::Arc<dyn Fn(usize, f64) -> f64 + Send + Sync>),
}

impl SignalSpec {
    pub fn sine(freq: f64) -> Self {
        Self::Sine {
            freq,
            amp: 1.0,
            phase: 0.0,
        }
    }
    pub fn tone(amp: f64) -> Self {
        Self::Sine {
            freq: 60.0,
            amp,
            phase: 0.0,
        }
    }

    fn eval(&self, i: usize, t: f64) -> f64 {
        use std::f64::consts::PI;
        match self {
            Self::Sine { freq, amp, phase } => amp * (2.0 * PI * freq * t + phase).sin(),
            Self::Square { freq, amp, duty } => {
                if (t * freq).fract() < *duty {
                    *amp
                } else {
                    -*amp
                }
            }
            Self::Sawtooth { freq, amp } => amp * (2.0 * (t * freq).fract() - 1.0),
            Self::Ramp { slope, offset } => slope * t + offset,
            Self::Constant { v } => *v,
            Self::Step { levels, per } => {
                if levels.is_empty() {
                    0.0
                } else {
                    levels[(i / (*per).max(1)).min(levels.len() - 1)]
                }
            }
            Self::Impulse { at_idx, amp } => {
                if i == *at_idx {
                    *amp
                } else {
                    0.0
                }
            }
            Self::WhiteNoise { seed, amp } => {
                let mut rng = TestRng::new(seed.wrapping_add(i as u64));
                amp * rng.next_f64()
            }
            Self::RandomWalk { seed, step } => {
                let mut rng = TestRng::new(*seed as u64);
                (0..=i).map(|_| step * rng.next_f64()).sum()
            }
            Self::Chirp { f0, f1, dur } => {
                let k = if *dur > 0.0 { (f1 - f0) / dur } else { 0.0 };
                (2.0 * PI * (f0 * t + 0.5 * k * t * t)).sin()
            }
            Self::SumOfSines(parts) => parts
                .iter()
                .map(|(f, a)| a * (2.0 * PI * f * t).sin())
                .sum(),
            Self::Custom(f) => f(i, t),
        }
    }
}

/// Generate `n` samples of `spec` at sample rate `fs` starting at `t0`.
pub fn gen(spec: &SignalSpec, n: usize, fs: f64, t0: f64) -> Vec<PlotPoint> {
    (0..n)
        .map(|i| {
            let t = t0 + i as f64 / fs;
            PlotPoint {
                x: t,
                y: spec.eval(i, t),
            }
        })
        .collect()
}

/// Same as [`gen`] but injects NaN every `every` samples.
#[allow(dead_code)]
pub fn gen_with_gaps(
    spec: &SignalSpec,
    n: usize,
    fs: f64,
    t0: f64,
    every: usize,
) -> Vec<PlotPoint> {
    (0..n)
        .map(|i| {
            let t = t0 + i as f64 / fs;
            let y = if every > 0 && i % every == every - 1 {
                f64::NAN
            } else {
                spec.eval(i, t)
            };
            PlotPoint { x: t, y }
        })
        .collect()
}
