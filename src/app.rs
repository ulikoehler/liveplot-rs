use eframe::egui;

use crate::panels::panel_trait::Panel;
// use crate::panels::{
//     export_ui::ExportPanel, fft_ui::FftPanel, math_ui::MathPanel, scope_ui::ScopePanel,
//     thresholds_ui::ThresholdsPanel, traces_ui::TracesPanel, triggers_ui::TriggersPanel,
// };
use crate::panels::{scope_ui::ScopePanel, traces_ui::TracesPanel, math_ui::MathPanel, thresholds_ui::ThresholdsPanel, export_ui::ExportPanel, triggers_ui::TriggersPanel};

pub struct MainPanel {
    // Panels
    pub scope_panel: ScopePanel,
    pub right_side_panels: Vec<Box<dyn Panel>>,
    pub left_side_panels: Vec<Box<dyn Panel>>,
    pub bottom_panels: Vec<Box<dyn Panel>>,
    pub detached_panels: Vec<Box<dyn Panel>>,
    pub empty_panels: Vec<Box<dyn Panel>>,
}

impl MainPanel {
    pub fn new(rx: std::sync::mpsc::Receiver<crate::sink::MultiSample>) -> Self {
        Self {
            scope_panel: ScopePanel::new(rx),
            right_side_panels: vec![Box::new(TracesPanel::default()), Box::new(MathPanel::default()), Box::new(ThresholdsPanel::default()), Box::new(TriggersPanel::default())], 
            //vec![Box::new(TracesPanel::default()), Box::new(MathPanel::default()), Box::new(ThresholdsPanel::default()), Box::new(TriggersPanel::default()), Box::new(ExportPanel::default())],
            left_side_panels: vec![],
            bottom_panels: vec![], //vec![Box::new(FftPanel::default())],
            detached_panels: vec![],
            empty_panels: vec![Box::new(ExportPanel::default())],
        }
    }

    pub fn update(&mut self, ui: &mut egui::Ui) {
        self.update_data();

        // Render UI
        self.render_menu(ui);
        self.render_panels(ui);

        // Draw additional overlay objects from other panels (e.g., thresholds)
        egui::CentralPanel::default().show_inside(ui, |ui| {
            // Temporarily take panel lists to build a local overlay drawer without borrowing self
            let mut left = std::mem::take(&mut self.left_side_panels);
            let mut right = std::mem::take(&mut self.right_side_panels);
            let mut bottom = std::mem::take(&mut self.bottom_panels);
            let mut detached = std::mem::take(&mut self.detached_panels);
            let mut empty = std::mem::take(&mut self.empty_panels);

            let mut draw_overlays = |plot_ui: &mut egui_plot::PlotUi, data: &crate::data::scope::ScopeData| {
                for p in right
                    .iter_mut()
                    .chain(left.iter_mut())
                    .chain(bottom.iter_mut())
                    .chain(detached.iter_mut())
                    .chain(empty.iter_mut())
                {
                    p.draw(plot_ui, data);
                }
            };

            self.scope_panel.render_panel(ui, &mut draw_overlays);

            // Return panel lists back to self
            self.left_side_panels = left;
            self.right_side_panels = right;
            self.bottom_panels = bottom;
            self.detached_panels = detached;
            self.empty_panels = empty;
        });
    }

    fn update_data(&mut self) {
        let data = self.scope_panel.update_data();

        for p in &mut self.left_side_panels {
            p.update_data(data);
        }
        for p in &mut self.right_side_panels {
            p.update_data(data);
        }
        for p in &mut self.bottom_panels {
            p.update_data(data);
        }
        for p in &mut self.detached_panels {
            p.update_data(data);
        }
        for p in &mut self.empty_panels {
            p.update_data(data);
        }
    }

