# Volcano ↔ egui_plot Parity E2E Test Plan

Full behavioral + visual comparison of the `egui_plot` render path
(`volcano_enabled = false`) and the VolcanoPlot render path
(`volcano_enabled = true`). Every scenario runs **the same scope
state through both backends** and asserts the observable outcome is
equivalent. The goal: VolcanoPlot is a drop-in renderer swap, not a
feature change.

## 1. Harness Infrastructure

Existing building blocks (`tests/e2e/harness.rs`,
`tests/e2e/volcano_parity.rs`):

- `make_test_bed()` — `LivePlotPanel` inside `egui_kittest::Harness`
  (software rasterizer; `harness.render() -> RgbaImage` available).
- `make_app_bed()` — the real `LivePlotApp` (eframe `App`), closest
  to the shipped binary — use for anything touching app-level state
  (controllers, persistence, screenshot capture, hotkeys).
- `wgpu_bed()` — wgpu-backed harness; `harness.render()` returns the
  GPU framebuffer. Used for pixel-content assertions; **skips
  gracefully when no adapter exists** (`LIVEPLOT_SNAPSHOT` optional).
- `feed_sine(&bed, name, n, freq, fs, t0)` — deterministic data feed
  via `PlotSink` — the same channel the live app uses.
- `steps(&mut bed, n)` / `drag(&mut bed, from, to, button)` /
  `input_mut().events.push(Event::MouseWheel|Key|Pointer*)` — input
  injection.
- `scope(&bed, i)` / `scope_mut(&mut bed, i)` — ScopeData access.
- `find_panel::<T>(bed)` / `find_panel_mut::<T>(bed)` — side panels.
- `ScopeData` observables — `last_plot_screen_rect`, `last_plot_bounds`,
  `rendered_this_frame`, `legend_overlay_rect`, `volcano_surface`.

### Parity pattern

Every test runs the scenario twice:

```rust
for vol in [false, true] {
    let mut bed = make_test_bed();
    feed_sine(&bed, "sig", 2000, 1.0, 100.0, 0.0);
    scope_mut(&mut bed, 0).volcano_enabled = vol;
    steps(&mut bed, 20);
    // assert identical observable outcome for both
}
```

Assertions compare **what the user sees / the app reports**, not
render internals: bounds tuples, overlay rects, `rendered_this_frame`,
pixel content inside `last_plot_screen_rect`, `legend_overlay_rect`,
controller/event outputs, persistence round-trips.

### Pixel checks

`harness.render()` → `RgbaImage`. Two checks that have proven
decisive:

- **Colored-pixel count** in the plot rect (grid/labels are
  grayscale; traces are colored) — catches blank/invisible traces.
- **Label-region pixel variance** — counts distinct colors in the
  margin / legend rect — catches missing text, missing legend.

For VolcanoPlot-specific visuals (glyph ticks, muted grid, face
color) snapshot pairs live under `tests/e2e/snapshots/` with
`LIVEPLOT_SNAPSHOT=1` to regenerate reference PNGs.

---

## 2. Scope Rendering

### 2.1 Time scope
- renders traces with visible colored pixels in the plot rect (both)
- axes: x label "Time" + tick labels present (pixel-variance in
  margins), y label present
- grid: muted but visible — grid-line pixels inside axes
- margins: outer margin matches `panel_fill` (not a white/black box)
- epoch-magnitude x (~1.79e9): f32-translate keeps traces visible
  (regression: `volcano_epoch_data_renders`)

### 2.2 XY scope
- `xy_pairs` renders a joined curve (both)
- pair colors/styles match `pair_look`
- viewport covers both series (fit or manual)
- zoom on XY separates the joined traces (regression: the
  "spread over whole y" bug)

### 2.3 FFT embedded scope
- spectrum renders through the same path (`volcano_fft_scope_renders`)
- axis label "Frequency"/"Magnitude" render as text (not symbols —
  the dangling-CString regression)
- legend independent of main scope's legend (unique plot_id)
- short strip gets the reduced margins (52/10/34/6 px)

### 2.4 Log scales
- `x_axis.log_scale`/`y_axis.log_scale` transform applies (both)
- domain-clipped values (≤0) don't crash either path

---

## 3. Legend

- present when `show_legend && !force_hide_legend && entries` (both)
- `legend_overlay_rect` reports a real on-screen rect
- all four `LegendPosition` anchors place it inside the axes rect
- toggle entry → `PlotMemory.hidden_items` → trace hides on BOTH
  paths and the entry shows struck-through (not removed)
- alt-click isolate, right-click isolate — same behavior both
- `hidden_items` round-trips through state save/load
- **regression: legend visible in the same screenshots we take
  ourselves (`fft_demo_screenshot`, `volcano_epoch_data_renders`)**
- `force_hide_legend` + narrow/short suppression apply to both paths

---

## 4. Traces

- style combos render: solid/dashed/dotted/points-only, widths,
  markers, `show_points`, offsets
- `render_mode` controls **hidden** in volcano mode (raw points → GPU)
- hidden trace (`look.visible = false`) absent from plot + legend
  struck on both paths
- color palette assignment + `set_color` controller
- display alias `display_label` in legend

---

## 5. Viewport / Interaction

- **autoscroll**: trailing window advances with streaming x (both)
  — `volcano_epoch_autoscroll_advances`
- **scroll zoom**: bounds change AND persist across `live_update`
  — `volcano_zoom_persists_past_live_update`
