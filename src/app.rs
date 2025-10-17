use eframe::egui;

use crate::config::{LivePlotConfig, XDateFormat};
use crate::data::DataContext;
use crate::panels::panel_trait::{Panel, PanelState};
use crate::panels::{scope_ui::ScopePanel, traces_ui::TracesPanel, math_ui::MathPanel, thresholds_ui::ThresholdsPanel, triggers_ui::TriggersPanel, fft_ui::FftPanel, export_ui::ExportPanel};

pub struct MainPanelLayout {
    pub main_panels: Vec<Box<dyn Panel>>,
    pub right_side_panels: Vec<Box<dyn Panel>>,
    pub left_side_panels: Vec<Box<dyn Panel>>,
    pub bottom_panels: Vec<Box<dyn Panel>>,
    pub detached_panels: Vec<Box<dyn Panel>>,
}

impl MainPanelLayout {
    fn default_layout() -> Self {
        Self {
            main_panels: vec![Box::new(ScopePanel::default())],
            right_side_panels: vec![Box::new(TracesPanel::default()), Box::new(MathPanel::default()), Box::new(ThresholdsPanel::default()), Box::new(TriggersPanel::default()), Box::new(ExportPanel::default())],
            left_side_panels: vec![],
            bottom_panels: vec![Box::new(FftPanel::default())],
            detached_panels: vec![],
        }
    }
}

pub struct MainApp {
    pub data: DataContext,
    pub cfg: LivePlotConfig,
    pub layout: MainPanelLayout,
}

impl MainApp {
    pub fn new(_rx: std::sync::mpsc::Receiver<crate::sink::MultiSample>) -> Self {
        Self { data: DataContext::default(), cfg: LivePlotConfig::default(), layout: MainPanelLayout::default_layout() }
    }

    pub fn ui_embed(&mut self, ui: &mut egui::Ui) {
        // Menu bar placeholder
        egui::menu::bar(ui, |ui| {
            ui.menu_button("File", |ui| {
                ui.label("Save PNG…");
            });
            ui.menu_button("View", |ui| {
                ui.label("Toggle bars…");
            });
        });
        ui.separator();

        // Layout: left, right side optional; bottom optional; main center
        let show_left = !self.layout.left_side_panels.is_empty();
        let show_right = !self.layout.right_side_panels.is_empty();
        let show_bottom = !self.layout.bottom_panels.is_empty();

        if show_left {
            let mut list = std::mem::take(&mut self.layout.left_side_panels);
            egui::SidePanel::left("left_sidebar").show_inside(ui, |ui| {
                self.render_tabs(ui, &mut list, Area::Left);
            });
            self.layout.left_side_panels = list;
        }
        if show_right {
            let mut list = std::mem::take(&mut self.layout.right_side_panels);
            egui::SidePanel::right("right_sidebar").show_inside(ui, |ui| {
                self.render_tabs(ui, &mut list, Area::Right);
            });
            self.layout.right_side_panels = list;
        }

        if show_bottom {
            let mut list = std::mem::take(&mut self.layout.bottom_panels);
            egui::TopBottomPanel::bottom("bottom_bar").show_inside(ui, |ui| {
                self.render_tabs(ui, &mut list, Area::Bottom);
            });
            self.layout.bottom_panels = list;
        }

        let mut list = std::mem::take(&mut self.layout.main_panels);
        egui::CentralPanel::default().show_inside(ui, |ui| {
            self.render_tabs(ui, &mut list, Area::Main);
        });
        self.layout.main_panels = list;

        // Detached windows
        for p in &mut self.layout.detached_panels {
            if p.state().visible && p.state().detached {
                let mut open = true;
                egui::Window::new(p.name()).open(&mut open).show(ui.ctx(), |ui| {
                    p.render_panel(ui);
                });
                if !open { p.state_mut().visible = false; }
            }
        }
    }

