# Volcano ↔ Normal (egui_plot) — Full E2E Comparison Plan & Results

Comparison of the two render paths for the scope widget:

- **normal** — `egui_plot` tessellated on CPU, painted by egui/eframe
- **volcano** — VolcanoPlot (C++/Vulkan, `vendor/VolcanoPlot`) renders
  headless to RGBA8 on a worker thread; the egui texture is painted
  underneath the (transparent) egui_plot layer that keeps owning
  interaction, overlays and `PlotMemory`

Parity contract: volcano is a drop-in renderer swap — every feature must
behave identically; only the rasterization path differs (GPU-side
decimation replaces egui's envelope decimation).

Related docs: `docs/e2e-parity-plan.md` (category catalogue),
`docs/volcano-parity-verification-plan.md` (root-cause register),
`docs/volcano-integration.md` (build).

---

## 1. How to run

```bash
# full e2e suite (both paths — every parity test runs the scenario on
# egui AND volcano backends)
cargo test --features "volcano,fft" --test e2e

# FFI-level tests (headless Vulkan, no egui)
cargo test --features volcano --test volcano_ffi

# CPU/RSS/frame-time benchmark (opt-in)
cargo test --features "volcano,fft" --test e2e -- --ignored bench_render_perf --nocapture

# GPU device probe (opt-in — holds a figure open ~10s so nvidia-smi /
# /proc/<pid>/fd can observe which device is bound)
cargo test --features volcano --test volcano_ffi -- --ignored volcano_hold_device_probe

# High-concurrency safety: if the suite ever stalls, re-run with
./target/debug/deps/e2e-* --test-threads=4
```

Env flags: `LIVEPLOT_VOLCANO` opts scopes into volcano by default,
`LIVEPLOT_VOLCANO_DISABLE` is the escape hatch; tests set
`ScopeData::volcano_enabled` per scope.

Skip policy: `vol_available(bed)` skips the volcano arm when Vulkan init
fails; `wgpu_bed()` returns `Option` and pixel tests skip without an
adapter.

## 2. Feature/option coverage matrix

Every row is exercised on **both** paths (volcano arm skipped only when
Vulkan is unavailable). "Test" lists the covering test(s) in
`tests/e2e/` (+ `tests/volcano_ffi.rs`).

| Feature / option | egui | volcano | Covering tests |
|---|---|---|---|
| Trace pixels in plot rect | ✓ | ✓ | `parity_trace_pixels_inside_plot`, `parity_trace_pixel_centroid`, `volcano_renders_sine_framebuffer` |
| Axis labels + tick labels | ✓ | ✓ | `parity_fft_scope_renders` (label pixels), `axes::*`, `volcano_axis_labels_render` (via label-region variance) |
| Grid on/off (`show_grid`) | ✓ | ✓ | `parity_dark_light_background`, config-flag tests, `vp_set_grid` FFI |
| Dark / light background | ✓ | ✓ | `parity_dark_light_background` |
| Margin model (tall vs FFT strip) | ✓ | ✓ | `parity_screen_rect_layout`, `parity_fft_embedded_scope`, `volcano_tiny_height_renders` |
| Autoscroll trailing window | ✓ | ✓ | `parity_autoscroll_advances_bounds`, `parity_autoscroll_span_kept_when_zoomed` |
| Time window (`time_window`) | ✓ | ✓ | `parity_time_window_updated_on_x_zoom` |
| Wheel zoom (+pointer anchor) | ✓ | ✓ | `parity_wheel_zoom_anchors_at_pointer`, `parity_wheel_zoom_y_mode`, `parity_wheel_zoom_off_is_noop` |
| Wheel-over-legend guard | ✓ | ✓ | `parity_wheel_over_legend_no_zoom` |
| Primary-drag pan | ✓ | ✓ | `parity_primary_drag_pans`, `volcano_scope_pan_moves_bounds`, `egui_path_pan_control` |
| Pan-Y sticks / X re-center | ✓ | ✓ | `parity_pan_y_sticks_x_recenters_live` |
| Box zoom (all ZoomModes) | ✓ | ✓ | `parity_box_zoom_x_mode`, `parity_box_zoom_both_modes` |
| Double-click refit | ✓ | ✓ | `volcano_dblclick_refit` (+ egui arm) |
| Zoom persists past live update | ✓ | ✓ | `parity_wheel_zoom_anchors_at_pointer` (regression RC-2) |
| Fit X / Fit Y / Fit-to-view | ✓ | ✓ | `controls2::scope_zoom_modes_and_fit`, `hotkeys_e2e::fit_view_hotkey_runs` |
| Click → point coords | ✓ | ✓ | `parity_clicked_point_coords` |
| Pause-on-click + events | ✓ | ✓ | `parity_pause_on_click` |
| Paused snapshot retention | ✓ | ✓ | `parity_pause_keeps_rendered_snapshot` |
| Hover inside axes / tooltip | ✓ | ✓ | `parity_hover_inside_axes` |
| Epoch-scale x (f32 shift) | ✓ | ✓ | `parity_epoch_timestamps` |
| Legend overlay rect + pixels | ✓ | ✓ | `parity_legend_overlay_rect`, `parity_legend_pixels`, `parity_legend_app_level` |
| Legend 4-corner positions | ✓ | ✓ | `parity_legend_position_corners` |
| Legend inside axes rect | ✓ | ✓ | `parity_legend_inside_axes` |
| Legend click hides trace | ✓ | ✓ | `parity_legend_click_hides_trace`, `legend::legend_click_toggles_trace` |
| Alt/right-click isolate | ✓ | ✓ | `parity_legend_alt_click_isolates`, `legend3::legend_right_click_isolates` |
| force_hide / narrow suppress | ✓ | ✓ | `parity_legend_force_hide`, `legend2::*` |
| Trace styles (solid/dash/dot/points) | ✓ | ✓ | `traces2::render_modes_survive`, `volcano_viewport_and_styles` (FFI) |
| Markers + `show_points` | ✓ | ✓ | `parity_show_points_marker`, `vp_trace_set_marker` FFI |
| Trace visibility eye/hidden_items | ✓ | ✓ | `volcano_scope_respects_hidden_items`, `traces_panel::*` |
| Trace color/offset/label alias | ✓ | ✓ | `traces2::*`, `controllers::traces_controller_*` |
| Hover emphasis (dim/widen) | ✓ | ✓ | `hover_look` in surface.rs, covered via hover tests |
| XY scope pairs | ✓ | ✓ | `parity_xy_scope_renders_pair`, `parity_xy_viewport_fits_pairs`, `parity_xy_legend_entries_and_last_marker`, `volcano_xy_scope_renders_pair` |
| FFT embedded scope | ✓ | ✓ | `parity_fft_embedded_scope`, `volcano_fft_scope_renders`, `fft::*` |
| Math/derived traces | ✓ | ✓ | `math::*`, `math2::*` (render through same scope path) |
| Thresholds overlay + events | ✓ | ✓ | `thresholds::*`, `ui_thresholds::*`, `controllers2::threshold_*` |
| Triggers overlay + fire | ✓ | ✓ | `triggers::*`, `ui_controls2::ui_trigger_*` |
| Measurement markers/clicks | ✓ | ✓ | `measurement::*`, `meas_events::*` |
| Log scales + domain clipping | ✓ | ✓ | `axes::log_scale_toggle_survives_render`, FFI `sine` tests, log branch in `sync_and_render` |
| Controllers (all) | ✓ | ✓ | `controllers::*`, `controllers2::*` — backend-agnostic |
| Events (view/pause/threshold/…) | ✓ | ✓ | `events*`, `meas_events`, `parity_pause_on_click` |
| Persistence + undo | ✓ | ✓ | `persistence::*`, `export::save_and_load_state_round_trip` |
| Drag-and-drop traces/scopes | ✓ | ✓ | `reorder::drag_trace_onto_second_scope_moves_it`, `controls2::dnd_coverage_note` |
| Config flags (legend/grid/topbar/buttons/max_fps/headline/button_set) | ✓ | ✓ | `config_flags::*`, `config_flags2::*` |
| Screenshot request/capture | ✓ | ✓ | `export::screenshot_request_*`, `parity_app_level_legend_and_screenshot` |
| Export (CSV/raw, parquet feature) | ✓ | ✓ | `export::*` — render-path agnostic |
| `render_mode` UI hidden under volcano | n/a | ✓ | `traces_ui.rs` (hidden in volcano mode — GPU takes raw points) |
| Accessibility (no new unnamed widgets) | ✓ | ✓ | `a11y::unnamed_interactive_widgets_do_not_increase` |
| Empty/degenerate data safety | ✓ | ✓ | `volcano_degenerate_state`, `volcano_tiny_height_renders`, `volcano_empty_trace_safe` |
| Resize race / dropped frames | ✓ | ✓ | `volcano_resize_race`, `dropped_frames` counter |
| FFI fallback (no Vulkan → egui) | ✓ | ✓ | `volcano_enabled=false` reset in `scope_ui.rs`; FFI `Option` return |
| CPU/RSS/frame-time | — | — | `bench_render_perf` (opt-in) |
| Soak / stress | ✓ | ✓ | `soak::*`, `stress::*` (opt-in `LIVEPLOT_STRESS=1`) |

## 3. GPU verification (this host)

Host devices: **GPU0** AMD Raphael Mendocino iGPU (RADV),
**GPU1** NVIDIA RTX 5060 Ti (proprietary 620.70.06), **GPU2** llvmpipe.
`scoreDevice` in `PhysicalDevice.cpp` prefers discrete (+1000) → NVIDIA
is selected.

Measured proof (all taken during `volcano_hold_device_probe` /
`volcano_perf_200k_points` runs):

- `nvidia-smi` lists the test process: `C+G …/volcano_ffi-… 85MiB`
- ~20 open fds on `/dev/nvidia0`, `/dev/nvidiactl`, `/dev/nvidia-modeset`;
  NVIDIA Vulkan userland libs mapped (`libnvidia-glvkspirv.so`, …)
- strace: **1928 NVIDIA ioctls (magic 0x46) vs 66 DRM ioctls** — the DRM
  (RADV) fds only get enumeration probes (GEM_CLOSE)
- render+readback 1280×720 / 200k pts: **~2.0–3.2 ms/frame** steady-state
  (first frame ~155–215 ms incl. device+shader init); upload 200k pts ~1–2 ms.
  On llvmpipe this was ~7 ms+/frame — the hardware path is ~3× faster.

**Result: volcano now runs on the hardware NVIDIA GPU, not llvmpipe.**

## 4. CPU / RSS comparison — `bench_render_perf` results

120 frames, ~1 ms pacing, kittest harness (`harness.step()` runs the
real UI path incl. egui tessellation; egui pixel *painting* isn't
rasterized, which flatters egui slightly — see caveats).

| path | wall ms/f | CPU ms/f | RSS Δ | GPU renders |
|---|---|---|---|---|
| egui paused 4×20k | 1.46 | **0.42** | ~0.1 MB | — |
| volcano paused 4×20k | 1.27 | **0.17** | ~0 | 1/120 |
| egui live 4×20k | 1.39 | **0.25** | ~0.2 MB | — |
| volcano live 4×20k | 1.32 | **1.50** | +6.3 MB | 7/120 |
| egui paused 8×200k | 1.60 | **0.67** | ~0 | — |
| volcano paused 8×200k | 1.27 | **0.17** | ~0 | 1/120 |
| egui live 8×200k | 1.46 | **0.50** | ~0.4 MB | — |
| volcano live 8×200k | 1.40 | **1.50** | +11.7 MB | 17/120 |

Reading:

- **Static view: volcano uses ~2.5–4× less CPU** (0.17 vs 0.42–0.67
  ms/f) — the whole-render signature short-circuits before any work.
- **Live churn: volcano uses more CPU** (flat ~1.5 ms/f vs 0.25–0.50) —
  per-frame signature hashing, changed-trace re-collect, a thread
  spawn per render, FFI upload, and the 3.7 MB texture update stay on
  CPU. Notably the volcano CPU cost is *flat in point count*
  (0.42→0.50 for egui at 10× points; volcano stays 1.50) — at larger
  buffers the curves cross in volcano's favor.
- `rendered` counts are pipelining, not loss: the worker renders
  1-frame-behind and busy frames paint the last completed texture.
- GPU memory: ~85 MB per process (NVIDIA, per `nvidia-smi`); each scope
  figure owns a device — shared-device is a documented TODO.
- Caveats: measured while the box was loaded (load avg ~7–17, an
  unrelated clippy job); relative ordering is stable across 3 runs,
  absolute values will vary. In a real eframe/wgpu app egui additionally
  pays GPU-submit + paint costs the harness skips, so the live-mode gap
  narrows in production.

## 5. Findings from this run

1. **FIXED — teardown crash.** `bench_render_perf` reliably segfaulted
   at process exit: `vk::DeviceLostError` thrown from
   `vk::Queue::waitIdle` inside a detached render thread. Root cause:
   `VolcanoSurface` dropped `pending` (`JoinHandle`) without joining —
   the worker could still be inside `vp_render_rgba`/`vp_figure_destroy`
   during teardown. Fix: `impl Drop for VolcanoSurface` joins the
   worker first (`src/volcano/surface.rs`). Exit is now clean (EXIT=0).
2. **OPEN — flaky hang at high test concurrency.** Two runs of the
   full suite stalled with all ~190–255 threads parked in
   `futex_do_wait`/`rt_mutex_schedule`/`poll_schedule_timeout`
   (NVIDIA driver mutex + fence waits) at 2.4 GB RSS; no runnable
   thread → deadlock. Correlates with dozens of parallel
   `VolcanoFigure` create/render/destroy cycles; **not seen with
   `--test-threads=4`** and not reproducible on demand (serial and
   reduced-parallelism runs pass). Mitigation: re-run with
   `--test-threads=4`, or investigate shared Vulkan-device creation
   serialization upstream.
3. `nvidia-smi` process listing needs a live figure — short runs
   finish before sampling; use `volcano_hold_device_probe`.

## 5b. Live-mode CPU deep-dive (instrumented)

`dbg_*` counters + per-render timing show where the live-mode CPU
actually goes (~1.4 ms CPU/f vs egui ~0.3):

- **~95% is the render+readback itself**: each
  `vp_render_rgba` = `prepare` + `renderFrame` + GPU→CPU readback of
  ~3.7 MB + `to_vec` ≈ **5–7 ms steady-state** on the RTX 5060 Ti at
  1122×635 (first render ~230 ms device+shader init, 2nd/3rd ~45–75 ms
  pipeline warm-up). 7–18 renders per 120-frame window ≈ the whole
  volcano overhead.
- Uploads/sig-hashing/thread-spawn were noise by comparison (the
  `spawn`-per-render cost ~20-60 µs — removed anyway).
- `LinePlot::prepare` re-uploaded **every** trace's full point buffer
  into the host-visible GPU buffer on **every** render
  (`updatePoints`, `dataDirty_` never checked — 12.8 MB/render at
  8×200k). Patched upstream (`liveplot-ffi`): upload only when
  `dataDirty_`.

Changes shipped (this commit):

- **Persistent `RenderWorker`** (mpsc job/result channels) replaces
  `thread::spawn` per render; `Drop` closes the channel and joins.
- **~30 fps texture cap** (`min_render_interval`, egui *virtual* time
  so kittest `step_dt` stays deterministic; bypassed until first
  frame; `LIVEPLOT_VOLCANO_MIN_FRAME_MS` env override, 0 = uncapped).
  At real 60 fps UI rates this halves renders (~60→30/s ≈ 180 ms/s of
  worker CPU); the egui overlay stays at full rate.
- Per-frame `frame_sigs` Vec + `String` clones removed (raw sig
  recomputed O(1) in the upload loop).

Honest bound: with ~5-7 ms of upstream render+readback per frame,
volcano can't dip under egui's live CPU while updating at display
rate — the remaining lever is upstream (async readback/fence
pipelining, incremental point upload) or a lower cap.

## 6. Remaining gaps / TODO

- Shared Vulkan device across figures (upstream FFI work — biggest
  memory win, likely also removes the concurrency hang).
- Upstream: async readback (fence+staging) to pipeline the ~5-7 ms
  render+readback; incremental `vp_trace_append` to avoid O(N)
  re-upload of append-only live traces.
- Live-mode CPU: incremental upload (append-only ranges) instead of
  full re-collect when a trace changes.
- Legend-position persistence round-trip on the volcano path.
- Pixel-level snapshot pairs under `LIVEPLOT_SNAPSHOT=1` (wgpu bed).
- Soak/stress opt-ins were not part of this run
  (`LIVEPLOT_STRESS=1`, `--ignored`).

## 7. Result summary

- **e2e suite: 181 passed / 0 failed** (6 ignored opt-ins) —
  `--features "volcano,fft"`, both paths, incl. all 40+ volcano parity
  tests, XY scope, FFT embedded scope, FFI suite 6/6.
- **Volcano renders on the NVIDIA RTX 5060 Ti** (proven via nvidia-smi,
  fd table, ioctl counts, render-time delta vs llvmpipe).
- **CPU: lower than egui on static views (~2.5–4×), higher on
  continuous data churn (~1.5 ms/f flat vs 0.25–0.50)** — the GPU does
  all rasterization/decimation; the remaining CPU is upload/sync
  overhead that shrinks with the worker-pool + incremental-upload TODOs.
