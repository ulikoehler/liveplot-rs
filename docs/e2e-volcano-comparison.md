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

Final uncapped run — **no FPS cap, identical pacing for both paths**
(per user requirement: any throttling invalidates the comparison):

| path | wall ms/f | CPU ms/f | RSS Δ | GPU renders |
|---|---|---|---|---|
| egui paused 4×20k | 1.46 | **0.42** | ~0.1 MB | — |
| volcano paused 4×20k | 1.27 | **0.25** | ~0 | 1/120 |
| egui live 4×20k | 1.40 | **0.42** | ~0 | — |
| volcano live 4×20k | 1.28 | **1.17** | +2.8 MB | 11/120 |
| egui paused 8×200k | 1.60 | **0.50** | ~0 | — |
| volcano paused 8×200k | 1.27 | **0.17** | ~0 | 1/120 |
| egui live 8×200k | 1.48 | **0.42** | ~0.4 MB | — |
| volcano live 8×200k | 1.30 | **0.67** | +3.4 MB | 24/120 |

All run with the upstream fixes in §5b, the f64-upload /
device-transform round (§5c), and the command-recording round (§5d).
History: volcano live was 1.50 at first measurement, 1.25–1.42 after
the §5b fixes, 0.83/1.33 after §5c, now **0.67 at 8×200k / 1.17 at
4×20k**. Paused stays ~0.17–0.25.

Reading:

- **Static view: volcano uses ~2–3× less CPU** (0.17–0.25 vs
  0.42–0.50 ms/f) — the whole-render signature short-circuits before
  any work.
- **Live churn: volcano still uses more CPU than egui** (0.67–1.17 vs
  0.42) but the gap narrowed ~2.2×: the GPU-resident transform +
  tessellation removed the per-trace O(N) host pass, and the §5d round
  cut per-trace barriers + the ColorImage conversion (steady-state
  `vp_render_rgba_view` ~1.4 ms wall; the worker thread is ~all
  command recording + submit — GPU waits cost ~0 CPU).
- `rendered` counts are pipelining, not loss: the worker renders
  1-frame-behind and busy frames paint the last completed texture —
  same cadence as egui, just fewer *unique* frames when the worker is
  saturated.
- GPU memory: ~85 MB per process (NVIDIA, per `nvidia-smi`); each scope
  figure owns a device — shared-device is a documented TODO. The f64
  upload path adds one raw `dvec2` buffer per trace (2× the f32 point
  buffer until the GPU narrow runs).
- Caveats: measured while the box was loaded (load avg ~7–17);
  relative ordering is stable across runs, absolute values will vary.
  In a real eframe/wgpu app egui additionally pays GPU-submit + paint
  costs the harness skips, so the live-mode gap narrows in production.

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

Phase-by-phase instrumentation (`prepare`/`renderFrame`/`readback`,
pre-pass timing, per-thread CPU via `/proc/thread-self/stat`) showed
where the ~1.3–1.4 ms CPU/f went, and each identified hotspot was
fixed:

1. **Per-frame readback allocation + double submit** —
   `vp_render_rgba` ran `renderFrame` (with its own sync), then
   `readbackRgba8` VMA-allocated a fresh 3.7 MB staging buffer,
   allocated a one-time command buffer, did a *second* submit +
   `queue.waitIdle`, then memcpy'd staging → vector → FFI buffer →
   Rust `to_vec`. Now: the image→staging copy is recorded in the
   *same* command buffer as the render (one submit, one fence), the
   staging buffer + fence are persistent (`HeadlessBackend`), and
   `readbackRgba8Into` memcpy's once into the caller's recycled
   buffer (`render_rgba_into` + `pixel_scratch` in Rust).
   Result: `readback` phase 1 ms → **0.1 ms**.
2. **Spin-waits burning CPU** — `endFrame`'s `waitForFences`,
   `VulkanGpuServices::submitPrePass`'s `queue.waitIdle`, and
   `OneTimeCommands`'s `queue.waitIdle` all spun in the driver while
   the GPU ran (~4.5 ms compute pre-pass). All three now poll
   `getFenceStatus` with a 100 µs sleep — same synchronous semantics,
   measured ~**0 ms thread-CPU** per render (was the dominant CPU
   sink).
