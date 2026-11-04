// volcano/ffi/volcano_ffi.cpp — C ABI implementation.
// One VpFigure owns a headless backend + renderer + figure/axes.
// Traces are LinePlots keyed by a monotonically increasing id; their
// data is kept CPU-side so the whole figure can be rebuilt on resize
// (plots hold GPU buffers tied to the backend — a backend rebuild
// without rebuilding the plots crashes in updatePoints).
#include "volcano_ffi.h"

#include <volcano/backend/Backend.hpp>
#include <volcano/plot/Axes.hpp>
#include <volcano/plot/Style.hpp>
#include <volcano/plot/plots/LinePlot.hpp>
#include <volcano/render/Renderer.hpp>

#include <cmath>
#include <cstring>
#include <memory>
#include <string>
#include <unordered_map>
#include <vector>

using namespace volcano;

struct VpFigure {
    backend::BackendDesc desc{};
    std::unique_ptr<backend::IBackend> backend;
    std::unique_ptr<render::Renderer> renderer;
    std::unique_ptr<plot::Figure> figure;
    plot::Axes* axes = nullptr;

    struct TraceData {
        std::vector<float> x, y;
        uint32_t rgba8 = 0xFFFFFFFF;
        float width = 1.5f;
        int style = 0;
    };
    std::unordered_map<uint64_t, TraceData> trace_data;
    std::unordered_map<uint64_t, plot::LinePlot*> traces;
    uint64_t next_id = 1;

    std::string x_label, y_label;
    bool white_bg = false;
    int w = 0, h = 0;
};

static void rebuild_figure(VpFigure* f) {
    f->figure = std::make_unique<plot::Figure>(1, 1);
    f->axes = f->figure->addAxes();
    f->axes->setStyle(f->white_bg ? plot::styles::defaultStyle()
                                  : plot::styles::darkBackground());
    if (!f->x_label.empty()) f->axes->style().xAxis.label = f->x_label;
    if (!f->y_label.empty()) f->axes->style().yAxis.label = f->y_label;
    f->traces.clear();
    for (auto& [id, td] : f->trace_data) {
        auto lp = std::make_shared<plot::LinePlot>(plot::Series2D{});
        auto* raw = lp.get();
        f->axes->addPlot(std::move(lp));
        raw->setData(td.x, td.y);
        auto& s = raw->series();
        s.color = plot::Color::fromRgba8(
            (td.rgba8 >> 24) & 0xFF, (td.rgba8 >> 16) & 0xFF,
            (td.rgba8 >> 8) & 0xFF, td.rgba8 & 0xFF);
        s.lineWidth = td.width;
        s.lineStyle = td.style == 1 ? plot::LineStyle::Dashed
                      : td.style == 2 ? plot::LineStyle::Dotted
                                      : plot::LineStyle::Solid;
        s.marker = td.style == 3 ? plot::MarkerStyle::Circle
                                 : plot::MarkerStyle::None;
        f->traces[id] = raw;
    }
}

static bool init(VpFigure* f, int w, int h) {
    // Tear down the old figure *before* the backend: plots own GPU
    // buffers tied to the old device — destroying them after the
    // backend is gone trips vkDestroyBuffer on an invalid device.
    f->traces.clear();
    f->figure = std::make_unique<plot::Figure>(1, 1);
    f->axes = nullptr;
    f->renderer.reset();
    f->desc.width = static_cast<uint32_t>(w);
    f->desc.height = static_cast<uint32_t>(h);
    f->desc.samples = vk::SampleCountFlagBits::e4;
    f->backend = backend::createHeadlessBackend(f->desc);
    if (!f->backend) return false;
    f->renderer = std::make_unique<render::Renderer>(*f->backend);
    f->w = w;
    f->h = h;
    rebuild_figure(f);
    return true;
}