    fn render_tabs(&mut self, ui: &mut egui::Ui, list: &mut Vec<Box<dyn Panel>>, area: Area) {
        let count = list.len();
        let show_actions = matches!(area, Area::Left | Area::Right | Area::Bottom);

        let mut clicked: Option<usize> = None;

        if count > 0 {
            // Decide if actions fit on the same row; if not, render them on a new row.
            let actions_need_row_below = if show_actions {
                let available = ui.available_width();
                // Estimate width of tabs/labels
                let button_font = egui::TextStyle::Button.resolve(ui.style());
                let txt_width = |text: &str, ui: &egui::Ui| -> f32 {
                    ui.fonts(|f| f.layout_no_wrap(text.to_owned(), button_font.clone(), egui::Color32::WHITE).rect.width())
                };
                let pad = ui.spacing().button_padding.x * 2.0 + ui.spacing().item_spacing.x;
                let tabs_w: f32 = match count {
                    0 => 0.0,
                    1 => {
                        if matches!(area, Area::Main) { 0.0 } else { txt_width(list[0].name(), ui) + pad }
                    }
                    _ => list.iter().map(|p| txt_width(p.name(), ui) + pad).sum(),
                };
                let actions_w = txt_width("Pop out", ui) + pad + txt_width("Hide", ui) + pad;
                tabs_w + actions_w > available
            } else { false };

            ui.horizontal(|ui| {
                match count {
                    0 => {}
                    1 => {
                        // For Main: don't show a label if only one
                        if !matches!(area, Area::Main) { ui.strong(list[0].name()); }
                    }
                    _ => {
                        for (i, p) in list.iter_mut().enumerate() {
                            let active = p.state().visible && !p.state().detached;
                            if ui.selectable_label(active, p.name()).clicked() { clicked = Some(i); }
                        }
                    }
                }

                if show_actions && !actions_need_row_below {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button("Hide").clicked() {
                            for p in list.iter_mut() { if !p.state().detached { p.state_mut().visible = false; } }
                        }
                        if ui.button("Pop out").clicked() {
                            for p in list.iter_mut() { if p.state().visible && !p.state().detached { p.state_mut().detached = true; } }
                        }
                    });
                }
            });

            if show_actions && actions_need_row_below {
                ui.horizontal(|ui| {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button("Hide").clicked() {
                            for p in list.iter_mut() { if !p.state().detached { p.state_mut().visible = false; } }
                        }
                        if ui.button("Pop out").clicked() {
                            for p in list.iter_mut() { if p.state().visible && !p.state().detached { p.state_mut().detached = true; } }
                        }
                    });
                });
            }

            // Apply clicked selection when multiple tabs are present
            if count > 1 {
                if let Some(i) = clicked {
                    for (j, p) in list.iter_mut().enumerate() {
                        if j == i { p.state_mut().visible = true; p.state_mut().detached = false; }
                        else if !p.state().detached { p.state_mut().visible = false; }
                    }
                }
            }
        }

        ui.separator();
        // Body: find first attached+visible panel
        if let Some((idx, _)) = list.iter().enumerate().find(|(_i,p)| p.state().visible && !p.state().detached) {
            let p = &mut list[idx];
            p.render_panel(ui);
        } else {
            ui.label("No panel active");
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Area { Main, Left, Right, Bottom }

impl eframe::App for MainApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            self.ui_embed(ui);
        });
        ctx.request_repaint_after(std::time::Duration::from_millis(16));
    }
}

pub fn run_liveplot(rx: std::sync::mpsc::Receiver<crate::sink::MultiSample>, cfg: LivePlotConfig) -> eframe::Result<()> {
    let mut app = MainApp::new(rx);
    let title = cfg.title.clone().unwrap_or_else(|| "LivePlot".to_string());
    let opts = cfg.native_options.clone().unwrap_or_default();
    eframe::run_native(&title, opts, Box::new(|_cc| Ok(Box::new(app))))
}