3. **Per-trace GPU→CPU round-trips** — for traces > 50k points,
   `LinePlot::preDraw` ran `envelopeColumns` (compute min/max
   reduction) **per trace per frame**, each call allocating an output
   buffer + descriptor pool + `OneTimeCommands` submit/wait, then
   copying all columns back to host vectors. `renderFrame` measured
   ~5.2 ms, of which ~3.5 ms was this pre-pass — mostly CPU. The
   FFI now sets `OffloadConfig.envelope = Cpu` so large traces take
   the fully GPU-resident `transformPoints` + `tessellateDevice`
   path instead (same all-points stroking semantics as egui — no
   host round-trip). `preDraw` now also honors the existing
   `OffloadConfig.envelope`/`xform` switches (previously ignored on
   the native path). Result: `renderFrame` 5.2 → **3.3 ms**, pre-pass
   3.5 → 1.8 ms.
4. **Per-frame full point re-upload** — `LinePlot::prepare` memcpy'd
   every trace's full buffer into the host-visible GPU buffer every
   render (12.8 MB/render at 8×200k) despite `dataDirty_`. Now gated.
   (VolcanoPlot `liveplot-ffi` commit 5d63520.)
5. **Rust-side overhead** — `thread::spawn` per render → persistent
   `RenderWorker` (mpsc job/result channels; `Drop` joins);
   per-frame `frame_sigs` Vec + `String` clones removed; pixel
   buffer recycled through the channel instead of `to_vec`.

**No FPS cap**: an earlier ~30 fps texture-update cap was removed —
it made the egui-vs-volcano comparison invalid (different effective
rates). Both paths now update at identical pacing.

### 5c. f64 GPU upload + device-resident transform (this round)

Two further bottlenecks were removed — both verified with in-worker
timing before/after:

1. **f64 upload with GPU narrowing.** The FFI handed `&[[f64; 2]]`
   through three O(N) host passes: de-interleave into `td.x`/`td.y`
   (f32), re-interleave into `series_.points`, then memcpy into the
   GPU buffer. Now `vp_trace_set` memcpy's the raw interleaved dvec2
   data once into a recycled `rawF64_` vector, `updatePointsF64`
   memcpy's it once into a host-visible f64 storage buffer, and a
   `GL_ARB_gpu_shader_fp64` compute pass narrows it into the
   existing f32 `pointBuffer_` — zero host conversion.
   `shaderFloat64` is enabled conditionally at device creation
   (supported on all enumerated devices here: NVIDIA, RADV, llvmpipe);
   on devices without it the pipeline fails lazily and the plot pins
   permanently to the f32 path (`f64Failed_`). `series_.points` is
   now materialized lazily via `ensureCpuPoints()` — only traces that
   actually need CPU data (markers, picking, CPU fallback paths)
   pay the f64→f32 conversion. Note: this moves *where* the
   narrowing runs, not the precision — rendered coordinates are still
   f32; a full-f64 pipeline is a different (much larger) change.
2. **Device-resident transform + tessellation.** `GpuLineRendererVk`
   had never implemented `transformPoints`/`tessellateDevice` (they
   existed only on the WebGPU op-stream), so every oversampled trace
   ran `pixelPoints()` + a full point upload on the host per frame —
   the dominant ~2 ms/render. The Vulkan impl now compiles a compute
   pass from the shared `TransformGlsl` scale functions (same
   `u_viewMinSpan`/`u_rect`/scale-code convention as the vertex
   path, same `maskPointsForScales` NaN semantics), writes pixel-space
   vec2s to a shared device buffer, and strokes them with a second
   descriptor set on the existing tessellate pipeline — no host pass
   at all. WAR/RAW barriers serialize the shared buffer between
   traces; descriptor sets are cached per (src,dst) pair.
