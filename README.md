# liveplot
https://img.shields.io/crates/v/:crate

![Crates.io Version](https://img.shields.io/crates/v/liveplot)

![LivePlot screenshot](docs/liveplot%20screenshot.png)

This crate provides a reusable plotting UI you can feed with a stream of `(timestamp, value)` samples.
gRPC input is provided as an example of how to use the library, not as a built-in dependency.
A minimal example that produces a continuous 3 Hz sine wave sampled at 1 kHz is included as [examples/sine.rs](examples/sine.rs).
## Features

#### Tiles

liveplot provides a `egui_tiles` API if enabled via the `tiles` feature. See the [examples/embedded_tiles.rs](examples/embedded_tiles.rs) example for usage.

![LivePlot embedded tiles screenshot](docs/liveplot%20embedded%20tiles.png)

#### Two-point analysis

You can select one or two points on the plot to see the values and also delta-X and delta-Y plus slope between the points. You can also compare two different traces using this feature. There is also a "free" selection which does not track the nearest trace point.

![LivePlot screenshot](docs/liveplot%20point%20and%20slope.png)

A minimal example that produces a continuous 3 Hz sine wave sampled at 1 kHz is included as [examples/sine.rs](examples/sine.rs).

#### Multi-trace plotting

Display multiple named traces in a single plot with a shared X-axis. A legend appears automatically when more than one trace is visible.

You can set a global Y-axis unit label and optionally enable a log10 Y scale. When log scale is enabled, each trace is transformed as `log10(value + offset)`; non-positive samples are omitted from the plot. Per-trace Y offsets can be adjusted in the Traces dialog.

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
- `FFTController` — observe and request FFT panel visibility and size (when the `fft` feature is enabled).
- `TracesController` — observe and modify trace colors/visibility, per-trace Y offsets, marker selection, and global Y unit and Y log mode.

#### Threshold detection and event logging

Detect when a trace exceeds a condition for a minimum duration and keep a rolling log of events. Open the `Thresholds…` dialog to add/edit/remove detectors and to browse or export events.

Supported conditions:

- `GreaterThan { value }` — active while the trace is above `value`.
- `LessThan { value }` — active while the trace is below `value`.
- `InRange { low, high }` — active while `low ≤ value ≤ high`.

Each threshold has:

- A unique `name` and the `trace` to monitor.
- `min_duration` — condition must hold continuously for at least this time (default 2 ms) to emit an event (debounce).
- `max_events` — cap per-threshold history to bound memory usage (oldest events are dropped).

For every recorded event the UI stores and shows:

- `start` and `end` timestamps (formatted using the current X-axis date/time format),
- `duration` in milliseconds,
- `trace` and `threshold` names,
- `area` — integrated excess while the condition was active. For `GreaterThan`, area is `∫(value - threshold) dt`; for `LessThan`, `∫(threshold - value) dt`; for `InRange`, `∫(value - low) dt`.

Events appear in the `Threshold events` table inside the dialog. You can filter by threshold name and `Export to CSV` the currently visible entries. The total number of events since app start is shown on the toolbar button as a quick indicator.

Programmatic API is available via `ThresholdController` to add/remove thresholds and subscribe to events from your own code:

```rust
use liveplot::{ThresholdController, ThresholdDef, ThresholdKind, TraceRef};

// Create a controller and add it to `LivePlotConfig` before starting the UI.
let controller = ThresholdController::new();

// Define a threshold: "gt_0_8" while `signal` > 0.8 for at least 2 ms; keep up to 100 events.
let def = ThresholdDef {
    name: "gt_0_8".into(),
    target: TraceRef("signal".into()),
    kind: ThresholdKind::GreaterThan { value: 0.8 },
    min_duration_s: 0.002,
    max_events: 100,
};
controller.add_threshold(def);

// Subscribe to events (non-blocking mpsc channel)
let rx = controller.subscribe();
std::thread::spawn(move || {
    while let Ok(evt) = rx.recv() {
        println!(
            "Threshold {} on {}: start={:.3}s, dur={:.3}ms, area={:.6}",
            evt.threshold, evt.trace, evt.start_t, evt.duration * 1000.0, evt.area
        );
    }
});
```

In the UI, thresholds can be created/edited interactively, and any events recorded while paused operate on the per-trace snapshots, just like other analysis features.

#### Y axis unit, log scale, and per-trace offsets

Open the `Traces…` dialog to:

- Choose the marker mode: snap to a specific trace or use Free placement.
- Toggle `Y axis log scale` (base-10).
- Edit the `Y unit` string appended to tick labels and readouts (e.g., `V`, `A`, `°C`).
- Set an individual `Offset` per trace, applied before plotting and the optional log transform.

Programmatically, use `LivePlotConfig { y_unit, y_log, .. }` to set defaults at startup, and `TracesController` to modify them at runtime:

```rust
use liveplot::TracesController;
let traces = TracesController::new();
traces.request_set_color("signal", [255, 128, 0]);
traces.request_set_visible("noise", false);
traces.request_set_offset("signal", 1.5);
traces.request_set_y_unit(Some("V"));
traces.request_set_y_log(true);
```

#### Flexible time axis formatting

Format X-axis values (timestamps) using `XDateFormat` to suit your display needs.

#### Marker trace selection and free mode

Choose a specific trace for point snapping, or use the free mode to place markers anywhere in the plot without snapping to data points.

#### Interactive plot controls

Pan with the left mouse, use box-zoom with right drag, and reset the view from the toolbar. A small on-screen hint summarizes the available interactions.

#### Math (virtual) traces

Create derived traces from existing ones (oscilloscope-style Math). Click the `Math…` button to open a dialog that lets you define and manage math traces. Supported operations:

- Add/Subtract N traces with individual gains
- Multiply or Divide two traces
- Differentiate one trace numerically
- Integrate one trace numerically (with configurable initial value)
- Filter one trace: Lowpass, Highpass, or Bandpass (first-order) with configurable cutoff(s)
- Track Min or Max of a trace (optionally with exponential decay)

Math traces auto-update as input traces change and behave like normal traces (legend, export, selection, FFT, etc.).

Programmatic API is also available if you build your own UI around the library. For example:

```rust
use liveplot::{MathTraceDef, MathKind, FilterKind, TraceRef, LivePlotApp};
// assuming you have a LivePlotApp `app`
let def = MathTraceDef { name: "sum".into(), color_hint: None, kind: MathKind::Add { inputs: vec![(TraceRef("a".into()), 1.0), (TraceRef("b".into()), -1.0)] } };
// app.add_math_trace(def);
```

## Install

First, since this is a [Rust](https://www.rust-lang.org/) crate, you need to have Rust installed. If you don't have it yet, I recommend installing the latest version from [rustup.rs](https://rustup.rs/). I do not recommend using the outdated Rust from your Linux distribution's package manager.

Add `liveplot` to your project's `Cargo.toml` dependencies. The crate is published on crates.io as `liveplot` and depends on `eframe`/`egui` for the UI. A minimal example dependency entry:

```toml
[dependencies]
liveplot = "0.1"
```

If you want to enable optional features such as `fft` (FFT computation and panel), `parquet` (for Parquet export) or `grpc` (for examples using gRPC streaming), enable them in the dependency:

```toml
[dependencies]
liveplot = { version = "0.1", features = ["fft", "parquet", "grpc"] }
```

You can also use the Git repository directly if you want the latest code from the master branch:

```toml
[dependencies]
liveplot = { git = "https://github.com/ulikoehler/liveplot", branch = "master" }
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
use liveplot::{channel_multi, run_multi};
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

A minimal example that produces a continuous 3 Hz sine wave sampled at 1 kHz is included as [examples/sine.rs](examples/sine.rs).

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

A minimal example that produces a continuous 3 Hz sine wave sampled at 1 kHz is included as [examples/sine.rs](examples/sine.rs).

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

## Built-in example: `custom_colors`

Demonstrates setting per-trace colors via the API using `TracesController`.

Run it with:

```bash
cargo run --example custom_colors
```

## Built-in example: `lots_of_tiny_plots`

Shows a 20×15 grid of tiny embedded plots. Each cell renders the same sine waveform
with a different phase offset and unique color; this example exercises embedding
many `MainPanel` instances in a compact layout.

Run it with:

```bash
cargo run --example lots_of_tiny_plots
```

You can adjust samples-per-second and sine frequency:

```bash
cargo run --example lots_of_tiny_plots -- -s 10 -h 2.5
cargo run --example lots_of_tiny_plots -- --samples-per-second 10.0 --hz 2.5
```

![Lots of tiny plots screenshot](docs/liveplot%20lots%20of%20tiny%20plots.png)

## Built-in example: `thresholds_sine`

Demonstrates adding a simple threshold (e.g., greater than 0.8 for at least 2 ms) and printing events in the console using `ThresholdController`.

Run it with:

```bash
cargo run --example thresholds_sine
```

## Built-in example: `sine_cosine_delayed_snapshot`

Shows saving raw data to CSV/Parquet after a short delay and/or from a paused snapshot.
Requires the optional `parquet` feature to enable Parquet export.

Run it with:

```bash
cargo run --example sine_cosine_delayed_snapshot --features parquet
```

## Built-in example: `window_control`

Demonstrates using `WindowController` and (optionally) monitor geometry to position and size the window.

Run it with:

```bash
cargo run --example window_control --features window_control_display_info
```

## Live CSV tail example: `csv_tail`

This example demonstrates monitoring a CSV file that is continuously appended by an external process (for example, a data logger or the provided Python script) and plotting the values as they appear. It polls the file every 20 ms, reads any newly appended complete lines, and streams them into the UI.

Companion writer (Python, ~1 kHz): [examples/csv_writer.py](examples/csv_writer.py).

### CSV format

The CSV can optionally start with a header line to name the traces:

```
index,timestamp_micros,<trace1>,<trace2>,...
```

Data lines must contain at least three columns:

```
<u64_index>,<i64_timestamp_micros>,<f64_value_for_trace1>[,<f64_value_for_trace2>...]
```

- Empty lines and incomplete lines are ignored.
- If the timestamp cannot be parsed, the current time is used.
- Non-numeric value cells for a column are skipped for that sample.
- If there is no header, columns after the timestamp are auto-named `col1`, `col2`, ...

### Running the demo

In terminal 1, start the 1 kHz CSV writer (creates the file and writes a header if missing):

```bash
python3 examples/csv_writer.py live_data.csv
```

In terminal 2, run the tailing UI (defaults to `live_data.csv` and starts at end of file like `tail -f`):

```bash
cargo run --example csv_tail
```

Options:

- Start from the beginning (read existing content first):

```bash
cargo run --example csv_tail -- --from-start
```

- Specify a different path:

```bash
cargo run --example csv_tail -- /path/to/other.csv
```

### How it works

The example opens or waits for the CSV file, then in a background thread it:

- Polls the file every 20 ms to detect appended bytes.
- Reads new bytes and accumulates into a buffer until newline-delimited lines are complete.
- Parses complete lines only and sends one `MultiSample` per numeric column after the timestamp.
- Detects file truncation/rotation by comparing the current length with the last read position and reopens if needed.

This is useful for integrating `liveplot` into pipelines where data is produced by an external process with minimal coupling.

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
