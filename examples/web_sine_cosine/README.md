# web_sine_cosine

This example demonstrates running a simple sine/cosine live plot in the browser using
`eframe`/`egui` with WebAssembly. It is structured as a self-contained crate under
`examples/web_sine_cosine` so that it can be built both natively and for the web.

## Overview

- The application (`src/main.rs`) maintains two rolling buffers of points for sine and
  cosine signals.  A `Plot` from `egui_plot` updates every frame.
- Conditional compilation distinguishes between the native and `wasm32` entry
  points.  The WASM path uses `eframe::WebRunner` and a `<canvas>` element with the
  id `the_canvas_id`.

## Building & running

### Native

```bash
cd examples/web_sine_cosine
cargo run
```

### Web (WASM)

Requires [Trunk](https://trunkrs.dev/).  Install with

```sh
cargo install trunk
```

Then serve the app:

```bash
cd examples/web_sine_cosine
trunk serve
```

The example listens on port `8080` by default and will open `http://localhost:8080`
if your browser is configured to auto-open from Trunk.  The generated `index.html`
contains a full-window `<canvas>` and the JS glue inserted by Trunk loads the
compiled WASM module.

### Notes

- `Cargo.toml` in the example overrides eframe dependencies to use the `wgpu`
  feature, which is compatible with the project’s main crate and works on all
  targets including `wasm32-unknown-unknown`.
- The `Trunk.toml` file configures port and other server options.
- The example can be built for release with `trunk build --release`.

Feel free to inspect the source and adapt it for your own web-enabled
`egui` applications.