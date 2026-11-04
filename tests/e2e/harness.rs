//! Shared e2e harness: builds a `LivePlotPanel` inside an `egui_kittest`
//! `Harness` and feeds it synthetic trace data through the public sink API.

use liveplot::data::scope::ScopeData;
use liveplot::panels::Panel;
use liveplot::{channel_plot, LivePlotPanel, PlotSink};

use crate::signals::{gen, SignalSpec};

pub const W: f32 = 1200.0;
pub const H: f32 = 800.0;

pub struct TestBed {
    pub harness: egui_kittest::Harness<'static, LivePlotPanel>,
    pub sink: PlotSink,
}

pub fn make_test_bed() -> TestBed {
    make_test_bed_with(egui::vec2(W, H))
}

pub fn make_test_bed_with(size: egui::Vec2) -> TestBed {
    let (sink, rx) = channel_plot();
    let panel = LivePlotPanel::new(rx);
    let mut harness = egui_kittest::Harness::builder()
        .with_size(size)
        // Font setup must happen before the first pass — the builder runs
        // one frame immediately.
        .with_context_setup(|ctx| {
            egui_material_icons::initialize(ctx);
        })
        // Several scope toolbar widgets (sliders, drag values) have no
        // accessible name today — that's tracked as a follow-up (CAT-A11Y),
        // not an e2e failure condition.
        .with_accessibility_check(false)
        .with_step_dt(1.0 / 60.0)
        .build_ui_state(
            move |ui, panel| {
                liveplot::data::hotkeys::handle_hotkeys(panel, ui.ctx());
                // Embedders' real pattern: apply external controllers
                // (no-op when none are attached) then render.
                panel.apply_controllers_embedded(ui.ctx());
                panel.update(ui);
            },
            panel,
        );
    harness.step();
    TestBed { harness, sink }
}

/// App-level harness: runs the full `LivePlotApp` (eframe::App), so
/// postprocess logic — event emission, save/load state, screenshot
/// capture routing — is covered too.
pub struct AppBed {
    pub harness: egui_kittest::Harness<'static, liveplot::LivePlotApp>,
    pub sink: PlotSink,
}

pub fn make_app_bed() -> AppBed {
    let (sink, rx) = channel_plot();
    let mut harness = egui_kittest::Harness::builder()
        .with_size(egui::vec2(W, H))
        .with_context_setup(|ctx| {
            egui_material_icons::initialize(ctx);
        })
        .with_accessibility_check(false)
        .with_step_dt(1.0 / 60.0)
        .build_eframe(move |_cc| liveplot::LivePlotApp::new(rx));
    harness.step();
    AppBed { harness, sink }
}

// ── Steps ───────────────────────────────────────────────────────────────────

pub fn steps(bed: &mut TestBed, n: usize) {
    for _ in 0..n {
        bed.harness.step();
    }
}

// ── Feeding ─────────────────────────────────────────────────────────────────

pub fn feed_spec(bed: &TestBed, name: &str, spec: &SignalSpec, n: usize, fs: f64, t0: f64) {
    let trace = bed.sink.create_trace(name, None);
    bed.sink.send_points(&trace, gen(spec, n, fs, t0)).unwrap();
}

pub fn feed_sine(bed: &TestBed, name: &str, n: usize, freq: f64, fs: f64, t0: f64) {
    feed_spec(bed, name, &SignalSpec::sine(freq), n, fs, t0);
}

pub fn feed_n_traces(bed: &TestBed, count: usize, n: usize, fs: f64) {
    for i in 0..count {
        feed_spec(
            bed,
            &format!("sig_{i:03}"),
            &SignalSpec::SumOfSines(vec![(1.0 + i as f64, 1.0)]),
            n,
            fs,
            0.0,
        );
    }
}

// ── State accessors ─────────────────────────────────────────────────────────

pub fn scopes(bed: &TestBed) -> Vec<&ScopeData> {
    bed.harness.state().liveplot_panel.get_data()
}

