// gRPC client code extracted from main.rs
use tonic::Request;
use std::sync::mpsc::Sender;
use crate::sine::v1::{sine_wave_client::SineWaveClient, SubscribeRequest};
use crate::sink::{PlotCommand, PlotPoint};

pub fn spawn_grpc_client(tx: Sender<PlotCommand>) {
    std::thread::spawn(move || {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async move {
            let mut client = match SineWaveClient::connect("http://127.0.0.1:50051").await {
                Ok(c) => c,
                Err(e) => {
                    eprintln!("Failed to connect to gRPC server: {e}");
                    return;
                }
            };
            let mut stream = match client.subscribe(Request::new(SubscribeRequest{})).await {
                Ok(resp) => resp.into_inner(),
                Err(e) => {
                    eprintln!("Failed to subscribe: {e}");
                    return;
                }
            };
            // Register a trace ID 1 named "signal" once
            let _ = tx.send(PlotCommand::RegisterTrace { id: 1, name: "signal".to_string(), info: None });
            while let Ok(Some(sample)) = stream.message().await {
                let t_s = (sample.timestamp_micros as f64) * 1e-6;
                let _ = tx.send(PlotCommand::Point { trace_id: 1, point: PlotPoint { x: t_s, y: sample.value } });
            }
        });
    });
}
