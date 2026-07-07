use eframe::egui;
use once_cell::sync::OnceCell;
use std::sync::Mutex;

const ICON_SIZE: u32 = 64;

const SVG_RISING: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="{COLOR}" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
  <path d="M2 18 L9 18 L9 6 L18 6"/>
  <path d="M18 6 L15 4 M18 6 L15 8"/>
</svg>"#;

const SVG_FALLING: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="{COLOR}" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
  <path d="M2 6 L9 6 L9 18 L18 18"/>
  <path d="M18 18 L15 16 M18 18 L15 20"/>
</svg>"#;

const SVG_BOTH: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="{COLOR}" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
  <path d="M2 18 L6 18 L6 6 L12 6 L12 18 L18 18"/>
  <path d="M18 18 L15 16 M18 18 L15 20"/>
</svg>"#;

fn color_to_hex(c: egui::Color32) -> String {
    format!("#{:02X}{:02X}{:02X}", c.r(), c.g(), c.b())
}

fn rasterize_svg(svg_template: &str, color: egui::Color32, size: u32) -> Option<egui::ColorImage> {
    let svg = svg_template.replace("{COLOR}", &color_to_hex(color));
    let options = usvg::Options::default();
    let tree = usvg::Tree::from_str(&svg, &options).ok()?;
    let mut pixmap = tiny_skia::Pixmap::new(size, size)?;
    resvg::render(&tree, tiny_skia::Transform::from_scale(
        size as f32 / 24.0,
        size as f32 / 24.0,
    ), &mut pixmap.as_mut());
    let pixels = pixmap.take();
    Some(egui::ColorImage::from_rgba_unmultiplied(
        [size as usize, size as usize],
        &pixels,
    ))
}

struct IconCache {
    color: Option<egui::Color32>,
    rising: Option<egui::TextureHandle>,
    falling: Option<egui::TextureHandle>,
    both: Option<egui::TextureHandle>,
}

static ICON_CACHE: OnceCell<Mutex<IconCache>> = OnceCell::new();

fn cache() -> &'static Mutex<IconCache> {
    ICON_CACHE.get_or_init(|| {
        Mutex::new(IconCache {
            color: None,
            rising: None,
            falling: None,
            both: None,
        })
    })
}

pub enum EdgeIcon {
    Rising,
    Falling,
    Both,
}

impl EdgeIcon {
    fn svg(&self) -> &'static str {
        match self {
            EdgeIcon::Rising => SVG_RISING,
            EdgeIcon::Falling => SVG_FALLING,
            EdgeIcon::Both => SVG_BOTH,
        }
    }

    fn name(&self) -> &'static str {
        match self {
            EdgeIcon::Rising => "edge-icon-rising",
            EdgeIcon::Falling => "edge-icon-falling",
            EdgeIcon::Both => "edge-icon-both",
        }
    }

    fn slot<'a>(&self, cache: &'a mut IconCache) -> &'a mut Option<egui::TextureHandle> {
        match self {
            EdgeIcon::Rising => &mut cache.rising,
            EdgeIcon::Falling => &mut cache.falling,
            EdgeIcon::Both => &mut cache.both,
        }
    }
}

/// Get (or create) the texture handle for an edge icon.
/// Re-rasterizes when the theme color changes.
pub fn edge_icon_handle(ctx: &egui::Context, icon: EdgeIcon) -> Option<egui::TextureId> {
    let color = ctx.global_style().visuals.text_color();
    let mut guard = cache().lock().ok()?;
    let cache = &mut *guard;
    let need_rebuild = cache.color != Some(color);
    if need_rebuild {
        cache.color = Some(color);
        cache.rising = None;
        cache.falling = None;
        cache.both = None;
    }
    let slot = icon.slot(cache);
    if slot.is_none() {
        if let Some(image) = rasterize_svg(icon.svg(), color, ICON_SIZE) {
            *slot = Some(ctx.load_texture(icon.name(), image, egui::TextureOptions::LINEAR));
        }
    }
    slot.as_ref().map(|h| h.id())
}

/// Render an edge icon image widget at the given size in points.
#[allow(dead_code)]
pub fn edge_icon(ui: &mut egui::Ui, icon: EdgeIcon, size: f32) -> Option<egui::Response> {
    let tex_id = edge_icon_handle(ui.ctx(), icon)?;
    Some(
        ui.add(
            egui::Image::from_texture(egui::load::SizedTexture {
                id: tex_id,
                size: egui::Vec2::splat(size),
            })
            .fit_to_exact_size(egui::Vec2::splat(size)),
        ),
    )
}

/// Get an `egui::Image` for use as an atom in `Button::selectable((image, "text"))`.
pub fn edge_icon_image(ctx: &egui::Context, icon: EdgeIcon, size: f32) -> Option<egui::Image<'static>> {
    let tex_id = edge_icon_handle(ctx, icon)?;
    Some(
        egui::Image::from_texture(egui::load::SizedTexture {
            id: tex_id,
            size: egui::Vec2::splat(size),
        })
        .fit_to_exact_size(egui::Vec2::splat(size)),
    )
}