3. **Zero-copy readback view + worker-side `ColorImage`.**
   `IBackend::rgba8View` exposes the persistent staging buffer through
   `vp_render_rgba_view`; the worker thread builds the `ColorImage`
   directly from it — removes the staging→Vec memcpy *and* moves the
   u8→Color32 conversion off the UI thread (fallback: `render_rgba_into`
   when a backend has no view).

Effect: steady-state `vp_render_rgba_view` wall ~3.3 ms → ~1.6 ms;
volcano live 8×200k CPU 1.25 → **0.83 ms/f**; worker CPU per render
~5.5 → ~2.6 ms.

### 5d. Command-recording reduction round

Phase timing inside `renderFrameSubset` showed the draw pass is
negligible (**0.07 ms/f**) — prepass (0.86) and `endFrame`
submit/fence (0.47) dominated. That killed the static-layer
command-buffer-cache idea and focused the round on the compute prepass:

- **Fused f64 narrow into the transform** (`transformPointsF64`): a
  `dvec2`-sourcing variant of the data→pixel shader narrows inline
  (`vec2(dvec2)` keeps the CPU path's rounding), removing one
  dispatch + barrier + descriptor bind per trace per frame. The f32
  `pointBuffer_` now only materializes when a consumer actually reads
  it.
- **xform arena**: transform outputs bump-allocate disjoint regions of
  a shared device buffer (`xformBuf_` + `outBase` push constant), so
  the per-trace WAR barrier on the previously shared buffer is gone;
  `tessellateDevice` takes an `inBase` offset.
- **Merged submissions**: the compute prepass records into the frame
  command buffer before `beginRenderPass()` (new `IBackend` hook) —
  one `vkQueueSubmit` + fence wait per frame instead of two.
- **Batched vertex barriers**: `GpuLineRenderer::endPreDraw()` records
  a single compute→vertex-input barrier after the preDraw loop instead
  of one per `tessellateDevice` (the host-upload `tessellateChunk`
  path keeps its own — `MplCanvas` drives it from a separate pre-pass
  command buffer).
- **`ColorImage` via memcpy** (Rust): `Color32` is `repr(C) [u8;4]` in
  RGBA order — identical layout to the readback view, so a flat copy
  replaces `from_rgba_unmultiplied`'s per-pixel map (~0.64 → ~0.09 ms
  on the worker).

Effect: `vp_render_rgba_view` wall ~1.6 → ~1.4 ms
(prep ~0.28 + renderFrame ~1.18 + view ~0); live 8×200k CPU 0.83 →
**0.67 ms/f**, live 4×20k 1.33 → **1.17 ms/f**.

`vp_trace_append` was evaluated and deferred: the live data source is
a bounded `VecDeque` sliding window — front-trim makes cheap
prefix-anchored append detection unreliable, and a GPU ring buffer
(head-offset shader reads + linearization for CPU consumers) is a much
larger change for the ~0.3 ms it would save. Documented as a TODO.

### 5e. Persistent per-trace mesh cache (pan-reuse)

preDraw used to re-record transform + tessellate dispatches for *every*
trace every render, while live figures typically churn only one.
`LinePlot` now owns a persistent region inside `outBuf_`
(`[0, persistOff_)`, ahead of the per-frame scratch arena) and caches
the tessellated `Mesh` against a key covering every input: data
version, transform view/scales, axes rect, stroke params, color.

