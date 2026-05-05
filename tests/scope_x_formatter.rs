use liveplot::data::scope::{AxisSettings, AxisType, ScopeData, ScopeType, XDateFormat};
use liveplot::data::x_formatter::XFormatter;

#[test]
fn scope_auto_x_formatter_switches_for_time_and_xy() {
    // Default scope is a time scope with Auto formatter
    let mut scope = ScopeData::default();
    assert_eq!(scope.scope_type, ScopeType::TimeScope);
    // Auto on a time axis should produce a time-like string
    let t = 1_700_000_000.0_f64; // a large epoch second
    let out = scope.x_axis.format_value(t, 4, 1.0);
    assert!(out.contains(":") || out.contains("-"));

    // Switch to XY scope and ensure formatter auto-selects numeric formatting
    scope.scope_type = ScopeType::XYScope;
    scope.x_axis.axis_type = AxisType::Value(None);
    scope.x_axis.name = Some("X".to_string());
    scope.x_axis.x_formatter = XFormatter::Auto;

    let v = 1234.5678_f64;
    let out2 = scope.x_axis.format_value(v, 2, 1.0);
    // numeric formatting should not contain time separators
    assert!(!out2.contains(":") && !out2.contains("-"));
}
