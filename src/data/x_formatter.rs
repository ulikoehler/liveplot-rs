//! X-axis value formatters: decimal, scientific, and intelligent time formatting.
//!
//! The main entry point for users is [`XFormatter`], which can be set on an axis to
//! control how tick labels and cursor readouts are rendered. The default (`Auto`) picks
//! a sensible formatter based on the axis type.

use chrono::{Datelike, TimeZone, Timelike};

// ─────────────────────────────────────────────────────────────────────────────
// EpochUnit
// ─────────────────────────────────────────────────────────────────────────────

/// The unit in which raw X values are expressed when using [`TimeFormatter`].
///
/// All values are stored as `f64` in the data pipeline; this enum tells the
/// formatter how to interpret them (what "1 unit" means in wall-clock terms).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EpochUnit {
    /// Values are seconds since the UNIX epoch (e.g. `1_700_000_000.0`).
    Seconds,
    /// Values are milliseconds since the UNIX epoch (e.g. `1_700_000_000_000.0`).
    Milliseconds,
    /// Values are microseconds since the UNIX epoch.
    Microseconds,
    /// Values are nanoseconds since the UNIX epoch.
    Nanoseconds,
}

impl EpochUnit {
    /// How many of this unit make up one second.
    ///
    /// ```
    /// # use liveplot::data::x_formatter::EpochUnit;
    /// assert_eq!(EpochUnit::Milliseconds.units_per_second(), 1_000.0);
    /// assert_eq!(EpochUnit::Nanoseconds.units_per_second(), 1_000_000_000.0);
    /// ```
    pub fn units_per_second(&self) -> f64 {
        match self {
            EpochUnit::Seconds => 1.0,
            EpochUnit::Milliseconds => 1_000.0,
            EpochUnit::Microseconds => 1_000_000.0,
            EpochUnit::Nanoseconds => 1_000_000_000.0,
        }
    }

    /// Convert a value expressed in this epoch unit to seconds.
    ///
    /// ```
    /// # use liveplot::data::x_formatter::EpochUnit;
    /// assert!((EpochUnit::Milliseconds.to_seconds(3_000.0) - 3.0).abs() < 1e-12);
    /// ```
    pub fn to_seconds(&self, value: f64) -> f64 {
        value / self.units_per_second()
    }
}

impl Default for EpochUnit {
    fn default() -> Self {
        EpochUnit::Seconds
    }
}

impl std::fmt::Display for EpochUnit {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EpochUnit::Seconds => write!(f, "s"),
            EpochUnit::Milliseconds => write!(f, "ms"),
            EpochUnit::Microseconds => write!(f, "µs"),
            EpochUnit::Nanoseconds => write!(f, "ns"),
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// TimeResolution
// ─────────────────────────────────────────────────────────────────────────────

/// Granularity of the sub-second portion shown in a time label.
///
/// Variants are ordered from coarsest (`Seconds`) to finest (`Nanoseconds`).
/// This ordering is used to enforce `min_resolution` / `max_resolution` bounds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum TimeResolution {
    /// No sub-second digits: `HH:MM:SS`.
    Seconds,
    /// Three decimal digits: `HH:MM:SS.mmm`.
    Milliseconds,
    /// Six decimal digits: `HH:MM:SS.mmmuuu`.
    Microseconds,
    /// Nine decimal digits: `HH:MM:SS.mmmuuunnn`.
    Nanoseconds,
}

