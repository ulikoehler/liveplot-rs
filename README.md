# liveplot-rs

Live plotting library for timestamped data streams using egui/eframe.

![LivePlot-RS screenshot](docs/liveplot-rs%20screenshot.png)

This crate provides a reusable plotting UI you can feed with a stream of `(timestamp, value)` samples.
gRPC input is provided as an example of how to use the library, not as a built-in dependency.

## Library usage (single trace)

Add `liveplot-rs` as a dependency, send your samples through a standard `std::sync::mpsc::Receiver<Sample>`, and call `liveplot_rs::run(rx)`.

```rust
use std::sync::mpsc;
use liveplot_rs::{Sample, run};

fn main() -> eframe::Result<()> {
	let (tx, rx) = mpsc::channel();
	// In your producer thread/task, send Sample { index, value, timestamp_micros }
	std::thread::spawn(move || {
		// ... produce data and tx.send(sample).ok();
	});
	run(rx)
}
```

	## Multiple traces

	You can stream multiple traces distinguished by a trace name. Create a multi-trace channel with `channel_multi()` and run the UI with `run_multi(rx)`.

	```rust
	use liveplot_rs::{channel_multi, run_multi};

	fn main() -> eframe::Result<()> {
		let (sink, rx) = channel_multi();
		std::thread::spawn(move || {
			// produce data for different traces, e.g., "sine" and "cosine"
			// sink.send_value(index, value, timestamp_micros, "sine").ok();
			// sink.send_value(index, value, timestamp_micros, "cosine").ok();
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
