//! CAT-CFG — `LivePlotConfig` / `FeatureFlags` applied at app level.

use egui_kittest::kittest::Queryable;
use liveplot::config::{LivePlotConfig, ScopeButton};

use crate::harness::*;
use crate::signals::{gen, SignalSpec};

#[test]
fn feature_flags_hide_legend_and_grid() {
    let mut bed = make_app_bed();
    let trace = bed.sink.create_trace("a", None);
    bed.sink
        .send_points(&trace, gen(&SignalSpec::sine(1.0), 200, 100.0, 0.0))
        .unwrap();
    {
        let mut cfg = LivePlotConfig::default();
        cfg.features.legend = false;
        cfg.features.grid = false;
        bed.harness.state_mut().apply_config(&mut cfg);
    }
    for _ in 0..3 {
        bed.harness.step();
    }
    {
        let scopes = bed.harness.state().main_panel.liveplot_panel.get_data();
        let s = scopes[0];
        assert!(!s.show_legend, "features.legend=false should hide legend");
        assert!(!s.show_grid, "features.grid=false should hide grid");
        assert!(
            bed.harness.state().main_panel.liveplot_panel.scope_panels()[0]
                .legend_overlay_rect()
                .is_none(),
            "no legend overlay should be rendered"
        );
    }
}

#[test]
fn config_applies_axis_and_limit_settings() {
    let mut bed = make_app_bed();
    {
        let mut cfg = LivePlotConfig::default();
        cfg.time_window_secs = 4.0;
        cfg.y_unit = Some("V".into());
        cfg.max_points = 500;
        cfg.y_log = true;
        bed.harness.state_mut().apply_config(&mut cfg);
    }
    bed.harness.step();
    {
        let app = bed.harness.state();
        let scopes = app.main_panel.liveplot_panel.get_data();
        assert_eq!(scopes[0].time_window, 4.0);
        assert!(matches!(
            &scopes[0].y_axis.axis_type,
            liveplot::data::scope::AxisType::Value(vf) if vf.unit.as_deref() == Some("V")
        ));
        assert!(scopes[0].y_axis.log_scale);
        assert_eq!(app.main_panel.traces_data.max_points, 500);
    }
}

#[test]
fn custom_top_bar_buttons_hide_others() {
    let mut bed = make_app_bed();
    {
        let mut cfg = LivePlotConfig::default();
        cfg.layout.top_bar_buttons = Some(vec![ScopeButton::Traces, ScopeButton::PauseResume]);
        bed.harness.state_mut().apply_config(&mut cfg);
    }
    bed.harness.step();
    assert!(
        bed.harness
            .query_all_by_label_contains("Traces")
            .next()
            .is_some(),
        "configured button should render"
    );
    assert!(
        bed.harness
            .query_all_by_label_contains("Thresholds")
            .next()
            .is_none(),
        "unconfigured button should not render"
    );
}

#[test]
fn headline_renders_in_app() {
    let mut bed = make_app_bed();
    {
        let mut cfg = LivePlotConfig::default();
        cfg.headline = Some("Test Bench".into());
        bed.harness.state_mut().apply_config(&mut cfg);
    }
    bed.harness.step();
    assert!(
        bed.harness
            .query_all_by_label_contains("Test Bench")
            .next()
            .is_some(),
        "headline should render in the UI"
    );
}
