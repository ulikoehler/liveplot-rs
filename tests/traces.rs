use liveplot::color_scheme;
use liveplot::data::trace_look::TraceLook;
use liveplot::data::traces::{TraceData, TraceRef, TracesCollection};
use liveplot::sink::PlotCommand;
use egui::Color32;

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
    color_scheme::set_global_palette(vec![
        Color32::from_rgb(9, 9, 9),
        Color32::from_rgb(8, 8, 8),
    ]);
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
    color_scheme::set_global_palette(vec![
        Color32::from_rgb(1, 2, 3),
        Color32::from_rgb(4, 5, 6),
    ]);
    assert_eq!(TraceLook::alloc_color(0), Color32::from_rgb(1, 2, 3));
    assert_eq!(TraceLook::alloc_color(1), Color32::from_rgb(4, 5, 6));
    assert_eq!(TraceLook::alloc_color(2), Color32::from_rgb(1, 2, 3));
}