    fn render_menu(&mut self, ui: &mut egui::Ui) {
        // Render Menu

        egui::MenuBar::new().ui(ui, |ui| {
            self.scope_panel.render_menu(ui);

            let data = self.scope_panel.get_data_mut();

            for p in &mut self.left_side_panels {
                p.render_menu(ui, data);
            }
            for p in &mut self.right_side_panels {
                p.render_menu(ui, data);
            }
            for p in &mut self.bottom_panels {
                p.render_menu(ui, data);
            }
            for p in &mut self.detached_panels {
                p.render_menu(ui, data);
            }
            for p in &mut self.empty_panels {
                p.render_menu(ui, data);
            }

            ui.menu_button("Panels", |ui| {
                for p in &mut self.left_side_panels {
                    if ui
                        .selectable_label(p.state_mut().visible, p.title())
                        .clicked()
                    {
                        p.state_mut().detached = false;
                        p.state_mut().visible = true;
                    }
                }
                for p in &mut self.right_side_panels {
                    if ui
                        .selectable_label(p.state_mut().visible, p.title())
                        .clicked()
                    {
                        p.state_mut().detached = false;
                        p.state_mut().visible = true;
                    }
                }
                for p in &mut self.bottom_panels {
                    if ui
                        .selectable_label(p.state_mut().visible, p.title())
                        .clicked()
                    {
                        p.state_mut().detached = false;
                        p.state_mut().visible = true;
                    }
                }
                for p in &mut self.detached_panels {
                    if ui
                        .selectable_label(p.state_mut().visible, p.title())
                        .clicked()
                    {
                        p.state_mut().detached = true;
                        p.state_mut().visible = true;
                    }
                }
            });
        });
    }

    fn render_panels(&mut self, ui: &mut egui::Ui) {
        // Layout: left, right side optional; bottom optional; main center
        let show_left = !self.left_side_panels.is_empty()
            && self
                .left_side_panels
                .iter()
                .any(|p| p.state().visible && !p.state().detached);
        let show_right = !self.right_side_panels.is_empty()
            && self
                .right_side_panels
                .iter()
                .any(|p| p.state().visible && !p.state().detached);
        let show_bottom = !self.bottom_panels.is_empty()
            && self
                .bottom_panels
                .iter()
                .any(|p| p.state().visible && !p.state().detached);

        if show_left {
            let mut list = std::mem::take(&mut self.left_side_panels);
            egui::SidePanel::left("left_sidebar")
                .resizable(true)
                .default_width(280.0)
                .min_width(160.0)
                .show_inside(ui, |ui| {
                    self.render_tabs(ui, &mut list, egui::Align::Min);
                });
            self.left_side_panels = list;
        }
        if show_right {
            let mut list = std::mem::take(&mut self.right_side_panels);
            egui::SidePanel::right("right_sidebar")
                .resizable(true)
                .default_width(320.0)
                .min_width(200.0)
                .show_inside(ui, |ui| {
                    self.render_tabs(ui, &mut list, egui::Align::Max);
                });
            self.right_side_panels = list;
        }

        if show_bottom {
            let mut list = std::mem::take(&mut self.bottom_panels);
            egui::TopBottomPanel::bottom("bottom_bar")
                .resizable(true)
                .default_height(220.0)
                .min_height(120.0)
                .show_inside(ui, |ui| {
                    self.render_tabs(ui, &mut list, egui::Align::Max);
                });
            self.bottom_panels = list;
        }

        // Detached windows
        // Detached left windows
        for p in &mut self.left_side_panels {
            if p.state().visible && p.state().detached {
                p.show_detached_dialog(ui.ctx(), self.scope_panel.get_data_mut());
            }
        }

        // Detached right windows
        for p in &mut self.right_side_panels {
            if p.state().visible && p.state().detached {
                p.show_detached_dialog(ui.ctx(), self.scope_panel.get_data_mut());
            }
        }

        // Detached bottom windows
        for p in &mut self.bottom_panels {
            if p.state().visible && p.state().detached {
                p.show_detached_dialog(ui.ctx(), self.scope_panel.get_data_mut());
            }
        }

        for p in &mut self.detached_panels {
            if p.state().visible && p.state().detached {
                p.show_detached_dialog(ui.ctx(), self.scope_panel.get_data_mut());
            }
        }
    }

