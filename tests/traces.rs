use egui::Color32;
use liveplot::color_scheme;
use liveplot::data::trace_look::TraceLook;
use liveplot::data::traces::{TraceData, TraceRef, TracesCollection};
use liveplot::sink::PlotCommand;

#[test]
fn cap_and_decimate_reduces_points() {
    let pts: Vec<[f64; 2]> = (0..10_000).map(|i| [i as f64, i as f64]).collect();
    let result = TraceData::cap_and_decimate(&pts, (0.0, 9999.0), 2000);
    assert!(
        result.len() <= 2001,
        "result should have at most 2001 points (2000 + last), got {}",
        result.len()
    );
    assert!(
        result.len() > 1000,
        "result should have significant decimation, got {}",
        result.len()
    );
    // First and last points should be preserved
    assert_eq!(result[0], [0.0, 0.0]);
    assert_eq!(*result.last().unwrap(), [9999.0, 9999.0]);
}

#[test]
fn cap_and_decimate_respects_bounds() {
    let pts: Vec<[f64; 2]> = (0..100).map(|i| [i as f64, i as f64]).collect();
    let result = TraceData::cap_and_decimate(&pts, (10.0, 20.0), 2000);
    assert!(result.iter().all(|p| p[0] >= 10.0 && p[0] <= 20.0));
    assert_eq!(result.len(), 11); // 10..=20 inclusive
}

#[test]
fn cap_and_decimate_no_decimation_when_under_limit() {
    let pts: Vec<[f64; 2]> = (0..100).map(|i| [i as f64, i as f64]).collect();
    let result = TraceData::cap_and_decimate(&pts, (0.0, 99.0), 2000);
    assert_eq!(result.len(), 100);
}

#[test]
fn recolor_changes_existing_traces() {
    // create collection with two traces
    let (tx, rx) = std::sync::mpsc::channel();
    let mut col = TracesCollection::new(rx);
    // register two traces via commands
    let _ = tx.send(PlotCommand::RegisterTrace {
        id: 1,
        name: "a".to_string(),
        info: None,
    });
    let _ = tx.send(PlotCommand::RegisterTrace {
        id: 2,
        name: "b".to_string(),
        info: None,
    });
    let new = col.update();
    assert_eq!(new.len(), 2);
    // initial palette must be default dark
    let first_color = col.get_trace(&TraceRef("a".into())).unwrap().look.color;
    assert_ne!(first_color, Color32::GRAY); // sanity
                                            // set a simple custom palette
    color_scheme::set_global_palette(vec![Color32::from_rgb(9, 9, 9), Color32::from_rgb(8, 8, 8)]);
    col.recolor_using_palette();
    assert_eq!(
        col.get_trace(&TraceRef("a".into())).unwrap().look.color,
        Color32::from_rgb(9, 9, 9)
    );
    assert_eq!(
        col.get_trace(&TraceRef("b".into())).unwrap().look.color,
        Color32::from_rgb(8, 8, 8)
    );
}

#[test]
fn next_color_index_avoids_collision_after_removal() {
    color_scheme::set_global_palette(vec![
        Color32::from_rgb(1, 1, 1),
        Color32::from_rgb(2, 2, 2),
        Color32::from_rgb(3, 3, 3),
    ]);
    let (tx, rx) = std::sync::mpsc::channel();
    let mut col = TracesCollection::new(rx);
    // Register 3 traces → indices 0, 1, 2
    let _ = tx.send(PlotCommand::RegisterTrace {
        id: 1,
        name: "a".into(),
        info: None,
    });
    let _ = tx.send(PlotCommand::RegisterTrace {
        id: 2,
        name: "b".into(),
        info: None,
    });
    let _ = tx.send(PlotCommand::RegisterTrace {
        id: 3,
        name: "c".into(),
        info: None,
    });
    let _ = col.update();
    // Remove "b" (index 1) → used slots are {0, 2}
    col.remove_trace(&TraceRef("b".into()));
    // Next index should be 1 (first unused slot)
    assert_eq!(col.next_color_index(), 1);
}

