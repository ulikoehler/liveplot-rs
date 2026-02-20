use liveplot::data::x_formatter::*;

// Helper: build a UTC timestamp as seconds
fn utc_secs(year: i32, month: u32, day: u32, h: u32, m: u32, s: u32) -> f64 {
    use chrono::{NaiveDate, NaiveDateTime, NaiveTime, TimeZone};
    let ndt = NaiveDateTime::new(
        NaiveDate::from_ymd_opt(year, month, day).unwrap(),
        NaiveTime::from_hms_opt(h, m, s).unwrap(),
    );
    chrono::Utc.from_utc_datetime(&ndt).timestamp() as f64
}

// Helper: build a LOCAL timestamp as seconds
fn local_secs(year: i32, month: u32, day: u32, h: u32, m: u32, s: u32) -> f64 {
    use chrono::{Local, NaiveDate, NaiveDateTime, NaiveTime, TimeZone};
    let ndt = NaiveDateTime::new(
        NaiveDate::from_ymd_opt(year, month, day).unwrap(),
        NaiveTime::from_hms_opt(h, m, s).unwrap(),
    );
    Local
        .from_local_datetime(&ndt)
        .earliest()
        .map(|dt| dt.timestamp() as f64)
        .unwrap_or(0.0)
}

#[test]
fn epoch_unit_units_per_second() {
    assert_eq!(EpochUnit::Seconds.units_per_second(), 1.0);
    assert_eq!(EpochUnit::Milliseconds.units_per_second(), 1_000.0);
    assert_eq!(EpochUnit::Microseconds.units_per_second(), 1_000_000.0);
    assert_eq!(EpochUnit::Nanoseconds.units_per_second(), 1_000_000_000.0);
}

#[test]
fn determine_resolution_wide_range_returns_seconds() {
    let tf = TimeFormatter::default();
    assert_eq!(tf.determine_resolution(7_200.0), TimeResolution::Seconds);
}

#[test]
fn determine_resolution_just_below_ms_threshold() {
    let tf = TimeFormatter::default();
    assert_eq!(
        tf.determine_resolution(3_599.0),
        TimeResolution::Milliseconds
    );
}

#[test]
fn format_no_date_when_range_within_day() {
    let tf = TimeFormatter::default();
    let t = utc_secs(2024, 1, 15, 12, 0, 0);
    let range = (t - 5.0, t + 5.0);
    let out = tf.format(t, range);
    let colon_count = out.chars().filter(|&c| c == ':').count();
    assert_eq!(colon_count, 2, "Expected HH:MM:SS format, got: {}", out);
    assert!(!out.contains('-'), "Unexpected date in: {}", out);
}

#[test]
fn format_shows_date_when_range_crosses_midnight() {
    let tf = TimeFormatter::default();
    let t_before_midnight = local_secs(2024, 1, 15, 23, 59, 55);
    let t_after_midnight = local_secs(2024, 1, 16, 0, 0, 5);
    let range = (t_before_midnight, t_after_midnight);
    let out = tf.format(t_before_midnight, range);
    assert!(out.contains('-'), "Expected date in: {}", out);
}

#[test]
fn format_shows_year_when_year_changes() {
    let tf = TimeFormatter::default();
    let t_dec31 = local_secs(2023, 12, 31, 23, 59, 55);
    let t_jan01 = local_secs(2024, 1, 1, 0, 0, 5);
    let out = tf.format(t_dec31, (t_dec31, t_jan01));
    assert!(
        out.contains("2023") || out.contains("2024"),
        "No year in: {}",
        out
    );
}

// Additional coverage (representative subset) — decimal/scientific helpers
#[test]
fn decimal_formatter_default_uses_caller_dec_pl() {
    let df = DecimalFormatter::default();
    assert_eq!(df.format(3.14159, 2), "3.14");
}

#[test]
fn scientific_formatter_with_unit() {
    let sf = ScientificFormatter {
        significant_digits: Some(2),
        unit: Some("Hz".to_string()),
    };
    let s = sf.format(2_000.0, 2);
    assert!(s.ends_with("Hz"), "Got: {}", s);
}