impl std::fmt::Display for TimeResolution {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TimeResolution::Seconds => write!(f, "Seconds"),
            TimeResolution::Milliseconds => write!(f, "Milliseconds"),
            TimeResolution::Microseconds => write!(f, "Microseconds"),
            TimeResolution::Nanoseconds => write!(f, "Nanoseconds"),
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// TimeFormatter
// ─────────────────────────────────────────────────────────────────────────────

/// Intelligent timestamp formatter for X-axis tick labels and cursor readouts.
///
/// # Format
/// The output follows ISO 8601 but with `T` replaced by a space and timezone
/// designators removed, e.g. `2024-01-15 13:45:30.000`.
///
/// # Adaptive behaviour (driven by the visible X range)
/// * **Date hiding** – the date portion (`MM-DD` or `YYYY-MM-DD`) is hidden
///   unless the visible X range crosses a calendar-date boundary, or
///   [`force_date_visible`](Self::force_date_visible) is `true`.
/// * **Year hiding** – the year is hidden unless the range crosses a year
///   boundary, or [`force_show_year`](Self::force_show_year) is `true`.
///   (The year is only ever shown when the date is already shown.)
/// * **Milliseconds** – shown if the visible range is below
///   [`milliseconds_threshold`](Self::milliseconds_threshold) (default 3 600 s = 1 hour).
/// * **Microseconds** – shown if the visible range is below
///   [`microseconds_threshold`](Self::microseconds_threshold) (default 60 s = 1 minute)
///   *and* the available pixel width is sufficient.
/// * **Nanoseconds** – shown if the visible range is below
///   [`nanoseconds_threshold`](Self::nanoseconds_threshold) (default 1 s)
///   *and* the available pixel width is sufficient.
/// * [`min_resolution`](Self::min_resolution) and [`max_resolution`](Self::max_resolution)
///   act as hard floor / ceiling on the granularity shown, overriding the above thresholds.
#[derive(Debug, Clone)]
pub struct TimeFormatter {
    /// Unit of the raw X values passed to [`format`](Self::format).
    pub epoch_unit: EpochUnit,

    /// Always show the date part (`MM-DD` / `YYYY-MM-DD`), even when the
    /// visible range fits within a single calendar day.
    pub force_date_visible: bool,

    /// Always show the four-digit year when the date is shown.
    pub force_show_year: bool,

    /// Visible range (in **seconds**) below which milliseconds are shown.
    ///
    /// Default: `3_600.0` (1 hour).
    pub milliseconds_threshold: f64,

    /// Visible range (in **seconds**) below which microseconds are shown.
    ///
    /// Default: `60.0` (1 minute).
    pub microseconds_threshold: f64,

    /// Visible range (in **seconds**) below which nanoseconds are shown.
    ///
    /// Default: `1.0` (1 second).
    pub nanoseconds_threshold: f64,

    /// Hard floor on granularity: never produce output *coarser* than this
    /// resolution, even if the thresholds would hide sub-second digits.
    ///
    /// Example: `TimeResolution::Milliseconds` always shows `.mmm`.
    /// Default: [`TimeResolution::Seconds`].
    pub min_resolution: TimeResolution,

    /// Hard ceiling on granularity: never produce output *finer* than this
    /// resolution, even if the visible range is tiny.
    ///
    /// Example: `TimeResolution::Milliseconds` never shows µs or ns.
    /// Default: [`TimeResolution::Nanoseconds`].
    pub max_resolution: TimeResolution,

    /// If set, pixel width available for the label. When the window is too
    /// narrow (< 120 px by default), sub-millisecond digits are suppressed.
    pub available_width_pixels: Option<f32>,
}

impl Default for TimeFormatter {
    fn default() -> Self {
        Self {
            epoch_unit: EpochUnit::Seconds,
            force_date_visible: false,
            force_show_year: false,
            milliseconds_threshold: 3_600.0,
            microseconds_threshold: 60.0,
            nanoseconds_threshold: 1.0,
            min_resolution: TimeResolution::Seconds,
            max_resolution: TimeResolution::Nanoseconds,
            available_width_pixels: None,
        }
    }
}

impl TimeFormatter {
    /// Create a `TimeFormatter` pre-configured for the given epoch unit,
    /// with all other settings at their defaults.
    pub fn for_epoch_unit(epoch_unit: EpochUnit) -> Self {
        Self {
            epoch_unit,
            ..Self::default()
        }
    }

