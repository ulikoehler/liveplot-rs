//! `PlotBackend` — a drawing/query abstraction over the plotting
//! surface so the overlay callbacks (`draw_overlays`) run unchanged on
//! both the `egui_plot` path and the VolcanoPlot path.
//!
//! `EguiBackend` forwards to a real `egui_plot::PlotUi`.
//! `VolcanoPainterBackend` turns each `PlotItem` into epaint shapes via
//! `PlotItem::shapes()` against a `PlotTransform` we own — identical
//! styling (dashes, markers, text layout) without the `Plot` widget.

use std::ops::RangeInclusive;

use egui::{Context, Pos2, Rect, Response, Vec2b};

/// The egui path writes data bounds into the plot transform padded by
/// 5 % per side (`x_space`/`y_space` in `render_plot`), then subtracts
/// `0.05/1.1` of the transform span on read-back. The volcano path
/// must map the same padded view onto its axes rect so overlays,
/// pointer math and `last_plot_bounds` agree — these helpers are the
/// single source of that convention.
#[cfg(feature = "volcano")]
pub(crate) fn pad_axis_bounds(b: [f64; 2]) -> [f64; 2] {
    let s = (b[1] - b[0]) * 0.05;
    [b[0] - s, b[1] + s]
}

/// Inverse of [`pad_axis_bounds`]: view span -> inner data bounds.
#[cfg(feature = "volcano")]
pub(crate) fn unpad_axis_bounds(v: [f64; 2]) -> [f64; 2] {
    let s = (v[1] - v[0]) * (0.05 / 1.1);
    [v[0] + s, v[1] - s]
}
use egui_plot::{
    HLine, Line, PlotBounds, PlotImage, PlotPoint, PlotTransform, PlotUi, Points, Polygon, Text,
    VLine,
};

pub trait PlotBackend {
    fn line(&mut self, item: Line<'static>);
    fn points(&mut self, item: Points<'static>);
    fn hline(&mut self, item: HLine);
    fn vline(&mut self, item: VLine);
    fn text(&mut self, item: Text);
    fn polygon(&mut self, item: Polygon<'static>);
    fn image(&mut self, item: PlotImage);

    fn plot_bounds(&self) -> PlotBounds;
    fn ctx(&self) -> &Context;
    fn response(&self) -> &Response;
    fn pointer_coordinate(&self) -> Option<PlotPoint>;
    fn plot_from_screen(&self, pos: Pos2) -> PlotPoint;
    fn set_plot_bounds_x(&mut self, range: RangeInclusive<f64>);
    fn set_plot_bounds_y(&mut self, range: RangeInclusive<f64>);
    fn zoom_bounds_around_hovered(&mut self, zoom_factor: egui::Vec2);
    fn transform(&self) -> PlotTransform;
}

/// Adapter over the real `egui_plot` path.
pub struct EguiBackend<'a, 'b>(pub &'b mut PlotUi<'a>);

impl<'a> PlotBackend for EguiBackend<'a, '_> {
    fn line(&mut self, item: Line<'static>) {
        self.0.line(item);
    }
    fn points(&mut self, item: Points<'static>) {
        self.0.points(item);
    }
    fn hline(&mut self, item: HLine) {
        self.0.hline(item);
    }
    fn vline(&mut self, item: VLine) {
        self.0.vline(item);
    }
    fn text(&mut self, item: Text) {
        self.0.text(item);
    }
    fn polygon(&mut self, item: Polygon<'static>) {
        self.0.polygon(item);
    }
    fn image(&mut self, item: PlotImage) {
        self.0.image(item);
    }
    fn plot_bounds(&self) -> PlotBounds {
        self.0.plot_bounds()
    }
    fn ctx(&self) -> &Context {
        self.0.ctx()
    }
    fn response(&self) -> &Response {
        self.0.response()
    }
    fn pointer_coordinate(&self) -> Option<PlotPoint> {
        self.0.pointer_coordinate()
    }
    fn plot_from_screen(&self, pos: Pos2) -> PlotPoint {
        self.0.plot_from_screen(pos)
    }
    fn set_plot_bounds_x(&mut self, range: RangeInclusive<f64>) {
        self.0.set_plot_bounds_x(range);
    }
    fn set_plot_bounds_y(&mut self, range: RangeInclusive<f64>) {
        self.0.set_plot_bounds_y(range);
    }
    fn zoom_bounds_around_hovered(&mut self, zoom_factor: egui::Vec2) {
        self.0.zoom_bounds_around_hovered(zoom_factor);
    }
    fn transform(&self) -> PlotTransform {
        *self.0.transform()
    }
}

/// Paints egui_plot items through an egui `Painter` using a
/// `PlotTransform` we own — used when VolcanoPlot owns the rendering
/// and no `egui_plot::Plot` widget exists.
pub struct VolcanoPainterBackend<'a> {
    ui: &'a mut egui::Ui,
    response: &'a Response,
    /// Data-space bounds of `transform.frame()`.
    bounds: PlotBounds,
    transform: PlotTransform,
}

impl<'a> VolcanoPainterBackend<'a> {
    /// `ui` supplies the painter + input; `response` is the allocated
    /// plot widget's response; `frame` is the axes rect in screen
    /// coords; `bounds` the data-space bounds mapped onto it.
    pub fn new(
        ui: &'a mut egui::Ui,
        response: &'a Response,
        frame: Rect,
        bounds: PlotBounds,
    ) -> Self {
        let transform = PlotTransform::new(frame, bounds, Vec2b::new(false, false));
        Self {
            ui,
            response,
            bounds,
            transform,
        }
    }

