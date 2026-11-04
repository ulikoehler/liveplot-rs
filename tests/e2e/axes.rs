//! CAT-AX — axis formatting, log scale, auto-fit, grid through the live app.

use crate::harness::*;
use crate::signals::SignalSpec;

#[test]
fn auto_fit_tracks_data_extrema() {
    let mut bed = make_test_bed();
    feed_spec(
        &bed,
        "a",
        &SignalSpec::Ramp {
            slope: 1.0,
            offset: -5.0,
        },
        100,
        10.0,
        0.0,
    );
    {
        let sc = scope_mut(&mut bed, 0);
        sc.y_axis.auto_fit = true;
        sc.x_axis.auto_fit = true;
    }
    steps(&mut bed, 4);
    let s = scope(&bed, 0);
    let (xb, yb) = s.last_plot_bounds.unwrap();
    assert!(xb[1] >= xb[0]);
    // y spans roughly [-5, 5); auto-fit should contain the data.
    assert!(yb[0] <= -5.0 + 0.5 && yb[1] >= 3.0, "y bounds {yb:?}");
}

#[test]
fn log_scale_toggle_survives_render() {
    let mut bed = make_test_bed();
    feed_spec(
        &bed,
        "a",
        &SignalSpec::Custom(std::sync::Arc::new(|i, _| (i + 1) as f64)),
        100,
        10.0,
        0.0,
    );
    scope_mut(&mut bed, 0).y_axis.log_scale = true;
    steps(&mut bed, 3);
    assert!(scope(&bed, 0).y_axis.log_scale);
}

#[test]
fn axis_format_value_time_and_value() {
    let mut bed = make_test_bed();
    let s = scope(&bed, 0);
    // Time axis (default) formats epoch seconds.
    let t = s.x_axis.format_value(3600.0, None);
    assert!(!t.is_empty());
    // Value axis on y produces a number string.
    let v = s.y_axis.format_value(1234.5, Some(1.0));
    assert!(!v.is_empty());
}
