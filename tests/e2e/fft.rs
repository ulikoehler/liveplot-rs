//! CAT-FFT — FFT panel end-to-end (requires the `fft` cargo feature).
//! Run with: cargo test --test e2e --features fft

use liveplot::panels::fft_ui::FftPanel;
use liveplot::panels::Panel;
use liveplot::TraceRef;

use crate::harness::*;
use crate::signals::SignalSpec;

fn wait_for_spectrum(bed: &mut TestBed, name: &str, max_steps: usize) -> Vec<[f64; 2]> {
    for _ in 0..max_steps {
        bed.harness.step();
        let done = find_panel::<FftPanel>(bed).is_some_and(|fp| {
            fp.fft_data
                .fft_traces
                .get(&TraceRef(name.into()))
                .is_some_and(|e| !e.live.is_empty())
        });
        if done {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    find_panel::<FftPanel>(bed)
        .and_then(|fp| {
            fp.fft_data
                .fft_traces
                .get(&TraceRef(name.into()))
                .map(|e| e.live.iter().copied().collect())
        })
        .unwrap_or_default()
}

fn show_fft(bed: &mut TestBed) {
    let fp = find_panel_mut::<FftPanel>(bed).expect("FftPanel exists");
    fp.state_mut().visible = true;
    steps(bed, 2);
}

#[test]
fn fft_panel_computes_spectrum_of_tone() {
    let mut bed = make_test_bed();
    // 60 Hz tone at 1 kHz — land inside an FFT bin when fft_size ≤ 4096.
    feed_spec(&bed, "tone", &SignalSpec::tone(1.0), 8192, 1000.0, 0.0);
    show_fft(&mut bed);

    let spec = wait_for_spectrum(&mut bed, "tone", 200);
    assert!(!spec.is_empty(), "no FFT spectrum produced");

    let peak = spec
        .iter()
        .max_by(|a, b| a[1].partial_cmp(&b[1]).unwrap())
        .unwrap();
    // FFT size default (4096 or 8192) → bin width 1000/4096 ≈ 0.244 Hz.
    assert!(
        (peak[0] - 60.0).abs() < 1.0,
        "peak at {} Hz, expected ~60 Hz",
        peak[0]
    );
}

#[test]
fn fft_shows_warning_for_insufficient_data() {
    let mut bed = make_test_bed();
    feed_spec(&bed, "short", &SignalSpec::sine(5.0), 32, 100.0, 0.0);
    show_fft(&mut bed);
    steps(&mut bed, 5);
    let fp = find_panel::<FftPanel>(&bed).unwrap();
    // Entry exists with "Need N samples" style info (fft_size default >> 32).
    let entry = fp.fft_data.fft_traces.get(&TraceRef("short".into()));
    assert!(entry.is_some(), "short trace should appear in fft panel");
    assert!(
        entry.unwrap().info.contains("Need") || entry.unwrap().info.contains("Computing"),
        "unexpected info: {}",
        entry.unwrap().info
    );
}

#[test]
fn fft_legend_anchors_right_top() {
    // Regression: same corner-stretch bug as the scope legend (LGD-*).
    let mut bed = make_test_bed();
    feed_spec(&bed, "tone", &SignalSpec::tone(1.0), 8192, 1000.0, 0.0);
    show_fft(&mut bed);
    wait_for_spectrum(&mut bed, "tone", 200);
    steps(&mut bed, 3);

    let fp = find_panel::<FftPanel>(&bed).unwrap();
    let scope_rect = fp.scope_ui.get_data().last_plot_screen_rect.unwrap();
    let legend = fp
        .scope_ui
        .legend_overlay_rect()
        .expect("fft legend overlay");
    let plot = egui::Rect::from_min_max(
        egui::pos2(scope_rect[0], scope_rect[1]),
        egui::pos2(scope_rect[2], scope_rect[3]),
    );
    assert!(
        legend.width() < plot.width() * 0.5,
        "{legend:?} vs {plot:?}"
    );
    assert!(
        (legend.right() - plot.right()).abs() < 30.0 && (legend.top() - plot.top()).abs() < 30.0,
        "fft legend not at top-right: {legend:?} vs {plot:?}"
    );
}
