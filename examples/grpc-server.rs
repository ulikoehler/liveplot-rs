use std::time::{Duration, SystemTime, UNIX_EPOCH};

use async_stream::try_stream;
use tonic::{Request, Response, Status};
use tokio::time::interval;

pub mod sine {
    pub mod v1 {
        tonic::include_proto!("sine.v1");
    }
}

use sine::v1::{sine_wave_server::{SineWave, SineWaveServer}, Sample, SubscribeRequest};

#[derive(Default)]
struct SineSvc;

use std::pin::Pin;
use futures_core::Stream;

#[tonic::async_trait]
impl SineWave for SineSvc {
    type SubscribeStream = Pin<Box<dyn Stream<Item = Result<Sample, Status>> + Send + 'static>>;

    async fn subscribe(
        &self,
        _request: Request<SubscribeRequest>,
    ) -> Result<Response<Self::SubscribeStream>, Status> {
        // Signal params
        const F_HZ: f64 = 5.0;         // 5 Hz
        const FS_HZ: f64 = 1000.0;     // 1 kHz -> 1 ms per sample
        const DT: Duration = Duration::from_millis(1);

        let mut n: i64 = 0;
        let mut ticker = interval(DT);

        let out = try_stream! {
            loop {
                ticker.tick().await;

                let t = n as f64 / FS_HZ; // seconds
                let val = (2.0_f64 * std::f64::consts::PI * F_HZ * t).sin();

                let now = SystemTime::now().duration_since(UNIX_EPOCH)
                    .map_err(|_| Status::internal("clock went backwards"))?;
                let sample = Sample {
                    value: val,
                    index: n,
                    timestamp_micros: now.as_micros() as i64,
                };

                n = n.saturating_add(1);
                yield sample;
            }
        };

        Ok(Response::new(Box::pin(out) as Self::SubscribeStream))
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let addr = "0.0.0.0:50051".parse()?;
    let svc = SineSvc::default();

    println!("SineWave gRPC server streaming on {}", addr);
    tonic::transport::Server::builder()
        .add_service(SineWaveServer::new(svc))
        .serve(addr)
        .await?;

    Ok(())
}