    fn render_tabs(
        &mut self,
        ui: &mut egui::Ui,
        list: &mut Vec<Box<dyn Panel>>,
        _align: egui::Align,
    ) {
        let count = list.len();

        let mut clicked: Option<usize> = None;

        let data = self.scope_panel.get_data_mut();

        if count > 0 {
            // Decide if actions fit on the same row; if not, render them on a new row.
            let actions_need_row_below = {
                let available = ui.available_width();
                // Estimate width of tabs/labels
                let button_font = egui::TextStyle::Button.resolve(ui.style());
                let txt_width = |text: &str, ui: &egui::Ui| -> f32 {
                    ui.fonts(|f| {
                        f.layout_no_wrap(text.to_owned(), button_font.clone(), egui::Color32::WHITE)
                            .rect
                            .width()
                    })
                };
                let pad = ui.spacing().button_padding.x * 2.0 + ui.spacing().item_spacing.x;
                let tabs_w: f32 = match count {
                    0 => 0.0,
                    1 => txt_width(list[0].title(), ui) + pad,
                    _ => list.iter().map(|p| txt_width(p.title(), ui) + pad).sum(),
                };
                let actions_w = txt_width("Pop out", ui) + pad + txt_width("Hide", ui) + pad;
                tabs_w + actions_w > available
            };

            ui.horizontal(|ui| {
                if count > 1 {
                    for (i, p) in list.iter_mut().enumerate() {
                        let active = p.state().visible && !p.state().detached;
                        if ui.selectable_label(active, p.title()).clicked() {
                            clicked = Some(i);
                        }
                    }
                } else {
                    let p = &mut list[0];
                    ui.label(p.title());
                    clicked = Some(0);
                }

                if !actions_need_row_below {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button("Hide").clicked() {
                            for p in list.iter_mut() {
                                if !p.state().detached {
                                    p.state_mut().visible = false;
                                }
                            }
                        }
                        if ui.button("Pop out").clicked() {
                            for p in list.iter_mut() {
                                if p.state().visible && !p.state().detached {
                                    p.state_mut().detached = true;
                                }
                            }
                        }
                    });
                }
            });

            if actions_need_row_below {
                ui.horizontal(|ui| {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button("Hide").clicked() {
                            for p in list.iter_mut() {
                                if !p.state().detached {
                                    p.state_mut().visible = false;
                                }
                            }
                        }
                        if ui.button("Pop out").clicked() {
                            for p in list.iter_mut() {
                                if p.state().visible && !p.state().detached {
                                    p.state_mut().detached = true;
                                }
                            }
                        }
                    });
                });
            }

            // Apply clicked selection when multiple tabs are present
            if count > 1 {
                if let Some(i) = clicked {
                    for (j, p) in list.iter_mut().enumerate() {
                        if j == i {
                            p.state_mut().visible = true;
                            p.state_mut().detached = false;
                        } else if !p.state().detached {
                            p.state_mut().visible = false;
                        }
                    }
                }
            }
        }

        ui.separator();
        // Body: find first attached+visible panel
        if let Some((idx, _)) = list
            .iter()
            .enumerate()
            .find(|(_i, p)| p.state().visible && !p.state().detached)
        {
            let p = &mut list[idx];
            p.render_panel(ui, data);
        } else {
            ui.label("No panel active");
        }
    }
}

pub struct MainApp {
    pub main_panel: MainPanel,
}

impl MainApp {
    pub fn new(rx: std::sync::mpsc::Receiver<crate::sink::MultiSample>) -> Self {
        Self {
            main_panel: MainPanel::new(rx),
        }
    }
}

impl eframe::App for MainApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            // Non-UI calculations first
            self.main_panel.update(ui);
        });
        ctx.request_repaint_after(std::time::Duration::from_millis(16));
    }
}

pub fn run_liveplot(rx: std::sync::mpsc::Receiver<crate::sink::MultiSample>) -> eframe::Result<()> {
    let app = MainApp::new(rx);

    let title = "LivePlot".to_string();
    let opts = eframe::NativeOptions {
        // initial_window_size: Some(egui::vec2(1280.0, 720.0)),
        ..Default::default()
    };
    eframe::run_native(&title, opts, Box::new(|_cc| Ok(Box::new(app))))
}
