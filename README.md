# liveplot-rs

Live plotting library for timestamped data streams using egui/eframe.

![LivePlot-RS screenshot](docs/liveplot-rs%20screenshot.png)

This crate provides a reusable plotting UI you can feed with a stream of `(timestamp, value)` samples.
gRPC input is provided as an example of how to use the library, not as a built-in dependency.

## Library usage

This crate now uses a unified multi-trace UI for all use-cases. For a single signal, just pick a default trace name like `"signal"`.

```rust
use liveplot_rs::{channel_multi, run_multi};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

fn main() -> eframe::Result<()> {
    let (sink, rx) = channel_multi();

    std::thread::spawn(move || {
        let mut n: u64 = 0;
        let dt = Duration::from_millis(1);
        loop {
            let now_us = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_micros() as i64;
            let value = (2.0 * std::f64::consts::PI * 3.0 * (n as f64 / 1000.0)).sin();
            let _ = sink.send_value(n, value, now_us, "signal");
            n = n.wrapping_add(1);
            std::thread::sleep(dt);
        }
    });

    run_multi(rx)
}
```

## gRPC example

To try streaming data via gRPC, enable the `grpc` feature and run the example server and client:

Start the server:

```bash
cargo run --example grpc-server --features grpc
```

Start the client UI:

```bash
cargo run --example client --features grpc
```

The examples use the proto in `proto/sine.proto` and are only compiled when the `grpc` feature is enabled.

## Built-in synthetic example: `sine`

A minimal example that produces a continuous 3 Hz sine wave sampled at 1 kHz is included as `examples/sine.rs`.

Run it with:

```bash
cargo run --example sine
```

This will open the plotting UI and stream a synthetic sine signal into it.

## Built-in synthetic example: `sine_cosine`

An example that produces both sine and cosine traces and displays them together with a legend.

Run it with:

```bash
cargo run --example sine_cosine
```

## Optional Parquet export

This crate supports exporting aligned multi-trace data to Apache Parquet via Apache Arrow, but Parquet support is optional and feature-gated to avoid pulling large dependencies by default.

To enable Parquet export, build with the `parquet` feature:

```bash
cargo build --features parquet
cargo run --features parquet --example sine
```

When enabled the UI's "Save raw data" dialog will offer both `CSV` and `Parquet` and `.parquet` files will contain an Arrow-compatible schema with the following columns:

- `timestamp_seconds: Float64` (non-null) — aligned timestamp in seconds
- `<trace_name>: Float64` (nullable) — one column per trace in the export order; missing values are recorded as NULL

If you build without the `parquet` feature the UI will only offer CSV export and attempting to export Parquet programmatically will return an error explaining the feature is not enabled.

Python example to read the resulting Parquet file into a Pandas DataFrame:

```python
import pandas as pd

df = pd.read_parquet("snapshot.parquet")
df.set_index("timestamp_seconds", inplace=True)
df.plot()
```

Note that if the timestamps of the different traces are not aligned, the resulting Parquet file may contain missing values.