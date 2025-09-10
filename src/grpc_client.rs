// gRPC client code extracted from main.rs
use tonic::Request;
use std::sync::mpsc::Sender;
use crate::sine::v1::{sine_wave_client::SineWaveClient, SubscribeRequest};
use crate::Sample;

pub fn spawn_grpc_client(tx: Sender<Sample>) {
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
            while let Ok(Some(sample)) = stream.message().await {
                let sample = Sample {
                    index: sample.index as u64,
                    value: sample.value,
                    timestamp_micros: sample.timestamp_micros,
                };
                if tx.send(sample).is_err() {
                    break;
                }
            }
        });
    });
}