- **Identical key** → the cached mesh is re-stamped, zero dispatches.
- **Pure viewport pan** (viewMin moved, everything else equal — the
  dominant live-scroll case): the mesh is baked for all N points, so
  the change is an exact rigid pixel translate. Reused via a new
  `u_off` push-constant term in the spine fill vertex shader
  (`drawTrianglesGpu`'s `offset` arg, default 0).
- **Anything else** → re-tessellate into the plot's persistent region.

`outBuf_` regrow bumps `gpuLine().meshEpoch()` so stale buffer handles
can never be drawn; destroyed stores return regions to a free list.
Measured in the bench: `pan=160 hit=11 miss=71` — all 7 unchanged
traces reuse every render; only the churned trace re-tessellates.
The bench's per-frame CPU stayed ~flat (~0.7 ms/f) because its
remaining cost is submit + fence + driver + memcpy paths, not dispatch
recording — the win is real GPU+recording headroom that matters more
at higher render rates and trace counts.

Remaining live-mode cost (~0.67–1.17 ms/f, still above egui's ~0.42
at these trace sizes):

- Per-render command recording + submit + fence wait (~1.2 ms wall in
  `renderFrame`; GPU-side waits sleep-poll at ~0 CPU — the rest is
  vkCmd*/VMA/descriptor work).
- `vp_trace_set` memcpy into `rawF64_` + `updatePointsF64` memcpy into
  the f64 GPU buffer per changed trace (~0.3 ms per 200k-point trace,
  split UI/worker).
- `ColorImage` memcpy (~0.09 ms) + egui texture `set_partial` on the
  UI thread.

Getting under egui needs bigger restructures upstream: deferred
readback (present frame N-1 while N renders — throughput, not CPU)
or shared-device contexts.

### 5f. Ring-buffered sliding-window upload + phase-grouped barriers

Two of the remaining levers landed this round:

- **`vp_trace_slide` (ring upload)** — the Rust side tracks
  `live_pushes`/`live_pops` counters on the deque store, so each sync
  gets the exact dropped-front count `d` and tail length in O(1)
  instead of anchor scanning (the old heuristic failed whenever the
  window slid past a full buffer between syncs, which happens at high
  feed rates). `LinePlot::slideDataF64` advances `rawFront_` (amortized
  prefix compaction when dead storage crosses a threshold), appends the
  transformed tail, and `LineRendererVk::updatePointsF64Append` writes
  the tail into the f64 ring at `(head + count) % cap` in at most two
  memcpy segments — no full re-upload. GPU shaders index ring data as
  `(srcHead + i) % srcCap`; `srcCap >= point_count` is the readback
  contract. Empty tails (`tail_n == 0`) still flow through so
  style-only updates aren't dropped. Bench: 22/30 live uploads take the
  slide path; the rest fall back to full upload when the window can't
  be proven contiguous.
- **Phase-grouped barriers** — `preDraw` now records transforms with a
  deferred barrier, `GpuLineRendererVk::endTransforms` emits one shared
  compute→compute barrier for all traces, and `preDraw2` records the
  dependent tessellation (stashed in `LinePlot::PendingTess` keyed by
  frame seq). N+1 per-trace RAW barriers collapse to one per frame;
  the final compute→vertex visibility barrier is unchanged.

## 6. Remaining gaps / TODO

- Shared Vulkan device across figures (upstream FFI work — biggest
  memory win, likely also removes the concurrency hang;
  `sharedGpuContext()` exists upstream but needs queue/pool
  synchronization across worker threads).
- Upstream: deferred/pipelined readback (present frame N-1 while N
  renders); GPU-resident `envelopeColumns` decimation without host
  readback (needs a device-visible envelope output feeding the
  tessellator + a stroker that tolerates 2-vert columns).
- ~~GPU-ring `vp_trace_append` for sliding-window live traces~~ —
  landed as `vp_trace_slide` in §5f.
- ~~Cached command buffers for the static layer~~ — evaluated in §5d:
  draw-call recording is ~0.07 ms/f, not worth the complexity.
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
- **CPU: lower than egui on static views (~2–3×), higher on
  continuous data churn (~0.67–1.17 ms/f vs 0.42)** — fair,
  uncapped, identical pacing for both paths. GPU waits cost ~0 CPU;
  the remainder is command recording/submit, one f64 upload memcpy
  per changed trace, and the worker-side `ColorImage` memcpy +
  texture upload per rendered frame (§5c/§5d). Volcano's live cost
  is *flat in point count* while egui grows with data — the curves
  cross at larger buffers.
- **f64 data path**: traces upload as raw interleaved `dvec2` and are
  narrowed to f32 on the GPU (`shaderFloat64`-gated, CPU fallback);
  CPU consumers materialize `series_.points` lazily.