    fn draw(&mut self, item: &dyn egui_plot::PlotItem) {
        let mut shapes = Vec::new();
        item.shapes(self.ui, &self.transform, &mut shapes);
        self.ui.painter().extend(shapes);
    }

    /// Screen position of a data-space point.
    pub fn screen_from_plot(&self, p: PlotPoint) -> Pos2 {
        self.transform.position_from_point(&p)
    }
}

impl PlotBackend for VolcanoPainterBackend<'_> {
    fn line(&mut self, item: Line<'static>) {
        self.draw(&item);
    }
    fn points(&mut self, item: Points<'static>) {
        self.draw(&item);
    }
    fn hline(&mut self, item: HLine) {
        self.draw(&item);
    }
    fn vline(&mut self, item: VLine) {
        self.draw(&item);
    }
    fn text(&mut self, item: Text) {
        self.draw(&item);
    }
    fn polygon(&mut self, item: Polygon<'static>) {
        self.draw(&item);
    }
    fn image(&mut self, item: PlotImage) {
        self.draw(&item);
    }
    fn plot_bounds(&self) -> PlotBounds {
        self.bounds
    }
    fn ctx(&self) -> &Context {
        self.ui.ctx()
    }
    fn response(&self) -> &Response {
        self.response
    }
    fn pointer_coordinate(&self) -> Option<PlotPoint> {
        self.response.hover_pos().map(|p| self.plot_from_screen(p))
    }
    fn plot_from_screen(&self, pos: Pos2) -> PlotPoint {
        self.transform.value_from_position(pos)
    }
    fn set_plot_bounds_x(&mut self, range: RangeInclusive<f64>) {
        let [_, y0, _, y1] = [
            self.bounds.min()[0],
            self.bounds.min()[1],
            self.bounds.max()[0],
            self.bounds.max()[1],
        ];
        self.bounds = PlotBounds::from_min_max([*range.start(), y0], [*range.end(), y1]);
        self.transform.set_bounds(self.bounds);
    }
    fn set_plot_bounds_y(&mut self, range: RangeInclusive<f64>) {
        let x0 = self.bounds.min()[0];
        let x1 = self.bounds.max()[0];
        self.bounds = PlotBounds::from_min_max([x0, *range.start()], [x1, *range.end()]);
        self.transform.set_bounds(self.bounds);
    }
    fn zoom_bounds_around_hovered(&mut self, _zoom_factor: egui::Vec2) {
        // Volcano widget handles its own zoom input.
    }
    fn transform(&self) -> PlotTransform {
        self.transform
    }
}