    /// Format a single X value (`value_raw`, expressed in [`epoch_unit`](Self::epoch_unit))
    /// given the visible X range `x_range_raw` expressed in the same unit.
    ///
    /// The visible range is used to:
    /// * detect date / year changes (for adaptive date display), and
    /// * select the appropriate sub-second precision level.
    ///
    /// # Panics
    /// Does not panic; out-of-range timestamps fall back to the UNIX epoch.
    pub fn format(&self, value_raw: f64, x_range_raw: (f64, f64)) -> String {
        let ups = self.epoch_unit.units_per_second();

        let value_secs = value_raw / ups;
        let range_start_secs = x_range_raw.0 / ups;
        let range_end_secs = x_range_raw.1 / ups;

        // Ensure range_start <= range_end for span calculations
        let (range_lo_secs, range_hi_secs) = if range_start_secs <= range_end_secs {
            (range_start_secs, range_end_secs)
        } else {
            (range_end_secs, range_start_secs)
        };
        let range_span_secs = range_hi_secs - range_lo_secs;

        let start_dt = secs_to_local(range_lo_secs);
        let end_dt = secs_to_local(range_hi_secs);
        let value_dt = secs_to_local(value_secs);

        // ── Date / year visibility ───────────────────────────────────────────
        let date_changes = start_dt.date_naive() != end_dt.date_naive();
        let year_changes = start_dt.year() != end_dt.year();

        let show_date = date_changes || self.force_date_visible;
        let show_year = show_date && (year_changes || self.force_show_year);

        // ── Determine sub-second resolution ──────────────────────────────────
        let resolution = self.determine_resolution(range_span_secs);

        // ── Build the base time string (date part + HH:MM:SS) ────────────────
        let base = if show_date {
            if show_year {
                value_dt.format("%Y-%m-%d %H:%M:%S").to_string()
            } else {
                value_dt.format("%m-%d %H:%M:%S").to_string()
            }
        } else {
            value_dt.format("%H:%M:%S").to_string()
        };

        // ── Append fractional seconds ─────────────────────────────────────────
        match resolution {
            TimeResolution::Seconds => base,
            TimeResolution::Milliseconds => {
                let ms = value_dt.nanosecond() / 1_000_000;
                format!("{}.{:03}", base, ms)
            }
            TimeResolution::Microseconds => {
                let us = value_dt.nanosecond() / 1_000;
                format!("{}.{:06}", base, us)
            }
            TimeResolution::Nanoseconds => {
                let ns = value_dt.nanosecond();
                format!("{}.{:09}", base, ns)
            }
        }
    }

    /// Select the appropriate [`TimeResolution`] for the given visible span.
    ///
    /// The result is clamped to `[min_resolution, max_resolution]`.
    pub fn determine_resolution(&self, range_span_secs: f64) -> TimeResolution {
        // Start at the coarsest allowed level and increase granularity based on thresholds.
        let mut res = self.min_resolution;

        if TimeResolution::Milliseconds > self.min_resolution
            && TimeResolution::Milliseconds <= self.max_resolution
            && range_span_secs < self.milliseconds_threshold
        {
            res = TimeResolution::Milliseconds;
        }

        if TimeResolution::Microseconds <= self.max_resolution
            && range_span_secs < self.microseconds_threshold
            && !self.too_narrow_for_sub_millisecond()
        {
            res = TimeResolution::Microseconds;
        }

        if TimeResolution::Nanoseconds <= self.max_resolution
            && range_span_secs < self.nanoseconds_threshold
            && !self.too_narrow_for_sub_millisecond()
        {
            res = TimeResolution::Nanoseconds;
        }

        // Clamp to [min_resolution, max_resolution]
        res.max(self.min_resolution).min(self.max_resolution)
    }