- **primary-drag pan**: bounds shift (both)
- **secondary-drag box zoom**: bounds become the box (both)
- **double-click**: autofit → user_bounds released, live window resumes
- **Fit X / Fit Y / Fit to View**: relinquishes user view (both)
- **x-axis zoom mode off**: scroll doesn't move x (both)
- **paused**: bounds frozen, paused-branch recenter matches
- **fit after zoom**: auto_fit re-enables → user view released

---

## 6. Overlays

All drawn through `draw_overlays` → `&mut dyn PlotBackend` —
`EguiBackend` (egui) and `VolcanoPainterBackend` (volcano) must
produce the same result:

- threshold lines (`hline`) at configured levels + colors
- trigger level + edge marker
- measurement markers + x/y cursor readout
- XY highlight-newest-point marker
- `clicked_point`/`clicked_screen_pos` populated identically

---

## 7. Side Panels

Every panel renders and drives identical behavior on both paths:

- **Traces panel**: visibility eye, color editor, look editor
  (render-mode combo hidden under volcano), XY-pair editor
- **Math panel**: formula builder, derived channels render on both
- **Thresholds panel**: builder → overlay appears on both
- **Triggers panel**: definition → overlay + fire events on both
- **Measurement panel**: click-capture works, markers on both
- **Color scheme**: palette changes affect traces on both
- **FFT panel**: visible flag, embedded scope (§2.3)
- **Export panel**: screenshot request + CSV/raw export identical
- **Hotkeys panel**: listing + hotkey actions on both
- **Top bar**: all buttons (Scopes/Traces/Math/Hotkeys/Thresholds/
  Triggers/Measurement/Color/FFT/Export/Pause/Clear/Screenshot/Undo)

---

## 8. Controllers

Every `liveplot::controllers::*` request produces the same result:

- `TracesController`: set_visible, set_color, axis/limit requests,
  snapshot listing
- `ScopesController`: add/remove, state snapshot
- `ThresholdController`: add/remove, publish events
- `FftController`: show, panel state
- `WindowController`: info
- `LivePlotController`: clear_all, save_state writes file
- `EventController`: subscribe_all → identical event stream

---

## 9. Events

- `CLICK|PAUSE` / `CLICK|RESUME` via `pause_on_click`
- `VIEW_CHANGE` emitted with the same `x_range`/`y_range`
- threshold `fired` events with identical meta
- `trace_visibility` events on legend toggle
- `data_update` events on feed
- `scope_manage` on add/remove

---

## 10. Persistence

- scope state round-trip: `legend_position`, `hidden_items`,
  `auto_fit`, `time_window`, paused
- `undo`/`redo` of user settings changes
- restored `trace_order` for pending traces

---

## 11. Drag-and-Drop

- trace drag from one scope onto another → moves + legend updates
  on both paths
- XY-pair drag composition
- scope reorder via DnD

---

## 12. Config Flags

`LivePlotConfig` / feature flags behave identically:

- `show_legend`/`show_grid`/`top_bar`/`buttons`
- `max_fps` clamp — volcano still renders under cap
- `auto_fit.auto_fit_to_view` startup
- `headline` custom widget renders on both
- `button_set` without `clear_all` hides the rest

---

## 13. Accessibility

- `unnamed_interactive_widgets_do_not_increase` on both paths
  (the volcano widget must not add unnamed interactive widgets
  beyond the egui plot's count)

---

## 14. Performance / GPU

The whole point of volcano mode — assert it, not just observe it:

- **CPU**: render+readback runs off the UI thread (pending-worker);
  UI step time in volcano ≤ egui step time for the same stream
- **GPU downsampling**: all raw points uploaded (`LinePlot`
  `setData`); VolcanoPlot samples per canvas width — assert the
  FFI receives `tr.live.len()` points, not a downsampled subset
- **memory**: each scope owns a backend; measure `volcano_surface`
  count == scope count; the shared-device change is a TODO, not a
  test blocker
- **60 fps**: a busy UI frame never blocks on `render_rgba` —
  assert `pending.is_some()` after a sync, not a join stall

---

## 15. Regression Cases (the ones that bit us)

Every historical bug becomes a named test:

- `volcano_epoch_data_renders` — f32-precision translate
- `volcano_epoch_autoscroll_advances` — live window scrolls
- `volcano_zoom_persists_past_live_update` — user_bounds
- `volcano_empty_trace_safe` — no crash on empty spectrum
- `volcano_resize_race` — size-mismatch px upload
- `volcano_legend_independent_fft` — separate plot_id
- `volcano_axis_labels_render` — "Frequency"/"Magnitude" pixels
- `volcano_margin_adapts_height` — FFT strip vs tall scope
- `volcano_egui_build_without_volcano` — `cargo check --features fft`
- `fft_demo_screenshot` — legend + traces visible in both modes

---

## 16. Test Matrix (execution)

```
harness level   volcano  egui
------------------------------------------
panel-level     ✓        ✓   (behavioral)
app-level       ✓        ✓   (controllers, persistence, events)
wgpu pixel      ✓        ✓   (colored px, legend px, label px)
soak/stress     ✓        ✓   (LIVEPLOT_STRESS=1 opt-in)
```

Current state: `volcano_scope` (6) + `volcano_parity` (8) +
`zz_snap` regression set — 156/161 e2e green on both paths.

## 17. Gaps / TODO

- CPU/RAM measurement test (needs a timing harness around `step()`)
- Shared-Vulkan-device across figures (upstream FFI work — biggest
  remaining memory win)
- Legend-position persistence round-trip on volcano path
- The `zz_snap` set should merge into `volcano_parity` once stable.
