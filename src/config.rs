use chrono::Local;

/// Formatting options for the x-value (time) shown in point labels.
#[derive(Debug, Clone, Copy)]
pub enum XDateFormat {
    /// Local time with date, ISO8601-like: YYYY-MM-DD HH:MM:SS
    Iso8601WithDate,
    /// Local time, time-of-day only: HH:MM:SS
    Iso8601Time,
}

impl Default for XDateFormat {
    fn default() -> Self {
        XDateFormat::Iso8601Time
    }
}

impl XDateFormat {
    /// Format an `x` value (seconds since UNIX epoch as f64) according to the selected format.
    pub fn format_value(&self, x_seconds: f64) -> String {
        let secs = x_seconds as i64;
        let nsecs = ((x_seconds - secs as f64) * 1e9) as u32;
        let dt_utc = chrono::DateTime::from_timestamp(secs, nsecs)
            .unwrap_or_else(|| chrono::DateTime::from_timestamp(0, 0).unwrap());
        match self {
            XDateFormat::Iso8601WithDate => dt_utc
                .with_timezone(&Local)
                .format("%Y-%m-%d %H:%M:%S")
                .to_string(),
            XDateFormat::Iso8601Time => dt_utc.with_timezone(&Local).format("%H:%M:%S").to_string(),
        }
    }
}