    /// Returns `true` when `available_width_pixels` is set and below the
    /// threshold (120 px) that makes sub-millisecond labels impractical.
    fn too_narrow_for_sub_millisecond(&self) -> bool {
        self.available_width_pixels
            .map(|w| w < 120.0)
            .unwrap_or(false)
    }
}

/// Convert seconds-since-epoch (as `f64`) to [`chrono::DateTime<chrono::Local>`].
/// Clamped to valid range; values outside fall back to the UNIX epoch.
fn secs_to_local(secs: f64) -> chrono::DateTime<chrono::Local> {
    if !secs.is_finite() {
        return chrono::Local
            .timestamp_opt(0, 0)
            .single()
            .unwrap_or_default();
    }
    let s = secs.floor() as i64;
    let ns_frac = ((secs - s as f64) * 1e9).round() as u32;
    let ns_frac = ns_frac.min(999_999_999);
    chrono::DateTime::from_timestamp(s, ns_frac)
        .unwrap_or_else(|| chrono::DateTime::from_timestamp(0, 0).unwrap())
        .with_timezone(&chrono::Local)
}

// ─────────────────────────────────────────────────────────────────────────────
// DecimalFormatter
// ─────────────────────────────────────────────────────────────────────────────

/// A plain decimal number formatter with optional fixed decimal places.
///
/// If `decimal_places` is `None`, the number of places is taken from the
/// `dec_pl` argument passed at format time (matches legacy behaviour).
#[derive(Debug, Clone, PartialEq)]
pub struct DecimalFormatter {
    /// Fixed number of decimal places to render, or `None` to use the
    /// caller-supplied `dec_pl` value.
    pub decimal_places: Option<usize>,
    /// Optional unit suffix appended after the number (e.g. `"V"`).
    pub unit: Option<String>,
}

impl Default for DecimalFormatter {
    fn default() -> Self {
        Self {
            decimal_places: None,
            unit: None,
        }
    }
}

impl DecimalFormatter {
    /// Format `value` with the given fallback decimal-place count.
    pub fn format(&self, value: f64, dec_pl: usize) -> String {
        let places = self.decimal_places.unwrap_or(dec_pl);
        let s = format!("{:.*}", places, value);
        match &self.unit {
            Some(u) => format!("{} {}", s, u),
            None => s,
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// ScientificFormatter
// ─────────────────────────────────────────────────────────────────────────────

/// A scientific-notation formatter: renders values as `1.23e4` style.
///
/// Avoids leading `+` signs and zero-padded exponents for compactness.
#[derive(Debug, Clone, PartialEq)]
pub struct ScientificFormatter {
    /// Number of digits after the decimal point in the mantissa.
    ///
    /// If `None`, falls back to the caller-supplied `dec_pl`.
    pub significant_digits: Option<usize>,
    /// Optional unit suffix.
    pub unit: Option<String>,
}

impl Default for ScientificFormatter {
    fn default() -> Self {
        Self {
            significant_digits: None,
            unit: None,
        }
    }
}

impl ScientificFormatter {
    /// Format `value` in scientific notation.
    pub fn format(&self, value: f64, dec_pl: usize) -> String {
        let digits = self.significant_digits.unwrap_or(dec_pl);
        let formatted = format_scientific(value, digits);
        match &self.unit {
            Some(u) => format!("{} {}", formatted, u),
            None => formatted,
        }
    }
}

/// Render `value` as compact scientific notation like `1.23e5` or `-4.00e-2`.
/// Returns `"0"` for zero and handles `NaN` / `±inf` gracefully.
fn format_scientific(value: f64, digits: usize) -> String {
    if value == 0.0 {
        return format!("{:.*}", digits, 0.0_f64);
    }
    if !value.is_finite() {
        return format!("{}", value);
    }
    let sign = if value < 0.0 { -1.0 } else { 1.0 };
    let abs_val = value.abs();
    let exp = abs_val.log10().floor() as i32;
    let mantissa = sign * abs_val / 10f64.powi(exp);
    if exp == 0 {
        format!("{:.*}", digits, mantissa)
    } else {
        format!("{:.*}e{}", digits, mantissa, exp)
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// XFormatter  (the main enum exported to users)
// ─────────────────────────────────────────────────────────────────────────────

/// Selects how X-axis values are formatted (tick labels, cursor readouts, etc.).
///
/// Set this on [`crate::data::scope::AxisSettings::x_formatter`].
///
/// # Auto-selection rules
/// * For **time axes** (`AxisType::Time`): uses [`TimeFormatter`] with default settings.
/// * For **value axes** (`AxisType::Value`): uses [`DecimalFormatter`] with default settings.
#[derive(Debug, Clone)]
pub enum XFormatter {
    /// Automatically pick the best formatter based on the axis type.
    ///
    /// * Time axis → [`TimeFormatter::default()`] (EpochUnit: Seconds)
    /// * Value axis → [`DecimalFormatter::default()`]
    Auto,

    /// Fixed decimal notation: `123.456`.
    Decimal(DecimalFormatter),

    /// Scientific notation: `1.23e5`.
    Scientific(ScientificFormatter),

    /// Intelligent timestamp formatter (see [`TimeFormatter`]).
    Time(Box<TimeFormatter>),
}

impl Default for XFormatter {
    fn default() -> Self {
        XFormatter::Auto
    }
}

impl XFormatter {
    /// Convenience constructor for a `Time` variant.
    pub fn time(tf: TimeFormatter) -> Self {
        XFormatter::Time(Box::new(tf))
    }

    /// Convenience constructor for `Auto`.
    pub fn auto() -> Self {
        XFormatter::Auto
    }

    /// Return `true` if this formatter is axis-type-agnostic (i.e. `Auto`).
    pub fn is_auto(&self) -> bool {
        matches!(self, XFormatter::Auto)
    }

    /// Format a value for a **time axis** (values in seconds since epoch).
    ///
    /// `x_bounds` is the full visible range `(x_min, x_max)` in the **same**
    /// unit as `value` (seconds for the built-in time axis).
    /// `dec_pl` is the fallback decimal-place count used by non-time formatters.
    pub fn format_time_value(
        &self,
        value: f64,
        x_bounds: (f64, f64),
        dec_pl: usize,
        _step: f64,
    ) -> String {
        match self {
            XFormatter::Auto | XFormatter::Time(_) => {
                let tf = match self {
                    XFormatter::Time(tf) => tf.as_ref(),
                    _ => return Self::default_time_format(value, x_bounds),
                };
                tf.format(value, x_bounds)
            }
            XFormatter::Decimal(df) => df.format(value, dec_pl),
            XFormatter::Scientific(sf) => sf.format(value, dec_pl),
        }
    }

    /// Format a value for a **value (non-time) axis**.
    ///
    /// `step` is the tick or view step used for auto-scientific decisions.
    pub fn format_numeric_value(&self, value: f64, dec_pl: usize, step: f64) -> String {
        match self {
            XFormatter::Auto | XFormatter::Decimal(_) => {
                let df = match self {
                    XFormatter::Decimal(df) => return df.format(value, dec_pl),
                    _ => &DecimalFormatter::default(),
                };
                // Auto numeric: use the same adaptive logic as the existing code.
                format_adaptive_numeric(value, dec_pl, step, df.unit.as_deref())
            }
            XFormatter::Scientific(sf) => sf.format(value, dec_pl),
            XFormatter::Time(tf) => {
                // Unusual: time formatter on a non-time axis. Format using empty range.
                tf.format(value, (value, value))
            }
        }
    }

    fn default_time_format(value: f64, x_bounds: (f64, f64)) -> String {
        let tf = TimeFormatter::default();
        tf.format(value, x_bounds)
    }
}

/// Adaptive numeric formatter matching the legacy `format_value_numeric` logic.
fn format_adaptive_numeric(v: f64, dec_pl: usize, step: f64, unit: Option<&str>) -> String {
    let sci = if step.is_finite() && step != 0.0 {
        let exp = step.abs().log10().floor() as i32;
        exp < -(dec_pl as i32) || exp >= dec_pl as i32
    } else {
        false
    };

    let formatted = if sci {
        format_scientific(v, dec_pl)
    } else {
        format!("{:.*}", dec_pl, v)
    };

    match unit {
        Some(u) => format!("{} {}", formatted, u),
        None => formatted,
    }
}

// tests moved to `tests/x_formatter.rs`
