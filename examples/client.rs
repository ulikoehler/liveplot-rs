use tonic::Request;
use std::sync::mpsc;

// Import the library (multi-trace only)
use liveplot::{run_liveplot, LivePlotConfig};
use liveplot::sink::{PlotCommand, PlotPoint};

// Include the generated proto just for the example
pub mod sine { pub mod v1 { tonic::include_proto!("sine.v1"); } }
use sine::v1::{sine_wave_client::SineWaveClient, SubscribeRequest};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Channel to hand samples into the plotter (multi-trace)
    let (tx, rx) = mpsc::channel::<PlotCommand>();

    // Spawn the UI on a separate thread because eframe runs a native event loop
    let ui_handle = std::thread::spawn(move || {
        // Run the UI until the window is closed (single trace labeled "signal")
    if let Err(e) = run_liveplot(rx, LivePlotConfig::default()) {
            eprintln!("UI error: {e}");
        }
    });

    // Connect to the gRPC server and forward samples into the channel
    let mut client = SineWaveClient::connect("http://127.0.0.1:50051").await?;
    let mut stream = client.subscribe(Request::new(SubscribeRequest{})).await?.into_inner();

    // Register trace once
    let _ = tx.send(PlotCommand::RegisterTrace { id: 1, name: "signal".into(), info: None });
    while let Some(sample) = stream.message().await? {
        let t_s = (sample.timestamp_micros as f64) * 1e-6;
        let cmd = PlotCommand::Point { trace_id: 1, point: PlotPoint { x: t_s, y: sample.value } };
        if tx.send(cmd).is_err() { break; }
    }

    // Wait for UI thread to finish
    let _ = ui_handle.join();
    Ok(())
}
