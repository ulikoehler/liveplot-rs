use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use super::traces::{TraceState, TracesData};
use crate::data::trace_look::TraceLook;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TraceRef(pub String);

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct BiquadParams { pub b: [f64; 3], pub a: [f64; 3] }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum FilterKind {
	Lowpass { cutoff_hz: f64 },
	Highpass { cutoff_hz: f64 },
	Bandpass { low_cut_hz: f64, high_cut_hz: f64 },
	BiquadLowpass { cutoff_hz: f64, q: f64 },
	BiquadHighpass { cutoff_hz: f64, q: f64 },
	BiquadBandpass { center_hz: f64, q: f64 },
	Custom { params: BiquadParams },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MathKind {
	Add { inputs: Vec<(TraceRef, f64)> },
	Multiply { a: TraceRef, b: TraceRef },
	Divide { a: TraceRef, b: TraceRef },
	Differentiate { input: TraceRef },
	Integrate { input: TraceRef, y0: f64 },
	Filter { input: TraceRef, kind: FilterKind },
	MinMax { input: TraceRef, decay_per_sec: Option<f64>, mode: MinMaxMode },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MathTraceDef { pub name: String, pub color_hint: Option<[u8; 3]>, pub kind: MathKind }

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum MinMaxMode { Min, Max }

#[derive(Debug, Default, Clone)]
pub struct MathRuntimeState {
	pub last_t: Option<f64>,
	pub accum: f64,
	pub x1: f64, pub x2: f64, pub y1: f64, pub y2: f64,
	pub x1b: f64, pub x2b: f64, pub y1b: f64, pub y2b: f64,
	pub min_val: f64, pub max_val: f64, pub last_decay_t: Option<f64>,
	pub prev_in_t: Option<f64>, pub prev_in_v: f64,
}
impl MathRuntimeState { pub fn new() -> Self { Self { last_t: None, accum: 0.0, x1: 0.0, x2: 0.0, y1: 0.0, y2: 0.0, x1b: 0.0, x2b: 0.0, y1b: 0.0, y2b: 0.0, min_val: f64::INFINITY, max_val: f64::NEG_INFINITY, last_decay_t: None, prev_in_t: None, prev_in_v: 0.0 } } }

#[derive(Default)]
pub struct MathData {
	pub defs: Vec<MathTraceDef>,
	pub state: HashMap<String, MathRuntimeState>,
}

impl MathData {
	pub fn add_def(&mut self, def: MathTraceDef) {
		if !self.defs.iter().any(|d| d.name == def.name) {
			self.defs.push(def);
		}
	}
	pub fn remove_def(&mut self, name: &str) { self.defs.retain(|d| d.name != name); self.state.remove(name); }
	pub fn reset_storage(&mut self, name: &str) { self.state.insert(name.to_string(), MathRuntimeState::default()); }
	pub fn reset_all_storage(&mut self) { self.state.clear(); }

	pub fn calculate(&mut self, traces: &mut TracesData) {
		// Build source snapshots as Vec for compute
		let mut sources: HashMap<String, Vec<[f64; 2]>> = HashMap::new();
		for (name, tr) in traces.traces.iter() {
			// Use current live deque; if marked invisible, still can be source
			let v: Vec<[f64; 2]> = tr.live.iter().copied().collect();
			sources.insert(name.clone(), v);
		}
		// For pruning new outputs
		let prune_delta = traces.time_window * 1.15;

		for def in self.defs.clone() { // clone names/kinds to avoid borrow issues
			let name = def.name.clone();
			let cut = sources.values().flat_map(|v| v.last()).map(|p| p[0]).max_by(|a,b| a.partial_cmp(b).unwrap()).map(|t| t - prune_delta);
			let prev_out: Option<Vec<[f64;2]>> = traces.traces.get(&name).map(|tr| tr.live.iter().copied().collect());
			let prev_slice = prev_out.as_ref().map(|v| v.as_slice());
			let state = self.state.entry(name.clone()).or_insert_with(MathRuntimeState::new);
			let out = compute_math_trace(&def, &sources, prev_slice, cut, state);
			// Update traces map
			let entry = traces.traces.entry(name.clone()).or_insert_with(|| {
				traces.trace_order.push(name.clone());
				TraceState { name: name.clone(), look: alloc_color_for_index(traces.trace_order.len()-1), offset: 0.0, live: Default::default(), snap: None, info: String::new() }
			});
			entry.live.clear();
			for p in out { entry.live.push_back(p); }
			if let Some(rgb) = def.color_hint { entry.look.color = egui::Color32::from_rgb(rgb[0], rgb[1], rgb[2]); }
			entry.info = describe_def(&def);
		}
	}
}

fn alloc_color_for_index(idx: usize) -> TraceLook {
	let palette = [
		egui::Color32::from_rgb(0x3b,0x82,0xf6),
		egui::Color32::from_rgb(0x10,0xb9,0x81),
		egui::Color32::from_rgb(0xf5,0x93,0x00),
		egui::Color32::from_rgb(0xef,0x44,0x44),
		egui::Color32::from_rgb(0x8b,0x5c,0xff),
	];
	let color = palette[idx % palette.len()];
	TraceLook { color, ..Default::default() }
}

pub fn compute_math_trace(
	def: &MathTraceDef,
	sources: &HashMap<String, Vec<[f64; 2]>>,
	prev_output: Option<&[[f64; 2]]>,
	prune_before: Option<f64>,
	state: &mut MathRuntimeState,
) -> Vec<[f64; 2]> {
	use MathKind::*;
	let mut out: Vec<[f64; 2]> = if let Some(prev) = prev_output { if let Some(cut) = prune_before { prev.iter().copied().filter(|p| p[0] >= cut).collect() } else { prev.to_vec() } } else { Vec::new() };
	match &def.kind {
		Add { inputs } => {
			let mut grid: Vec<f64> = union_times(inputs.iter().map(|(r, _)| r), sources);
			grid.sort_by(|a,b| a.partial_cmp(b).unwrap()); grid.dedup_by(|a,b| (*a-*b).abs()<1e-15);
			let mut caches: HashMap<String,(usize,f64)> = Default::default();
			let mut get_val = |name: &str, t: f64| -> Option<f64> { let data = sources.get(name)?; let (idx, last) = caches.entry(name.to_string()).or_insert((0,f64::NAN)); while *idx+1 < data.len() && data[*idx+1][0] <= t { *idx += 1; } *last = data[*idx][1]; Some(*last) };
			out.clear();
			for &t in &grid { let mut sum = 0.0; let mut any=false; for (r,k) in inputs { if let Some(v)=get_val(&r.0,t){ sum += k*v; any=true; } } if any { if let Some(cut)=prune_before{ if t<cut{continue;} } out.push([t,sum]); } }
		}
		Multiply { a, b } | Divide { a, b } => {
			let mut grid: Vec<f64> = union_times([a,b].into_iter(), sources);
			grid.sort_by(|a,b| a.partial_cmp(b).unwrap()); grid.dedup_by(|x,y| (*x-*y).abs()<1e-15);
			let mut caches: HashMap<String,(usize,f64)> = Default::default();
			let mut get_val = |name: &str, t: f64| -> Option<f64> { let data = sources.get(name)?; let (idx, last) = caches.entry(name.to_string()).or_insert((0,f64::NAN)); while *idx+1 < data.len() && data[*idx+1][0] <= t { *idx += 1; } *last = data[*idx][1]; Some(*last) };
			out.clear();
			for &t in &grid { if let Some(cut)=prune_before{ if t<cut{continue;} }
				if let (Some(va), Some(vb)) = (get_val(&a.0,t), get_val(&b.0,t)) {
					if matches!(&def.kind, Multiply{..}) { out.push([t, va*vb]); }
					else if vb.abs()>1e-12 { out.push([t, va/vb]); }
				}
			}
		}
		Differentiate { input } => {
			let data = match sources.get(&input.0) { Some(v)=>v, None=>return out };
			out.clear();
			let mut prev: Option<(f64,f64)> = None;
			for &p in data.iter() { let t=p[0]; let v=p[1]; if let Some(cut)=prune_before{ if t<cut { prev=Some((t,v)); continue; } } if let Some((t0,v0))=prev { let dt=t-t0; if dt>0.0 { out.push([t,(v-v0)/dt]); } } prev=Some((t,v)); }
		}
		Integrate { input, y0 } => {
			let data = match sources.get(&input.0) { Some(v)=>v, None=>return out };
			let mut accum = if state.prev_in_t.is_none() { *y0 } else { state.accum };
			let mut prev_t = state.prev_in_t; let mut prev_v = if state.prev_in_t.is_none() { None } else { Some(state.prev_in_v) };
			let mut start_idx = 0usize; if let Some(t0)=state.prev_in_t { start_idx = match data.binary_search_by(|p| p[0].partial_cmp(&t0).unwrap()) { Ok(mut i)=>{ while i<data.len() && data[i][0] <= t0 { i+=1; } i }, Err(i)=>i } };
			for p in data.iter().skip(start_idx) { let t=p[0]; let v=p[1]; if let Some(cut)=prune_before{ if t<cut { continue; } } if let (Some(t0),Some(v0))=(prev_t,prev_v){ let dt=t-t0; if dt>0.0 { accum += 0.5*(v+v0)*dt; } } prev_t=Some(t); prev_v=Some(v); out.push([t,accum]); }
			state.accum=accum; state.last_t=prev_t; state.prev_in_t=prev_t; state.prev_in_v=prev_v.unwrap_or(state.prev_in_v);
		}
		Filter { input, kind } => {
			let data = match sources.get(&input.0) { Some(v)=>v, None=>return out };
			let mut x1=state.x1; let mut x2=state.x2; let mut y1=state.y1; let mut y2=state.y2; let mut last_t=state.prev_in_t;
			let mut x1b=state.x1b; let mut x2b=state.x2b; let mut y1b=state.y1b; let mut y2b=state.y2b;
			let mut start_idx=0usize; if let Some(t0)=state.prev_in_t { start_idx = match data.binary_search_by(|p| p[0].partial_cmp(&t0).unwrap()) { Ok(mut i)=>{ while i<data.len() && data[i][0] <= t0 { i+=1; } i }, Err(i)=>i } };
			for p in data.iter().skip(start_idx) { let t=p[0]; let x=p[1]; if let Some(cut)=prune_before{ if t<cut { continue; } } let dt = if let Some(t0)=last_t { (t-t0).max(1e-9) } else { 1e-3 };
				let y = match kind {
					FilterKind::Lowpass { cutoff_hz } => { let p = first_order_lowpass(*cutoff_hz, dt); biquad_step(p, x, x1, x2, y1, y2) }
					FilterKind::Highpass { cutoff_hz } => { let p = first_order_highpass(*cutoff_hz, dt); biquad_step(p, x, x1, x2, y1, y2) }
					FilterKind::Bandpass { low_cut_hz, high_cut_hz } => { let p1 = first_order_highpass(*low_cut_hz, dt); let z1 = biquad_step(p1, x, x1, x2, y1, y2); let p2 = first_order_lowpass(*high_cut_hz, dt); biquad_step(p2, z1, x1b, x2b, y1b, y2b) }
					FilterKind::BiquadLowpass { cutoff_hz, q } => { let p = biquad_lowpass(*cutoff_hz, *q, dt); biquad_step(p, x, x1, x2, y1, y2) }
					FilterKind::BiquadHighpass { cutoff_hz, q } => { let p = biquad_highpass(*cutoff_hz, *q, dt); biquad_step(p, x, x1, x2, y1, y2) }
					FilterKind::BiquadBandpass { center_hz, q } => { let p = biquad_bandpass(*center_hz, *q, dt); biquad_step(p, x, x1, x2, y1, y2) }
					FilterKind::Custom { params } => { biquad_step(*params, x, x1, x2, y1, y2) }
				};
				match kind {
					FilterKind::Bandpass { low_cut_hz, .. } => { let p1 = first_order_highpass(*low_cut_hz, dt); let z1 = biquad_step(p1, x, x1, x2, y1, y2); x2=x1; x1=x; y2=y1; y1=z1; x2b=x1b; x1b=z1; y2b=y1b; y1b=y; }
					_ => { x2=x1; x1=x; y2=y1; y1=y; }
				}
				last_t=Some(t); out.push([t,y]);
			}
			state.x1=x1; state.x2=x2; state.y1=y1; state.y2=y2; state.last_t=last_t; state.prev_in_t=last_t; state.prev_in_v = data.last().map(|p| p[1]).unwrap_or(state.prev_in_v);
			state.x1b=x1b; state.x2b=x2b; state.y1b=y1b; state.y2b=y2b;
		}
		MinMax { input, decay_per_sec, mode } => {
			let data = match sources.get(&input.0) { Some(v)=>v, None=>return out };
			let mut min_v=state.min_val; let mut max_v=state.max_val; let mut last_decay_t=state.last_decay_t; let mut start_idx=0usize; if let Some(t0)=state.prev_in_t { start_idx= match data.binary_search_by(|p| p[0].partial_cmp(&t0).unwrap()) { Ok(mut i)=>{ while i<data.len() && data[i][0] <= t0 { i+=1; } i }, Err(i)=>i } };
			for p in data.iter().skip(start_idx) { let t=p[0]; let v=p[1]; if let Some(cut)=prune_before{ if t<cut { continue; } } if let Some(decay)=decay_per_sec { if let Some(t0)=last_decay_t { let dt=(t-t0).max(0.0); if dt>0.0 { let k=(-decay*dt).exp(); min_v = min_v.min(v)*k + v*(1.0-k); max_v = max_v.max(v)*k + v*(1.0-k); } } }
				if min_v.is_infinite() { min_v = v; } if max_v.is_infinite() { max_v = v; } min_v=min_v.min(v); max_v=max_v.max(v); last_decay_t=Some(t); let y = match mode { MinMaxMode::Min => min_v, MinMaxMode::Max => max_v }; out.push([t,y]); }
			state.min_val=min_v; state.max_val=max_v; state.last_decay_t=last_decay_t; state.prev_in_t=data.last().map(|p| p[0]); state.prev_in_v=data.last().map(|p| p[1]).unwrap_or(state.prev_in_v);
		}
	}
	out
}

fn union_times<'a>(it: impl IntoIterator<Item=&'a TraceRef>, sources: &HashMap<String, Vec<[f64;2]>>) -> Vec<f64> {
	let mut v = Vec::new(); for r in it { if let Some(d)=sources.get(&r.0){ v.extend(d.iter().map(|p| p[0])); } } v
}

#[inline]
fn first_order_lowpass(fc: f64, dt: f64) -> BiquadParams { let rc = 1.0 / (2.0 * std::f64::consts::PI * fc.max(1e-9)); let alpha = dt / (rc + dt); BiquadParams { b: [alpha, 0.0, 0.0], a: [1.0, -(1.0 - alpha), 0.0] } }
#[inline]
fn first_order_highpass(fc: f64, dt: f64) -> BiquadParams { let rc = 1.0 / (2.0 * std::f64::consts::PI * fc.max(1e-9)); let alpha = rc / (rc + dt); BiquadParams { b: [alpha, -alpha, 0.0], a: [1.0, -alpha, 0.0] } }
#[inline]
fn biquad_step(p: BiquadParams, x0: f64, x1: f64, x2: f64, y1: f64, y2: f64) -> f64 { let a0 = if p.a[0].abs()<1e-15 {1.0} else {p.a[0]}; let b0=p.b[0]/a0; let b1=p.b[1]/a0; let b2=p.b[2]/a0; let a1=p.a[1]/a0; let a2=p.a[2]/a0; b0*x0 + b1*x1 + b2*x2 - a1*y1 - a2*y2 }
#[inline]
fn biquad_lowpass(fc: f64, q: f64, dt: f64) -> BiquadParams { let fs=(1.0/dt).max(1.0); let w0=2.0*std::f64::consts::PI*(fc.max(1e-9)/fs); let cosw0=w0.cos(); let sinw0=w0.sin(); let q=q.max(1e-6); let alpha=sinw0/(2.0*q); let b0=(1.0-cosw0)*0.5; let b1=1.0-cosw0; let b2=(1.0-cosw0)*0.5; let a0=1.0+alpha; let a1=-2.0*cosw0; let a2=1.0-alpha; BiquadParams{ b:[b0,b1,b2], a:[a0,a1,a2] } }
#[inline]
fn biquad_highpass(fc: f64, q: f64, dt: f64) -> BiquadParams { let fs=(1.0/dt).max(1.0); let w0=2.0*std::f64::consts::PI*(fc.max(1e-9)/fs); let cosw0=w0.cos(); let sinw0=w0.sin(); let q=q.max(1e-6); let alpha=sinw0/(2.0*q); let b0=(1.0+cosw0)*0.5; let b1=-(1.0+cosw0); let b2=(1.0+cosw0)*0.5; let a0=1.0+alpha; let a1=-2.0*cosw0; let a2=1.0-alpha; BiquadParams{ b:[b0,b1,b2], a:[a0,a1,a2] } }
#[inline]
fn biquad_bandpass(fc: f64, q: f64, dt: f64) -> BiquadParams { let fs=(1.0/dt).max(1.0); let w0=2.0*std::f64::consts::PI*(fc.max(1e-9)/fs); let cosw0=w0.cos(); let sinw0=w0.sin(); let q=q.max(1e-6); let alpha=sinw0/(2.0*q); let b0=alpha; let b1=0.0; let b2=-alpha; let a0=1.0+alpha; let a1=-2.0*cosw0; let a2=1.0-alpha; BiquadParams{ b:[b0,b1,b2], a:[a0,a1,a2] } }

fn describe_def(def: &MathTraceDef) -> String {
	use MathKind::*;
	match &def.kind {
		Add { inputs } => {
			if inputs.is_empty() { return "sum()".to_string(); }
			let s = inputs.iter()
				.map(|(r,k)| if (*k-1.0).abs()<1e-12 { r.0.clone() } else { format!("{k}*{}", r.0) })
				.collect::<Vec<_>>().join(" + ");
			format!("{s}")
		}
		Multiply { a, b } => format!("{} * {}", a.0, b.0),
		Divide { a, b } => format!("{} / {}", a.0, b.0),
		Differentiate { input } => format!("d({})/dt", input.0),
		Integrate { input, y0 } => format!("∫ {} dt  (y0 = {})", input.0, y0),
		Filter { input, kind } => {
			let k = match kind {
				FilterKind::Lowpass { cutoff_hz } => format!("LP {:.3} Hz", cutoff_hz),
				FilterKind::Highpass { cutoff_hz } => format!("HP {:.3} Hz", cutoff_hz),
				FilterKind::Bandpass { low_cut_hz, high_cut_hz } => format!("BP {:.3}-{:.3} Hz", low_cut_hz, high_cut_hz),
				FilterKind::BiquadLowpass { cutoff_hz, q } => format!("BQLP {:.3} Hz, Q={:.2}", cutoff_hz, q),
				FilterKind::BiquadHighpass { cutoff_hz, q } => format!("BQHP {:.3} Hz, Q={:.2}", cutoff_hz, q),
				FilterKind::BiquadBandpass { center_hz, q } => format!("BQBP {:.3} Hz, Q={:.2}", center_hz, q),
				FilterKind::Custom { .. } => "Custom".to_string(),
			};
			format!("{k}({})", input.0)
		}
		MinMax { input, decay_per_sec, mode } => {
			let which = match mode { MinMaxMode::Min => "min", MinMaxMode::Max => "max" };
			if let Some(d) = decay_per_sec { format!("{}({}, decay={} 1/s)", which, input.0, d) } else { format!("{}({})", which, input.0) }
		}
	}
}
