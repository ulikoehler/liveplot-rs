use std::sync::mpsc;
use tonic::Request;

// Import the library (multi-trace only)
use liveplot::{run_liveplot, LivePlotConfig, MultiSample};

// Include the generated proto just for the example
pub mod sine {
    pub mod v1 {
        tonic::include_proto!("sine.v1");
    }
}
use sine::v1::{sine_wave_client::SineWaveClient, SubscribeRequest};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Channel to hand samples into the plotter (multi-trace)
    let (tx, rx) = mpsc::channel::<MultiSample>();

    // Spawn the UI on a separate thread because eframe runs a native event loop
    let ui_handle = std::thread::spawn(move || {
        // Run the UI until the window is closed (single trace labeled "signal")
        if let Err(e) = run_liveplot(rx, LivePlotConfig::default()) {
            eprintln!("UI error: {e}");
        }
    });

    // Connect to the gRPC server and forward samples into the channel
    let mut client = SineWaveClient::connect("http://127.0.0.1:50051").await?;
    let mut stream = client
        .subscribe(Request::new(SubscribeRequest {}))
        .await?
        .into_inner();

    while let Some(sample) = stream.message().await? {
        let s = MultiSample {
            index: sample.index as u64,
            value: sample.value,
            timestamp_micros: sample.timestamp_micros,
            trace: "signal".to_string(),
            info: None,
        };
        // Stop sending if the receiver is gone
        if tx.send(s).is_err() {
            break;
        }
    }

    // Wait for UI thread to finish
    let _ = ui_handle.join();
    Ok(())
}
