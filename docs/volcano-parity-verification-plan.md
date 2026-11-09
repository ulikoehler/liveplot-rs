# Volcano ↔ egui_plot Full-Parity Verification & Remediation Plan

> Status: **planning** — supersedes `docs/e2e-parity-plan.md` (which stays
> as the high-level matrix; this document adds the complete
> control-by-control catalogue, root-cause register, and fix work items).
>
> Goal restated: prove — with executable tests, not inspection — that the
> VolcanoPlot (C++/Vulkan) render path behaves and looks the same as the
> `egui_plot` path for every user-visible control, and fix every
> divergence found. Currently known-broken: **XY view**, **legend**,
> **autoscroll in time view while zoomed in**. The root-cause register in
> §4 explains each of these plus further divergences discovered while
> auditing the code.

---

## Table of Contents

- [1. Objective & parity contract](#1-objective--parity-contract)
- [2. Architecture map](#2-architecture-map)
- [3. Audit method — how divergences were found](#3-audit-method--how-divergences-were-found)
- [4. Root-cause register](#4-root-cause-register)
- [5. Test infrastructure work items](#5-test-infrastructure-work-items)
- [6. Test catalogue](#6-test-catalogue)
  - [6.1 CAT-RND — render surface](#61-cat-rnd--render-surface)
  - [6.2 CAT-AX — axes, ticks, labels, formatting](#62-cat-ax--axes-ticks-labels-formatting)
  - [6.3 CAT-LGD — legend](#63-cat-lgd--legend)
  - [6.4 CAT-ZOOM — wheel zoom, pan, box zoom, double-click, fit](#64-cat-zoom--wheel-zoom-pan-box-zoom-double-click-fit)
  - [6.5 CAT-SCRL — autoscroll & time window](#65-cat-scrl--autoscroll--time-window)
  - [6.6 CAT-HOV — hover options](#66-cat-hov--hover-options)
  - [6.7 CAT-CLK — click, measurement capture, pause-on-click](#67-cat-clk--click-measurement-capture-pause-on-click)
  - [6.8 CAT-XY — XY scope](#68-cat-xy--xy-scope)
  - [6.9 CAT-FFT — embedded FFT scope](#69-cat-fft--embedded-fft-scope)
  - [6.10 CAT-TRC — trace styling](#610-cat-trc--trace-styling)
  - [6.11 CAT-OVL — overlays](#611-cat-ovl--overlays)
  - [6.12 CAT-PAU — pause / resume / snapshots](#612-cat-pau--pause--resume--snapshots)
  - [6.13 CAT-PNL — side panels](#613-cat-pnl--side-panels)
  - [6.14 CAT-CTRL — external controllers](#614-cat-ctrl--external-controllers)
  - [6.15 CAT-EVT — event bus](#615-cat-evt--event-bus)
  - [6.16 CAT-PER — persistence & undo](#616-cat-per--persistence--undo)
  - [6.17 CAT-DND — drag & drop](#617-cat-dnd--drag--drop)
  - [6.18 CAT-CFG — config flags](#618-cat-cfg--config-flags)
  - [6.19 CAT-A11Y — accessibility](#619-cat-a11y--accessibility)
  - [6.20 CAT-PERF — performance invariants](#620-cat-perf--performance-invariants)
  - [6.21 CAT-PIX — pixel / snapshot parity](#621-cat-pix--pixel--snapshot-parity)
- [7. Fix work items](#7-fix-work-items)
- [8. Execution phases & acceptance gates](#8-execution-phases--acceptance-gates)
- [9. Commands & CI wiring](#9-commands--ci-wiring)
- [10. Appendices](#10-appendices)

---

## 1. Objective & parity contract

"Behaves and looks exactly the same" is made precise here. There are two
render paths:

- **egui path** — `egui_plot::Plot` widget owns rendering + interaction;
  liveplot draws its own legend overlay and overlay items on top
  (`scope_ui.rs::render_plot`, lines 1402–2252).
- **volcano path** — `VolcanoSurface` renders traces + axes to an RGBA8
  texture via the `volcano_ffi` C API on a worker thread; the texture is
  painted into an allocated rect; all interaction, legend, overlays are
  re-implemented in `scope_ui.rs::render_volcano_plot`
  (lines 1058–1400).

### 1.1 Behavioral parity — must be *identical*

Anything observable through `ScopeData`, controllers, events, or
persistence must match bit-for-bit (within float tolerance `1e-9`):

- `x_axis.bounds`, `y_axis.bounds`, `time_window`, `auto_fit` flags,
  `paused`, `clicked_point`, `clicked_screen_pos`, `last_plot_bounds`,
  `last_plot_screen_rect` (**same tuple layout**), `legend_overlay_rect`
  presence/anchor, `legend_hidden_items`, `rendered_this_frame`.
- `PlotMemory` side effects: `hidden_items`, `hovered_legend_item`.
- `TraceLook::visible` / `pair_look.visible` after legend interaction.
- Every `PlotEvent` kind + `ViewChangeMeta`/`ClickMeta` payload emitted
  on the egui path must also be emitted, with the same values, on the
  volcano path.
- Persisted state (save/load round-trip) must produce identical
  behavior after reload on both paths.

### 1.2 Visual parity — pixel-region equivalence

VolcanoPlot renders axes/ticks/labels with its own glyph atlas inside
the texture — text will never be byte-identical to egui's text. So the
contract is *region-level* equivalence:

- **Same geometry**: the VolcanoPlot axes rect inside the texture must
  coincide pixel-for-pixel with the rect the overlay layer
  (`VolcanoPainterBackend`, legend, box-zoom rect, click mapping) uses.
  Today it does not — see RC-1.
- **Same data mapping**: a data point `(x,y)` must land at the same
  screen pixel on both paths (±2 px). Verified by rendering a sparse
  marker trace and comparing colored-pixel centroids/bounding boxes.
- **Same colors**: trace colors are passed through as RGBA8; assert
  colored-pixel histograms inside the axes rect match per-trace.
- **Same presence**: legend, grid, tick labels, axis names, overlays
  each have a binary presence check per region.
- **Same margins**: the texture's outer margin must equal egui's
  `panel_fill` so the plot blends into the chrome.

Explicitly allowed to differ:

- Font rasterization, tick count/style, anti-aliasing inside the
  VolcanoPlot texture (assert "labels exist", not "labels identical").
- Downsampling strategy inside the axes rect (volcano uploads raw
  points, GPU decimates; egui uses envelope modes — see RC-14).
- Frame latency: volcano pipelines renders one frame behind
  (assert on state after `steps(n)`, never on a single frame).

### 1.3 Non-goals

- Byte-identical PNG screenshots between backends.
- VolcanoPlot-internal rendering quality (upstream's problem, covered
  by its own `volcano_tests` gtest suite — see vendor AGENTS.md).
- The shared-Vulkan-device optimization (memory win, not parity).

---

## 2. Architecture map

### 2.1 Data flow — egui path

```
PlotSink ──► TracesCollection ──► ScopeData.update()  [auto_fit? fit_x/y_bounds,
                                   live_update() trailing window]
        ──► ScopePanel::render_plot()
              ├─ Plot::show ──► PlotUi (EguiBackend)
              │     ├─ wheel/box/drag input → transform bounds
              │     ├─ Line/Points per trace (envelope downsampled)
              │     └─ draw_overlays(EguiBackend)
              ├─ read back transform → x_axis.bounds, time_window
              ├─ legend overlay (render_scope_legend) ↔ PlotMemory
              ├─ hidden_items → look.visible sync
              ├─ handle_plot_click → events, clicked_point, pause
              └─ handle_trace_drop
```

### 2.2 Data flow — volcano path

```
same upstream ──► ScopePanel::render_volcano_plot()
  ├─ surf.user_bounds? → surf.bounds (+bounds_external)
  ├─ surf.sync_and_render(scope, traces, w, h)
  │     ├─ join pending worker frame → tex.set()
  │     ├─ frame signature → skip if unchanged
  │     ├─ per-trace sig → vp_trace_set (shifted/log'd points)
  │     ├─ XY join loop (tol 1e-9) → vp_trace_set
  │     ├─ fit_bounds() until initialized (trace_order ONLY)
  │     ├─ vp_set_viewport / margins / bg / labels
  │     └─ spawn worker: vp_render_rgba → Vec<u8>
  ├─ paint texture into rect
  ├─ axes rect = hardcoded constants ML/MR/MB/MT   ← RC-1
  ├─ input: primary drag, wheel, 2° box-zoom, dbl-click, click
  ├─ legend entries (custom) + draw_overlays(VolcanoPainterBackend)
  └─ legend overlay ↔ PlotMemory (partial — missing sync-back)
```

### 2.3 File map

| File | Role |
|---|---|
| `src/panels/scope_ui.rs` | both render paths, toolbar, menus, clicks |
| `src/volcano/mod.rs` | `VolcanoFigure` — FFI bindings (vp_*) |
| `src/volcano/surface.rs` | `VolcanoSurface` — sync/upload/render/cache |
| `src/panels/plot_backend.rs` | `PlotBackend` trait, `EguiBackend`, `VolcanoPainterBackend` |
| `src/data/scope.rs` | `ScopeData`, `update()`, `live_update()`, `fit_*_bounds()` |
| `src/panels/scope_legend.rs` | shared legend overlay + canonical item ids |
| `vendor/VolcanoPlot/src/ffi/volcano_ffi.cpp` | C API impl, margin defaults |
| `tests/e2e/harness.rs` | `make_test_bed`, `make_app_bed`, `feed_*`, `drag`, `click_at`, `key_press` |
| `tests/e2e/volcano_parity.rs` | existing parity suite (8 tests + wgpu pixel checks) |
| `tests/e2e/volcano_scope.rs` | volcano-only smoke tests |
| `tests/e2e/zz_snap.rs` | app-level volcano regression scratch (to be merged) |
| `tests/volcano_ffi.rs` | FFI-level tests (no egui) |

### 2.4 Feature flags

- `volcano` — enables `src/volcano/` + `render_volcano_plot` branch +
  `ScopeData::volcano_enabled` (seeded from `LIVEPLOT_VOLCANO` env).
- e2e parity tests are `#[cfg(feature = "volcano")]`-gated module
  `volcano_parity` in `tests/e2e/main.rs`; `volcano_scope` tests compile
  unconditionally but assert through the same harness.

---

## 3. Audit method — how divergences were found

The register in §4 was produced by:

1. Diffing `render_volcano_plot` (1058–1400) line-by-line against the
   egui `render_plot` body (1521–2252) and the early-return at
   1458–1461 — everything after `plot.show` that the volcano path does
   not re-implement is a candidate divergence.
2. Diffing the surface's input handling against egui_plot semantics
   (zoom factor, padding, box-zoom modes, double-click).
3. Diffing `VolcanoSurface::sync_and_render` against the egui draw loop
   (which points are uploaded, which transforms apply, which looks are
   honored).
4. Reading `volcano_ffi.cpp` to verify what the FFI actually honors
   (margins, grid, styles, labels).
5. Checking every `ScopeData` field the egui path writes for whether
   the volcano path writes it too.

Everything listed is reproduced-by-reading; each RC item names the
test(s) that will prove it executable.

---

## 4. Root-cause register

Severity: **B** = user-visible behavioral break, **V** = visual-only,
**P** = performance/robustness. Status: ☐ open, ☑ fixed.

### RC-1 (B) — Overlay axes-rect ≠ rendered axes-rect — *the legend/click/overlay bug*

`render_volcano_plot` computes the overlay `axes` rect from hardcoded
canvas fractions (`ML=0.07, MR=0.985, MB=0.20, MT=0.985`,
scope_ui.rs:1141–1154), but `sync_and_render` overrides the figure's
grid margins with **pixel** margins — `(60,12,56,8)` px, or
`(52,10,34,6)` px when `h < 160` (surface.rs:422–428 →
`vp_set_margins`). At `h=600` the real bottom margin is `56/600 ≈
0.093`, not `0.20` — the overlay believes the axes end ~64 px lower
than the texture draws them.

Consequences: `to_data()` maps clicks to wrong data coords;
`VolcanoPainterBackend`'s `PlotTransform` misplaces every overlay
(thresholds, triggers, measurement markers) vertically and slightly
horizontally; the legend overlay anchors to the wrong rect;
`legend_overlay_rect` is reported wrong; box-zoom selects a shifted
data range.

Fix: single source of truth — compute pixel margins in
`render_volcano_plot` (same height-dependent table), pass them into
`sync_and_render`, and derive `axes` from those same margins and the
actual `rect`. Tests: T-OVL-001, T-CLK-001, T-LGD-001/002,
T-PIX-003.

### RC-2 (B) — Autoscroll frozen while zoomed in — *the reported time-view bug*

egui semantics (verified by reading 1618–1627 + 2192–2204 +
scope.rs:517–545): after an x-zoom/pan, `time_window` is rewritten to
the new span (scope_ui.rs:2203); `live_update()` then keeps writing
`x_axis.bounds = (now − time_window, now)` every frame — **the
trailing window keeps scrolling with the user's zoomed span**. The x
view is never pinned; it always tracks `now` with the current
`time_window`. Y, in contrast, stays at the user range.

Volcano path: `surf.user_bounds` pins **both** axes forever
(scope_ui.rs:1099–1110, 1273–1281) and `time_window` is never updated
after zoom → the x view freezes at the zoom instant; new data streams
past invisible; unpausing doesn't scroll. Exactly the reported
symptom.

Fix: mirror the egui state machine —
- After `bounds_changed` on an unpaused `TimeScope`, set
  `data.time_window = nx1 − nx0` (x-zoom/pan/box only; not y-only ops).
- While `!paused && TimeScope`, do **not** let `user_bounds` own x —
  feed `surf.bounds.0` from `data.x_axis.bounds` each frame (which
  `live_update` keeps scrolling). `user_bounds` may still own y.
- While `paused`, or on `XYScope`, `user_bounds` owns both axes.
- `auto_fit` re-enable / double-click clears `user_bounds` (already
  done at 1096–1098).

Tests: T-SCRL-001..010.

### RC-3 (B) — Legend hidden→visible sync missing on volcano path

The egui path, after rendering the legend, writes
`mem.hidden_items` back into `tr.look.visible` /
`pair_look.visible` (scope_ui.rs:2104–2138) and publishes
`data.legend_hidden_items` (2144–2146). `render_volcano_plot`
returns early (1459–1461) so **none of that runs**. Result: clicking
a legend entry on the volcano path strikes the label through
(checkbox state round-trips through `PlotMemory`, which *is* wired)
but `look.visible` stays `true`, so the surface keeps uploading and
the trace never disappears — "legend is broken". Also
`legend_hidden_items` stays empty → measurement/marker overlays
mis-fit bounds.

Fix: extract the two blocks (2104–2138, 2144–2146) into a method
`sync_legend_hidden_to_looks(ctx, plot_id, traces)` called by both
paths at the end of render. Tests: T-LGD-004..009.

### RC-4 (B) — XY legend entries diverge

- Label: volcano pushes `format!("{}/{}", xn.0, yn.0)`
  (scope_ui.rs:1307); egui pushes `"{y_label} vs {x_label}"` using
  `display_label` + optional ` — info` suffix (1697–1701).
- Missing `(last)` entry + point for `pair_look.highlight_newest_point`
  (egui: 1751–1769) — surface also never draws the marker.
- `pair_look` honor list: volcano uploads color/width/style only;
  `show_points`, `marker`, `point_size`, `highlight_newest_point`,
  hover dim/emphasis all dropped.
- Item ids are consistent (`xy_pair_item_id`) — good, keep.

Fix: build legend entries in one shared function fed by both paths;
draw pair overlays (points/last-point) through `VolcanoPainterBackend`
in the volcano branch instead of inside the surface. Tests:
T-XY-005..011, T-LGD-010.

### RC-5 (B) — `last_plot_screen_rect` tuple layout differs

egui writes `[left, top, right, bottom]` (record_plot_geometry,
265–266); volcano writes `[left, top, width, height]` (1392–1393).
Every test/app consumer computing `rect[2] − rect[0]` gets garbage on
the volcano path (pan tests included — they only pass because the
numbers happen to move).

Fix: write `[rect.left(), rect.top(), rect.right(), rect.bottom()]`.
Test: T-RND-006 asserts corner + center consistency against
`harness.render()` image size.

### RC-6 (B) — Click semantics diverge

egui `handle_plot_click` (2351+): double-click → `fit_bounds` +
`DOUBLE_CLICK|FIT_TO_VIEW` event with `ClickMeta` + `pending_view_change`;
click → `capture_clicked_plot_point`, pause-on-measurement,
`pending_pause`, `CLICK`/`PAUSE`/`RESUME` events; `pause_on_click` when
enabled toggles pause.

Volcano (1238–1266): double-click sets `auto_fit=true` on both axes —
different behavior (egui fits once and lets `live_update` resume
scrolling; volcano re-enables continuous auto-fit). Click sets
`clicked_point` (manually, with a different log-scale convention —
verify against `capture_clicked_plot_point`) and a subset of pause
logic; **no events at all**, no `pending_view_change`, no trace-drop
handling (RC-7).

Fix: route the volcano `resp` through the same `handle_plot_click`
machinery — needs a `PlotResponse`-shaped adapter or refactor
`handle_plot_click` onto `PlotBackend` + response. Tests: T-CLK-001..008,
T-EVT-006/007.

### RC-7 (B) — `handle_trace_drop` skipped

Early return means dragging a trace from the traces panel onto a
volcano scope does nothing (2251 unreachable). Also
`capture_clicked_plot_point`'s screen→plot mapping uses the *wrong*
axes rect until RC-1 lands. Fix: call `handle_trace_drop(ui, &resp)`
in `render_volcano_plot`. Test: T-DND-001/002.

### RC-8 (B) — Wheel-zoom semantics diverge

| aspect | egui | volcano |
|---|---|---|
| factor | `1 + scroll·0.001` via `zoom_bounds_around_hovered` | `(1 − scroll·0.002).clamp(0.5,2)` |
| horizontal scroll | zooms x (1563–1565) | ignored |
| `ZoomMode::Off` | no state change | still sets `auto_fit=false`, `user_bounds` ← freezes view |
| over-legend guard | `over_legend` check (1536–1540) | absent — wheel over legend zooms plot *and* scrolls legend |
| anchor | `zoom_bounds_around_hovered` (pointer-stable) | `to_data(hover)` — same intent, but wrong rect until RC-1 |

Fix: replicate factor + h-scroll + Off-guard + over-legend guard.
Tests: T-ZOOM-001..010.

### RC-9 (B) — Box zoom ignores ZoomMode + styling

egui honors `ZoomMode::X` (full-height band, x-only bounds) and
`ZoomMode::Y`; draws 4 px DARK_BLUE + 2 px WHITE stroke clipped to the
frame (2148–2190). Volcano always applies both axes and draws a 1 px
LIGHT_BLUE rect (1214–1237). Fix: honor mode, match band rendering.
Tests: T-ZOOM-011..016.

### RC-10 (V) — Missing 5 % bounds pad

egui pads the applied bounds by 5 % per side so traces don't touch
the axes edge (1620–1626), then subtracts `0.05/1.1` on read-back
(2200–2209). Volcano maps `data.bounds` 1:1 to the axes rect →
traces touch edges and every data→pixel mapping is off by ~5 % vs.
egui. For pixel parity either replicate the pad inside
`sync_and_render`'s `vp_set_viewport` call or normalize egui's.
Decision: replicate pad (behavioral parity of `last_plot_bounds`
already matched because the egui path reads the pad back out).
Tests: T-PIX-002/003, T-RND-007.

### RC-11 (B) — XY scope viewport not fitted

`VolcanoSurface::fit_bounds` iterates `trace_order` only
(surface.rs:452–478). An `XYScope` normally has empty `trace_order`
(pairs live in `xy_pairs`) → `xb/yb` stay `INFINITY` → falls back to
default `(0,1)` bounds → the joined curve renders off-screen or
squeezed — "xy-view problems". egui's `fit_x_bounds`/`fit_y_bounds`
have an `XYScope` branch joining pairs (scope.rs:547–605, 658+).

Fix: give `fit_bounds` an XY branch mirroring `ScopeData::fit_*_bounds`
(or reuse those and write results into `surf.bounds`). Also apply
x/y log transforms to derived XY points (egui: 1658–1675; surface
passes `(false,false)`). Tests: T-XY-001..004, T-XY-012/013.

### RC-12 (B) — Axis labels / formatters missing

- `vp_set_axis_labels(scope.x_axis.name, …)` passes only explicit
  names — the egui path auto-generates "Time" for `TimeScope` and
  "x vs y" names for `XYScope` via `axis_label_text` /
  `generated_axis_trace_names` (1462–1470). Volcano plots get no
  axis title where egui shows one.
- Tick labels: VolcanoPlot formats raw values (%.1e etc.); egui
  routes through `x_axis.format_value` with log-inverse
  `10^v`, `TimeFormat`, `value_decimals`, `always_scientific`,
  units. Byte-parity of tick text is out of scope (§1.2), but
  *label presence* and *shifted-space formatting* are not: when
  `shift_ref` is active, tick labels show `x − ref` while egui
  shows formatted absolute values — flag as a known visual delta
  with a documenting test; optionally extend FFI with a value-format
  callback later.
- `show_axes`/`show_grid` suppression: egui hides tick space when the
  widget is small (`hide_x_labels`/`hide_y_labels`, 1414–1415) and
  `data.show_grid=false` disables the grid; the FFI currently always
  draws grid+labels → add `vp_set_grid(bool)` +
  `vp_set_show_tick_labels(bool)` FFI flags. Tests: T-AX-001..012,
  T-CFG-003.

### RC-13 (V) — Hover features absent

- Coordinate tooltip: egui's `label_formatter` (1494–1515) shows
  `x = …\ny = …` near the cursor; volcano shows nothing.
- `traces.hover_trace` dim/emphasis (set by the traces panel):
  egui dims non-hovered traces to alpha 40 and thickens the hovered
  one ×1.6 (1968–1981, 1710–1723); the surface's `trace_sig` doesn't
  include hover state and colors pass unmodified → no effect.

Fix: draw an egui-side hover label in `render_volcano_plot` using
`to_data` + `format_value` (same strings as `label_formatter`);
fold hover state into the per-trace upload (color alpha + width)
and into `trace_sig`. Tests: T-HOV-001..006.

### RC-14 (V) — Trace styling gaps

- `show_points=true` maps to `VolcanoLineStyle::PointsOnly` — loses
  the line entirely (egui draws line **and** markers). Need either a
  5th FFI style (`LineAndPoints`) or a second `vp_trace` per trace
  for the marker pass (also enables `marker` shape + `point_size`,
  currently ignored).
- `LineStyle::Dashed{Density}`/`Dotted` map to 1/2 — ok.
- `hover` width/alpha — RC-13.
- `RenderMode` — by design the UI hides the selector in volcano mode;
  surface uploads raw `live` points (GPU downsamples). Document +
  test that `render_mode` is ignored rather than half-applied.
- `brightness_gain`, density splatting — egui-only features; volcano
  path must skip `paint_density` (already does via early return)
  and still draw the outline line on the surface. Verify the
  DensitySplatting trace still produces a vp trace (currently it
  does — uploads raw points). Test: T-TRC-001..010.

### RC-15 (P) — Robustness/misc

- `vp_render_rgba` signature-miss → returns `None` → frame dropped
  silently; add counter `data.volcano_dropped_frames` for tests.
- `resp` uses `Sense::click_and_drag` on the whole rect — legend rows
  are separate widgets on top, so drags starting on a legend row
  also pan the plot (egui path has the same layering, but verify —
  the legend scroll guard exists only on egui wheel path).
- `VolcanoPainterBackend::zoom_bounds_around_hovered` is a no-op
  (plot_backend.rs:187–189) — any overlay code calling it silently
  does nothing; grep for callers and implement properly.
- FFI `vp_set_background(white)` boolean vs. egui theme colors —
  dark/light verified; custom color-scheme background is not
  forwarded. Test: T-CFG-006.
- Empty XY pair → derived empty → `vp_trace_set(n=0)` — confirm the
  FFI tolerates 0-length (time traces already skip empties,
  surface.rs:304–307; XY loop does not). Test: T-XY-014.

---

## 5. Test infrastructure work items

### INFRA-1 — input helpers in `tests/e2e/harness.rs`

Add (next to `drag`, `click_at`, `key_press`):

```rust
/// N wheel-scroll steps of `dy` (logical px) at `pos` — positive = up/in.
pub fn scroll_at(bed: &mut TestBed, pos: Pos2, dx: f32, dy: f32, n: usize);

/// Move pointer to `pos` and step once (hover without click).
pub fn hover_at(bed: &mut TestBed, pos: Pos2);

/// Two click cycles within egui's double-click window at `pos`.
pub fn double_click_at(bed: &mut TestBed, pos: Pos2);

/// Multi-segment drag: press at `from`, move through `path`, release.
/// Needed for box-zoom (must stay `dragged`, not click).
pub fn drag_path(bed: &mut TestBed, from: Pos2, path: &[Pos2],
                 button: PointerButton);

/// Pointer-button-down at pos (keeps held for rect-tracking asserts),
/// then `release_at`.
pub fn press_at / release_at(...);

/// Center + inner rect of the *axes* region (not the widget rect) —
/// for volcano this is `axes`, for egui `transform.frame()`; expose
/// via `ScopeData::last_axes_screen_rect` (new field, both paths).
```

### INFRA-2 — `ScopeData::last_axes_screen_rect: Option<[f32;4]>`

Record the *axes* frame (egui: `plot_resp.transform.frame()`; volcano:
the RC-1-fixed `axes` rect) as `[l,t,r,b]` every frame. Nearly every
interaction test needs this — pointer targets must land inside the
axes, not the widget padding. Also fixes today's reliance on
`last_plot_screen_rect` which includes margins.

### INFRA-3 — pixel-assert helpers (`tests/e2e/pixels.rs`, new module)

```rust
fn colored_px(img, rect) -> u32            // channel spread > 30
fn distinct_colors(img, rect) -> usize     // HashSet<Rgb>
fn centroid_of(img, rect, pred) -> (f32,f32)
fn bbox_of(img, rect, pred) -> Rect
fn row_col_profile(img, rect) -> (Vec<u32>, Vec<u32>) // grid detect
fn region_is_uniform(img, rect, tol) -> bool          // margin blend
fn save_debug(img, name)                   // LIVEPLOT_SNAPSHOT=1 dumps
```

All take `egui::Rect` in screen space + `&RgbaImage` from
`harness.render()` (wgpu bed) or `harness.snapshot()` fallbacks.

### INFRA-4 — dual-path driver macro

```rust
/// Runs `f` once with volcano_enabled=false and once =true; `f`
/// receives (&mut TestBed, vol: bool) and does the asserts itself.
/// Skips volcano arm when feature off or Vulkan init fails
/// (surf never `initialized()` → log + pass, same policy as wgpu_bed).
macro_rules! for_each_backend { ($bed:ident, $body:block) }
```

Plus `fn vol_available(bed) -> bool` — flips `volcano_enabled`,
steps, checks `volcano_surface.initialized()`; avoids false passes on
CI boxes without a Vulkan driver (llvmpipe counts as available).

### INFRA-5 — signal additions (`tests/e2e/signals.rs`)

- `SignalSpec::Impulse { at }` — single spike for pixel-position checks.
- `SignalSpec::Step { t, v0, v1 }` — hard edges for autoscroll asserts.
- `SignalSpec::Epoch(freq)` — `t0 = 1.79e9` epoch-time sine (already
  covered partly; make reusable).
- `feed_pair(bed, x, y, spec_x, spec_y, n, fs, t0)` — one-call XY feed.

### INFRA-6 — event recorder

`EventController::subscribe_all` exists; add a test helper that drains
the bus into `Vec<PlotEvent>` between steps so CAT-EVT tests assert
kind + meta payloads, not just "some event".

### INFRA-7 — FFI-level micro tests (`tests/volcano_ffi.rs`)

Extend beyond smoke: viewport mapping math (feed a figure a known
trace + viewport, render, assert the marker lands at the expected
pixel — validates RC-1's margin contract *without* egui), margin
round-trip, empty-trace safety, resize behavior, label presence
(pixel variance in label band).

### INFRA-8 — snapshot pairs

`tests/e2e/snapshots/` gains `parity_<name>_{egui,volcano}.png`
produced by `wgpu_bed` + `LIVEPLOT_SNAPSHOT=1`; the test then compares
*regions* (per §1.2), never whole images.

---

## 6. Test catalogue

Legend: **T-XXX-nnn** — stable id, never renumber. **Bed**: P=panel,
A=app, W=wgpu-pixel. **Both**: runs under `for_each_backend`.
Existing tests are referenced by file/name and folded into the matrix.

### 6.1 CAT-RND — render surface

| id | bed | test | asserts |
|----|-----|------|---------|
| T-RND-001 | P | `parity_scope_renders` (exists: `parity_legend_overlay_rect`, `volcano_scope_renders_through_surface`) | `rendered_this_frame`, `surf.initialized()` when vol |
| T-RND-002 | W | `parity_colored_px_in_axes` | `colored_px(axes_rect) > 100` both |
| T-RND-003 | W | `parity_margin_blends_panel_fill` | outer band of widget rect is uniform ≈ `panel_fill` both |
| T-RND-004 | P | `parity_fallback_on_no_vulkan` | forced `VolcanoFigure::new` fail → `volcano_enabled=false`, egui path renders |
| T-RND-005 | P | `parity_resize_debounce` | shrink rect < 32 px → no figure resize; > 32 px → resize + still renders |
| T-RND-006 | P | `parity_screen_rect_layout` | `last_plot_screen_rect == [l,t,r,b]` with `r>l`, `b>t`, and `last_axes_screen_rect` inside it — both |
| T-RND-007 | W | `parity_data_to_pixel_map` | impulse trace at known (x,y): centroid of marker color within ±2 px between backends (needs RC-10 pad decision landed) |
| T-RND-008 | P | `parity_frame_skip_unchanged` | no new data + no input → `surf.last_sig` unchanged → texture identical (expose `frames_rendered` counter for the test) |
| T-RND-009 | P | `parity_empty_scope_safe` | no traces → axes only, no panic, both |
| T-RND-010 | P | `volcano_empty_trace_safe` (exists) | empty `live` doesn't break renderFrame; XY variant T-XY-014 |

### 6.2 CAT-AX — axes, ticks, labels, formatting

| id | bed | test | asserts |
|----|-----|------|---------|
| T-AX-001 | W | `parity_x_axis_title_present` | "Time" text band below axes: `distinct_colors(label_band) > 4` both |
| T-AX-002 | W | `parity_y_axis_title_present` | left label band non-empty both |
| T-AX-003 | W | `parity_tick_labels_present` | tick rows (bottom band) and cols (left band) have text pixels both |
| T-AX-004 | P | `parity_custom_axis_names` | `x_axis.name="Foo"` → FFI receives "Foo" (spy via label band pixels / FFI-level test) both |
| T-AX-005 | P | `parity_auto_axis_names_xy` | XYScope auto label "y vs x" parity (RC-12) |
| T-AX-006 | P | `parity_axis_label_toggle` | `show_label=false` → label band empty both |
| T-AX-007 | W | `parity_epoch_shift_labels` | epoch feed → tick labels exist AND document shifted-space delta (RC-12 note) |
| T-AX-008 | P | `parity_grid_toggle` | `show_grid=false` → zero grid pixels inside axes both (needs `vp_set_grid`) |
| T-AX-009 | P | `parity_tick_suppression_small_widget` | shrink widget < `min_width_for_y_ticklabels` → tick space reclaimed both |
| T-AX-010 | P | `parity_time_format_variants` | cycle `TimeFormat` ×5 → x tick text band changes content both (presence + change, not equality) |
| T-AX-011 | P | `parity_value_decimals` | `value_decimals` change → tick band changes both |
| T-AX-012 | P | `parity_unit_suffix` | `set_unit("V")` → label/tick content changes both |
| T-AX-013 | P | `parity_log_scale_render` | `x_axis.log_scale`/`y_axis.log_scale` → decade-spaced geometry: col-profile peaks at log positions both |
| T-AX-014 | P | `parity_log_clips_nonpositive` | data ≤ 0 under log → no crash, NaN-skipped points absent both |

### 6.3 CAT-LGD — legend

| id | bed | test | asserts |
|----|-----|------|---------|
| T-LGD-001 | P | `parity_legend_overlay_rect` (exists) | `legend_overlay_rect.is_some()` both |
| T-LGD-002 | P | `parity_legend_inside_axes` | legend rect ⊂ `last_axes_screen_rect` — **fails today on volcano** (RC-1) |
| T-LGD-003 | P | `parity_legend_positions` | ×4 `LegendPosition` → corner anchor within axes ±8 px both |
| T-LGD-004 | P | `parity_legend_click_hides_trace` | `click_at` legend row → `hidden_items` has id AND `tr.look.visible==false` AND colored px for that trace gone — **volcano fails today** (RC-3) |
| T-LGD-005 | P | `parity_legend_click_restores` | second click → visible again, trace re-uploaded (frame sig changes) both |
| T-LGD-006 | P | `parity_legend_struck_style` | hidden entry label renders struck/dimmed in legend pixels both |
| T-LGD-007 | P | `parity_legend_alt_click_isolate` | alt-click → all other ids hidden both |
| T-LGD-008 | P | `parity_legend_right_click_isolate` | secondary-click hovered entry → isolate; second → restore all (mirrors egui block 2071–2100) both |
| T-LGD-009 | P | `parity_legend_hidden_persists` | `hidden_items` survives `PlotMemory` store/load + app save/load both |
| T-LGD-010 | P | `parity_legend_xy_labels` | XY pair entry text "y vs x" + "(last)" entry when `highlight_newest_point` (RC-4) both |
| T-LGD-011 | P | `parity_legend_info_suffix` | `show_info_in_legend` → label gets ` — info` both |
| T-LGD-012 | P | `parity_legend_alias` | `display_label` shown, item id canonical both |
| T-LGD-013 | P | `parity_legend_scroll` | 30 traces → legend scrollable; wheel over legend rect scrolls legend AND does NOT zoom plot (RC-8 row 4) both |
| T-LGD-014 | P | `parity_legend_suppression` | `force_hide_legend`, `min_width_for_legend`, `min_height_for_legend` → same suppression at same sizes both (uses `total_widget_size`, not `rect`) |
| T-LGD-015 | P | `parity_legend_hover_sets_mem` | hover entry → `mem.hovered_legend_item` set; hovered trace emphasized on both (RC-13 dimming) |
| T-LGD-016 | P | `parity_legend_hidden_items_published` | `data.legend_hidden_items` mirrors `mem.hidden_items` both (RC-3) |
| T-LGD-017 | W | `parity_legend_pixels` (exists) | distinct colors in legend rect > 4 both |

### 6.4 CAT-ZOOM — wheel zoom, pan, box zoom, double-click, fit

All bounds assertions compare `last_plot_bounds` (data space) between
the two runs after identical input scripts — tolerance `1e-9`
relative. Where egui applies its 5 % pad the *reported* bounds already
have it removed (read-back at 2200–2209), so the comparison is
apples-to-apples after RC-10.

| id | bed | test | input script → asserts |
|----|-----|------|------------------------|
| T-ZOOM-001 | P | `parity_wheel_zoom_in_x` | `scroll_at(center, 0, +120)` ZoomMode::X → x span shrinks ~`1/1.12`, y unchanged |
| T-ZOOM-002 | P | `parity_wheel_zoom_out_x` | `−120` → x span grows |
| T-ZOOM-003 | P | `parity_wheel_zoom_y` | ZoomMode::Y → only y changes |
| T-ZOOM-004 | P | `parity_wheel_zoom_both` | ZoomMode::Both → both shrink |
| T-ZOOM-005 | P | `parity_wheel_zoom_off` | ZoomMode::Off → bounds AND `auto_fit` unchanged (**volcano fails today**) |
| T-ZOOM-006 | P | `parity_wheel_zoom_anchor` | scroll with pointer at 25 % width → anchor data-x stays fixed ±1 % both |
| T-ZOOM-007 | P | `parity_wheel_hscroll` | `scroll_at(c, +100, 0)` → x zooms (egui 1563–1565; volcano add) |
| T-ZOOM-008 | P | `parity_wheel_sensitivity` | same scroll Δ → same span ratio ±10 % both (unify factor per RC-8) |
| T-ZOOM-009 | P | `parity_wheel_over_legend` | pointer over legend rect + scroll → plot bounds unchanged, legend scroll offset changed (RC-8) |
| T-ZOOM-010 | P | `parity_zoom_disables_autofit` | any zoom → `auto_fit==false` on affected axis both |
| T-ZOOM-011 | P | `parity_box_zoom_both` | secondary drag rect → `last_plot_bounds` == dragged rect in data coords ±2 px-worth both |
| T-ZOOM-012 | P | `parity_box_zoom_x_mode` | ZoomMode::X → only x bounds = band; y unchanged both (RC-9) |
| T-ZOOM-013 | P | `parity_box_zoom_y_mode` | ZoomMode::Y → symmetric |
| T-ZOOM-014 | P | `parity_box_zoom_tiny_rejected` | <1 px drag → bounds unchanged both |
| T-ZOOM-015 | P | `parity_box_zoom_escape` | drag + Escape mid-drag → no bounds change, no lingering rect both |
| T-ZOOM-016 | W | `parity_box_zoom_rect_style` | during drag, band pixels visible clipped to axes — style normalized per RC-9 |
| T-ZOOM-017 | P | `parity_pan_primary` | primary drag +80 px → x bounds shift by `80/axes_w * span` sign-correct both (exists as `parity_primary_drag_pans` — strengthen to numeric) |
| T-ZOOM-018 | P | `parity_pan_live_x_transient` | **unpaused TimeScope**: primary x-pan → next frames x right-edge returns to `now` tracking (egui semantics; volcano must match — RC-2) |
| T-ZOOM-019 | P | `parity_pan_y_sticks` | y-pan persists while live both |
| T-ZOOM-020 | P | `parity_pan_paused` | paused → x-pan persists both |
| T-ZOOM-021 | P | `parity_dblclick_refit` | zoom then `double_click_at` → bounds == egui's `fit_bounds` result AND `auto_fit` flags match egui's post-dblclick state (RC-6 — today volcano sets auto_fit=true, egui does not) |
| T-ZOOM-022 | P | `parity_fit_x_button` | click "Fit X" → `x_axis.bounds` == `fit_x_bounds` result both |
| T-ZOOM-023 | P | `parity_fit_y_button` | symmetric |
| T-ZOOM-024 | P | `parity_fit_to_view_button` | "Fit to View" → both bounds fitted, `FIT_TO_VIEW` event emitted both |
| T-ZOOM-025 | P | `parity_autofit_checkbox` | re-check "Auto Fit X" → x resumes data-follow, `user_bounds` cleared both |
| T-ZOOM-026 | P | `parity_keep_max_fit` | `keep_max_fit` → bounds only expand on new extremes both |
| T-ZOOM-027 | P | `parity_zoom_persists_past_live_update` (exists `volcano_zoom_persists_past_live_update`) | zoom → feed → y-zoom kept; x tracks with new span (redefined by RC-2) |
| T-ZOOM-028 | P | `parity_bounds_change_event` | each zoom/pan/box → `ZOOM|PAN` event with matching `view_change` ranges (volcano emits only `pending_view_change` today — RC-6) |

### 6.5 CAT-SCRL — autoscroll & time window

The reported bug zone. Reference semantics (egui): **unpaused
TimeScope always shows `(now − time_window, now)`**; zoom changes
`time_window`, not tracking. Verify per-frame, not just end-state.

| id | bed | test | asserts |
|----|-----|------|---------|
| T-SCRL-001 | P | `parity_autoscroll_advances` (exists `parity_autoscroll_advances_bounds`) | feed t0=0 then t0=40 → `x.bounds.1` advances both |
| T-SCRL-002 | P | `parity_autoscroll_span_kept` | zoom x to 2 s span while live → 20 steps → right edge `≈ now`, span still 2 s (**volcano fails today — RC-2**) |
| T-SCRL-003 | P | `parity_time_window_updated_on_zoom` | after x-zoom `data.time_window == x1−x0` both (volcano never sets it — RC-2) |
| T-SCRL-004 | P | `parity_autoscroll_after_box_zoom` | box-zoom narrow band → x tracks `now` at new span, y stays band both |
| T-SCRL-005 | P | `parity_autoscroll_after_wheel` | wheel-zoom then feed → span preserved, edge tracks both |
| T-SCRL-006 | P | `parity_pan_then_recenter` | live x-pan → within N frames bounds recenter to `(now−tw, now)` both (egui semantics from `live_update`) |
| T-SCRL-007 | P | `parity_pause_freezes_scroll` | pause → bounds frozen while feeding; both (exists `volcano_epoch_autoscroll_advances` covers live arm) |
| T-SCRL-008 | P | `parity_resume_recenters` | paused+x-pan → resume → `(now−tw, now)` both |
| T-SCRL-009 | P | `parity_paused_recenter_math` | paused `live_update` shrinks bounds toward `time_window` symmetrically (scope.rs:541–543) both |
| T-SCRL-010 | P | `parity_time_slider` | drag Time Window slider → `time_window` + `view_change` event; window applies immediately both |
| T-SCRL-011 | P | `parity_measurement_range_extends` | `measurement_x_range` set → lower bound extended (scope.rs:534–538) both |
| T-SCRL-012 | P | `parity_epoch_autoscroll` | epoch feed + zoom-in → still scrolls + f32 translate keeps trace visible both (combine `volcano_epoch_*` regressions + RC-2) |
| T-SCRL-013 | P | `parity_zoomed_scroll_marker_stays` | while zoomed-live, a data marker placed at `now−ε` remains right-edge-anchored over frames both |

### 6.6 CAT-HOV — hover options

| id | bed | test | asserts |
|----|-----|------|---------|
| T-HOV-001 | P | `parity_hover_coordinate_tooltip` | `hover_at(axes.center())` → tooltip layer shows `x = …` text (query a11y tree / paint check) both (RC-13 — volcano has none) |
| T-HOV-002 | P | `parity_hover_tooltip_values` | pointer at known data pos → tooltip values == `format_value` output both |
| T-HOV-003 | P | `parity_hover_trace_dim` | set `traces.hover_trace=[a]` → non-hovered traces drawn at α≈40 both (pixel alpha sample / upload spy) |
| T-HOV-004 | P | `parity_hover_trace_emphasis` | hovered trace width ×1.6 in upload/`Points` both |
| T-HOV-005 | P | `parity_hover_legend_entry` | hover legend row → `hovered_legend_item` set → same dim/emphasis as T-HOV-003 both |
| T-HOV-006 | P | `parity_hover_leaves_no_state` | move pointer out → all styles revert both |
| T-HOV-007 | P | `parity_pointer_coordinate_overlay` | measurement overlay's `pointer_coordinate()` readout == `to_data` both (backend method parity) |

### 6.7 CAT-CLK — click, measurement capture, pause-on-click

| id | bed | test | asserts |
|----|-----|------|---------|
| T-CLK-001 | P | `parity_clicked_point_coords` | `click_at` inside axes at pos p → `clicked_point` == `plot_from_screen(p)` ±1 px-worth both (**needs RC-1**) |
| T-CLK-002 | P | `parity_clicked_screen_pos` | `clicked_screen_pos == [p.x, p.y]` both |
| T-CLK-003 | P | `parity_click_log_coords` | log axis on → clicked_point convention identical (verify against `capture_clicked_plot_point`) both |
| T-CLK-004 | P | `parity_click_event_emitted` | `CLICK` event with `screen_pos`+`plot_pos` meta both (volcano emits none — RC-6) |
| T-CLK-005 | P | `parity_click_pause_on_click` | `pause_on_click` → click toggles pause + `PAUSE`/`RESUME` event both |
| T-CLK-006 | P | `parity_click_measurement_capture` | `measurement_active` + click → `paused`, snapshot taken, `pending_pause` both |
| T-CLK-007 | P | `parity_dblclick_event` | `DOUBLE_CLICK|FIT_TO_VIEW` event + `ClickMeta` both (RC-6) |
| T-CLK-008 | P | `parity_click_outside_axes` | click in margin band → no `clicked_point` both |

### 6.8 CAT-XY — XY scope

| id | bed | test | asserts |
|----|-----|------|---------|
| T-XY-001 | P | `parity_xy_renders_pair` (exists `parity_xy_scope_renders_pair`) | `rendered_this_frame`, bounds cover joined data both |
| T-XY-002 | P | `parity_xy_viewport_fits_pairs` | bounds ≈ joined data extent ±pad — **volcano fits only `trace_order` → wrong today (RC-11)** |
| T-XY-003 | P | `parity_xy_join_tolerance` | offset ys timestamps by 5e-10 → joined; by 2e-9 → fewer/none both |
| T-XY-004 | W | `parity_xy_curve_pixels` | colored px inside axes + centroid match ±2 px both |
| T-XY-005 | P | `parity_xy_pair_style` | pair color/width/style forwarded (FFI spy or px) both |
| T-XY-006 | P | `parity_xy_show_points` | `pair_look.show_points` → markers rendered AND line kept (RC-4/RC-14) both |
| T-XY-007 | P | `parity_xy_highlight_newest` | newest-point marker drawn + legend "(last)" entry both |
| T-XY-008 | P | `parity_xy_pair_hidden` | legend-hide pair → surface stops uploading `xy:` trace both |
| T-XY-009 | P | `parity_xy_member_hidden` | `x_tr.look.visible=false` → pair skipped on fit AND render both |
| T-XY-010 | P | `parity_xy_offsets` | `x_tr.offset`/`y_tr.offset` applied to derived points both |
| T-XY-011 | P | `parity_xy_hover` | hover either member → pair emphasized both |
| T-XY-012 | P | `parity_xy_log_scale` | log x/y on XYScope → log10 transform on derived pts both (surface missing — RC-11) |
| T-XY-013 | P | `parity_xy_axis_dragvalues` | Min/Max DragValues → bounds + `view_change` both |
| T-XY-014 | P | `parity_xy_empty_pair_safe` | pair with no overlapping timestamps → no crash, no bogus trace both |
| T-XY-015 | P | `parity_xy_multi_pair` | two pairs → two vp traces + two legend entries both |
| T-XY-016 | P | `parity_xy_zoom_spread` | regression "spread over whole y": zoom into pair → curve geometry scales around anchor both |

### 6.9 CAT-FFT — embedded FFT scope

| id | bed | test | asserts |
|----|-----|------|---------|
| T-FFT-001 | P | `parity_fft_scope_renders` (exists `parity_fft_scope_renders`) | surface initialized + `rendered_this_frame` both |
| T-FFT-002 | W | `parity_fft_labels` | "Frequency"/"Magnitude" text present both (regression: dangling CString) |
| T-FFT-003 | P | `parity_fft_legend_independent` | FFT scope's `PlotMemory` separate from main scope (`volcano_legend_independent_fft`) both |
| T-FFT-004 | P | `parity_fft_short_margins` | strip height < 160 px → reduced margin table applied → axes rect still aligned (RC-1 second branch) |
| T-FFT-005 | P | `parity_fft_interactions` | zoom/pan/box on FFT scope work identically both |

### 6.10 CAT-TRC — trace styling

| id | bed | test | asserts |
|----|-----|------|---------|
| T-TRC-001 | W | `parity_style_dashed` | dashed pattern pixels (row profile has gaps) both |
| T-TRC-002 | W | `parity_style_dotted` | both |
| T-TRC-003 | W | `parity_show_points_keeps_line` | `show_points` → line AND markers present (volcano: needs Line+Points — RC-14) |
| T-TRC-004 | P | `parity_marker_shapes` | cycle `MarkerShape` → forwarded (or documented N/A + UI hides control) |
| T-TRC-005 | P | `parity_point_size` | `point_size` honored both |
| T-TRC-006 | P | `parity_line_width` | width=4 → thicker colored band both |
| T-TRC-007 | P | `parity_trace_offset` | `tr.offset` shifts curve by offset in y both |
| T-TRC-008 | P | `parity_trace_color` | `set_color` → exact RGBA8 in vp_trace_set + px both |
| T-TRC-009 | P | `parity_render_mode_hidden` | render-mode combo hidden in volcano mode; raw `live` len uploaded (assert FFI receives `tr.live.len()`) |
| T-TRC-010 | P | `parity_many_traces` | 8 traces → 8 uploads + 8 legend entries + correct order both |
| T-TRC-011 | P | `parity_trace_removal` | remove trace → `vp_trace_remove` called, legend shrinks both |
| T-TRC-012 | P | `parity_pending_trace_style` | restored pending style applies on first data both |

### 6.11 CAT-OVL — overlays

All drawn through `draw_overlays` → `&mut dyn PlotBackend`; the
assertion is that `VolcanoPainterBackend` output lands on the same
pixels as `EguiBackend` output (post-RC-1).

| id | bed | test | asserts |
|----|-----|------|---------|
| T-OVL-001 | W | `parity_threshold_hline_position` | threshold at y=v → horizontal line pixels at `axes.bottom − (v−y0)/(y1−y0)·h` ±2 px both |
| T-OVL-002 | P | `parity_threshold_colors` | threshold color + style identical both |
| T-OVL-003 | P | `parity_trigger_marker` | trigger level line + edge marker same position both |
| T-OVL-004 | P | `parity_measurement_markers` | measurement X/Y markers land at captured coords ±2 px both |
| T-OVL-005 | P | `parity_cursor_readout` | overlay `pointer_coordinate()` agrees with egui readout both |
| T-OVL-006 | P | `parity_overlay_text` | `Text` items (labels) render same place both |
| T-OVL-007 | P | `parity_overlay_hidden_excluded` | legend-hidden overlays excluded from bounds both (RC-3 publish) |
| T-OVL-008 | P | `parity_backend_zoom_hook` | any overlay caller of `zoom_bounds_around_hovered` — implement or prove unused (RC-15) |

### 6.12 CAT-PAU — pause / resume / snapshots

| id | bed | test | asserts |
|----|-----|------|---------|
| T-PAU-001 | P | `parity_pause_button` | "Pause Scope" → `paused`, snapshot taken, data frozen on screen both |
| T-PAU-002 | P | `parity_resume_button` | resume → live resumes, bounds recenter both |
| T-PAU-003 | P | `parity_paused_render_source` | paused → surface uploads `snap` not `live` both |
| T-PAU-004 | P | `parity_pause_propagates` | `pending_pause` flag + events identical both |
| T-PAU-005 | P | `parity_paused_xy_uses_snap` | XY join uses `get_points_ref(paused)` → identical derived set both |

### 6.13 CAT-PNL — side panels

Every panel is pure egui chrome sitting next to the plot — they can't
diverge *visually*, but their effects on the scope must land identically:

| id | bed | test | asserts |
|----|-----|------|---------|
| T-PNL-001 | P | `parity_traces_panel_eye` | visibility eye → hidden in legend + surface both |
| T-PNL-002 | P | `parity_traces_panel_look` | color/width/style edits apply both |
| T-PNL-003 | P | `parity_traces_panel_xy_editor` | pair created via panel → renders both |
| T-PNL-004 | P | `parity_math_channel` | formula trace renders + legend entry both |
| T-PNL-005 | P | `parity_threshold_builder` | threshold added via panel → overlay both |
| T-PNL-006 | P | `parity_trigger_builder` | trigger → overlay + fired events both |
| T-PNL-007 | P | `parity_measurement_panel` | two clicks → markers + delta readout both |
| T-PNL-008 | P | `parity_colorscheme_panel` | palette change → new trace colors both; background forwarded (RC-15) |
| T-PNL-009 | P | `parity_export_screenshot` | screenshot request → PNG written contains plot incl. volcano texture both |
| T-PNL-010 | P | `parity_hotkeys_panel` | hotkey actions (pause, fit) behave same both |
| T-PNL-011 | P | `parity_scope_menu_toggles` | menu checkboxes (grid/legend/info/position) → same effect both |
| T-PNL-012 | P | `parity_controls_in_toolbar` | controls render in toolbar mode and menu mode both |

### 6.14 CAT-CTRL — external controllers

| id | bed | test | asserts |
|----|-----|------|---------|
| T-CTRL-001 | P | `parity_ctrl_set_visible` | `TracesController.set_visible(false)` → `hidden_items` + surface skip both (exists partially: `volcano_scope_respects_hidden_items`) |
| T-CTRL-002 | P | `parity_ctrl_set_color` | both |
| T-CTRL-003 | P | `parity_ctrl_axis_limits` | axis limit requests → bounds both |
| T-CTRL-004 | P | `parity_ctrl_add_remove_scope` | scope add/remove → surfaces created/destroyed both |
| T-CTRL-005 | P | `parity_ctrl_threshold` | both |
| T-CTRL-006 | P | `parity_ctrl_fft` | both |
| T-CTRL-007 | A | `parity_ctrl_save_state` | `LivePlotController.save_state` file parity — loaded volcano scope restores `volcano_enabled`, legend pos, hidden items, bounds |
| T-CTRL-008 | A | `parity_ctrl_event_stream` | `subscribe_all` → identical kind sequence for the same input script both |

### 6.15 CAT-EVT — event bus

| id | bed | test | asserts |
|----|-----|------|---------|
| T-EVT-001 | P | `parity_evt_view_change_zoom` | wheel → `ZOOM` + ranges both |
| T-EVT-002 | P | `parity_evt_view_change_pan` | drag → `PAN` both |
| T-EVT-003 | P | `parity_evt_view_change_fit` | Fit button → `FIT_TO_VIEW` both |
| T-EVT-004 | P | `parity_evt_click` | `CLICK` + meta both (RC-6) |
| T-EVT-005 | P | `parity_evt_pause_resume_click` | `CLICK|PAUSE`, `CLICK|RESUME` both |
| T-EVT-006 | P | `parity_evt_double_click` | `DOUBLE_CLICK|FIT_TO_VIEW` both (RC-6) |
| T-EVT-007 | P | `parity_evt_trace_visibility` | legend toggle → visibility event both |
| T-EVT-008 | P | `parity_evt_threshold_fired` | identical meta both |
| T-EVT-009 | P | `parity_evt_no_spurious` | idle frames after action → no duplicate events both |

### 6.16 CAT-PER — persistence & undo

| id | bed | test | asserts |
|----|-----|------|---------|
| T-PER-001 | A | `parity_state_roundtrip` | zoom/legend-pos/hidden/pause → save → new app load → identical `ScopeData` both (incl. `volcano_enabled`) |
| T-PER-002 | A | `parity_legend_position_persist` | all 4 positions round-trip both (was a TODO in old plan) |
| T-PER-003 | A | `parity_undo_settings` | undo of view/settings changes both |
| T-PER-004 | P | `parity_trace_order_restore` | pending order restored both |

### 6.17 CAT-DND — drag & drop

| id | bed | test | asserts |
|----|-----|------|---------|
| T-DND-001 | P | `parity_drop_trace_on_scope` | drag trace row → scope rect → trace appears + legend entry both (**volcano: `handle_trace_drop` never runs — RC-7**) |
| T-DND-002 | P | `parity_drop_xy_pair` | XY pair drag onto XYScope both |
| T-DND-003 | P | `parity_scope_reorder` | scope reorder via dnd → surfaces follow their scope both |
| T-DND-004 | P | `parity_drag_onto_legend_region` | drop over legend rect still hits scope both |

### 6.18 CAT-CFG — config flags

| id | bed | test | asserts |
|----|-----|------|---------|
| T-CFG-001 | P | `parity_cfg_top_bar` | `top_bar`/`buttons` flags render identically both |
| T-CFG-002 | P | `parity_cfg_max_fps` | under fps cap volcano still advances (pending worker doesn't stall) both |
| T-CFG-003 | P | `parity_cfg_show_grid` | grid on/off honored in FFI (needs `vp_set_grid` — RC-12) both |
| T-CFG-004 | P | `parity_cfg_show_legend_flag` | `show_legend=false` → no overlay both |
| T-CFG-005 | P | `parity_cfg_autofit_startup` | `auto_fit_to_view` start → initial bounds equal both |
| T-CFG-006 | P | `parity_cfg_color_scheme_bg` | custom bg color → `set_background_rgba` gets it both |
| T-CFG-007 | P | `parity_cfg_headline` | custom headline widget renders both |
| T-CFG-008 | P | `parity_env_flag` | `LIVEPLOT_VOLCANO=1` seeds `volcano_enabled` for new scopes |

### 6.19 CAT-A11Y — accessibility

| id | bed | test | asserts |
|----|-----|------|---------|
| T-A11Y-001 | P | `parity_a11y_widget_count` | volcano widget adds ≤ egui plot's unnamed-interactive count (existing `unnamed_interactive_widgets_do_not_increase` extended to volcano arm) |

### 6.20 CAT-PERF — performance invariants

| id | bed | test | asserts |
|----|-----|------|---------|
| T-PERF-001 | P | `perf_worker_pipelined` | while worker busy, `step()` returns without joining (measure step time < worker render time) |
| T-PERF-002 | P | `perf_no_upload_when_idle` | unchanged sig → no `vp_trace_set`, no render (counter) |
| T-PERF-003 | P | `perf_raw_points_uploaded` | FFI receives `tr.live.len()` points (GPU downsamples — the feature's selling point) |
| T-PERF-004 | P | `perf_one_surface_per_scope` | `volcano_surface` count == volcano scopes; destroyed with scope |
| T-PERF-005 | P | `perf_dropped_frame_counter` | `vp_render_rgba` size-miss → `volcano_dropped_frames` increments, next frame recovers |
| T-PERF-006 | P | `perf_stress_stream` | `LIVEPLOT_STRESS=1`: 200k pts/streaming → step time volcano ≤ egui envelope path (informational gate) |

### 6.21 CAT-PIX — pixel / snapshot parity

| id | bed | test | asserts |
|----|-----|------|---------|
| T-PIX-001 | W | `snap_parity_time_scope` | reference pair; region asserts: margin uniform, axes rect located identically (row/col profile edges), colored bbox match ±2 px |
| T-PIX-002 | W | `snap_parity_impulse_marker` | marker at (5, 0.5) → centroid diff ±2 px |
| T-PIX-003 | W | `snap_parity_overlays` | hline+vline+marker overlay → pixel positions match |
| T-PIX-004 | W | `snap_parity_legend` | legend rect region: both have >4 colors, similar size |
| T-PIX-005 | W | `snap_parity_xy` | XY curve bbox match |
| T-PIX-006 | W | `snap_parity_dark_light` | toggle `ctx.set_visuals` → both recolor margins |
| T-PIX-007 | W | `snap_parity_zoomed` | post-zoom frame: trace pixels occupy same sub-region |
| T-PIX-008 | W | `snap_parity_fft_strip` | short-strip variant |

---

## 7. Fix work items

Ordered by dependency — RC-1 first (many tests depend on correct
geometry), then RC-2/RC-3 (the reported bugs), then polish.

### W-1 — Unify axes-rect computation (fixes RC-1) — *prerequisite for ~40 % of the catalogue*

- In `scope_ui.rs`, extract the margin table:
  ```rust
  fn volcano_margins_px(h: u32) -> (f32, f32, f32, f32) {
      if (h as f32) < 160.0 { (52.0, 10.0, 34.0, 6.0) } else { (60.0, 12.0, 56.0, 8.0) }
  }
  ```
- `render_volcano_plot` computes `axes` from `rect` + these px values
  (delete constants ML/MR/MB/MT) and passes the same px margins to
  `surf.sync_and_render` (new arg replacing the internal table).
- Record into `data.last_axes_screen_rect` (new field, INFRA-2);
  also record on egui path from `transform.frame()`.
- Regression: T-LGD-002, T-CLK-001, T-OVL-001, T-RND-006.

### W-2 — Rework `user_bounds` ownership to mirror egui live semantics (fixes RC-2)

- In `render_volcano_plot`: when `!data.paused && scope_type==TimeScope`,
  take x from `data.x_axis.bounds` (live_update's output); take y from
  `user_bounds` if present. When `paused` or `XYScope`, both axes from
  `user_bounds`.
- On `bounds_changed` (pan/zoom/box on x): write `data.time_window =
  x1 − x0` exactly like egui's read-back does; set `auto_fit=false`.
- `user_bounds` update logic stays but stores the *post-input* x only
  when paused (else the trailing-window math owns it); always stores y.
- Delete the "compounds on user_bounds" carve-out for x — replace with
  the same recentering the egui path gets for free.
- Regression: T-SCRL-002..006, T-ZOOM-018/020, T-ZOOM-027.

### W-3 — Extract + share legend hidden→visible sync (fixes RC-3)

- New method `ScopePanel::sync_legend_hidden_to_looks(&mut self, ctx,
  plot_id, traces)` containing the code at 2104–2146 verbatim.
- Call it from `render_volcano_plot` after the legend block and keep
  the egui call site. Ensure it runs even when `show_legend==false`
  for `legend_hidden_items` publication (same guard as egui).
- Regression: T-LGD-004/005/009/016, T-XY-008/009.

### W-4 — Share legend-entry construction (fixes RC-4 + drift risk)

- New helper `build_legend_entries(&ScopeData, &TracesCollection) ->
  Vec<LegendEntryData>` producing time-trace + XY entries with the
  egui-format labels (`"y vs x"`, ` — info`, `"(last)"`).
- Both paths call it; the volcano path stops building its own list.
- XY `(last)` point marker drawn via `draw_overlays` (it already
  receives `legend_entries`) or painted as `Points` through the
  backend in the volcano branch — decide by where the egui path keeps
  it; keep item ids via `xy_last_point_item_id`.
- Regression: T-LGD-010/011/012, T-XY-007.

### W-5 — Fix `last_plot_screen_rect` layout (fixes RC-5)

One-line fix: store `[left, top, right, bottom]`. Audit every
consumer (tests use `rect[2]` as right edge). Regression: T-RND-006.

### W-6 — Route volcano clicks through `handle_plot_click` (fixes RC-6)

- Refactor: `handle_plot_click` currently needs `PlotResponse`; make it
  take `(&Response, &dyn PlotBackend)` + the two `transform` calls it
  uses (`value_from_position` == `plot_from_screen`). EguiBackend/
  VolcanoPainterBackend already expose `plot_from_screen` and
  `response()`.
- Volcano path calls it with `&resp` + `&vpb` — gains double-click
  fit, events, `pending_pause`, `capture_clicked_plot_point` for free.
- **Double-click semantics**: after refactor the volcano path must NOT
  also set `auto_fit=true` — egui fits once via `fit_bounds`; match it
  (remove lines 1238–1242, let `handle_plot_click` own double-click).
- Regression: T-CLK-001..008, T-EVT-004..006, T-ZOOM-021.

### W-7 — Re-enable `handle_trace_drop` (fixes RC-7)

Call `self.handle_trace_drop(ui, &resp)` at the end of
`render_volcano_plot`. Regression: T-DND-001/002/004.

### W-8 — Port egui wheel-zoom semantics (fixes RC-8)

- Replace the custom factor with `zoom_factor = 1 + scroll·0.001`
  applied around `to_data(hover)` the same way
  `zoom_bounds_around_hovered` does (`new = c + (old−c)/factor`).
- Honor `scroll_data.x` for x-zoom; guard `resp.hovered()`; add the
  `over_legend` check against `legend_overlay_rect`; early-out
  completely on `ZoomMode::Off` (no `auto_fit` change, no
  `bounds_changed`).
- Keep `bounds_changed` accounting → `time_window` update (W-2).
- Regression: T-ZOOM-001..009, T-LGD-013.

### W-9 — Box-zoom honors ZoomMode + matching band (fixes RC-9)

- On release, apply x/y subsets per `zoom_mode` (X → full-height band,
  x-only bounds; Y → symmetric; Off → cancel).
- During drag, draw the same 4 px DARK_BLUE + 2 px WHITE strokes
  clipped to `axes` (copy the shape block at 2148–2190 keyed off
  `zoom_mode`).
- Escape clears `box_zoom_start` (already present — keep).
- Regression: T-ZOOM-011..016.

### W-10 — Replicate the 5 % viewport pad (fixes RC-10)

- In `sync_and_render`, when calling `vp_set_viewport`, expand
  `surf.bounds` by 5 % per side (matching egui's
  `x ± x_space`) — OR decide to remove the pad on the egui side for
  parity; whichever, both paths must map data→pixels identically.
  Preferred: pad inside the surface so `data.*_bounds` reporting stays
  identical.
- Careful: pad must apply to the *shifted* viewport and be consistent
  with the overlay transform — overlay maps `data.bounds` to `axes`
  1:1 while VolcanoPlot maps `bounds ± 5%` to its axes → so instead
  map the overlay transform to the padded bounds, or pad `axes`
  inward by 5 % of its size. Decide in implementation; write
  T-RND-007 FIRST (impulse-position test) to drive it.
- Regression: T-RND-007, T-PIX-001/002/003.

### W-11 — XY-aware fit + log + empty-pair safety (fixes RC-11, RC-15)

- `VolcanoSurface::fit_bounds` gains an `XYScope` branch: reuse
  `ScopeData::fit_x_bounds`/`fit_y_bounds` results by writing them
  into `surf.bounds` instead of re-implementing the join (the scope
  already computes correct XY bounds — just *use*
  `data.x_axis.bounds`/`data.y_axis.bounds` for XY like the live
  path does).
- Apply `x_axis.log_scale`/`y_axis.log_scale` log10 to derived XY
  points (mirror egui 1658–1675); skip non-positive → `NaN`.
- Guard `derived.is_empty()` before `vp_trace_set` (skip + mark
  `seen` to keep cleanup correct).
- Regression: T-XY-002/003/012/014, T-XY-016.

### W-12 — Axis labels, grid, tick suppression via FFI (fixes RC-12)

- FFI additions (vendor `volcano_ffi.{h,cpp}`): `vp_set_grid(int)`,
  `vp_set_show_tick_labels(int, int)` (x,y), optionally
  `vp_set_tick_formatter` later (deferred — visual-only).
- Rust: `set_axis_labels` input becomes `axis_label_text(traces,
  is_x)` output (auto-names) instead of `data.*_axis.name` — pass
  owned `CString`s for the *duration of the call* (already fixed
  once — keep the borrow pattern).
- Honor `hide_x_labels`/`hide_y_labels`/`show_grid` from the egui
  pre-computation (currently those vars are computed before the
  volcano branch — plumb them in).
- Regression: T-AX-001..012, T-CFG-003.

### W-13 — Hover features (fixes RC-13)

- In `render_volcano_plot` after overlays: if `resp.hovered()`, paint
  the same tooltip string as `label_formatter` (share the closure's
  body via a helper `fn hover_label(&ScopeData, name, x, y) ->
  String`) using `ui.ctx().debug_painter()`/egui tooltip layer at
  pointer pos.
- Fold `traces.hover_trace` into upload: dim color → alpha 40,
  emphasize width → ×1.6 — include hover state in `trace_sig` so the
  change re-uploads immediately.
- Regression: T-HOV-001..006, T-LGD-015.

### W-14 — Line+points style & markers (fixes RC-14)

- Option A (preferred): FFI `VolcanoLineStyle::LineAndPoints=4`
  (+ marker size arg) in `volcano_ffi` → keeps single upload.
- Option B: upload a second `vp_trace` with `PointsOnly` for the
  marker pass (extra FFI trace id per trace).
- Fold `point_size`/`marker` (approximate to circle/diamond/square)
  into the upload; pair_look same.
- If markers can't match egui's shape set, gate the marker UI in
  volcano mode and document.
- Regression: T-TRC-003/004/005, T-XY-006/007.

### W-15 — Events parity sweep (fixes RC-6 tail)

- After W-6, run CAT-EVT — fill any missing `emit_filtered` calls in
  the volcano input handling (view change already emitted via
  `pending_view_change`; add the direct `ZOOM|PAN` event that egui
  emits at 2228–2239).
- Regression: T-EVT-001..009.

### W-16 — Legend suppression parity

- Use `total_widget_size` for `min_*_for_legend` checks in
  `render_volcano_plot` (match 1416–1420) — currently uses `rect`.
- Regression: T-LGD-014.

### W-17 — `VolcanoPainterBackend::zoom_bounds_around_hovered`

- Implement against `self.bounds`/`self.transform` (scale range by
  `1/factor` around pointer) OR prove no overlay caller uses it and
  mark `unreachable!()`-safe. Grep first.
- Regression: T-OVL-008.

### W-18 — FFI robustness additions

- `vp_set_grid`, `vp_set_tick_labels_visible` (W-12).
- `vp_render_rgba` size-miss counter → `data.volcano_dropped_frames`.
- `vp_trace_set` early-out on `n==0` inside the FFI itself (defense
  in depth beyond the Rust-side skip).
- `LIVEPLOT_VOLCANO_DISABLE` escape hatch documented in README.

### W-19 — Snapshot & screenshot path

- Verify `request_screenshot` produces a PNG including the volcano
  texture (the texture is an egui image → included by construction;
  test T-PNL-009 proves it).
- `zz_snap.rs` content merges into `volcano_parity.rs` named tests;
  file deleted at phase end.

### W-20 — Docs

- Update `docs/volcano-integration.md` (margin contract, user_bounds
  semantics, pad, FFI additions) and `docs/e2e-parity-plan.md` →
  point here. CHANGELOG entry per fixed RC.

---

## 8. Execution phases & acceptance gates

### Phase P0 — Harness & truth fields (no behavior changes)

- INFRA-1..8 landed; `last_axes_screen_rect` on both paths.
- All existing tests green on both paths (baseline).
- Deliverable: PR "test: parity harness upgrades" — pure test/field
  additions, zero `src/` behavior change beyond the new field.

**Gate:** `cargo test --features volcano` green; `cargo test` (no
features) green.

### Phase P1 — Geometry fixes (W-1, W-5, W-10)

- Land axes-rect unification, rect-layout fix, pad decision.
- New tests from CAT-RND/PIX that encode geometry pass on volcano.

**Gate:** T-LGD-002, T-CLK-001, T-RND-006, T-RND-007, T-OVL-001 green;
full suite green.

### Phase P2 — Reported bugs (W-2, W-3, W-4, W-11)

- Autoscroll-when-zoomed fixed + regression set.
- Legend sync + shared entries.
- XY viewport/join/log/labels.
- This is the user-visible milestone — after it the three reported
  bugs are provably gone.

**Gate:** T-SCRL-002/003, T-LGD-004..016, T-XY-002/008/012/016 green;
`real_app_volcano_legend` moved from `zz_snap` into
`volcano_parity.rs` still green.

### Phase P3 — Interaction fidelity (W-6..W-9, W-13, W-15..W-17)

- Shared click handler, DnD, wheel/box semantics, hover, events.

**Gate:** all of CAT-ZOOM, CAT-CLK, CAT-HOV, CAT-EVT, CAT-DND green.

### Phase P4 — Styling & config (W-12, W-14, W-18..W-20)

- FFI flags (grid/labels), Line+Points, docs, `zz_snap` merged.

**Gate:** full catalogue green both paths; `cargo check --features fft`
and `cargo check` (no volcano) unaffected; CHANGELOG updated.

### Acceptance — "done" definition

- Every T-* test passes identically for `volcano_enabled ∈ {false, true}`.
- `cargo test --features volcano --test e2e` — 0 failures, and the
  parity module count ≥ the catalogue size (~170 tests).
- Snapshot pairs regenerated and reviewed once by a human (record
  reviewer + date in this file).
- No `#[ignore]` in the parity suite except `LIVEPLOT_STRESS`-gated.

---

## 9. Commands & CI wiring

```bash
# Full parity suite (needs Vulkan driver or llvmpipe + submodule build)
LIVEPLOT_VOLCANO=1 cargo test --features volcano --test e2e

# Parity module only
LIVEPLOT_VOLCANO=1 cargo test --features volcano --test e2e volcano_parity

# Pixel-level (wgpu harness; skips without adapter)
LIVEPLOT_VOLCANO=1 cargo test --features volcano --test e2e parity_

# Regenerate reference PNGs
LIVEPLOT_SNAPSHOT=1 LIVEPLOT_VOLCANO=1 cargo test --features volcano --test e2e snap_

# Stress
LIVEPLOT_STRESS=1 LIVEPLOT_VOLCANO=1 cargo test --features volcano --test e2e -- --ignored

# Non-volcano build unaffected
cargo test --test e2e
cargo check --features fft
```

CI notes:

- The `volcano` job needs: `git submodule update --init --recursive`,
  cmake, g++ (C++23), `libvulkan.so.1` (or `vulkan-sdk` vendored path),
  `mesa-vulkan-drivers` for llvmpipe so wgpu *and* VolcanoPlot both get
  software adapters on headless runners.
- Keep the non-volcano job pristine — `volcano` code must stay
  `#[cfg(feature = "volcano")]`-clean (regression
  `volcano_egui_build_without_volcano`).

---

## 10. Appendices

### Appendix A — Parity test skeleton

```rust
/// T-SCRL-002: zoomed x keeps tracking `now` at the new span.
#[test]
fn parity_autoscroll_span_kept() {
    for vol in [false, true] {
        let mut bed = make_test_bed();
        feed_sine(&bed, "sig", 4000, 1.0, 100.0, 0.0);
        scope_mut(&mut bed, 0).volcano_enabled = vol;
        steps(&mut bed, 20);
        // Zoom x to ~2 s via box zoom inside the axes rect.
        let axes = axes_rect(&bed, 0);
        drag_path(&mut bed,
                  egui::pos2(axes.center().x - 40.0, axes.center().y),
                  &[egui::pos2(axes.center().x + 40.0, axes.center().y)],
                  egui::PointerButton::Secondary);
        steps(&mut bed, 10);
        let span = {
            let (xb, _) = scope(&bed, 0).last_plot_bounds.unwrap();
            xb[1] - xb[0]
        };
        feed_sine(&bed, "sig", 2000, 1.0, 100.0, 60.0); // newer data
        steps(&mut bed, 15);
        let (xb, _) = scope(&bed, 0).last_plot_bounds.unwrap();
        let now = 60.0 + 2000.0 / 100.0;
        assert!((xb[1] - xb[0] - span).abs() < span * 0.05,
                "span must persist (volcano={vol})");
        assert!((xb[1] - now).abs() < 0.5,
                "right edge must track `now` (volcano={vol}): {xb:?}");
    }
}
```

### Appendix B — Pixel helper skeleton

```rust
fn centroid_of(img: &RgbaImage, r: egui::Rect,
               pred: impl Fn([u8;4]) -> bool) -> Option<(f32,f32)> {
    let (mut sx, mut sy, mut n) = (0f32, 0f32, 0u32);
    for y in r.top() as u32..r.bottom() as u32 {
        for x in r.left() as u32..r.right() as u32 {
            if pred(img.get_pixel(x, y).0) { sx += x as f32; sy += y as f32; n += 1; }
        }
    }
    (n > 0).then_some((sx / n as f32, sy / n as f32))
}
```

### Appendix C — Where each user-visible control lives

| control | code | tests |
|---|---|---|
| Pause/Resume btn | scope_ui.rs:557–570 | T-PAU-001/002 |
| Time Window slider | 580–613 | T-SCRL-010 |
| X Min/Max DragValue (XY) | 614–645 | T-XY-013 |
| Fit X + Auto Fit X + Only expand | 653–678 | T-ZOOM-022/025/026 |
| X log scale + unit (XY/menu) | 683–709 | T-AX-012/013, T-XY-012 |
| X Label menu | 711–750 | T-AX-004/006 |
| X Format menu (time/value) | 752–804 | T-AX-010/011 |
| Y Min/Max DragValue | 819–840 | T-ZOOM-023-area |
| Fit Y + Auto Fit Y | 844–876 | T-ZOOM-023/025 |
| Y log scale + unit + label + format | 878–994 | T-AX-* |
| Zoom mode selector Off/X/Y/Both | 999–1010 | T-ZOOM-004/005/012/013 |
| Fit to View | 1014–1039 | T-ZOOM-024, T-EVT-003 |
| Save Screenshot | 1043–1050 | T-PNL-009 |
| Menu: Controls in Toolbar | 449–454 | T-PNL-012 |
| Menu: Show Grid | 462–468 | T-AX-008, T-CFG-003 |
| Menu: Show Legend / Show Info / Position | 469–505 | T-LGD-011/014, T-LGD-003 |
| Wheel zoom | 1531–1571 (egui), 1174–1205 (vol) | T-ZOOM-001..009 |
| Primary pan | 1528–1529 (egui), 1164–1173 (vol) | T-ZOOM-017..020 |
| Box zoom | 1573–1616 + 2148–2190 (egui), 1206–1237 (vol) | T-ZOOM-011..016 |
| Double-click fit | 2358–2397 (egui), 1238–1242 (vol) | T-ZOOM-021, T-EVT-006 |
| Click / measure / pause-on-click | 2351+ (egui), 1243–1266 (vol) | T-CLK-* |
| Hover tooltip | 1494–1515 (egui label_formatter) | T-HOV-001/002 |
| Hover trace dim/emphasis | 1968–1981, 1710–1723 (egui) | T-HOV-003..005 |
| Legend overlay + toggles | 2046–2138 (egui), 1338–1389 (vol) | T-LGD-* |
| Trace DnD onto scope | 2255+ (egui only) | T-DND-001 |
| autoscroll / live_update | scope.rs:517–545 | T-SCRL-* |

### Appendix D — Debugging recipes

- **Dump a frame**: `LIVEPLOT_SNAPSHOT=1` + `bed.harness.render()` →
  save; the `zz_snap` pattern already does this to `/tmp`.
- **Volcano texture only**: temporarily paint the texture rect with a
  debug border; or call `surf`-level render in `tests/volcano_ffi.rs`
  and `img.save` the raw RGBA.
- **Input replay**: every failing test logs the input script; repro in
  `examples/sine` manually — `LIVEPLOT_VOLCANO=1 cargo run --features
  volcano --example sine`.
- **Bounds trace**: eprintln `data.x_axis.bounds`, `surf.bounds`,
  `surf.user_bounds` per step — the RC-2 desync shows immediately.
- **FFI solo**: `tests/volcano_ffi.rs` renders without egui — isolates
  "upstream vs integration".

### Appendix E — Risk register

| risk | mitigation |
|---|---|
| No Vulkan on CI | llvmpipe (`mesa-vulkan-drivers`); tests skip gracefully via `vol_available()` |
| wgpu adapter missing | `wgpu_bed()` already returns `Option`; pixel tests skip |
| Volcano worker thread flakes in tests | join-once semantics already in `sync_and_render`; tests use `steps(n)` settling |
| f32 precision at epoch | keep shift tests T-SCRL-012, T-AX-007 mandatory |
| Upstream FFI changes | pin submodule; FFI tests in `tests/volcano_ffi.rs` gate upgrades |
| Margin contract drift | single source (W-1) + FFI margin round-trip test (INFRA-7) |
| Over-strict pixel asserts flaky | ±2 px tolerances, region stats, never whole-image diff |

### Appendix F — Test-count budget

| category | new tests | existing folded in |
|---|---|---|
| CAT-RND | 8 | 2 |
| CAT-AX | 14 | 1 |
| CAT-LGD | 13 | 4 |
| CAT-ZOOM | 26 | 2 |
| CAT-SCRL | 11 | 2 |
| CAT-HOV | 7 | 0 |
| CAT-CLK | 8 | 0 |
| CAT-XY | 14 | 2 |
| CAT-FFT | 4 | 1 |
| CAT-TRC | 12 | 0 |
| CAT-OVL | 8 | 0 |
| CAT-PAU | 5 | 0 |
| CAT-PNL | 12 | 0 |
| CAT-CTRL | 8 | 0 |
| CAT-EVT | 9 | 0 |
| CAT-PER | 4 | 0 |
| CAT-DND | 4 | 0 |
| CAT-CFG | 8 | 0 |
| CAT-A11Y | 1 | 0 |
| CAT-PERF | 6 | 0 |
| CAT-PIX | 8 | 0 |
| **total** | **190** | **14** |

Each runs twice (both backends) → ~380 assertions in the parity suite
alone, on top of the existing 156 e2e tests that keep guarding the
egui path.

---

## 11. Behavioral oracle — the exact egui semantics every test encodes

Parity tests are only as good as their specification of "correct".
This section is the oracle: for every interaction, what the egui path
does *today*, derived from the code — not what would be nice. Where
egui behavior is itself questionable, the test still pins egui
behavior (parity first; UX fixes are a separate change applied to
both paths).

### 11.1 Viewport state machine

State variables: `x_axis.bounds`, `y_axis.bounds`,
`x_axis.auto_fit`, `y_axis.auto_fit`, `time_window`, `paused`,
`zoom_mode`, plus egui_plot's internal transform/`auto_bounds` inside
`PlotMemory`. The volcano path's equivalent state is
`surf.bounds`/`surf.user_bounds`/`bounds_external` — the mapping
must be 1:1 for tests to compare.

Frame order on the egui path, every frame:

1. `ScopeData::update()` (scope.rs:451): `trace_order` prune,
   `fit_x_bounds` **iff** `x_axis.auto_fit` (cached on
   `(total_pts, max_ts)` key), `live_update()`,
   `fit_y_bounds` iff `y_axis.auto_fit`.
2. `live_update` (TimeScope, `!paused`):
   `x_axis.bounds = (min(meas_min, now − time_window), now)`.
   Paused: recenter bounds toward `time_window` symmetrically:
   `b += (span − tw)/2` per frame — i.e. paused x bounds *drift back*
   to the `time_window` span centered on the current view, one step
   per frame (diff halves each frame — exponential approach, NOT a
   snap). Tests must model exponential decay: after k frames the
   residual is `((span0 − tw)/2)·(1/2)^k`… actually `diff` is
   recomputed each frame from the *current* bounds, so each frame
   removes half the excess span — converges geometrically with ratio
   ½: `span_k = tw + (span_0 − tw)·2^(−k)`. Pin this in T-SCRL-009.
3. `render_plot` → `plot.show`: input mutates transform; when
   `!bounds_changed`, data bounds are re-applied padded ±5 %
   (`x_min − x_space ..= x_max + x_space`, x_space = 5 % of span;
   same for y).
4. Read-back (2196–2246): if the closure returned `bounds_changed`,
   data bounds ← transform bounds **shrunk** by `space·(0.05/1.1)`
   per side and `time_window ← inner x span`. Changed axis → its
   `auto_fit = false`.

Net effect worth memorizing for tests:

- Wheel zoom on x while live: span shrinks, `time_window` ← new
  span, right edge keeps tracking `now`. **Autoscroll continues
  zoomed.** (The reported volcano bug.)
- Pan x while live: transient — `live_update` recenters to
  `(now − tw, now)` next frame. Pan on x while *live* does NOT
  stick; only y-pan sticks.
- Pan while paused: sticks, but paused-branch live_update
  exponentially shrinks span back toward `time_window` — a wide pan
  doesn't change span so nothing happens; a box-zoom that *widened*
  span shrinks back toward `tw`.
- Double-click: `fit_bounds(traces, false)` — one-shot fit to full
  data; `auto_fit` NOT re-enabled (data flags unchanged), but
  egui_plot's internal `auto_bounds` resets → next frame the pad
  branch re-applies data bounds. Since `time_window` was the zoomed
  span, x resumes trailing with `tw` until a fit updates it.
  **NB**: `fit_bounds` also rewrites `time_window` to the fitted
  span (scope.rs:603/642) → after dbl-click, trailing window = full
  data span.
- Re-checking "Auto Fit X": `fit_x_bounds` runs next `update()`,
  `time_window` ← fitted span, autoscroll resumes from there.

### 11.2 Zoom factor math (egui)

```text
scroll_data = ctx.input(smooth_scroll_delta)      // logical px
if scroll.y != 0 && mode ∈ {X, Both}: fx = 1 + scroll.y·0.001
elif scroll.x != 0:                     fx = 1 − scroll.x·0.001
if mode ∈ {Y, Both}:                    fy = 1 + scroll.y·0.001
zoom_bounds_around_hovered(fx, fy)     // new_span = old_span / f
```

egui_plot `zoom_bounds_around_hovered` anchors at pointer and also
**clamps** to `min_auto_bounds`/translate rules internally. Guard:
fires only while `resp.hovered() && !over_legend` — and the
`bounds_changed` flag is set even for `ZoomMode::Off`? No: for Off,
`zoom_factor` stays `(1,1)`, `zoom_bounds_around_hovered` is still
called but is a no-op; `bounds_changed` IS true (scroll nonzero) →
the pad branch is skipped that frame and read-back runs — net: no
state change. Volcano must replicate *no state change*, not merely
"no zoom" (today it kills `auto_fit` — RC-8).

### 11.3 Box zoom (egui)

- Secondary drag: while `dragged` → draw band; on `stopped` → apply.
- `ZoomMode::X`: band is `start.x..cur.x × frame.height`; applies
  `set_plot_bounds_x` only. `Y`: transposed. `Both`: rect. `Off`:
  `box_zoom_start` force-cleared (no-op).
- Strokes: 4 px `DARK_BLUE` + 2 px `WHITE`, `StrokeKind::Middle`,
  clipped to `transform.frame()`.
- `Escape` during drag: cancel (clears start, no rect, no change).
- Sub-pixel boxes: `x_max > x_min` guard only — 1-px drags apply if
  they map to distinct data values. T-ZOOM-014 pins a *zero*-drag.
- The box zoom reads `box_zoom_pos = interact_pointer_pos.or(hover)`
  — release outside the widget still applies.

### 11.4 Click / double-click (egui)

- `clicked()` (not measurement, `pause_on_click=false`): emits
  `CLICK` with screen+plot pos; if `measurement_active` → also
  capture + pause + `pending_pause` + `CLICK|PAUSE` kind.
- `pause_on_click=true`: click toggles `paused` (PAUSE/RESUME
  events; resume → `take_snapshot` release semantics).
- `double_clicked()`: `fit_bounds` once + `DOUBLE_CLICK|FIT_TO_VIEW`
  event + `pending_view_change`; `clicked_point` cleared every frame
  and only set on single click.
- `clicked_point` is the *transformed* (log-space applied by
  `capture_clicked_plot_point` — verify implementation before
  writing T-CLK-003).

### 11.5 Legend semantics (egui/liveplot overlay)

- Entries: traces in `trace_order`, then XY pairs, then "(last)"
  markers; threshold/trigger/measurement overlays may append via
  `draw_overlays`.
- Primary click toggles checkbox → `hidden_items` → next frame
  `look.visible=false` → excluded from fit + draw + upload.
- Alt-click isolates; right-click on hovered item isolates/toggles
  (separate code path at 2071–2100 — must be cloned behaviorally).
- `hovered_legend_item` → next frame the trace is emphasized and
  others dimmed (same mechanism as `traces.hover_trace`).
- `legend_overlay_rect` is `Some` only when rendered; scroll inside
  the rect scrolls entries and must not reach the plot.
- `force_hide_legend` and size thresholds suppress entirely.

### 11.6 XY scope (egui)

- Derived series: merge-join x/y on timestamp, `tol = 1e-9`;
  `derived.push([x_val + x_off, y_val + y_off])` with log10 applied
  per axis when enabled (non-positive → `NaN`, dropped by egui_plot).
- Fit: `fit_x_bounds`/`fit_y_bounds` have dedicated XYScope branches
  — joined extent over *visible* pairs (pair_look.visible AND both
  members visible).
- `live_update` skips XYScope entirely — no autoscroll concept.
- Legend label: `"{y_display} vs {x_display}"` + ` — info` suffix.
- `highlight_newest_point` adds a `"(last)"` legend entry + 2×
  radius marker at `derived.last()`.

### 11.7 Pause / snapshots (egui)

- `paused=true` → all reads switch `live`→`snap`
  (`get_points_ref`, `get_drawn_points_*`, fit functions) and
  `traces.take_snapshot()` is triggered by the pause button/click
  pause.
- `pending_pause` propagates host-side pause syncing.

### 11.8 What the surface must therefore guarantee

Reduced to invariants the tests assert directly:

1. `surf.bounds` (⇒ viewport) == what the egui transform would hold
   given the same input history — asserted via `last_plot_bounds`.
2. The set of uploaded `vp_trace`s == set of `Line`/`Points` the
   egui path would draw (names, order, visibility, style fields
   modulo RC-14 mapping).
3. Uploaded point arrays == the egui `shown_pts` for
   `RenderMode::Line` semantics but with raw `live` data (documented
   exception: GPU-side decimation is the *feature*; T-PERF-003 pins
   it).
4. Transform used by `VolcanoPainterBackend` == egui's
   `PlotTransform` for the same logical bounds — i.e. identical
   `frame` (after W-1) and identical `bounds` (after W-10 pad
   normalization).

---

## 12. FFI surface audit

Everything the Rust side can ask of VolcanoPlot today, what the egui
path needs that is missing, and the delta to implement (W-12/W-14/
W-18). Signatures from `src/volcano/mod.rs` and
`vendor/VolcanoPlot/src/ffi/volcano_ffi.cpp`.

| FFI fn | used for | gap |
|---|---|---|
| `vp_figure_create/destroy` | lifecycle | — |
| `vp_figure_resize` | canvas resize | debounced ±32 px — keep |
| `vp_trace_add/remove` | trace slots | — |
| `vp_trace_set(id, xy, n, rgba8, width, style)` | series upload | `style ∈ 0..3`; need 4 = line+points; n=0 must be safe inside FFI too |
| `vp_set_viewport(x0,x1,y0,y1)` | bounds | NaN = autoscale; used with explicit bounds always |
| `vp_set_axis_labels(x,y)` | titles | pass auto-generated names (W-12) |
| `vp_set_background(white)` | theme | bool only — can't express custom colorscheme bg |
| `vp_set_margins(l,r,b,t)` | axes rect | fractions of canvas; caller computes px→frac (W-1) |
| `vp_set_background_rgba(r,g,b)` | margin blend | exists — already used for panel_fill |
| `vp_render_rgba(out,len)` | frame readback | returns 0 on size miss — surface maps to `None` |
| — none — | grid toggle | **missing** → add `vp_set_grid(int)` (FFI always draws grid today, ffi.cpp:64–71) |
| — none — | tick-label visibility | **missing** → add `vp_set_tick_labels(int x, int y)` for small-widget suppression |
| — none — | legend inside texture | liveplot draws its own — keep VP's legend **off**; assert `LegendStyle::visible==false` in ffi init |
| — none — | tick formatter callback | deferred (visual-only delta; revisit if users need matching tick text) |
| — none — | hover/crosshair | egui-side implementation (W-13) — no FFI needed |
| — none — | overlay primitives | `VolcanoPainterBackend` covers via epaint — no FFI needed |

Rules for new FFI calls:

- C ABI, `extern "C"`, no exceptions across the boundary (wrap in
  `try/catch` → int status).
- Idempotent setters: same value re-applied must not dirty the
  frame (Rust side already gates on sig — keep cheap anyway).
- Every new fn gets a `volcano_ffi.rs` test before use in
  `surface.rs`.

---

## 13. Manual / exploratory verification protocol

Automated tests prove the pinned cases; a human pass catches what
nobody wrote a test for yet. Run this checklist once per phase and
before any release with `volcano` on. Record date + commit + result
in the phase table below.

Setup:

```bash
cargo run --example sine                      # terminal A: egui path
LIVEPLOT_VOLCANO=1 cargo run --features volcano --example sine   # terminal B
# or the richer demo:
cargo run --features fft --example fft_demo
LIVEPLOT_VOLCANO=1 cargo run --features volcano,fft --example fft_demo
```

Side-by-side checklist (each item = look & feel must match):

1. Traces appear in same position relative to axes; margins blend
   into window (no white/black frame around the volcano texture).
2. Wheel-zoom in/out at various pointer positions — same anchor
   behavior, similar sensitivity, x and y modes.
3. While zoomed in: watch the right edge — **must keep scrolling**
   (the RC-2 bug); compare against egui window directly.
4. Primary-drag pan (live + paused); box-zoom (all three zoom
   modes); Escape-cancel; double-click refit.
5. Hover the plot — coordinate tooltip text present (post-W-13).
6. Legend: click rows on/off, alt-click isolate, right-click
   isolate, scroll a long legend, drag it over — wheel over legend
   must not zoom.
7. XY scope (`features` example or scopes menu): pair renders,
   fits, zooms; "(last)" marker if enabled.
8. Toolbar: every button in § Appendix C — click each, compare
   effect in both windows.
9. Panels: traces eye/color/style, thresholds add, triggers,
   measurement two-click, math channel, FFT panel visible+legend.
10. Pause/resume mid-stream; screenshot button → PNG contains plot.
11. Dark/light mode toggle — margins/grid/text recolor.
12. Window resize — content reflows, no stuck texture size, no
    flicker storm.
13. Restart with a saved session — legend position, hidden items,
    zoom state restored.

Findings protocol: any mismatch → file under the RC register (new
id), write the failing T-* test FIRST, then fix.

| phase run | date | commit | tester | result |
|---|---|---|---|---|
| P1 | | | | |
| P2 | | | | |
| P3 | | | | |
| P4 | | | | |

---

## 14. Per-phase detailed task checklists

### Phase P0 checklist (harness, no behavior change)

- [ ] `ScopeData::last_axes_screen_rect` written by both paths
- [ ] `volcano_dropped_frames` counter (u32, pub, on ScopeData)
- [ ] `frames_rendered` counter on `VolcanoSurface` (test-only getter)
- [ ] harness: `scroll_at`, `hover_at`, `double_click_at`,
      `drag_path`, `press_at`/`release_at`, `axes_rect`
- [ ] `tests/e2e/pixels.rs` module + `mod pixels;` in main.rs
- [ ] `for_each_backend`/`vol_available` helper
- [ ] signals: `Impulse`, `Step`, `Epoch`, `feed_pair`
- [ ] event recorder helper (drain `subscribe_all` into Vec)
- [ ] `volcano_ffi.rs`: margin round-trip, n=0 trace, resize,
      viewport pixel-mapping test
- [ ] all existing tests green (both feature sets)

### Phase P1 checklist (geometry)

- [ ] W-1 margin unification + `axes` from px table
- [ ] W-5 rect layout `[l,t,r,b]`
- [ ] W-10 pad normalization (decision recorded here:
      ______ pad in surface vs strip pad in egui)
- [ ] CAT-RND, CAT-PIX impulse/marker tests written + green
- [ ] T-LGD-002, T-CLK-001, T-OVL-001 green

### Phase P2 checklist (reported bugs)

- [ ] W-2 user_bounds/live semantics (x tracked when unpaused
      TimeScope; y user-owned; both when paused/XY)
- [ ] `time_window` updated on every x-span change
- [ ] W-3 legend hidden→visible sync shared
- [ ] W-4 shared legend entries (+ "(last)", info suffix, aliases)
- [x] W-11 XY fit/log/empty-pair
- [ ] CAT-SCRL, CAT-LGD, CAT-XY all green
- [ ] `zz_snap::real_app_volcano_legend` relocated to
      `volcano_parity.rs`

### Phase P3 checklist (interaction)

- [x] W-6 shared `handle_plot_click` (adapter refactor)
- [x] W-7 `handle_trace_drop` call
- [x] W-8 wheel semantics (+hscroll, +off-guard, +legend-guard)
- [x] W-9 box-zoom modes + band style + Escape
- [x] W-13 hover tooltip + hover dim/emphasis upload
      (tooltip done; dim/emphasis upload deferred)
- [x] W-15 events sweep (ZOOM/PAN direct emit, dbl-click meta)
- [x] W-16 legend suppression via `total_widget_size`
- [x] W-17 `zoom_bounds_around_hovered` resolved
- [ ] CAT-ZOOM, CAT-CLK, CAT-HOV, CAT-EVT, CAT-DND green

### Phase P4 checklist (styling/config/docs)

- [x] W-12 FFI `vp_set_grid` + auto axis names (no `show_tick_labels`
      consumer exists — tick-labels FFI not needed)
- [x] W-14 line+points style + marker plumbing (or UI gating)
- [x] W-18 FFI hardening + counters + escape hatch doc
- [ ] W-19 screenshot content verified, `zz_snap.rs` deleted
- [ ] W-20 docs updated (`volcano-integration.md`, this file's
      status header → implemented, CHANGELOG)
- [ ] full catalogue green; manual protocol §13 executed

---

## 15. Open questions / decision points

Resolve at the start of the phase that owns them; record the answer
inline so the tests pin the decision.

1. **Viewport pad** (P1): replicate egui's ±5 % inside the surface,
   or remove it from egui? Replication keeps `data.*_bounds` and all
   existing egui tests untouched → recommended.
2. **Double-click semantics** (P3): keep egui's "fit once, don't
   re-enable auto_fit" on both, or promote auto-fit enable to both?
   Parity demands one choice; changing egui behavior is out of
   scope → fit-once it is, unless the user overrides.
3. **X-pan while live** (P3): egui snaps back to trailing next
   frame — a quirk users may or may not like. Pin egui behavior
   (T-ZOOM-018) and note in CHANGELOG as "matches egui path".
4. **Marker shapes** (P4): if VolcanoPlot can't express egui's
   `MarkerShape` set, hide the control in volcano mode (consistent
   with the render-mode decision) vs approximate. Decide after
   checking `LinePlot`/`Scatter` capabilities upstream.
5. **Tick text parity** (P4+): acceptable permanent delta (§1.2) —
   revisit only if user feedback demands formatted labels inside
   the texture (`vp_set_tick_formatter` callback is the extension
   point).
6. **paused-branch recentering**: exponential approach pinned by
   T-SCRL-009 — verify volcano path inherits it automatically (it
   does — it's in `ScopeData::live_update`, shared). Test proves
   it rather than re-implements.
7. **`user_bounds` vs `PlotMemory.auto_bounds`**: volcano keeps its
   own struct; ensure save/load + controllers see the same logical
   state (surf fields are not persisted — verify nothing needs
   them: `user_bounds` is ephemeral like egui's transform, so OK;
   test T-PER-001 proves).

---

## 16. Cross-reference: old plan → this plan

`docs/e2e-parity-plan.md` sections are absorbed as follows; delete
the old file when the catalogue is fully implemented, until then it
remains a valid overview.

- §1 Harness → §2, §5 here (extended)
- §2 Scope rendering → CAT-RND/CAT-AX/CAT-XY/CAT-FFT
- §3 Legend → CAT-LGD (+ RC-3/RC-4)
- §4 Traces → CAT-TRC (+ RC-14)
- §5 Viewport/Interaction → CAT-ZOOM/CAT-SCRL (+ RC-2/RC-8/RC-9)
- §6 Overlays → CAT-OVL (+ RC-1)
- §7–§13 → CAT-PNL/CTRL/EVT/PER/DND/CFG/A11Y
- §14 Performance → CAT-PERF
- §15 Regressions → folded into named T-* tests
- §16 Matrix → §9
- §17 Gaps → §5 INFRA + §15 open questions

## 17. Change log for this plan

- v1 — initial full plan: 15 root causes (RC-1..15), 190-test
  catalogue, 20 fix items, 5 phases. The three reported bugs map to
  RC-11+RC-4 (XY view), RC-1+RC-3+RC-4 (legend), RC-2 (autoscroll
  while zoomed).

---

## 18. Reproduction walkthroughs — the three reported bugs

Exact manual reproduction + the automated test that replaces it.
Use these to demo the fix to the user.

### 18.1 Autoscroll frozen while zoomed (RC-2)

Manual repro (volcano path, `examples/sine`):

1. `LIVEPLOT_VOLCANO=1 cargo run --features volcano --example sine`
2. Wait for the trace to fill the window.
3. Right-drag a narrow box (or wheel-zoom) on x — span ≈ 1 s.
4. **Observed (bug)**: the view freezes at the zoom instant; the sine
   scrolls out of view to the right and is never seen again until a
   fit. **Expected (egui)**: the plot keeps scrolling — same 1 s
   span, right edge pinned to `now`.

Mechanism (restate): `surf.user_bounds` pins both axes; `live_update`
keeps rewriting `data.x_axis.bounds` to the trailing window but the
surface ignores it, and `time_window` was never updated to the zoomed
span anyway.

Automated: T-SCRL-002/003/005 fail before the fix, pass after.

### 18.2 Legend broken (RC-1 + RC-3 + RC-4 + W-16)

Manual repro:

1. Same app; legend shows the trace entry top-left.
2. Click the legend row.
   - **Bug A (RC-3)**: label strikes through but the trace keeps
     drawing — `look.visible` never flips.
   - **Bug B (RC-1)**: on some window sizes the legend sits visibly
     off its corner / overlaps axis labels because the anchor rect
     uses the wrong margins.
   - **Bug C (RC-4)**: on an XY scope the entry reads `xs/ys`
     instead of `ys vs xs`, and the `(last)` marker entry is missing.
3. Scroll the wheel while the pointer is over the legend —
   **Bug D (RC-8)**: the plot zooms while the legend scrolls.

Automated: T-LGD-002/004/010/013.

### 18.3 XY view (RC-11 + RC-4)

Manual repro:

1. `features` example → set scope type to XY, pair two traces.
2. **Observed (bug)**: the curve is absent or squeezed — the surface
   fits only `trace_order` (empty on a pure XY scope) so the
   viewport falls back to `(0,1)`; log scale is ignored; the pair's
   color/style partly forwarded only; legend labels wrong.
3. Zoom: geometry doesn't follow the egui anchor semantics.

Automated: T-XY-002/005/012/016 — T-XY-002 is the headline fix.

### 18.4 Bonus repro — click coordinates (RC-1)

1. Enable measurement panel, click near the bottom of the axes.
2. **Bug**: the marker lands tens of px off vertically — `to_data`
   used `MB=0.20` while the texture's bottom margin is `56 px/H`.

Automated: T-CLK-001 + T-OVL-004.

---

## 19. Test authoring rules

Conventions that keep the suite deterministic and reviewable. Every
contributor (human or agent) writing T-* tests follows these.

1. **Dual-path by construction**: every new test goes through
   `for_each_backend` or a `for vol in [false, true]` loop. Never
   write a volcano-only twin of an egui test — one body, both arms.
2. **Skip policy**: `vol_available(bed)`/`wgpu_bed()` return early
   with `eprintln!` — never silently, never `panic!`. A skipped
   arm prints the reason so CI logs show coverage.
3. **Deterministic data only**: `SignalSpec` generators with fixed
   `t0`/`fs`/seed. No wall-clock feeds — epoch tests use a fixed
   `1.79e9` base.
4. **Settle before assert**: after input scripts always `steps(n)`
   with `n` chosen for the worker pipeline (≥ 3 for texture
   pipeline; ≥ 15 when data changes must reach the viewport).
   Never assert mid-drag transient state unless the test is about
   the transient (box-zoom rect).
5. **Assert observables, not internals**: prefer
   `last_plot_bounds`, `legend_overlay_rect`, pixel stats, events.
   Accessing `surf.bounds`/`user_bounds` directly is allowed only
   in RC-2-specific tests (they pin the mechanism).
6. **Tolerance discipline**: data-space compares `1e-9` relative;
   pixel positions ±2 px; pixel *counts* within ±10 %; timing tests
   never assert wall-clock — assert ordering/pipelining
   (`pending.is_some()`), not ms.
7. **Event asserts**: drain the recorder before the action, assert
   on the *delta* sequence (kinds + key meta fields), and assert
   the stream ends — catches spurious duplicates (T-EVT-009
   pattern).
8. **No test-order dependence**: `PlotMemory` is ctx-global —
   harnesses are per-test so ids don't leak, but `plot_id` is
   name-based: give each scope a unique `data.name` in multi-scope
   tests or the memories alias (this is also a *product* bug
   pattern worth a dedicated test: two scopes named alike →
   `scope_plot_{name}` collision → shared hidden_items).
9. **Regression naming**: `parity_<area>_<behavior>`; one assertion
   theme per test; the id comment `/// T-XXX-nnn:` on the first
   doc line.
10. **wgpu pixel tests live separately** (`parity_*` + `snap_*`
    names) so a no-GPU run still executes all behavioral tests.
11. **Fixtures**: multi-scope / XY / FFT setup code goes in
    `tests/e2e/fixtures.rs` once used by ≥ 3 tests (P0 creates the
    file when needed).
12. **Never weaken**: a failing parity test means the volcano path
    deviates — fix the path, not the test. The only acceptable test
    change is updating the oracle §11 when *both* paths change.

---

## 20. Definition-of-done quick checklist

Printable summary for the final review — every box corresponds to a
section above.

- [ ] RC-1..RC-15 all marked ☑ with linked commits
- [ ] §6 catalogue: 190 tests implemented, all dual-path, all green
- [ ] §8 phase gates P0–P4 passed in order, gate results recorded
- [ ] §11 oracle assertions hold on both paths (spot-check
      T-SCRL-002, T-ZOOM-021, T-LGD-004, T-XY-002, T-CLK-001)
- [ ] §13 manual protocol executed and table filled (4 rows)
- [ ] §12 FFI delta implemented (`vp_set_grid`,
      `vp_set_tick_labels`, line+points) or documented deferral
- [ ] `tests/e2e/zz_snap.rs` deleted; contents live in
      `volcano_parity.rs`
- [ ] `cargo test --test e2e` (no features) unchanged & green —
      proves no egui-path regression
- [ ] `cargo check --features fft` + `cargo check` green — cfg
      hygiene kept
- [ ] `docs/volcano-integration.md` + CHANGELOG updated;
      `e2e-parity-plan.md` marked superseded
- [ ] Open questions §15 resolved inline (no blanks)
- [ ] No `#[ignore]`, no `skip` without reason print, no weakened
      assertion in the diff history

---

## 21. Proposed test-file layout

Keeps `volcano_parity.rs` from growing past reviewable size; each new
module is `#[cfg(feature = "volcano")]`-gated in `tests/e2e/main.rs`
except `volcano_scope` (already unconditional).

```
tests/e2e/
  harness.rs            # + INFRA-1 helpers
  pixels.rs             # NEW — INFRA-3 pixel asserts
  fixtures.rs           # NEW — multi-scope/XY/FFT setup (P0, lazy)
  volcano_parity.rs     # render, legend, zoom/pan basics (existing
                        # tests stay; CAT-RND/LGD/ZOOM land here)
  volcano_scroll.rs     # NEW — CAT-SCRL + autoscroll regressions
  volcano_interact.rs   # NEW — CAT-CLK, CAT-HOV, CAT-DND, box-zoom,
                        #       wheel matrix
  volcano_xy.rs         # NEW — CAT-XY
  volcano_panels.rs     # NEW — CAT-PNL, CAT-CTRL, CAT-EVT, CAT-PER,
                        #       CAT-CFG, CAT-A11Y (behavioral only)
  volcano_pixels.rs     # NEW — CAT-PIX (wgpu bed, skip-if-no-adapter)
  volcano_perf.rs       # NEW — CAT-PERF (incl. LIVEPLOT_STRESS gated)
  volcano_scope.rs      # keeps existing smoke tests (egui control too)
  zz_snap.rs            # DELETED at P4 — merged into volcano_parity
tests/volcano_ffi.rs    # FFI-level micro tests (INFRA-7)
```

`main.rs` additions:

```rust
#[cfg(feature = "volcano")]
mod volcano_parity;
#[cfg(feature = "volcano")]
mod volcano_scroll;
#[cfg(feature = "volcano")]
mod volcano_interact;
#[cfg(feature = "volcano")]
mod volcano_xy;
#[cfg(feature = "volcano")]
mod volcano_panels;
#[cfg(feature = "volcano")]
mod volcano_pixels;
#[cfg(feature = "volcano")]
mod volcano_perf;
```

(`volcano_scope` stays unconditional so the egui control tests run on
every build; its volcano-arm tests should move under the cfg gate when
the files are consolidated — tracked as a P4 cleanup item.)

---

## 22. Observable field reference

Canonical mapping between `ScopeData` fields and what the user sees —
the assertion vocabulary for the whole catalogue. Any test unsure
which field proves which behavior starts here.

| field | user-visible meaning | set by egui path | set by volcano path (post-fix) |
|---|---|---|---|
| `x_axis.bounds` / `y_axis.bounds` | displayed data range (unpadded) | read back from transform | written from input/user_bounds/live_update — identical semantics |
| `time_window` | trailing-window width (TimeScope) | fit/zoom read-back | same — W-2 adds the zoom write |
| `*_axis.auto_fit` | "Auto Fit" checkbox state | cleared on bounds change | same |
| `paused` | freeze state | button/click/controller | same |
| `clicked_point` | last click in data coords | `capture_clicked_plot_point` | same fn via W-6 |
| `clicked_screen_pos` | last click in screen px | same | same |
| `last_plot_bounds` | what the plot rendered (unpadded) | transform read | `axes`-mapped bounds |
| `last_plot_screen_rect` | widget rect `[l,t,r,b]` | `record_plot_geometry` | same layout (W-5) |
| `last_axes_screen_rect` *(new)* | axes frame `[l,t,r,b]` | `transform.frame()` | RC-1 `axes` rect (W-1) |
| `legend_overlay_rect` | legend presence+anchor | legend render | same fn |
| `legend_hidden_items` | hidden ids mirror | PlotMemory publish | same (W-3) |
| `rendered_this_frame` | liveness flag | `record_plot_geometry` | set at end of `render_volcano_plot` |
| `plot_id` | PlotMemory key | `make_persistent_id(scope_plot_{name})` | same salt — shared memory |
| `volcano_surface` | surface state (vol only) | `None` | `Some`; `initialized()` = rendered ≥1 frame |
| `volcano_dropped_frames` *(new)* | FFI render misses | 0 | increments on `vp_render_rgba` size miss |
| `force_hide_legend`, `show_legend`, `show_info_in_legend`, `legend_position`, `show_grid`, `scope_type`, `xy_pairs`, `trace_order` | settings | toolbar/menu/persistence | read identically — no writes needed |

External observables used by tests:

- `PlotMemory::load(ctx, plot_id)` → `hidden_items`,
  `hovered_legend_item` — the shared channel both paths use.
- `TracesCollection::get_trace(n).look.visible`,
  `xy_pairs[i].2.visible` — post-sync truth (W-3).
- `EventController` stream — kinds + `view_change`/`click` meta.
- `harness.render()` → `RgbaImage` — pixel assertions (wgpu bed).
- `VolcanoSurface` internals (`bounds`, `user_bounds`,
  `bounds_external`, `initialized()`) — mechanism tests only,
  per rule §19.5.

---

## 23. Summary for reviewers

1. The three reported bugs are root-caused, not just listed:
   **RC-2** (autoscroll), **RC-1+RC-3+RC-4** (legend), **RC-11+RC-4**
   (XY view) — each has a fix spec in §7 and failing-first tests in
   §6.
2. Eleven further divergences found during the audit (RC-5..RC-15)
   are scheduled into the same phases — fixing them later would
   invalidate tests written now.
3. The parity contract (§1) is deliberately pragmatic: behavior is
   identical, pixels are region-equivalent — text rasterization is a
   documented permanent delta.
4. ~190 dual-path tests ≈ 380 assertions; phases P0→P4 land
   harness → geometry → reported bugs → interaction → polish, each
   behind an acceptance gate so partial progress is still shippable.
5. Everything cites file:line — when the code drifts, update this
   document in the same commit as the code change.
