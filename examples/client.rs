use tonic::Request;

pub mod sine { pub mod v1 { tonic::include_proto!("sine.v1"); } }
use sine::v1::{sine_wave_client::SineWaveClient, SubscribeRequest};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut client = SineWaveClient::connect("http://127.0.0.1:50051").await?;
    let mut stream = client.subscribe(Request::new(SubscribeRequest{})).await?.into_inner();

    let mut count = 0usize;
    while let Some(sample) = stream.message().await? {
        println!("n={} value={:.6} t_us={}", sample.index, sample.value, sample.timestamp_micros);
        count += 1;
    }
    Ok(())
}