extern "C" {

VpFigure* vp_figure_create(int width, int height) {
    auto* f = new VpFigure();
    if (!init(f, width, height)) {
        delete f;
        return nullptr;
    }
    return f;
}

void vp_figure_destroy(VpFigure* fig) { delete fig; }

int vp_figure_resize(VpFigure* fig, int width, int height) {
    if (!fig || width <= 0 || height <= 0) return 0;
    if (width == fig->w && height == fig->h) return 1;
    return init(fig, width, height) ? 1 : 0;
}

uint64_t vp_trace_add(VpFigure* fig) {
    if (!fig) return 0;
    uint64_t id = fig->next_id++;
    fig->trace_data[id] = VpFigure::TraceData{};
    auto lp = std::make_shared<plot::LinePlot>(plot::Series2D{});
    auto* raw = lp.get();
    fig->axes->addPlot(std::move(lp));
    fig->traces[id] = raw;
    return id;
}

void vp_trace_set(VpFigure* fig, uint64_t id, const double* xy, size_t n,
                  uint32_t rgba8, float width, int style) {
    if (!fig) return;
    auto td_it = fig->trace_data.find(id);
    if (td_it == fig->trace_data.end()) return;
    auto& td = td_it->second;
    td.x.assign(n, 0.0f);
    td.y.assign(n, 0.0f);
    for (size_t i = 0; i < n; ++i) {
        td.x[i] = static_cast<float>(xy[2 * i]);
        td.y[i] = static_cast<float>(xy[2 * i + 1]);
    }
    td.rgba8 = rgba8;
    td.width = width;
    td.style = style;

    auto it = fig->traces.find(id);
    if (it == fig->traces.end()) return;
    auto* lp = it->second;
    lp->setData(td.x, td.y);
    auto& s = lp->series();
    s.color = plot::Color::fromRgba8(
        (rgba8 >> 24) & 0xFF, (rgba8 >> 16) & 0xFF, (rgba8 >> 8) & 0xFF,
        rgba8 & 0xFF);
    s.lineWidth = width;
    s.lineStyle = style == 1 ? plot::LineStyle::Dashed
                  : style == 2 ? plot::LineStyle::Dotted
                               : plot::LineStyle::Solid;
    s.marker = style == 3 ? plot::MarkerStyle::Circle
                          : plot::MarkerStyle::None;
}

void vp_trace_remove(VpFigure* fig, uint64_t id) {
    if (!fig) return;
    auto it = fig->traces.find(id);
    if (it != fig->traces.end()) {
        fig->axes->removePlot(it->second);
        fig->traces.erase(it);
    }
    fig->trace_data.erase(id);
}

void vp_set_viewport(VpFigure* fig, double xmin, double xmax, double ymin,
                     double ymax) {
    if (!fig || !fig->axes) return;
    if (!(std::isnan(xmin) || std::isnan(xmax)))
        fig->axes->setXlim(static_cast<float>(xmin), static_cast<float>(xmax));
    if (!(std::isnan(ymin) || std::isnan(ymax)))
        fig->axes->setYlim(static_cast<float>(ymin), static_cast<float>(ymax));
}

void vp_set_axis_labels(VpFigure* fig, const char* x_label,
                        const char* y_label) {
    if (!fig) return;
    if (x_label) fig->x_label = x_label;
    if (y_label) fig->y_label = y_label;
    if (fig->axes) {
        if (x_label) fig->axes->style().xAxis.label = x_label;
        if (y_label) fig->axes->style().yAxis.label = y_label;
    }
}

void vp_set_background(VpFigure* fig, int white) {
    if (!fig) return;
    fig->white_bg = white != 0;
    if (fig->axes) {
        fig->axes->setStyle(white ? plot::styles::defaultStyle()
                                  : plot::styles::darkBackground());
    }
}

size_t vp_render_rgba(VpFigure* fig, uint8_t* out, size_t out_len) {
    if (!fig || !out) return 0;
    size_t need = static_cast<size_t>(fig->w) * fig->h * 4;
    if (out_len < need) return 0;
    fig->renderer->prepare(*fig->figure);
    fig->renderer->renderFrame(*fig->figure);
    auto px = fig->backend->readbackRgba8();
    if (px.size() != need) return 0;
    std::memcpy(out, px.data(), need);
    return need;
}

int vp_figure_width(const VpFigure* fig) { return fig ? fig->w : 0; }
int vp_figure_height(const VpFigure* fig) { return fig ? fig->h : 0; }

} // extern "C"
