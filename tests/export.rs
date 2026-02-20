use liveplot::data::export::*;
use liveplot::data::traces::TraceRef;
use std::collections::HashMap;

fn mk_series(map: &[(&str, &[(f64, f64)])]) -> (Vec<TraceRef>, HashMap<TraceRef, Vec<[f64; 2]>>) {
    let mut order: Vec<TraceRef> = Vec::new();
    let mut series: HashMap<TraceRef, Vec<[f64; 2]>> = HashMap::new();
    for (name, pts) in map {
        order.push(TraceRef(name.to_string()));
        let vec: Vec<[f64; 2]> = pts.iter().map(|(t, v)| [*t, *v]).collect();
        series.insert(TraceRef(name.to_string()), vec);
    }
    (order, series)
}

#[test]
fn aligns_within_tolerance() {
    let tol = 1e-9;
    let (order, series) = mk_series(&[
        ("a", &[(0.0, 1.0), (1.0, 2.0)]),
        ("b", &[(0.0 + 1e-10, 10.0), (2.0, 20.0)]),
    ]);
    let rows = align_series(&order, &series, tol);
    assert_eq!(rows.len(), 3);
    assert!((rows[0].0 - 0.0).abs() <= tol);
    assert_eq!(rows[0].1, vec![Some(1.0), Some(10.0)]);
}

#[test]
fn writes_expected_csv() {
    let tol = 1e-9;
    let (order, series) = mk_series(&[
        ("sine", &[(0.0, 0.1), (1.0, 0.2)]),
        ("cos", &[(0.0 + 5e-10, 1.1), (2.0, 1.2)]),
    ]);
    let rows = align_series(&order, &series, tol);
    let mut buf = Vec::new();
    write_aligned_rows_csv(&mut buf, &order, &rows).unwrap();
    let s = String::from_utf8(buf).unwrap();
    let lines: Vec<&str> = s.trim().split('\n').collect();
    assert_eq!(lines[0], "timestamp_seconds,sine,cos");
}
