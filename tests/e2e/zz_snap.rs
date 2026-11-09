use crate::harness::*;
use liveplot::panels::fft_ui::FftPanel;
use liveplot::Panel;

/// Mirrors `fft_demo`: two traces + FFT panel visible. Screenshot
/// both volcano and egui paths to check the legend visually.
fn fft_demo_bed(volcano: bool) -> TestBed {
    let mut bed = make_test_bed();
    feed_sine(&bed, "two_tone", 4096, 5.0, 1000.0, 0.0);
    feed_sine(&bed, "swept_freq", 4096, 50.0, 1000.0, 0.0);
    {
        let fp = find_panel_mut::<FftPanel>(&mut bed).expect("FftPanel");
        fp.state_mut().visible = true;
        fp.scope_ui.get_data_mut().volcano_enabled = volcano;
    }
    for s in scopes(&bed).iter() {
        let _ = s;
    }
    scope_mut(&mut bed, 0).volcano_enabled = volcano;
    steps(&mut bed, 40);
    bed
}

#[test]
fn fft_demo_screenshot() {
    for (vol, name) in [(false, "egui"), (true, "volcano")] {
        let mut bed = fft_demo_bed(vol);
        let s = scope(&bed, 0);
        eprintln!("[{name}] legend_rect={:?} traces={:?} rendered={}",
                  s.legend_overlay_rect, s.trace_order.len(), s.rendered_this_frame);
        if let Ok(img) = bed.harness.render() {
            img.save(format!("/tmp/fft_demo_{name}.png")).ok();
        }
    }
}
