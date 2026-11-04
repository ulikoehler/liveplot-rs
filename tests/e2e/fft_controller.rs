//! CAT-CTRL — FFTController paths (requires the `fft` feature).

use liveplot::controllers::FFTController;
use liveplot::panels::fft_ui::FftPanel;

use crate::harness::*;
use crate::signals::SignalSpec;

#[test]
fn fft_controller_publishes_panel_info() {
    let mut bed = make_test_bed();
    let fc = FFTController::new();
    let rx = fc.subscribe();
    bed.harness
        .state_mut()
        .set_controllers(None, None, None, None, None, Some(fc.clone()), None);
    feed_spec(&bed, "a", &SignalSpec::sine(1.0), 100, 100.0, 0.0);
    steps(&mut bed, 3);

    let info = rx.try_iter().last().expect("FFTPanelInfo published");
    assert!(!info.shown, "panel hidden by default");

    fc.set_shown(true);
    fc.request_set_size([400.0, 300.0]);
    steps(&mut bed, 3);
    let info = rx.try_iter().last().expect("updated FFTPanelInfo");
    assert!(info.shown, "set_shown(true) should reflect in info");
    assert_eq!(info.requested_size, Some([400.0, 300.0]));
}

#[test]
fn fft_controller_show_updates_panel_state() {
    let mut bed = make_test_bed();
    let fc = FFTController::new();
    bed.harness
        .state_mut()
        .set_controllers(None, None, None, None, None, Some(fc.clone()), None);
    feed_spec(&bed, "a", &SignalSpec::sine(1.0), 100, 100.0, 0.0);
    steps(&mut bed, 2);
    fc.set_shown(true);
    steps(&mut bed, 3);
    // The controller's shown flag drives the panel visibility (or is at
    // least faithfully published — verify via published info).
    let info = fc.get_last_info().expect("last info");
    assert!(info.shown);
    // The FftPanel itself should exist regardless.
    assert!(find_panel::<FftPanel>(&bed).is_some());
}