#[test]
fn recolor_by_order_assigns_palette_in_order() {
    let palette = vec![
        Color32::from_rgb(10, 10, 10),
        Color32::from_rgb(20, 20, 20),
        Color32::from_rgb(30, 30, 30),
    ];
    color_scheme::set_global_palette(palette.clone());
    let (tx, rx) = std::sync::mpsc::channel();
    let mut col = TracesCollection::new(rx);
    let _ = tx.send(PlotCommand::RegisterTrace {
        id: 1,
        name: "a".into(),
        info: None,
    });
    let _ = tx.send(PlotCommand::RegisterTrace {
        id: 2,
        name: "b".into(),
        info: None,
    });
    let _ = tx.send(PlotCommand::RegisterTrace {
        id: 3,
        name: "c".into(),
        info: None,
    });
    let _ = col.update();

    // Recolor in reverse order: c, b, a
    let order = vec![
        TraceRef("c".into()),
        TraceRef("b".into()),
        TraceRef("a".into()),
    ];
    col.recolor_by_order(&order);
    assert_eq!(
        col.get_trace(&TraceRef("c".into())).unwrap().look.color,
        palette[0]
    );
    assert_eq!(
        col.get_trace(&TraceRef("b".into())).unwrap().look.color,
        palette[1]
    );
    assert_eq!(
        col.get_trace(&TraceRef("a".into())).unwrap().look.color,
        palette[2]
    );
}

#[test]
fn next_color_index_sequential_when_palette_full() {
    let palette = vec![
        Color32::from_rgb(1, 1, 1),
        Color32::from_rgb(2, 2, 2),
        Color32::from_rgb(3, 3, 3),
    ];
    let pal_len = palette.len();
    color_scheme::set_global_palette(palette.clone());
    let (tx, rx) = std::sync::mpsc::channel();
    let mut col = TracesCollection::new(rx);

    // Fill all palette slots
    for i in 0..pal_len {
        let _ = tx.send(PlotCommand::RegisterTrace {
            id: i as u32 + 1,
            name: format!("t{}", i),
            info: None,
        });
    }
    let _ = col.update();

    // Now add 5 more traces — they should get distinct creation_index values
    // that wrap around the palette, not all 0.
    for i in 0..5 {
        let _ = tx.send(PlotCommand::RegisterTrace {
            id: (pal_len + i) as u32 + 1,
            name: format!("v{}", i),
            info: None,
        });
    }
    let _ = col.update();

    // Collect creation_indices of the 5 new traces
    let indices: Vec<usize> = (0..5)
        .map(|i| {
            col.get_trace(&TraceRef(format!("v{}", i).into()))
                .unwrap()
                .creation_index
        })
        .collect();

    // All indices must be distinct (the bug was that they all became 0)
    let unique: std::collections::HashSet<usize> = indices.iter().copied().collect();
    assert_eq!(
        unique.len(),
        5,
        "new traces should have distinct creation_index values, got {:?}",
        indices
    );

    // Colors should cycle through the palette (3 distinct colors for 5 traces)
    let colors: Vec<Color32> = indices.iter().map(|&idx| palette[idx % pal_len]).collect();
    let unique_colors: std::collections::HashSet<Color32> = colors.iter().copied().collect();
    assert_eq!(
        unique_colors.len(),
        3,
        "new traces should cycle through palette colors, got {:?}",
        colors
    );
    // No two consecutive new traces should share the same color
    for w in colors.windows(2) {
        assert_ne!(
            w[0], w[1],
            "consecutive traces should not share a color: {:?}",
            colors
        );
    }
}

#[test]
fn alloc_color_uses_global_palette() {
    // start with known palette
    color_scheme::set_global_palette(vec![Color32::from_rgb(1, 2, 3), Color32::from_rgb(4, 5, 6)]);
    assert_eq!(TraceLook::alloc_color(0), Color32::from_rgb(1, 2, 3));
    assert_eq!(TraceLook::alloc_color(1), Color32::from_rgb(4, 5, 6));
    assert_eq!(TraceLook::alloc_color(2), Color32::from_rgb(1, 2, 3));
}

// ── Envelope cache tests ──────────────────────────────────────────────

fn make_trace_with_points(n: usize) -> TraceData {
    let mut td = TraceData::default();
    for i in 0..n {
        let x = i as f64 * 0.001; // 1 kHz, 0..n ms
        let y = (i as f64 * 0.1).sin(); // varying y
        td.live.push_back([x, y]);
    }
    td
}

#[test]
fn envelope_preserves_extremes() {
    let mut td = TraceData::default();
    // 10000 points, each 100-point group has a known min at index 50 and max at index 0
    for i in 0..10_000usize {
        let x = i as f64 * 0.001;
        let group_local = i % 100;
        let y = if group_local == 0 {
            10.0 // max
        } else if group_local == 50 {
            -10.0 // min
        } else {
            0.0
        };
        td.live.push_back([x, y]);
    }
    // data range = 10.0, visible_width=10.0, screen_width=100 → bucket_width=0.1 → 100 buckets
    td.recompute_envelope(100, 10.0);
    let cache = td.envelope_cache.expect("cache should exist");
    assert_eq!(cache.buckets.len(), 100);
    // Each bucket covers 100 points, should have y_min=-10 and y_max=10
    for (i, b) in cache.buckets.iter().enumerate() {
        assert!(b.count > 0, "bucket {} should have points", i);
        assert_eq!(b.y_min, -10.0, "bucket {} should capture min", i);
        assert_eq!(b.y_max, 10.0, "bucket {} should capture max", i);
    }
}