pub fn scope(bed: &TestBed, idx: usize) -> &ScopeData {
    let data = scopes(bed);
    data[idx]
}

pub fn scope_mut(bed: &mut TestBed, idx: usize) -> &mut ScopeData {
    bed.harness
        .state_mut()
        .liveplot_panel
        .get_data_mut()
        .remove(idx)
}

/// Find a panel of type `T` in any dock list.
pub fn find_panel<T: liveplot::Panel + 'static>(bed: &TestBed) -> Option<&T> {
    for p in bed
        .harness
        .state()
        .left_side_panels
        .iter()
        .chain(bed.harness.state().right_side_panels.iter())
        .chain(bed.harness.state().bottom_panels.iter())
        .chain(bed.harness.state().detached_panels.iter())
        .chain(bed.harness.state().empty_panels.iter())
    {
        if let Some(t) = p.downcast_ref::<T>() {
            return Some(t);
        }
    }
    None
}

pub fn find_panel_mut<T: liveplot::Panel + 'static>(bed: &mut TestBed) -> Option<&mut T> {
    let panel = bed.harness.state_mut();
    for p in panel
        .left_side_panels
        .iter_mut()
        .chain(panel.right_side_panels.iter_mut())
        .chain(panel.bottom_panels.iter_mut())
        .chain(panel.detached_panels.iter_mut())
        .chain(panel.empty_panels.iter_mut())
    {
        if let Some(t) = p.downcast_mut::<T>() {
            return Some(t);
        }
    }
    None
}

pub fn panel_visible<T: 'static + Panel>(bed: &TestBed) -> bool {
    find_panel::<T>(bed).is_some_and(|p| p.state().visible)
}

// ── Input helpers ───────────────────────────────────────────────────────────

/// Click whatever node label-contains `label`. Panics if there is no match.
pub fn click_label_contains(bed: &mut TestBed, label: &str) {
    use egui_kittest::kittest::Queryable;
    let node = bed
        .harness
        .get_all_by_label_contains(label)
        .next()
        .unwrap_or_else(|| panic!("no node label-containing {label:?}"));
    node.click();
    bed.harness.step();
}

/// Simulate a primary click at a raw screen position (for painted widgets
/// like legend rows that have no accesskit node).
pub fn click_at(bed: &mut TestBed, pos: egui::Pos2) {
    use egui::Event;
    let events = vec![
        Event::PointerMoved(pos),
        Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed: true,
            modifiers: egui::Modifiers::NONE,
        },
        Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed: false,
            modifiers: egui::Modifiers::NONE,
        },
    ];
    for ev in events {
        bed.harness.input_mut().events.push(ev);
        bed.harness.step();
    }
}

/// Drag with the given pointer button between two screen positions.
pub fn drag(bed: &mut TestBed, from: egui::Pos2, to: egui::Pos2, button: egui::PointerButton) {
    use egui::Event;
    let ev = |pos, pressed| Event::PointerButton {
        pos,
        button,
        pressed,
        modifiers: egui::Modifiers::NONE,
    };
    for e in [
        Event::PointerMoved(from),
        ev(from, true),
        Event::PointerMoved(to),
        ev(to, false),
    ] {
        bed.harness.input_mut().events.push(e);
        bed.harness.step();
    }
}

pub fn key_press(bed: &mut TestBed, key: egui::Key) {
    key_press_mod(bed, egui::Modifiers::NONE, key);
}

pub fn key_press_mod(bed: &mut TestBed, modifiers: egui::Modifiers, key: egui::Key) {
    use egui::Event;
    bed.harness.input_mut().events.push(Event::Key {
        key,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers,
    });
    bed.harness.step();
    bed.harness.input_mut().events.push(Event::Key {
        key,
        physical_key: None,
        pressed: false,
        repeat: false,
        modifiers,
    });
    bed.harness.step();
}
