use egui::{Color32, Painter, Pos2, Rect};
use egui_plot::PlotTransform;

use super::traces::DensityCache;

/// Render a density cache onto the plot using the given painter and transform.
///
/// Each grid cell is painted as a filled rectangle whose alpha is proportional
/// to the point count in that cell, modulated by `brightness_gain`.
/// The base color is the trace's line color.
pub fn paint_density(
    painter: &Painter,
    transform: &PlotTransform,
    cache: &DensityCache,
    base_color: Color32,
    brightness_gain: f32,
    bounds: (f64, f64),
) {
    if cache.cells.is_empty() || cache.num_y_buckets == 0 {
        return;
    }

    let max_count = cache.max_count.max(1) as f32;

    for x_idx in 0..cache.num_x_buckets {
        let cell = &cache.cells[x_idx];
        if cell.counts.is_empty() {
            continue;
        }
        let x_min = cache.origin_x + x_idx as f64 * cache.bucket_width;
        let x_max = x_min + cache.bucket_width;

        // Skip cells entirely outside visible bounds
        if x_max < bounds.0 || x_min > bounds.1 {
            continue;
        }

        let screen_x_min = transform.position_from_point_x(x_min);
        let screen_x_max = transform.position_from_point_x(x_max);

        for y_idx in 0..cache.num_y_buckets {
            let count = cell.counts[y_idx];
            if count == 0 {
                continue;
            }

            let y_min = cache.origin_y + y_idx as f64 * cache.bucket_height;
            let y_max = y_min + cache.bucket_height;

            let screen_y_min = transform.position_from_point_y(y_min);
            let screen_y_max = transform.position_from_point_y(y_max);

            let rect = Rect::from_min_max(
                Pos2::new(screen_x_min.min(screen_x_max), screen_y_min.min(screen_y_max)),
                Pos2::new(screen_x_min.max(screen_x_max), screen_y_min.max(screen_y_max)),
            );

            let ratio = count as f32 / max_count;
            let intensity = ratio.powf(0.3) * brightness_gain;
            let alpha = (intensity * 255.0).max(120.0).min(255.0) as u8;
            let color = Color32::from_rgba_unmultiplied(
                base_color.r(),
                base_color.g(),
                base_color.b(),
                alpha,
            );

            painter.rect_filled(rect, 0.0, color);
        }
    }
}
