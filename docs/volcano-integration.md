# VolcanoPlot integration — design

Goal: feature-flagged replacement of `egui_plot` with VolcanoPlot
(C++23/Vulkan, vendored at `vendor/VolcanoPlot`) for scope + FFT plots.

## Investigation result: no shared-GPU path exists

- `bevy_egui` works because bevy's renderer **is** wgpu — egui draws as a
  render pass on the *same* device. That is same-API integration.
- VolcanoPlot is native Vulkan. egui (via eframe) renders with wgpu.
  wgpu has **no public external-memory/texture import API**, so a C++
  Vulkan framebuffer cannot be handed to egui zero-copy. dmabuf export
  exists in Vulkan but nothing in wgpu's stable API consumes it.
- Two present paths on one eframe window (Vulkan swapchain vs wgpu
  swapchain) is not supported by eframe/winit.

=> The only viable integration: **VolcanoPlot headless → RGBA8
   framebuffer → egui texture**, interactions forwarded by mutating the
   plot state and re-rendering.

## VolcanoPlot surfaces that make this easy

- `render::MplCanvas` — records display-space draw ops (`path`, `points`,
  `text`, `image`, `tris`, `instances`) during `beginFrame()` and serves
  an RGBA8 framebuffer on `endFrame()` in headless mode. Pixel space
  top-left Y-down — matches egui.
- `plot::Figure`/`Axes` + `Renderer::renderFrame` — matplotlib-style axes
  with auto ticks/grid/spines/legend, readback to RGBA8 buffer
  (`renderFrame`, "restore, readback" path).

## Plan

1. **`vendor/VolcanoPlot` FFI shim** (changes allowed in submodule):
   `include/volcano/ffi/volcano_ffi.h` + `src/ffi/volcano_ffi.cpp`, C API:
   - `vp_figure_create(w, h)` / `vp_figure_destroy`
   - `vp_set_trace_data(fig, id, xs, ys, n, color_rgba8, width, style)`
   - `vp_remove_trace(fig, id)`, `vp_set_viewport(fig, xmin,xmax,ymin,ymax)`,
     `vp_autoscale(fig)`
   - `vp_draw_hline/vline(fig, value, color, style)` — thresholds/triggers/measurements
   - `vp_render_rgba(fig, out_buf, out_len)` — one frame → buffer
   - `vp_set_font_scale/label/unit` — axis labels from `AxisSettings`
   Built via `add_library(volcano_ffi STATIC)` in the submodule's cmake;
   `build.rs` (feature `volcano`) invokes cmake and links it.

2. **Rust side**:
   - `src/volcano/` module behind `#[cfg(feature = "volcano")]`
   - `VolcanoPlotSurface` — owns figure handle + `egui::TextureHandle`;
     `render(width, height)` uploads framebuffer → texture.
   - Extract a `PlotBackend` abstraction in `scope_ui.rs`: the current
     `Plot::show` body becomes the `EguiPlotBackend`; a
     `VolcanoBackend` pushes series+overlay descriptors through the FFI
     and draws the resulting texture, mapping egui pointer events
     (scroll/drag/box-zoom/click) to `vp_set_viewport` +
     measurement-click callbacks so app behavior is unchanged.
   - Same for `fft_ui.rs` (user asked for everything incl. FFT).
   - Overlays where porting is impractical stay egui-drawn on top of
     the texture (already how the custom legend overlay works).

3. **Build**: `build.rs` gated on `volcano` feature — runs
   `cmake -B build-ffi -DCMAKE_BUILD_TYPE=Release` inside the submodule,
   links `libvolcano_ffi.a` + deps (Vulkan loader at runtime, not linked).
   CI/tests keep default features pure-Rust.

4. **Tests**: reuse the e2e harness — `#[cfg(feature = "volcano")]` tests
   render a scope via `VolcanoPlotSurface` on llvmpipe, assert texture
   non-empty + pixel checks, and run the existing scope tests where the
   abstraction allows.

## Known constraints

- Per-frame readback+upload cost (fine at scope sizes; llvmpipe slower).
- Zoom/pan is re-render driven — latency identical to egui redraw.
- VolcanoPlot axes/ticks inside the texture can't share egui's font/style.
- First milestone: figure renders a sine wave to a texture in-app.

## Build prerequisites (feature `volcano`)

```bash
git submodule update --init --recursive   # VolcanoPlot + nested glyb deps
LIVEPLOT_VOLCANO=1 cargo run --features volcano
```

- **Linux**: cmake + g++ (C++23) + Vulkan runtime (`libvulkan.so.1`).
  Headers/shaderc come from `vendor/vulkan-sdk` + `vendor/Vulkan-Headers`.
  The `glyb` patch (vendored zlib/libpng/freetype/harfbuzz) is applied
  by `build.rs` — no system font/image dev packages needed.
- **Windows**: LunarG Vulkan SDK with `VULKAN_SDK` set; MSVC multi-config
  is handled. `vendor/vulkan-sdk` contents are Linux-only.
- The submodule stays at stock upstream: the FFI shim lives in
  `vendor/volcano-ffi` (compiled via `cc` in `build.rs`); the glyb patch
  in `vendor/volcano-patches` is copied in at build time.
- Per-scope override: `ScopeData::volcano_enabled` (seeded once from
  the env var at construction).
- Test: `cargo test --features volcano --test volcano_ffi --test e2e volcano_scope`
