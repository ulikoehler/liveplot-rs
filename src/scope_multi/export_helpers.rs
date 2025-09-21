use std::collections::HashMap;

use crate::controllers::RawExportFormat;
use crate::export;

use super::types::TraceState;

/// Save all traces to path in the chosen format. If paused and snapshots exist, export snapshots; otherwise export live buffers.
pub(super) fn save_raw_data_to_path(
    fmt: RawExportFormat,
    path: &std::path::Path,
    paused: bool,
    traces: &std::collections::HashMap<String, TraceState>,
    trace_order: &Vec<String>,
) -> Result<(), Box<dyn std::error::Error>> {
    match fmt {
        RawExportFormat::Csv => save_as_csv(path, paused, traces, trace_order),
        RawExportFormat::Parquet => save_as_parquet(path, paused, traces, trace_order),
    }
}

fn save_as_csv(
    path: &std::path::Path,
    paused: bool,
    traces: &std::collections::HashMap<String, TraceState>,
    trace_order: &Vec<String>,
) -> Result<(), Box<dyn std::error::Error>> {
    // Build series map of the currently exported buffers (paused => snapshot if present)
    let mut series: HashMap<String, Vec<[f64;2]>> = HashMap::new();
    for name in trace_order.iter() {
        if let Some(tr) = traces.get(name) {
            let iter: Box<dyn Iterator<Item=&[f64;2]> + '_> = if paused { if let Some(snap) = &tr.snap { Box::new(snap.iter()) } else { Box::new(tr.live.iter()) } } else { Box::new(tr.live.iter()) };
            let vec: Vec<[f64;2]> = iter.cloned().collect();
            series.insert(name.clone(), vec);
        }
    }
    // Tolerance fixed to 1e-9 seconds
    export::write_csv_aligned_path(path, trace_order, &series, 1e-9)?;
    Ok(())
}

fn save_as_parquet(
    path: &std::path::Path,
    paused: bool,
    traces: &std::collections::HashMap<String, TraceState>,
    trace_order: &Vec<String>,
) -> Result<(), Box<dyn std::error::Error>> {
    #[cfg(feature = "parquet")]
    {
        // Build series map of the currently exported buffers (paused => snapshot if present)
        let mut series: HashMap<String, Vec<[f64;2]>> = HashMap::new();
        for name in trace_order.iter() {
            if let Some(tr) = traces.get(name) {
                let iter: Box<dyn Iterator<Item=&[f64;2]> + '_> = if paused { if let Some(snap) = &tr.snap { Box::new(snap.iter()) } else { Box::new(tr.live.iter()) } } else { Box::new(tr.live.iter()) };
                let vec: Vec<[f64;2]> = iter.cloned().collect();
                series.insert(name.clone(), vec);
            }
        }
        export::write_parquet_aligned_path(path, trace_order, &series, 1e-9)?;
        return Ok(());
    }
    #[cfg(not(feature = "parquet"))]
    {
        let _ = (path, paused, traces, trace_order);
        Err("Parquet export not available: build with feature `parquet`".into())
    }
}

/// Save a list of threshold events to a CSV file with columns:
/// end_time_seconds,threshold,trace,start_time_seconds,duration_seconds,area
pub(super) fn save_threshold_events_csv(path: &std::path::Path, events: &[&crate::thresholds::ThresholdEvent]) -> Result<(), Box<dyn std::error::Error>> {
    use std::io::Write;
    let mut f = std::fs::File::create(path)?;
    writeln!(f, "end_time_seconds,threshold,trace,start_time_seconds,duration_seconds,area")?;
    for e in events {
        writeln!(f, "{:.9},{},{},{:.9},{:.9},{:.9}", e.end_t, e.threshold, e.trace, e.start_t, e.duration, e.area)?;
    }
    Ok(())
}