#[test]
fn envelope_degrades_to_line_when_few_points() {
    let mut td = make_trace_with_points(50);
    td.recompute_envelope(800, 1.0);
    // 50 points <= 800 screen_width → no envelope needed
    assert!(td.envelope_cache.is_none());
}

#[test]
fn envelope_full_recompute_on_screen_width_change() {
    let mut td = make_trace_with_points(10_000);
    td.recompute_envelope(800, 1.0);
    let bw1 = td.envelope_cache.as_ref().unwrap().bucket_width;
    let nb1 = td.envelope_cache.as_ref().unwrap().buckets.len();
    td.recompute_envelope(1000, 1.0);
    let bw2 = td.envelope_cache.as_ref().unwrap().bucket_width;
    let nb2 = td.envelope_cache.as_ref().unwrap().buckets.len();
    // bucket_width should change (visible_width same, screen_width different)
    assert_ne!(bw1, bw2);
    // number of buckets should also change
    assert_ne!(nb1, nb2);
}

#[test]
fn envelope_full_recompute_on_visible_width_change() {
    let mut td = make_trace_with_points(10_000);
    td.recompute_envelope(100, 1.0);
    let bw1 = td.envelope_cache.as_ref().unwrap().bucket_width;
    td.recompute_envelope(100, 0.5);
    let bw2 = td.envelope_cache.as_ref().unwrap().bucket_width;
    // bucket_width should halve when visible_width halves
    assert!((bw1 - 2.0 * bw2).abs() < 1e-9);
}

#[test]
fn envelope_incremental_add_updates_single_bucket() {
    let mut td = make_trace_with_points(10_000);
    td.recompute_envelope(100, 10.0);
    let cache = td.envelope_cache.as_ref().unwrap();
    let bucket_count_before = cache.buckets[50].count;

    // Add a point in bucket 50's x-range
    let bucket_50_x_min = cache.buckets[50].x_min;
    let new_point = [bucket_50_x_min + 0.0001, 42.0];
    td.envelope_add_point(new_point);

    let cache = td.envelope_cache.as_ref().unwrap();
    assert_eq!(cache.buckets[50].count, bucket_count_before + 1);
    assert_eq!(cache.buckets[50].y_max, 42.0); // 42 > any sin value
                                               // Other buckets should have unchanged count
    assert_eq!(cache.buckets[51].count, bucket_count_before);
}

#[test]
fn envelope_incremental_add_appends_new_bucket() {
    let mut td = make_trace_with_points(10_000);
    td.recompute_envelope(100, 10.0);
    let n_buckets = td.envelope_cache.as_ref().unwrap().buckets.len();
    // Add a point just beyond the current data range (within rebalance limit)
    let data_max_x = td.live.back().unwrap()[0];
    let bucket_width = td.envelope_cache.as_ref().unwrap().bucket_width;
    let new_point = [data_max_x + bucket_width * 0.5, 5.0];
    td.envelope_add_point(new_point);
    let cache = td.envelope_cache.as_ref().unwrap();
    assert!(
        cache.buckets.len() > n_buckets,
        "new bucket should be appended"
    );
}

#[test]
fn envelope_incremental_remove_no_recompute_when_not_extreme() {
    let mut td = TraceData::default();
    // Fill with points where y=0 except first and last in each bucket
    for i in 0..10_000usize {
        let x = i as f64 * 0.001;
        let y = if i % 100 == 0 { 5.0 } else { 0.0 };
        td.live.push_back([x, y]);
    }
    td.recompute_envelope(100, 10.0);
    // Remove a point with y=0 (not min or max of its bucket)
    let removed = td.live[500]; // y=0, not extreme
    td.envelope_remove_point(removed);
    let cache = td.envelope_cache.as_ref().unwrap();
    // Bucket min/max should be unchanged (0 is not min or max)
    let bucket_idx = cache.bucket_index(removed[0]).unwrap();
    assert_eq!(cache.buckets[bucket_idx].y_min, 0.0);
    assert_eq!(cache.buckets[bucket_idx].y_max, 5.0);
}

#[test]
fn envelope_incremental_remove_recomputes_when_extreme() {
    let mut td = TraceData::default();
    for i in 0..10_000usize {
        let x = i as f64 * 0.001;
        let y = if i % 100 == 0 { 5.0 } else { 0.0 };
        td.live.push_back([x, y]);
    }
    td.recompute_envelope(100, 10.0);
    // Remove a point with y=5.0 (the max of its bucket)
    let removed = td.live[0]; // y=5.0, the max
    td.live.pop_front();
    td.envelope_remove_point(removed);
    let cache = td.envelope_cache.as_ref().unwrap();
    let bucket_idx = cache.bucket_index(removed[0]).unwrap();
    // After removing the max (y=5.0), the new max should be 0.0
    assert_eq!(cache.buckets[bucket_idx].y_max, 0.0);
}

