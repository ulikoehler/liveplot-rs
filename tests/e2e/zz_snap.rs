use crate::harness::*;
use crate::signals::SignalSpec;

/// Real LivePlotApp (what `run_liveplot` uses) mirroring fft_demo:
/// two traces + FFT panel, volcano on. Screenshot + legend check.
#[test]
fn real_app_volcano_legend() {
    let mut bed = make_app_bed();
    for s in bed
        .harness
        .state_mut()
        .main_panel
        .liveplot_panel
        .get_data_mut()
        .iter_mut()
    {
        s.volcano_enabled = true;
    }
    // fft_demo feeds two_tone + swept_freq.
    let two = bed.sink.create_trace("two_tone", None);
    bed.sink
        .send_points(
            &two,
            crate::signals::gen(&SignalSpec::sine(5.0), 4096, 1000.0, 0.0),
        )
        .unwrap();
    let sw = bed.sink.create_trace("swept_freq", None);
    bed.sink
        .send_points(
            &sw,
            crate::signals::gen(&SignalSpec::sine(50.0), 4096, 1000.0, 0.0),
        )
        .unwrap();
    for _ in 0..40 {
        bed.harness.step();
    }
    let (lr, tr, rd) = {
        let scopes = bed.harness.state().main_panel.liveplot_panel.get_data();
        (
            scopes[0].legend_overlay_rect,
            scopes[0].trace_order.len(),
            scopes[0].rendered_this_frame,
        )
    };
    eprintln!("app-level: legend_rect={lr:?} traces={tr} rendered={rd}");
    if let Ok(img) = bed.harness.render() {
        img.save("/tmp/real_app_volcano.png").ok();
    }
    assert!(lr.is_some(), "legend must render in the real app");
}
