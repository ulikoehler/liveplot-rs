# liveplot-rs

Live plotting library for timestamped data streams using egui/eframe.

![LivePlot-RS screenshot](docs/liveplot-rs%20screenshot.png)

This crate provides a reusable plotting UI you can feed with a stream of `(timestamp, value)` samples.
gRPC input is provided as an example of how to use the library, not as a built-in dependency.

## Features

#### Two-point analysis

You can select one or two points on the plot to see the values and also delta-X and delta-Y plus slope between the points. You can also compare two different traces using this feature. There is also a "free" selection which does not track the nearest trace point.

![LivePlot-RS screenshot](docs/liveplot-rs%20point%20and%20slope.png)

#### Multi-trace plotting

Display multiple named traces in a single plot with a shared X-axis. A legend appears automatically when more than one trace is visible.

#### Rolling time window and point cap

Control the visible time span (seconds) and limit the number of points kept per trace to manage memory and performance for long-running sessions.

#### Pause/resume with snapshot

Pause the live view to freeze all traces. While paused, computations and exports operate on a per-trace snapshot taken at the moment of pausing; resume to continue streaming.

#### FFT spectrum (optional `fft` feature)

An optional bottom panel shows magnitude spectra for all traces with per-trace overlays. Choose FFT size (power of two), select a window (Rect, Hann, Hamming, Blackman), toggle dB/linear magnitude, and auto-fit the axes. Build with `--features fft` to enable.

#### Data export (CSV, optional Parquet)

Export aligned raw time-domain data for all traces as CSV. With the optional `parquet` feature enabled, Parquet export (via Apache Arrow) is also available.

#### Viewport screenshots (PNG)

Capture the full UI viewport to a PNG file using the "Save PNG" action. Programmatic screenshots to a provided path are also supported.

#### Programmatic control via controllers

External code can observe and influence the UI through lightweight controllers:
- `WindowController` — observe window size and request size/position changes.
- `UiActionController` — pause/resume, trigger screenshots, export raw data, and subscribe/request raw FFT input data for a trace.
- `FftController` — observe and request FFT panel visibility and size (when the `fft` feature is enabled).

#### Flexible time axis formatting

Format X-axis values (timestamps) using `XDateFormat` to suit your display needs.

#### Marker trace selection and free mode

Choose a specific trace for point snapping, or use the free mode to place markers anywhere in the plot without snapping to data points.

#### Interactive plot controls

Pan with the left mouse, use box-zoom with right drag, and reset the view from the toolbar. A small on-screen hint summarizes the available interactions.

## Install

Add `liveplot-rs` to your project's `Cargo.toml` dependencies. The crate is published on crates.io as `liveplot-rs` and depends on `eframe`/`egui` for the UI. A minimal example dependency entry:

```toml
[dependencies]
liveplot-rs = "0.1"
```

If you want to enable optional features such as `fft` (FFT computation and panel), `parquet` (for Parquet export) or `grpc` (for examples using gRPC streaming), enable them in the dependency:

```toml
[dependencies]
liveplot-rs = { version = "0.1", features = ["fft", "parquet", "grpc"] }
```

You can also use the Git repository directly if you want the latest code from the master branch:

```toml
[dependencies]
liveplot-rs = { git = "https://github.com/ulikoehler/liveplot-rs", branch = "master" }
```

Run the included examples from the repository with `cargo run --example <name>` (run this in the crate root). For example:

```bash
cargo run --example sine
```

If you enabled the `grpc` feature, build or run with `--features grpc`:

```bash
cargo run --example grpc-server --features grpc
cargo run --example sine --features grpc
```

### Optional FFT feature

FFT computation and the bottom FFT panel are feature-gated to avoid pulling `rustfft` by default. To enable the FFT UI and functionality, build with the `fft` feature:

```bash
cargo run --example sine --features fft
```

When the `fft` feature is disabled (default), the UI won’t show the “Show FFT” button and no FFT code is compiled. Enabling `fft` compiles the spectrum calculation and reveals the FFT panel and settings in the UI.

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

## Simple example

A minimal example that produces a continuous 3 Hz sine wave sampled at 1 kHz is included as `examples/sine.rs`.

Run it with:

```bash
cargo run --example sine --features fft,parquet
```

(the features are optional for this example but showcase the full UI as shown in the screenshot above)

## gRPC example

To try streaming data via gRPC, enable the `grpc` feature and run the example server and client:

Start the server:

```bash
cargo run --example grpc-server --features grpc
```

Start the client UI:

```bash
cargo run --example sine --features grpc
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