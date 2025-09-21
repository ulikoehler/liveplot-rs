use liveplot::{channel_multi, run_multi};
use std::fs::OpenOptions;
use std::io::{Read, Seek, SeekFrom};
use std::path::PathBuf;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

// Simple CSV tailer and visualizer.
//
// CSV format expected:
//   header: index,timestamp_micros,<trace1>,<trace2>,...
//   data:   <u64>,<i64>,<f64>,<f64>,...
//
// - Lines without a full set of columns are ignored.
// - Empty lines are ignored.
// - Non-numeric fields in data lines cause that trace sample to be skipped.
// - If timestamp parsing fails, current time is used.
//
// Usage:
//   cargo run --example csv_tail -- [--from-start] [path/to/live_data.csv]
//
// By default it starts reading at the end (like `tail -f`).
// Pass --from-start to read existing data first.
//
// See examples/csv_writer.py for a companion 1 kHz CSV writer.

fn main() -> eframe::Result<()> {
    // Parse simple CLI args: optional --from-start and optional path
    let mut from_start = false;
    let mut csv_path: Option<PathBuf> = None;
    for arg in std::env::args().skip(1) {
        if arg == "--from-start" {
            from_start = true;
        } else if csv_path.is_none() {
            csv_path = Some(PathBuf::from(arg));
        }
    }
    let csv_path = csv_path.unwrap_or_else(|| PathBuf::from("live_data.csv"));

    eprintln!("[csv_tail] Monitoring {:?} (from_start={})", csv_path, from_start);

    let (sink, rx) = channel_multi();

    // Reader thread: poll file every 20 ms, read any newly appended bytes,
    // parse complete lines, and send samples to the plot sink.
    std::thread::spawn(move || {
        // Wait until file exists
        loop {
            if csv_path.exists() {
                break;
            }
            std::thread::sleep(Duration::from_millis(100));
        }

        let mut file = loop {
            match OpenOptions::new().read(true).open(&csv_path) {
                Ok(f) => break f,
                Err(e) => {
                    eprintln!("[csv_tail] Failed to open file: {}. Retrying...", e);
                    std::thread::sleep(Duration::from_millis(200));
                }
            }
        };

        // Position: end by default (tail) or start if requested
        let mut pos: u64 = if from_start {
            0
        } else {
            match file.metadata() { Ok(m) => m.len(), Err(_) => 0 }
        };

        // Accumulator for partial last line across polls
        let mut carry = String::new();
        // Header-derived trace names (columns after index + timestamp)
        let mut trace_names: Option<Vec<String>> = None;

        const POLL_MS: u64 = 20; // 50 Hz updates

        loop {
            // Handle rotations/truncations
            let len = match file.metadata() { Ok(m) => m.len(), Err(_) => 0 };
            if len < pos {
                // Truncated (e.g., recreated). Reset and try to re-open to refresh inode.
                eprintln!("[csv_tail] Detected truncation. Reopening...");
                if let Ok(f) = OpenOptions::new().read(true).open(&csv_path) { file = f; }
                pos = 0;
            }

            // Read any newly appended bytes without blocking
            if len > pos {
                let to_read = (len - pos) as usize;
                let mut buf = vec![0u8; to_read];
                if file.seek(SeekFrom::Start(pos)).is_ok() {
                    match file.read(&mut buf) {
                        Ok(n) if n > 0 => {
                            pos += n as u64;
                            let s = String::from_utf8_lossy(&buf[..n]);
                            carry.push_str(&s);
                        }
                        Ok(_) => {}
                        Err(e) => {
                            eprintln!("[csv_tail] Read error: {}", e);
                        }
                    }
                }
            }

            // Process complete lines. Keep last partial line in `carry`.
            // We will split by '\n' and reassemble the trailing partial if needed.
            if !carry.is_empty() {
                // Move out the buffered content to avoid borrowing while mutating `carry`
                let chunk = std::mem::take(&mut carry);
                let last_was_newline = chunk.ends_with('\n');
                let parts: Vec<&str> = chunk.split('\n').collect();
                if last_was_newline {
                    // All lines are complete; process all
                    for line in parts.into_iter() {
                        process_line(line, &mut trace_names, &sink);
                    }
                    // `carry` remains empty
                } else if !parts.is_empty() {
                    // Last element is partial; keep it in `carry`
                    for line in parts[..parts.len() - 1].iter().copied() {
                        process_line(line, &mut trace_names, &sink);
                    }
                    carry.push_str(parts[parts.len() - 1]);
                }
            }

            std::thread::sleep(Duration::from_millis(POLL_MS));
        }
    });

    run_multi(rx)
}

fn process_line(line: &str, trace_names: &mut Option<Vec<String>>, sink: &liveplot::sink::MultiPlotSink) {
    let line = line.trim();
    if line.is_empty() { return; }

    // Header? Expect at least 3 columns and non-numeric first cell
    if trace_names.is_none() {
        let cols: Vec<&str> = line.split(',').map(|s| s.trim()).collect();
        if cols.len() >= 3 {
            // Accept either explicit names or anything; header if first two are non-numeric words
            let first_is_num = cols[0].parse::<u64>().is_ok();
            let second_is_num = cols[1].parse::<i64>().is_ok();
            if !first_is_num || !second_is_num {
                let names: Vec<String> = cols[2..].iter().map(|s| s.to_string()).collect();
                if !names.is_empty() {
                    *trace_names = Some(names);
                    return; // header consumed
                }
            }
        }
        // If not header, we'll try to parse as data below.
    }

    // Data line
    let cols: Vec<&str> = line.split(',').map(|s| s.trim()).collect();
    if cols.len() < 3 { return; } // incomplete

    let idx = match cols[0].parse::<u64>() { Ok(v) => v, Err(_) => return };
    let ts = match cols[1].parse::<i64>() {
        Ok(v) => v,
        Err(_) => SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_micros() as i64).unwrap_or(0),
    };

    // Determine trace names: if not set, synthesize generic names based on column index
    let names: Vec<String> = match trace_names {
        Some(v) => v.clone(),
        None => (2..cols.len()).map(|i| format!("col{}", i-1)).collect(),
    };

    let value_cols = cols.len() - 2;
    let n_traces = names.len().min(value_cols);
    for i in 0..n_traces {
        if let Ok(val) = cols[2 + i].parse::<f64>() {
            let _ = sink.send_value(idx, val, ts, names[i].as_str());
        }
    }
}
