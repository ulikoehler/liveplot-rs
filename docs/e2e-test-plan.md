# LivePlot End-to-End Test Plan

> Status: **living document** — update the "Implementation status" column as
> tests land. Test IDs are stable; never renumber.

This document describes the complete end-to-end (e2e) test strategy for the
`liveplot` crate: what we test, how we test it, which dummy signals we feed
the application, and how stress/scale is exercised.

---

## Table of Contents

- [1. Goals and Non-Goals](#1-goals-and-non-goals)
- [2. Test Infrastructure](#2-test-infrastructure)
  - [2.1 Why egui_kittest](#21-why-egui_kittest)
  - [2.2 Harness architecture](#22-harness-architecture)
  - [2.3 Deterministic input](#23-deterministic-input)
  - [2.4 wgpu snapshot rendering](#24-wgpu-snapshot-rendering)
  - [2.5 File layout](#25-file-layout)
  - [2.6 Running the suite](#26-running-the-suite)
- [3. Dummy Signal Library](#3-dummy-signal-library)
  - [3.1 Waveforms](#31-waveforms)
  - [3.2 Timestamp modes](#32-timestamp-modes)
  - [3.3 Feed modes](#33-feed-modes)
  - [3.4 Producer presets](#34-producer-presets)
- [4. Feature Test Catalogue](#4-feature-test-catalogue)
  - [CAT-BOOT — Application boot & shell layout](#cat-boot--application-boot--shell-layout)
  - [CAT-SINK — Data channel commands](#cat-sink--data-channel-commands)
  - [CAT-TRC — Trace storage & limits](#cat-trc--trace-storage--limits)
  - [CAT-AX — Axes, formatting, grid](#cat-ax--axes-formatting-grid)
  - [CAT-VIEW — Zoom, pan, fit, time window](#cat-view--zoom-pan-fit-time-window)
  - [CAT-LGD — Legend overlay](#cat-lgd--legend-overlay)
  - [CAT-PAUSE — Pause / resume / snapshots](#cat-pause--pause--resume--snapshots)
  - [CAT-UI — Side panel chrome](#cat-ui--side-panel-chrome)
  - [CAT-TRACES — Traces panel](#cat-traces--traces-panel)
  - [CAT-MATH — Math panel](#cat-math--math-panel)
  - [CAT-THR — Thresholds panel](#cat-thr--thresholds-panel)
  - [CAT-TRG — Triggers panel](#cat-trg--triggers-panel)
  - [CAT-MEAS — Measurement & markers](#cat-meas--measurement--markers)
  - [CAT-FFT — FFT panel](#cat-fft--fft-panel)
  - [CAT-SCP — Multi-scope & tiling](#cat-scp--multi-scope--tiling)
  - [CAT-XY — XY scope](#cat-xy--xy-scope)
  - [CAT-CTRL — External controllers](#cat-ctrl--external-controllers)
  - [CAT-EVT — Event bus](#cat-evt--event-bus)
  - [CAT-PER — Persistence & undo](#cat-per--persistence--undo)
  - [CAT-EXP — Export & screenshots](#cat-exp--export--screenshots)
  - [CAT-CFG — Configuration & feature flags](#cat-cfg--configuration--feature-flags)
  - [CAT-HK — Hotkeys](#cat-hk--hotkeys)
  - [CAT-A11Y — Accessibility](#cat-a11y--accessibility)
- [5. Stress & Scale Tests](#5-stress--scale-tests)
  - [5.1 Signal-count stress](#51-signal-count-stress)
  - [5.2 Data-rate stress](#52-data-rate-stress)
  - [5.3 Scope-count stress](#53-scope-count-stress)
  - [5.4 Endurance / soak](#54-endurance--soak)
  - [5.5 Frame-time budgets](#55-frame-time-budgets)
- [6. Snapshot Tests](#6-snapshot-tests)
- [7. Feature-Gated Areas](#7-feature-gated-areas)
- [8. CI Integration](#8-ci-integration)
- [9. Conventions](#9-conventions)
- [10. Coverage Matrix](#10-coverage-matrix)
- [11. Known Limitations & Risks](#11-known-limitations--risks)
- [Appendix A — Harness API reference](#appendix-a--harness-api-reference)
- [Appendix B — Signal generator reference](#appendix-b--signal-generator-reference)

---

## 1. Goals and Non-Goals

### Goals

* Exercise the **real application** (`LivePlotPanel` / `LivePlotApp`) end to
  end: data in through the public `PlotSink` channel → per-frame ingestion →
  layout → plot rendering → user interaction → observable state.
* Cover **every user-visible feature**: scopes, axes, legend, zoom/pan,
  pause, side panels (traces, math, thresholds, triggers, measurement, FFT,
  export, color scheme, hotkeys), controllers, events, persistence, export.
* Provide a **deterministic dummy-signal library** for reproducible tests.
* Provide **stress tests** for many signals (trace count) and high data rate
  (points/sec), plus multi-scope pressure.
* **Regression coverage** for past bugs (legend corner anchoring, PlotMemory
  id pinning, extreme-zoom grid spacer, marker/measurement click routing).
* Run **headless** — no window, no GPU requirement for the functional suite.
* Keep the suite **fast** (< ~60 s debug) and **deterministic** (seeded
  randomness, fixed harness size, no wall-clock dependence inside the app
  where avoidable).

### Non-Goals

* Pixel-perfect screenshot comparison as the *primary* verification method
  (snapshots are a secondary, env-gated layer — see §6).
* Testing the OS windowing layer, native menus, clipboard, file dialogs
  (`rfd` native dialogs can't be driven by the harness; export is tested at
  the request/data level).
* gRPC network I/O (the `grpc` feature ships examples, not app logic; the
  proto round-trip is a unit-test concern).
* Web/wasm builds (the e2e suite runs on the host target only).

---

## 2. Test Infrastructure

### 2.1 Why egui_kittest

`egui_kittest` (v0.37, matches our egui/eframe versions) runs an egui
application inside a deterministic `Harness`:

* Drives real frames (`harness.step()` / `run_steps(n)`).
* Injects **synthetic input** (pointer pos, clicks, drags, scroll, keys,
  text) through `egui::RawInput`, exactly like a real backend.
* Exposes the **AccessKit tree** (`kittest::Queryable`): find widgets by
  label/role, click them, read state — no pixel scraping needed for most
  assertions.
* Can render frames via `egui-wgpu` (`wgpu` feature) for snapshot tests.
* Virtual clock: `step_dt` per frame — deterministic animation/timing.

`LivePlotPanel` (the `pub` widget inside `LivePlotApp`) is mounted directly
via `Harness::new_ui_state` — no eframe window needed. We additionally call
`hotkeys::handle_hotkeys` each frame so global shortcuts behave like the
standalone app.

### 2.2 Harness architecture

```
        ┌────────────────────────────────────────────────┐
        │                  #[test] fn …                   │
        │                                                │
        │   bed = make_test_bed()                        │
        │   feed(&bed, …)   ──────►  PlotSink ──► mpsc ──┐│
        │   bed.harness.step()                         ││
        │   bed.harness.get_by_label("FFT").click()    ││
        │   bed.harness.step()                         ││
        │   assert on harness.state()  ◄── LivePlotPanel◄┘
        │   assert on accesskit nodes                  │
        └────────────────────────────────────────────────┘
```

* `make_test_bed()` returns `TestBed { harness, sink }`.
* The harness wraps `LivePlotPanel::update` inside `new_ui_state`, so
  `harness.state()` yields `&LivePlotPanel` for deep assertions
  (`traces_data`, `liveplot_panel.get_data()`, `pending_requests`, …).
* Builder settings:
  * `with_size(1200×800)` default; overridden per test for responsive tests.
  * `with_context_setup` installs `egui_material_icons` (required — the app
    panics on missing icon glyphs otherwise).
  * `with_accessibility_check(false)` — several toolbar widgets currently
    lack accessible names (tracked under CAT-A11Y).
  * `with_step_dt(1.0/60.0)` for realistic frame pacing.
* Fonts: material icons installed in `context_setup`; everything else uses
  egui defaults + system fonts (deterministic across hosts? — *no*, system
  fonts differ; snapshot tests therefore keep a looser diff tolerance and
  are opt-in).

### 2.3 Deterministic input

| Aspect | Approach |
| ------ | -------- |
| Simulated time | `step_dt` per `step()` — the app sees egui's virtual clock |
| Data timestamps | Generators take explicit `t0`/`dt`; tests use `t0 = 1_000_000.0`-style epochs or `0.0` (never `SystemTime::now()` unless testing the epoch path) |
| Randomness | xorshift64 PRNG with fixed seeds (`TestRng`); noise signals reproducible bit-for-bit |
| Pointer | `hover_at`, `drag_to`, `click`, `scroll_delta` on explicit coordinates or node rects |
| Resize | `set_size(vec2)` mid-test |

### 2.4 wgpu snapshot rendering

* `harness.render()` produces an `RgbaImage` via `egui-wgpu` (feature `wgpu`
  + `snapshot` on `egui_kittest`).
* **Environment gate**: `LIVEPLOT_SNAPSHOT=1`. When unset → skip.
* Adapter absence is not a failure: `render()` returns `Err` → test logs a
  warning and skips (CI machines may lack a GPU; `llvmpipe` usually works).
* Snapshots stored under `tests/e2e/snapshots/`; dify-diff threshold
  `0.1 %` for UI chrome, `0.5 %` for plot content (AA noise).

### 2.5 File layout

```
tests/
  e2e/
    main.rs            # test binary root (mod declarations only)
    harness.rs         # TestBed, builders, feed helpers, node helpers
    signals.rs         # DummySignal library + TestRng
    asserts.rs         # domain assertions (legend rect, bounds, …)
    smoke.rs           # CAT-BOOT sanity tests
    sink.rs            # CAT-SINK command coverage
    traces_ui.rs       # CAT-TRC + CAT-TRACES
    scope_axes.rs      # CAT-AX
    view.rs            # CAT-VIEW (zoom/pan/fit)
    legend.rs          # CAT-LGD
    pause.rs           # CAT-PAUSE
    panels.rs          # CAT-UI chrome
    math.rs            # CAT-MATH
    thresholds.rs      # CAT-THR
    triggers.rs        # CAT-TRG
    measurement.rs     # CAT-MEAS
    fft.rs             # CAT-FFT       (cfg feature = "fft")
    scopes.rs          # CAT-SCP + CAT-XY
    controllers.rs     # CAT-CTRL
    events.rs          # CAT-EVT
    persistence.rs     # CAT-PER
    export.rs          # CAT-EXP       (parquet parts cfg-gated)
    config_flags.rs    # CAT-CFG
    hotkeys_e2e.rs     # CAT-HK
    stress.rs          # §5 (#[ignore]d heavy cases opt-in via env)
    snapshots.rs       # §6 (env-gated)
```

* One `#[test]` per case-ID where practical; table-driven tests for
  enumerations (e.g. all four legend positions in one test).
* Every test is independent: fresh `LivePlotPanel` + channel per test.

### 2.6 Running the suite

```bash
cargo test --test e2e                    # functional suite (default features)
cargo test --test e2e --features fft     # incl. FFT tests
cargo test --all-features --test e2e     # everything incl. parquet paths
LIVEPLOT_STRESS=1 cargo test --test e2e -- --ignored   # heavy stress
LIVEPLOT_SNAPSHOT=1 cargo test --test e2e snapshot     # snapshot layer
```

---

## 3. Dummy Signal Library

`tests/e2e/signals.rs` provides `SignalSpec` — a declarative description a
test feeds through `PlotSink`. All generators are pure functions of
`(index, dt)` → reproducible.

### 3.1 Waveforms

| Generator | Formula / behaviour | Purpose |
| --------- | ------------------- | ------- |
| `Sine { freq, amp, phase }` | `amp·sin(2πf t + φ)` | canonical trace, FFT ground truth |
| `Cosine` / `SumOfSines` | mixed tones | FFT multi-peak, dense traces |
| `Square { freq, amp, duty }` | ±amp | threshold crossings, sharp edges |
| `Triangle`, `Sawtooth` | linear ramps | differentiate test |
| `Pulse { period, width, amp }` | single pulses | triggers |
| `Step { times, levels }` | piecewise-constant | threshold in/out events, XY stages |
| `Constant { v }` | flat line | degenerate-bounds testing (y span 0) |
| `Ramp { slope, offset }` | `m·t+b` | integrate → parabola |
| `Chirp { f0, f1, dur }` | swept sine | spectrogram-like sanity, windowing |
| `WhiteNoise { seed, amp }` | xorshift64 uniform | density/splatting render, stress |
| `RandomWalk { seed, step }` | cumulative noise | long non-repeating streams |
| `Impulse { at, amp }` | single spike | edge cases, decimation preservation |
| `Burst { on, off, amp }` | gated noise | intermittent data |
| `Heartbeat` | synthetic ECG-ish | realistic demo shape |
| `Gaps { inner, gap_every, gap_len }` | wrap any signal; drops x-ranges | gap rendering |
| `NanInject { inner, every }` | inject NaN/Inf | non-finite handling |
| `Scaled { inner, k, off }` | `k·y+off` | offset/magnitude tests |
| `Late { inner, delay }` | shifts x | out-of-order handling |

`WhiteNoise`/`RandomWalk` use a shared `TestRng(seed)` so CI is stable.

### 3.2 Timestamp modes

| Mode | x values |
| ---- | -------- |
| `Absolute(epoch_t0, fs)` | `t0 + i/fs` — mimics `SystemTime` epochs |
| `Monotonic(fs)` | `i/fs` starting at 0 |
| `Irregular { fs, jitter }` | `dt` jittered ±jitter (deterministic) |
| `Bursts` | clusters of points then silence |
| `Duplicate` | repeated x values (degenerate input) |
| `OutOfOrder { shuffle }` | windowed shuffle of x order |

### 3.3 Feed modes

* `send_point` — one `PlotCommand::Point` per sample (worst-case path).
* `send_points(chunk)` — `PlotCommand::Points` batches (the common path).
* `set_data` — full-buffer replacement.
* `live_thread(rate)` — background thread producing at a real rate; used
  only in stress tests.
* `interleaved` — multi-trace round-robin feeding.

### 3.4 Producer presets

Named combos used across the suite:

* `demo_sine(fs=1 kHz, f=3 Hz)` — the example workload.
* `pair(x,y)` — two parametric signals for XY scope.
* `tone(60 Hz)` — FFT ground truth with exact bin alignment.
* `wideband` — noise + 3 tones for FFT/window checks.
* `steps_thr(±0.8)` — deterministic threshold crossing sequence.

---

## 4. Feature Test Catalogue

Legend: **P**rio (A = must-have, B = important, C = nice), **Type**:
F=functional via accesskit/state, S=state-only, N=snapshot, E=event-driven.

### CAT-BOOT — Application boot & shell layout

| ID | P | Type | Test | Expected |
| -- | - | ---- | ---- | -------- |
| BOOT-01 | A | F | Fresh harness, one step | No panic; accesskit tree non-empty (>10 nodes); one `ScopeData` exists |
| BOOT-02 | A | F | Default chrome | Top bar present with Scopes/Traces/Math/…/ClearAll entries; scope plot occupies central area |
| BOOT-03 | B | F | Headline/subheadline via `LivePlotApp` path (or config) | Banner text nodes exist |
| BOOT-04 | B | F | Second `step()` is idempotent | No duplicate widgets, no panic |
| BOOT-05 | B | F | `set_size(400×300)` tiny window | Top bar collapses to sidebar icons (responsive), no panic, no overlap nodes out of bounds |
| BOOT-06 | B | F | `set_size(1600×1000)` large window | Sidebar/legend thresholds satisfied; legend visible |
| BOOT-07 | B | S | `LivePlotPanel::new_with_id("x", 9)` | Second panel instance coexists without Id clash warnings |

### CAT-SINK — Data channel commands

Every `PlotCommand` variant gets coverage.

| ID | P | Type | Test | Expected |
| -- | - | ---- | ---- | -------- |
| SINK-01 | A | S | `RegisterTrace` then `Point` | `traces_data` contains trace with 1 pt |
| SINK-02 | A | S | `Points` 10k chunk | All 10k appended, order preserved |
| SINK-03 | A | S | `SetData` replaces buffer | Old points gone, new buffer present |
| SINK-04 | A | S | `ClearData` | Trace exists but `live.is_empty()` |
| SINK-05 | A | S | `SetPointsY` exact-x hit | y changed at that x |
| SINK-06 | B | S | `SetPointsY` unknown x | No-op, no panic |
| SINK-07 | A | S | `DeletePointsX` | Those xs removed |
| SINK-08 | A | S | `DeleteXRange` incl. NaN bounds | Range removed; NaN→ends honoured |
| SINK-09 | A | S | `ApplyYFnAtX` | `y=f(y)` applied at xs |
| SINK-10 | A | S | `ApplyYFnInXRange` | fn applied inside range only |
| SINK-11 | A | S | `SetTraceInfo` | `tr.info` updated; shows in legend when info enabled |
| SINK-12 | A | S | Feed while paused | Points land in `live`, scope shows `snap` instead |
| SINK-13 | B | S | Trace created but never fed | Appears in trace list (empty), no plot items |
| SINK-14 | B | S | `send_point` on dropped receiver | `Err(SendError)` — callers can detect UI exit |
| SINK-15 | B | S | 1000 `Point` msgs in one frame | All drained this frame |
| SINK-16 | B | S | Register same name twice | Behaviour defined & asserted (current: two `Trace`s, same label — document actual outcome) |
| SINK-17 | B | S | Points with `x` far in past/future | Bounds auto-fit still sane |

### CAT-TRC — Trace storage & limits

| ID | P | Type | Test | Expected |
| -- | - | ---- | ---- | -------- |
| TRC-01 | A | S | `max_points = N`; feed 2N | `live.len() == N`, oldest evicted |
| TRC-02 | A | S | `max_age_secs` eviction | Old samples dropped by age |
| TRC-03 | A | S | Eviction under continuous feed | Length stays ≤ limit after many frames |
| TRC-04 | A | S | `points_bounds` clamp via controller | `max_points` clamped to bounds |
| TRC-05 | B | S | `TraceLook` defaults | colour allocated, width 1.5, visible |
| TRC-06 | B | S | `recolor_using_palette` after scheme change | colours follow palette |
| TRC-07 | B | S | `hover_trace` set | non-hovered traces dim (state flag only) |
| TRC-08 | B | S | `display_label` alias | legend/list uses alias, item id stays canonical |
| TRC-09 | B | S | offset applied | drawn points `y+offset` (via `get_drawn_points`) |
| TRC-10 | B | S | `look.visible=false` | excluded from legend, bounds & draw |
| TRC-11 | C | S | `RenderMode` variants set | stored; MinMaxEnvelope + DensitySplatting produce decimated buckets |
| TRC-12 | B | S | Snapshot taken on pause | `snap` populated, `live` keeps streaming |
| TRC-13 | A | S | Interleaved feed of 8 traces | each has its own correct buffer (no cross-talk) |

### CAT-AX — Axes, formatting, grid

| ID | P | Type | Test | Expected |
| -- | - | ---- | ---- | -------- |
| AX-01 | A | S | Time axis formatting | `format_value` gives `hh:mm:ss` / adaptive for bounds width |
| AX-02 | A | S | Value axis formatting | decimals honoured, scientific for huge/small |
| AX-03 | A | S | `y_axis.unit` | label text contains unit; formatter appends it |
| AX-04 | A | S | `value_decimals` | tick labels rounded correctly |
| AX-05 | A | F | Toggle log y via scope settings | `y_axis.log_scale` true; positive-only data requirement handled (negative→ clamp) |
| AX-06 | B | S | `name`/`show_label` | axis label emitted (or suppressed) |
| AX-07 | B | F | `show_grid` off | no grid lines drawn (state + optional snapshot) |
| AX-08 | B | F | Extreme zoom-in (~1e-15 span) | tick labels still produced (regression: uniform grid spacer) |
| AX-09 | B | S | `auto_fit` x/y | bounds track data extrema each frame |
| AX-10 | B | S | `keep_max_fit` | bounds only grow while streaming |
| AX-11 | B | S | Hide tick labels below thresholds | `min_width_for_y_ticklabels` suppresses labels & axis space |
| AX-12 | C | F | `TimeFormatter` variants | date/time formats render |

### CAT-VIEW — Zoom, pan, fit, time window

| ID | P | Type | Test | Expected |
| -- | - | ---- | ---- | -------- |
| VIEW-01 | A | F | Primary-drag inside plot | `x_axis.bounds`/`y_axis.bounds` shift (pan) |
| VIEW-02 | A | F | Scroll wheel over plot centre | bounds shrink around pointer (zoom) |
| VIEW-03 | A | F | Secondary-drag box select | bounds == box (boxed zoom) |
| VIEW-04 | A | F | `Fit view` toolbar button | bounds reset to data extrema |
| VIEW-05 | B | F | `time_window` set | x span follows newest sample − window |
| VIEW-06 | B | F | ZoomMode::Both vs axis-locked | only allowed axes move |
| VIEW-07 | B | F | Scroll on axis edge | that axis zooms, other doesn't |
| VIEW-08 | B | S | `set_x_range` programmatic | bounds applied next frame |
| VIEW-09 | B | F | Escape during box-zoom | drag cancelled, bounds unchanged |
| VIEW-10 | C | F | Box-zoom with pointer leaving plot | drag tracked correctly |
| VIEW-11 | B | S | `pending_view_change` emitted | meta contains new bounds |

### CAT-LGD — Legend overlay

Regression area — the corner-stretch bug fixed in 2.5.4 lives here.

| ID | P | Type | Test | Expected |
| -- | - | ---- | ---- | -------- |
| LGD-01 | A | F | Feed 1 trace, `show_legend` | legend overlay rendered inside plot rect |
| LGD-02 | A | F | `LegendPosition::LeftTop` | overlay rect ≈ plot.left+pad, top+pad; width < 75% plot width |
| LGD-03 | A | F | `::RightTop` | overlay anchored to right edge, snug width (**regression**) |
| LGD-04 | A | F | `::LeftBottom` | anchored left-bottom, snug (**regression**) |
| LGD-05 | A | F | `::RightBottom` | anchored right-bottom, snug (**regression**) |
| LGD-06 | A | F | Click legend entry label | `legend_hidden_items`/`PlotMemory.hidden_items` gains id; line hidden next frame |
| LGD-07 | A | F | Click hidden entry again | re-shown |
| LGD-08 | B | F | Alt-click entry | all others hidden; alt-click again restores |
| LGD-09 | B | F | Right-click legend entry | others hidden; right-click again restores |
| LGD-10 | B | F | >20 entries, small plot | legend scrolls within max height — does not overflow plot |
| LGD-11 | B | F | `show_info_in_legend` + `tr.info` | info appended to label |
| LGD-12 | B | F | Width below `min_width_for_legend` | legend suppressed entirely |
| LGD-13 | B | F | `force_hide_legend` | no legend despite entries |
| LGD-14 | B | S | hidden trace excluded from auto-fit bounds | y-range tracks visible only |
| LGD-15 | B | F | Click on legend rect | does NOT pan/zoom plot (overlay swallows input) |
| LGD-16 | B | F | Legend labels use display alias | canonical id unaffected by alias |
| LGD-17 | A | F | Same tests LGD-02..05 on the FFT scope | identical corner anchoring |
| LGD-18 | C | F | Entry swatch colors | snapshot/visual check (S) |

### CAT-PAUSE — Pause / resume / snapshots

| ID | P | Type | Test | Expected |
| -- | - | ---- | ---- | -------- |
| PAUSE-01 | A | F | `Pause` toolbar/menu action | all scopes `paused`, snapshot taken |
| PAUSE-02 | A | F | Resume | `paused=false`, live data visible again |
| PAUSE-03 | A | F | Pause hotkey `P` | same as PAUSE-01 |
| PAUSE-04 | A | S | While paused | `snap` used for display; `live` still accumulates |
| PAUSE-05 | B | F | Click plot while paused (no measurement) | resumes |
| PAUSE-06 | B | F | Click while `measurement_active` | `clicked_point` set, stays paused |
| PAUSE-07 | B | S | `pause_state_changed` | emits transition once |
| PAUSE-08 | B | S | `take_explicit_pause` | consumed once |
| PAUSE-09 | B | F | Per-scope pause via scope menu | only that scope freezes |
| PAUSE-10 | C | S | Pause → screenshot → resume | screenshot captured from snap data |

### CAT-UI — Side panel chrome

| ID | P | Type | Test | Expected |
| -- | - | ---- | ---- | -------- |
| UI-01 | A | F | Click each menu button (Traces, Math, Thresholds, Triggers, Measurement, FFT, Export, Color Scheme, Hotkeys) | matching panel becomes visible in sidebar |
| UI-02 | A | F | Toggle same button twice | panel hides again |
| UI-03 | B | F | Panel "detach"/window button | moves to `detached_panels`, floating window |
| UI-04 | B | F | Reattach | back in dock |
| UI-05 | B | F | Multiple panels open | all render without id clash |
| UI-06 | B | F | Menu collapsed (narrow) | icon-only tooltips still toggle panels |
| UI-07 | B | F | `ClearAll` | all traces emptied + confirmed dialog flow (if any) |
| UI-08 | C | F | Focus request via `request_focus` | panel raised/selected |

### CAT-TRACES — Traces panel

| ID | P | Type | Test | Expected |
| -- | - | ---- | ---- | -------- |
| TRACES-01 | A | F | Open panel | every registered trace listed w/ colour swatch |
| TRACES-02 | A | F | Visibility checkbox | `tr.look.visible` toggles, plot + legend update |
| TRACES-03 | A | F | Rename/alias field | `display_label` changes in legend |
| TRACES-04 | B | F | Colour picker | `look.color` changes, persisted in settings |
| TRACES-05 | B | F | Width slider | `look.width` changes |
| TRACES-06 | B | F | Line style combo | `look.style` changes |
| TRACES-07 | B | F | Points/marker toggle | `show_points`/`marker`/`point_size` |
| TRACES-08 | B | F | Offset field | `tr.offset` applied |
| TRACES-09 | B | F | Render mode combo | `render_mode` change honoured |
| TRACES-10 | B | F | Drag re-order (egui_dnd) | `trace_order` reordered; legend order follows |
| TRACES-11 | B | F | "Move to scope" target | trace moves to chosen scope |
| TRACES-12 | B | F | Sort new traces alphabetically | new traces inserted sorted |
| TRACES-13 | C | F | Click row → edit mode | inline edit widgets appear |

### CAT-MATH — Math panel

`MathKind::{Add, Multiply, Divide, Differentiate, Integrate, Filter(7 kinds), MinMax(2 modes), Formula}` — each verified numerically against hand-computed expectations.

| ID | P | Type | Test | Expected |
| -- | - | ---- | ---- | -------- |
| MATH-01 | A | S | Add `{a}+{b}` on two sines | math trace live pts == a+b at union timestamps |
| MATH-02 | A | S | Add with weights/signs | `sign_i·x_i` applied |
| MATH-03 | A | S | Multiply | element-wise product |
| MATH-04 | A | S | Divide | quotient; division-by-0 → skipped/Inf handling |
| MATH-05 | A | S | Differentiate ramp | ≈slope within dt tolerance |
| MATH-06 | A | S | Integrate constant c | ≈c·t(+y0) |
| MATH-07 | A | S | Lowpass on step | output approaches level, never overshoots 1st-order |
| MATH-08 | A | S | Highpass on constant | output → 0 |
| MATH-09 | B | S | Bandpass | attenuates DC and HF |
| MATH-10 | B | S | Biquad LP/HP/BP | expected attenuation sign |
| MATH-11 | B | S | Filter custom params | uses supplied coeffs |
| MATH-12 | A | S | MinMax min & max, no decay | tracks extrema |
| MATH-13 | B | S | MinMax with decay | decays toward current value |
| MATH-14 | A | F | Formula `sin(2*pi*3*t)` pure-t | evaluates without input trace |
| MATH-15 | A | F | Formula `{a}*2+{b}` | union-grid interpolation |
| MATH-16 | B | F | Formula invalid expr | error surfaced, no panic |
| MATH-17 | B | F | Formula `{name with space}` | braces syntax resolves |
| MATH-18 | B | S | t-mode Absolute vs Resettable | correct t origin; ↻ button re-anchors |
| MATH-19 | B | S | NaN/Inf inputs | results skipped as gaps |
| MATH-20 | B | F | Math trace appears in scope & legend | entry with math name shown |
| MATH-21 | B | F | Remove math trace | gone from collection |
| MATH-22 | B | S | Source trace deleted | math output stops, entry cleaned |
| MATH-23 | C | F | LaTeX preview renders | preview node exists |
| MATH-24 | B | F | Edit existing math trace | recompute with new spec |
| MATH-25 | A | S | Stateless vs stateful recompute | resize/max_points change → stateful ops re-primed |

### CAT-THR — Thresholds panel

| ID | P | Type | Test | Expected |
| -- | - | ---- | ---- | -------- |
| THR-01 | A | F | Add GreaterThan threshold | appears in list + scope overlay + legend entry |
| THR-02 | A | S | Sine vs GT 0.5 | events emitted on up-crossings only |
| THR-03 | A | S | LessThan | down-crossings |
| THR-04 | A | S | InRange | enter/exit events |
| THR-05 | A | S | `is_active`/`excess` math | correct for each kind |
| THR-06 | B | F | Threshold line drawn | vline/hline at value on target scope |
| THR-07 | B | F | Alert markers at crossings | start markers rendered |
| THR-08 | B | F | Disable threshold | no events, no overlay |
| THR-09 | B | F | Remove threshold | overlay + events stop |
| THR-10 | B | F | Multiple thresholds same trace | independent events |
| THR-11 | B | S | Threshold events to `EventController` | `ThresholdMeta` delivered |
| THR-12 | B | F | Hidden threshold via legend | excluded from measurement x-range |
| THR-13 | B | S | Target trace deleted | threshold handled gracefully |
| THR-14 | C | F | Threshold style (colour/dash) | look applied |

### CAT-TRG — Triggers panel

| ID | P | Type | Test | Expected |
| -- | - | ---- | ---- | -------- |
| TRG-01 | A | F | Add edge trigger (rising) | listed, enabled |
| TRG-02 | A | S | Rising edge on step signal | trigger fires at crossing index |
| TRG-03 | A | S | Falling edge | fires on down edge |
| TRG-04 | B | S | Level + hysteresis | no double-fires inside hysteresis band |
| TRG-05 | B | F | Trigger line shown at level | vline/hline rendered |
| TRG-06 | B | F | Disable/enable | firing stops/resumes |
| TRG-07 | B | S | Trigger events | `TriggerMeta` dispatched |
| TRG-08 | C | S | Slope::Both | fires on both edges |
| TRG-09 | B | F | Remove trigger | gone |
| TRG-10 | B | F | Hidden via legend | stops firing/display |

### CAT-MEAS — Measurement & markers

| ID | P | Type | Test | Expected |
| -- | - | ---- | ---- | -------- |
| MEAS-01 | A | F | Add measurement | `measurement_active` on scope |
| MEAS-02 | A | F | Click plot twice | p1,p2 recorded; Δx/Δy shown |
| MEAS-03 | A | F | Click with marker selected | marker gets point, not measurement (routing) |
| MEAS-04 | A | F | `catch_trace` | marker snaps to nearest sample |
| MEAS-05 | B | F | Marker h/v lines + shape | rendered per `Marker` fields |
| MEAS-06 | B | F | Clear markers request | points cleared |
| MEAS-07 | B | F | Clear measurements | p1/p2 reset |
| MEAS-08 | B | S | Hidden marker excluded from x-range | regression test exists |
| MEAS-09 | B | S | Legend-hidden measurement excluded | regression (existing unit test parity) |
| MEAS-10 | B | F | Marker change event | `take_marker_change` yields list |
| MEAS-11 | B | F | Delete marker/measurement rows | gone from panel & scope |
| MEAS-12 | C | F | Measurement labels (Δt, 1/Δt) | text nodes present |

### CAT-FFT — FFT panel (feature `fft`)

| ID | P | Type | Test | Expected |
| -- | - | ---- | ---- | -------- |
| FFT-01 | A | F | Show FFT | bottom/docked panel appears; axes labelled Hz/Magnitude |
| FFT-02 | A | S | Pure tone 60 Hz @1 kHz | spectrum peak bin within ±1 bin of 60 Hz |
| FFT-03 | A | S | Two tones | two peaks at right bins |
| FFT-04 | B | S | Window variants | Hann/Blackman/etc. don't move peak freq |
| FFT-05 | B | S | Zero-pad 2×/4× | finer bin spacing, peak still correct |
| FFT-06 | A | F | dB toggle | y axis becomes dB; values ≈20·log10 |
| FFT-07 | A | F | fft_size slider change | recompute at new size |
| FFT-08 | A | F | Insufficient data | warning banner shown; trace info "Need N samples" |
| FFT-09 | B | S | Throttle interval | recompute obeys interval (no spin) |
| FFT-10 | B | F | Legend click hides trace | hidden trace: no recompute, no line |
| FFT-11 | B | S | Paused source | spectrum from `snap`, not live |
| FFT-12 | A | F | Legend positions ×4 on FFT scope | snug corner overlay (regression) |
| FFT-13 | B | S | Trace deleted | fft_traces entry pruned |
| FFT-14 | B | S | fft_size > max_points clamped | size rounds down to pow2 ≤ max |
| FFT-15 | B | S | Cache reuse | `mark_computed`/`needs_recompute` no redundant work |
| FFT-16 | C | S | Spectrum units label | Hz on x axis |

### CAT-SCP — Multi-scope & tiling

| ID | P | Type | Test | Expected |
| -- | - | ---- | ---- | -------- |
| SCP-01 | A | F | Scopes menu → "Add scope" | second `ScopeData` exists; tiles tree has 2 panes |
| SCP-02 | A | F | Remove scope | back to 1; traces not lost |
| SCP-03 | B | F | Move trace between scopes | only target scope lists it |
| SCP-04 | B | F | New trace → default-target scope | lands in designated scope |
| SCP-05 | B | F | Scope rename | title/tab updated |
| SCP-06 | B | F | Per-scope settings independent | legend/grid/pause differ |
| SCP-07 | B | F | Linear/Grid layout via tiles | panes arranged; all render |
| SCP-08 | B | F | Tabs behaviour | hidden tab scope `rendered_this_frame` toggles |
| SCP-09 | C | F | Scope title bars inside Linear/Grid | names visible (fix already shipped) |
| SCP-10 | B | S | `scope_states()` serialisation order | sorted by id |

### CAT-XY — XY scope

| ID | P | Type | Test | Expected |
| -- | - | ---- | ---- | -------- |
| XY-01 | A | F | `scope_type = XYScope` + pair (x,y) | line connects x-y series |
| XY-02 | A | F | Legend entry per pair | "y vs x" label; click hides pair |
| XY-03 | B | F | `highlight_newest_point` | extra "(last)" entry toggles marker |
| XY-04 | B | S | Hidden pair via PlotMemory | look.visible/`xy_pairs` synced |
| XY-05 | B | S | auto-fit bounds over pairs | bounds cover both axes' data |
| XY-06 | C | S | Missing partner trace | pair skipped gracefully |

### CAT-CTRL — External controllers

Each `*Controller` round-trip: request → applied next frame → snapshot published.

| ID | P | Type | Test | Expected |
| -- | - | ---- | ---- | -------- |
| CTRL-01 | A | S | `UiActionController` pause/resume | `LiveplotState.paused` flips |
| CTRL-02 | A | S | request screenshot | `pending_requests.screenshot` set → capture queued |
| CTRL-03 | A | S | `TracesController` set color/visible/offset/width/style | applied to trace look |
| CTRL-04 | A | S | y_unit / y_log requests | scopes updated |
| CTRL-05 | A | S | `TracesInfo` snapshot | names/colour/visibility correct |
| CTRL-06 | A | S | max_points / points_bounds | limits applied |
| CTRL-07 | B | S | `LiveplotController` clear-all | traces emptied |
| CTRL-08 | B | S | save/load state requests | `pending_requests` fields set |
| CTRL-09 | B | S | `ScopesController` add/remove | scope count changes + `ScopesState` published |
| CTRL-10 | B | S | `FFTController` show/hide/resize | panel state + `FFTPanelInfo` |
| CTRL-11 | B | S | `ThresholdController` add/remove | defs update; event cursor advances |
| CTRL-12 | B | S | `WindowController` size/pos | `WindowInfo` reports viewport rect |
| CTRL-13 | B | S | `hover_trace_request` | `traces.hover_trace` set |
| CTRL-14 | C | S | listener channels fan-out | multiple listeners each get snapshot |

### CAT-EVT — Event bus

| ID | P | Type | Test | Expected |
| -- | - | ---- | ---- | -------- |
| EVT-01 | A | S | `EventController` subscribe | receives `PlotEvent`s |
| EVT-02 | A | S | `DataUpdate` emitted on new points | meta carries count/timestamps |
| EVT-03 | A | S | `Pause`/`Resume` events | on transitions only |
| EVT-04 | B | S | `ViewChange` on pan/zoom/fit | bounds meta |
| EVT-05 | B | S | `Threshold`/`Trigger`/`Marker`/`Measurement`/`Click` metas | populated correctly |
| EVT-06 | B | S | `EventFilter` whitelist | only matching kinds arrive |
| EVT-07 | B | S | `ScopeManage` add/remove scope | meta correct |
| EVT-08 | B | S | `KeyPress`/`Resize`/`Export`/`YAxis`/`Trace`/`MathTrace` | each emitted on its action |
| EVT-09 | C | S | Slow subscriber backpressure | bounded channel behaviour defined |

### CAT-PER — Persistence & undo

| ID | P | Type | Test | Expected |
| -- | - | ---- | ---- | -------- |
| PER-01 | A | S | `settings_snapshot` per panel | JSON round-trips `*Serde` types |
| PER-02 | A | S | Full save (all panels+scopes) | restore reproduces: scope count, legend pos, traces order, looks, thresholds, measurements, fft cfg, math traces |
| PER-03 | A | S | Load legacy/partial JSON | defaults fill missing fields; no panic |
| PER-04 | B | S | Corrupt JSON | graceful error surfaced |
| PER-05 | A | F | User action → undo stack grows | entry recorded |
| PER-06 | A | F | Ctrl+Z | previous state restored |
| PER-07 | A | F | Ctrl+Y / Ctrl+Shift+Z | redo |
| PER-08 | B | S | `suppress_undo` during restore | no stack pollution |
| PER-09 | B | S | `detect_settings_change` | only fires on real deltas |
| PER-10 | B | S | `trace_order` preserved across save/load | ordering stable |

### CAT-EXP — Export & screenshots

| ID | P | Type | Test | Expected |
| -- | - | ---- | ---- | -------- |
| EXP-01 | A | S | CSV export request → file | `write_csv_aligned_path` output parses, rows aligned by timestamp |
| EXP-02 | A | S | CSV with gaps | empty cells at missing timestamps |
| EXP-03 | A | S | Parquet export (`parquet` feat) | file opens; columns/types correct |
| EXP-04 | B | S | Export panel opens | fields editable (path/format) |
| EXP-05 | B | F | Screenshot center panel | `pending_screenshot_capture` queued; with wgpu → PNG written |
| EXP-06 | B | F | Scope-rect screenshot | crop target = plot rect + axis label padding |
| EXP-07 | B | F | Export hotkey | request emitted |
| EXP-08 | C | S | Export `PlotEvent` | `ExportMeta` emitted |

### CAT-CFG — Configuration & feature flags

| ID | P | Type | Test | Expected |
| -- | - | ---- | ---- | -------- |
| CFG-01 | A | F | `FeatureFlags.legend=false` | no legend rendered |
| CFG-02 | A | F | `grid=false` | grid off in all scopes |
| CFG-03 | B | F | `pause_resume=false` | pause button absent |
| CFG-04 | B | F | `top_bar=false`, `sidebar=true` | chrome moved/collapsed accordingly |
| CFG-05 | B | F | Individual panel flags off | those menus absent |
| CFG-06 | B | F | `x_tick_labels=false`/`y_tick_labels=false` | suppressed |
| CFG-07 | B | S | `max_points`, `max_age_secs`, `time_window_secs`, `y_unit`, `y_log` via config | applied on startup |
| CFG-08 | B | F | `top_bar_buttons`/`sidebar_buttons` custom sets | only listed buttons appear |
| CFG-09 | B | F | `min_*` thresholds | collapse behaviour at boundaries |
| CFG-10 | B | S | `ColorScheme` applied once | `color_scheme_applied`, palette colours on traces |
| CFG-11 | B | S | `hotkeys` custom map | overrides take effect |
| CFG-12 | C | S | `overlays` callback | invoked each frame w/ scope |
| CFG-13 | B | F | `max_fps` clamp | repaint_interval set |

### CAT-HK — Hotkeys

| ID | P | Type | Test | Expected |
| -- | - | ---- | ---- | -------- |
| HK-01 | A | F | `P` | pause toggles |
| HK-02 | A | F | `T` | Traces panel toggles |
| HK-03 | A | F | `Ctrl+T` | Thresholds panel toggles |
| HK-04 | A | F | `F` | fit view |
| HK-05 | B | F | FFT hotkey | FFT panel toggles |
| HK-06 | B | F | `Ctrl+M`/`Shift+M` etc. | math/measurement toggles |
| HK-07 | B | F | Save PNG hotkey | screenshot request |
| HK-08 | B | F | Export-data hotkey | export request |
| HK-09 | B | F | Clear-all hotkey | buffers cleared |
| HK-10 | B | F | Reset measurements hotkey | measurement points cleared |
| HK-11 | B | F | Custom binding via config | custom key works, default disabled |
| HK-12 | B | F | Hotkey while typing in text field | not triggered |
| HK-13 | C | F | Hotkeys panel lists bindings | labels show key names |

### CAT-A11Y — Accessibility

| ID | P | Type | Test | Expected |
| -- | - | ---- | ---- | -------- |
| A11Y-01 | B | F | `with_accessibility_check(true)` audit | known-unlabelled widgets catalogued; incrementally fix |
| A11Y-02 | C | F | Legend entries expose names | accesskit label present |
| A11Y-03 | C | F | Panel buttons labelled | all menu buttons have names |

---

## 5. Stress & Scale Tests

Heavy cases are `#[ignore]` by default — run with
`LIVEPLOT_STRESS=1 cargo test --test e2e stress -- --ignored --test-threads=1`.
Light versions run unconditionally as regression guards.

Budgets are stated as *frame-step counts* (deterministic) plus generous
wall-clock ceilings to catch pathological slowdowns without flaky CI.

### 5.1 Signal-count stress

| ID | Workload | Assertions |
| -- | -------- | ---------- |
| STR-N-01 | 8 traces × 2 000 pts (light) | all traces drawn; legend scrolls; <5 s |
| STR-N-02 | 32 traces × 2 000 pts | `trace_order` complete; legend rect within bounds; <10 s |
| STR-N-03 | 64 traces × 2 000 pts | per-frame ingest drains; <20 s |
| STR-N-04 | 128 traces × 1 000 pts | no panic; traces_data.len()==128 |
| STR-N-05 | 256 traces × 500 pts | legend doesn't overflow plot; scroll works |
| STR-N-06 | 512 traces × 250 pts | upper bound sanity; memory OK |
| STR-N-07 | 32 traces, each with info+alias | legend shows 32 entries |
| STR-N-08 | 64 traces spread over 4 scopes | 16/scope; panes all render |

### 5.2 Data-rate stress

| ID | Workload | Assertions |
| -- | -------- | ---------- |
| STR-R-01 | 1 trace, 1 kHz × 60 s live | buffer capped at max_points; ingest drains |
| STR-R-02 | 1 trace, 10 kHz × 30 s | same, chunked feed |
| STR-R-03 | 10 kHz via `Point` msgs (worst path) | all drained per frame; no O(n²) blowup (wall clock budget) |
| STR-R-04 | 1 M points in a single `Points` msg | appended; eviction to max_points |
| STR-R-05 | 50 traces × 20 kHz for 5 s (1 M pts/s aggregate) | UI stays responsive: 60 steps complete < budget |
| STR-R-06 | Producer thread faster than consumer | mpsc backlog drains when steps resume |
| STR-R-07 | `max_points` small (1 024) + high rate | eviction keeps pace; newest shown |
| STR-R-08 | decimation/envelope path @ high rate | bucket count bounded by pixel width |
| STR-R-09 | FFT on 64 k-pt trace | dispatch completes; worker doesn't block frame |
| STR-R-10 | alternating bursts (1 s on/off) | no stale bounds between bursts |

### 5.3 Scope-count stress

| ID | Workload | Assertions |
| -- | -------- | ---------- |
| STR-S-01 | 4 scopes (2×2) × 4 traces each | all panes render, legends positioned per-scope |
| STR-S-02 | 9 scopes (3×3) | no id clashes; update_data per scope |
| STR-S-03 | 16 scopes (4×4) tiny panes | legend/tick suppression thresholds kick in |
| STR-S-04 | add+remove scopes in a loop | tiles tree stays consistent, no leaks |

### 5.4 Endurance / soak

| ID | Workload | Assertions |
| -- | -------- | ---------- |
| SOAK-01 (`ignore`) | 4 traces @1 kHz for 10 000 steps | no unbounded growth (memory sampled), eviction stable |
| SOAK-02 (`ignore`) | math+threshold+fft active soak | event cursors don't O(n²) |
| SOAK-03 (`ignore`) | rapid pause/resume ×1000 | snapshots consistent |

### 5.5 Frame-time budgets

Measured inside the harness (wall clock, debug build, lenient):

| Budget | Scope |
| ------ | ----- |
| ingest 10 k pts + step | < 250 ms |
| ingest 1 M pts + step | < 3 s |
| render step @ 32 traces | < 100 ms/step |
| full suite (non-ignored) | < 60 s |

Budgets are *guardrails*, not pass/fail latency SLOs — tune if flaky.

---

## 6. Snapshot Tests

`tests/e2e/snapshots.rs`, feature `egui_kittest/wgpu,snapshot`,
env-gated `LIVEPLOT_SNAPSHOT=1`. If `render()` fails (no adapter) → skip.

| ID | Scene |
| -- | ----- |
| SNAP-01 | Empty app, default chrome |
| SNAP-02 | Single sine, legend LeftTop |
| SNAP-03 | Legend in all 4 corners (one image each) — *direct visual regression for the corner bug* |
| SNAP-04 | 8-trace scope, legend RightTop |
| SNAP-05 | FFT panel open w/ tone spectrum |
| SNAP-06 | Thresholds + markers overlaid |
| SNAP-07 | XY scope pair |
| SNAP-08 | Traces panel open |
| SNAP-09 | Narrow window → collapsed chrome |
| SNAP-10 | Dark scheme + custom scheme |

Tolerances: UI chrome 0.1 %, plot canvas 0.5 %, font-heavy panels 1 %.
Store reference PNGs under `tests/e2e/snapshots/`, regenerate via
`LIVEPLOT_SNAPSHOT=update`.

---

## 7. Feature-Gated Areas

| Cargo feature | Tests gated behind | Notes |
| ------------- | ------------------ | ----- |
| `fft` | `#[cfg(feature = "fft")]` whole `fft.rs` | needs `rustfft` — CI runs `--all-features` |
| `parquet` | EXP-03 | `write_parquet_aligned_path` |
| `tiles` | SCP/STR-S (egui_tiles paths) | default feature — most tests require it |
| `grpc` | none in e2e (client/server examples, out of scope) | compile-checked only |
| `window_control_display_info` | none | example-only helper |

CI matrix (§8) runs the suite at `default`, `--features fft`,
`--all-features`.

---

## 8. CI Integration

* `cargo test --test e2e` in the main test job (alongside `cargo test`).
* `cargo test --all-features --test e2e` in a full job.
* `LIVEPLOT_STRESS=1 … -- --ignored` nightly or `on-demand` job.
* Snapshots: optional job with a lavapipe (`VK_ICD_FILENAMES`) or swiftshader
  adapter; failure mode = skip, never red.
* Artifacts: snapshot diffs uploaded on failure.

---

## 9. Conventions

* Test IDs referenced in `// CAT-XXX-NN` comments above each `#[test]`.
* Helpers live in `harness.rs` / `signals.rs` / `asserts.rs` — no logic
  duplication in tests.
* One concept per test; prefer table-driven for variants.
* Deterministic seeds only; `TestRng::new(seed)`.
* `harness.step()` after any UI interaction; `harness.run()` only where the
  app stops requesting repaint (the live plot never does → use `step`/
  `run_steps`, **not** `run`).
* Assertions order: internal state first (`harness.state()`), accesskit
  second, snapshots last.
* Time-sensitive code paths use explicit `step_dt`, never `sleep()` except
  in `live_thread` stress feeds (and there we assert on counts, not wall
  time).

---

## 10. Coverage Matrix

| Area | Unit tests (today) | e2e (this plan) | Status |
| ---- | ------------------ | --------------- | ------ |
| Traces collection | ✅ `tests/traces.rs` | CAT-TRC | harness done |
| Expressions/formula | ✅ `tests/expr.rs`, `math_traces.rs` | CAT-MATH | UI-level new |
| Markers/measurement | ✅ `marker_ui.rs`, `measurement_ui.rs` | CAT-MEAS | click-path coverage |
| Thresholds | partial | CAT-THR | events via UI |
| Triggers | — | CAT-TRG | new |
| Hotkeys logic | ✅ `hotkeys.rs` | CAT-HK | through real ctx |
| Undo | ✅ `undo.rs` | CAT-PER | e2e stack |
| Export | ✅ `export.rs` | CAT-EXP | file verification |
| Legend | — | CAT-LGD | **regression for 2.5.4** |
| FFT | — | CAT-FFT | new |
| Controllers | — | CAT-CTRL | new |
| Events | ✅ `events.rs` | CAT-EVT | e2e dispatch |
| Stress | — | §5 | new |
| Snapshots | — | §6 | new |

---

## 11. Known Limitations & Risks

* **No GPU on some CI** → snapshot layer skips silently; functional tests
  must not rely on `render()`.
* **System fonts vary** → text-node assertions use exact labels only where
  egui generates them; numeric formatting assertions go through
  `format_value`, not rendered text.
* **`Harness::run` unusable** (app repaints every frame) → we always
  `step()`/`run_steps(n)`; documented helper `steps(n)`.
* **mpsc receiver drain timing** — ingestion happens inside `update()`;
  tests must `step()` at least once after feeding before asserting.
* **Determinism of `SystemTime` paths** — tests avoid now()-dependent
  features unless the feature itself is under test.
* **File dialogs (`rfd`)** can't be driven → export tests target the
  `pending_requests`/writer level; dialog launch is untested (acceptable).
* **First-frame accessibility check** disabled globally; CAT-A11Y re-enables
  it in one dedicated audit test once widget naming improves.
* **Long suite runtime in debug** — heavy cases `#[ignore]`d; dev profile
  already uses `opt-level=2` for deps, which keeps them tractable.

---

## Appendix A — Harness API reference

```rust
let bed = make_test_bed();                       // 1200×800
let bed = make_test_bed_opts(|b| b.with_size(v));// custom
bed.sink.create_trace("sig", Some("info"));      // PlotSink API
bed.harness.step();                              // one frame
bed.harness.run_steps(30);                       // n frames
bed.harness.set_size(vec2(400.,300.));           // resize
bed.harness.get_by_label("FFT").click();         // accesskit click
bed.harness.query_by_label_contains("Traces");   // Option<Node>
bed.harness.input_mut().events.push(Event::…);   // raw input
let panel: &LivePlotPanel = bed.harness.state(); // state access
let panel: &mut _ = bed.harness.state_mut();
// helpers
feed(&bed, "sig", SignalSpec::Sine{freq:3.,amp:1.}, 1000);
feed_chunked(&bed, "sig", spec, chunk=100);
steps(&mut bed, 5);
legend_rect(&bed, scope_idx);   // ScopePanel::legend_overlay_rect getter
scope(&bed, i) -> &ScopeData;
scope_mut(&bed, i) -> &mut ScopeData;
click_menu(&mut bed, "FFT");
key(&mut bed, Key::P);
key_mod(&mut bed, Modifiers::CTRL, Key::T);
```

## Appendix B — Signal generator reference

```rust
SignalSpec::Sine { freq, amp, phase }
SignalSpec::Square { freq, amp, duty }
SignalSpec::Step { levels: &[f64], per: usize }
SignalSpec::Ramp { slope, offset }
SignalSpec::WhiteNoise { seed, amp }
SignalSpec::RandomWalk { seed, step }
SignalSpec::Impulse { at_idx, amp }
SignalSpec::Chirp { f0, f1 }
SignalSpec::Constant { v }
SignalSpec::SumOfSines(&[(freq,amp)])
SignalSpec::Gaps { inner: Box<SignalSpec>, gap_every, gap_len }
SignalSpec::Custom(Box<dyn Fn(usize,f64)->f64>)

fn gen(spec, n, dt, t0) -> Vec<PlotPoint>
```
