use chrono::Local;

#[derive(Debug, Clone, Copy)]
pub enum XDateFormat {
    Iso8601WithDate,
    Iso8601Time,
}

impl Default for XDateFormat {
    fn default() -> Self { XDateFormat::Iso8601Time }
}

impl XDateFormat {
    pub fn format_value(&self, x_seconds: f64) -> String {
        let secs = x_seconds as i64;
        let nsecs = ((x_seconds - secs as f64) * 1e9) as u32;
        let dt_utc = chrono::DateTime::from_timestamp(secs, nsecs)
            .unwrap_or_else(|| chrono::DateTime::from_timestamp(0, 0).unwrap());
        match self {
            XDateFormat::Iso8601WithDate => dt_utc.with_timezone(&Local).format("%Y-%m-%d %H:%M:%S").to_string(),
            XDateFormat::Iso8601Time => dt_utc.with_timezone(&Local).format("%H:%M:%S").to_string(),
        }
    }
}

pub struct LivePlotConfig {
    pub time_window_secs: f64,
    pub max_points: usize,
    pub x_date_format: XDateFormat,
    pub y_unit: Option<String>,
    pub y_log: bool,
    pub title: Option<String>,
    pub native_options: Option<eframe::NativeOptions>,
}

impl Default for LivePlotConfig {
    fn default() -> Self {
        Self {
            time_window_secs: 10.0,
            max_points: 10_000,
            x_date_format: XDateFormat::default(),
            y_unit: None,
            y_log: false,
            title: None,
            native_options: None,
        }
    }
}