#[test]
fn envelope_scroll_no_recompute() {
    let mut td = make_trace_with_points(10_000);
    td.recompute_envelope(100, 10.0);
    // Simulate scrolling: same screen_width and visible_width, bounds within cached range
    let needs_recompute = td.envelope_needs_recompute(100, 10.0, (2.0, 5.0));
    assert!(!needs_recompute, "scrolling should not trigger recompute");
}

#[test]
fn envelope_buckets_cover_all_data() {
    let mut td = make_trace_with_points(10_000);
    td.recompute_envelope(100, 10.0);
    let cache = td.envelope_cache.as_ref().unwrap();
    let data_min_x = td.live.front().unwrap()[0];
    let data_max_x = td.live.back().unwrap()[0];
    // First bucket should start at data_min_x
    assert!((cache.buckets[0].x_min - data_min_x).abs() < 1e-9);
    // Last bucket should cover data_max_x
    let last = cache.buckets.back().unwrap();
    assert!(last.x_max >= data_max_x || (last.x_min - data_max_x).abs() < cache.bucket_width);
}

// ── Density cache tests ───────────────────────────────────────────────

#[test]
fn density_recompute_creates_grid() {
    let mut td = make_trace_with_points(10_000);
    td.recompute_density(100, 10.0);
    let cache = td.density_cache.expect("density cache should exist");
    assert!(cache.num_x_buckets > 0);
    assert_eq!(cache.num_y_buckets, 200);
    assert!(cache.max_count > 0);
    // Total counts should equal number of points
    let total: u32 = cache
        .cells
        .iter()
        .flat_map(|c| c.counts.iter().map(|&v| v as u32))
        .sum();
    assert_eq!(total, 10_000);
}

#[test]
fn density_incremental_add_increments_cell() {
    let mut td = make_trace_with_points(10_000);
    td.recompute_density(100, 10.0);
    let cache = td.density_cache.as_ref().unwrap();
    let xi = cache.num_x_buckets / 2;
    let yi = cache.num_y_buckets / 2;
    let x = cache.origin_x + xi as f64 * cache.bucket_width + 1e-6;
    let y = cache.origin_y + yi as f64 * cache.bucket_height + 1e-6;
    let count_before = cache.cells[xi].counts[yi];
    td.density_add_point([x, y]);
    let cache = td.density_cache.as_ref().unwrap();
    assert_eq!(cache.cells[xi].counts[yi], count_before + 1);
}

#[test]
fn density_incremental_remove_decrements_cell() {
    let mut td = make_trace_with_points(10_000);
    td.recompute_density(100, 10.0);
    let cache = td.density_cache.as_ref().unwrap();
    // Find a non-empty cell
    let (xi, yi, count_before) = cache
        .cells
        .iter()
        .enumerate()
        .flat_map(|(xi, col)| {
            col.counts
                .iter()
                .enumerate()
                .map(move |(yi, &c)| (xi, yi, c))
        })
        .find(|(_, _, c)| *c > 0)
        .expect("at least one non-empty cell");
    let x = cache.origin_x + xi as f64 * cache.bucket_width + 1e-6;
    let y = cache.origin_y + yi as f64 * cache.bucket_height + 1e-6;
    td.density_remove_point([x, y]);
    let cache = td.density_cache.as_ref().unwrap();
    assert_eq!(cache.cells[xi].counts[yi], count_before - 1);
}

#[test]
fn density_add_outside_y_range_triggers_recompute() {
    let mut td = make_trace_with_points(10_000);
    td.recompute_density(100, 10.0);
    // Add a point with y far outside the current range
    let x = td.live.back().unwrap()[0] - 0.1;
    td.density_add_point([x, 1e6]);
    // Cache should be cleared (needs recompute)
    assert!(td.density_cache.is_none());
}

#[test]
fn density_scroll_no_recompute() {
    let mut td = make_trace_with_points(10_000);
    td.recompute_density(100, 10.0);
    let needs = td.density_needs_recompute(100, 10.0);
    assert!(!needs, "scrolling should not trigger density recompute");
}

#[test]
fn density_recompute_on_width_change() {
    let mut td = make_trace_with_points(10_000);
    td.recompute_density(100, 10.0);
    let bw1 = td.density_cache.as_ref().unwrap().bucket_width;
    td.recompute_density(200, 10.0);
    let bw2 = td.density_cache.as_ref().unwrap().bucket_width;
    assert_ne!(bw1, bw2);
}
