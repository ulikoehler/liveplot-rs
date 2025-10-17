// Reused from previous architecture: copied lightly
use std::sync::mpsc::{Receiver, Sender};

#[derive(Debug, Clone)]
pub struct MultiSample {
    pub index: u64,
    pub value: f64,
    pub timestamp_micros: i64,
    pub trace: String,
    pub info: Option<String>,
}

#[derive(Clone)]
pub struct MultiPlotSink {
    tx: Sender<MultiSample>,
}

impl MultiPlotSink {
    pub fn send(&self, sample: MultiSample) -> Result<(), std::sync::mpsc::SendError<MultiSample>> { self.tx.send(sample) }
    pub fn send_value<S: Into<String>>(&self, index: u64, value: f64, timestamp_micros: i64, trace: S) -> Result<(), std::sync::mpsc::SendError<MultiSample>> {
        let s = MultiSample { index, value, timestamp_micros, trace: trace.into(), info: None }; self.send(s)
    }
    pub fn send_value_with_info<S: Into<String>, I: Into<String>>(&self, index: u64, value: f64, timestamp_micros: i64, trace: S, info: I) -> Result<(), std::sync::mpsc::SendError<MultiSample>> {
        let s = MultiSample { index, value, timestamp_micros, trace: trace.into(), info: Some(info.into()) }; self.send(s)
    }
}

pub fn channel_multi() -> (MultiPlotSink, Receiver<MultiSample>) {
    let (tx, rx) = std::sync::mpsc::channel(); (MultiPlotSink { tx }, rx)
}
