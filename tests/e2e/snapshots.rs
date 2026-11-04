//! §6 — wgpu snapshot layer. Opt-in via `LIVEPLOT_SNAPSHOT=1`
//! (or `update` to write reference PNGs into tests/e2e/snapshots/).
//! Skips gracefully when no GPU adapter / wgpu renderer is available.

use liveplot::data::scope::LegendPosition;
use liveplot::LivePlotPanel;

use crate::harness::*;
use crate::signals::SignalSpec;

fn snap_dir() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/e2e/snapshots")
}

fn wgpu_bed() -> Option<TestBed> {
    if std::env::var("LIVEPLOT_SNAPSHOT").is_err() {
        return None;
    }
    let (sink, rx) = liveplot::channel_plot();
    let panel = LivePlotPanel::new(rx);
    let built = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        egui_kittest::Harness::builder()
            .with_size(egui::vec2(W, H))
            .wgpu()
            .with_context_setup(|ctx| {
                egui_material_icons::initialize(ctx);
            })
            .with_accessibility_check(false)
            .with_step_dt(1.0 / 60.0)
            .build_ui_state(
                move |ui, panel| {
                    liveplot::data::hotkeys::handle_hotkeys(panel, ui.ctx());
                    panel.apply_controllers_embedded(ui.ctx());
                    panel.update(ui);
                },
                panel,
            )
    }));
    match built {
        Ok(mut harness) => {
            harness.step();
            Some(TestBed { harness, sink })
        }
        Err(_) => {
            eprintln!("snapshot: no wgpu adapter available — skipping");
            None
        }
    }
}

fn snap(bed: &mut TestBed, name: &str) {
    match bed.harness.render() {
        Ok(img) => {
            std::fs::create_dir_all(snap_dir()).unwrap();
            let path = snap_dir().join(format!("{name}.png"));
            img.save(&path).unwrap();
        }
        Err(e) => eprintln!("snapshot {name}: render failed ({e}) — skipped"),
    }
}

#[test]
fn snapshot_scenes() {
    let Some(mut bed) = wgpu_bed() else { return };

    // Empty chrome.
    snap(&mut bed, "empty_app");

    // Single sine + legend in each corner.
    feed_spec(&bed, "sig", &SignalSpec::sine(3.0), 500, 1000.0, 0.0);
    steps(&mut bed, 3);
    snap(&mut bed, "sine_legend_lefttop");

    for (pos, name) in [
        (LegendPosition::RightTop, "sine_legend_righttop"),
        (LegendPosition::LeftBottom, "sine_legend_leftbottom"),
        (LegendPosition::RightBottom, "sine_legend_rightbottom"),
    ] {
        scope_mut(&mut bed, 0).legend_position = pos;
        steps(&mut bed, 2);
        snap(&mut bed, name);
    }

    // Many-trace scope.
    feed_n_traces(&bed, 8, 300, 500.0);
    steps(&mut bed, 3);
    snap(&mut bed, "many_traces");
}

#[test]
fn snapshot_narrow_window() {
    let Some(mut bed) = wgpu_bed() else { return };
    feed_sine(&bed, "a", 200, 1.0, 100.0, 0.0);
    steps(&mut bed, 3);
    bed.harness.set_size(egui::vec2(400.0, 300.0));
    steps(&mut bed, 3);
    snap(&mut bed, "narrow_window");
}
