//! CAT-CFG — remaining feature flags: pause, clear-all, tick labels.

use egui_kittest::kittest::Queryable;
use liveplot::config::{LivePlotConfig, ScopeButton};

use crate::harness::*;

/// CFG-03: `pause_resume=false` removes the Pause button.
#[test]
fn pause_resume_flag_hides_button() {
    let mut bed = make_app_bed();
    {
        let mut cfg = LivePlotConfig::default();
        cfg.features.pause_resume = false;
        bed.harness.state_mut().apply_config(&mut cfg);
    }
    bed.harness.step();
    assert!(
        bed.harness
            .query_all_by_label_contains("Pause")
            .next()
            .is_none(),
        "Pause button should be hidden"
    );
}

/// CFG-08: a button set without ClearAll hides it.
#[test]
fn button_set_without_clear_all_hides_it() {
    let mut bed = make_app_bed();
    {
        let mut cfg = LivePlotConfig::default();
        cfg.layout.top_bar_buttons = Some(vec![ScopeButton::Traces]);
        bed.harness.state_mut().apply_config(&mut cfg);
    }
    bed.harness.step();
    assert!(
        bed.harness
            .query_all_by_label_contains("Clear")
            .next()
            .is_none(),
        "Clear All should not render when unconfigured"
    );
}

/// CFG-04: `top_bar=false` collapses the menu bar into sidebar icons
/// (labels disappear from the labelled-button query).
#[test]
fn top_bar_flag_removes_top_bar() {
    let mut bed = make_app_bed();
    {
        let mut cfg = LivePlotConfig::default();
        cfg.features.top_bar = false;
        bed.harness.state_mut().apply_config(&mut cfg);
    }
    bed.harness.step();
    // Without the top bar the Scopes button is gone (sidebar still shows
    // icon buttons, but the labelled top-bar entries are absent).
    assert!(
        bed.harness
            .query_all_by_label_contains("Scopes")
            .next()
            .is_none(),
        "top bar should be hidden"
    );
}

/// CFG-13: `max_fps` clamps the repaint interval.
#[test]
fn max_fps_clamps_repaint() {
    let mut bed = make_app_bed();
    {
        let mut cfg = LivePlotConfig::default();
        cfg.max_fps = Some(10.0);
        bed.harness.state_mut().apply_config(&mut cfg);
    }
    bed.harness.step();
    let interval = bed.harness.state().repaint_interval;
    assert!(
        (interval.as_secs_f64() - 0.1).abs() < 0.01,
        "repaint interval should be ~100ms at 10fps, got {interval:?}"
    );
}
