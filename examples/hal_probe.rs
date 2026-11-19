//! Phase-0 spike for the shared-VkDevice plan: proves eframe/wgpu can
//! run on VolcanoPlot's own VkDevice (foreign device wrapped via
//! wgpu-hal `device_from_raw`), while a VolcanoPlot figure renders on
//! the same device in parallel.
//!
//! Run:
//!   cargo run --release --example hal_probe --features volcano

use liveplot::volcano::{self, VolcanoFigure};

fn main() {
    let shared = match volcano::init_shared() {
        Ok(s) => s,
        Err(e) => {
            eprintln!("shared-device bring-up failed: {e}");
            std::process::exit(2);
        }
    };
    println!(
        "wgpu on VP's VkDevice: adapter={:?}",
        shared.adapter.get_info()
    );

    // Sanity: trivial wgpu submit + poll on the foreign device.
    let enc = shared
        .device
        .create_command_encoder(&eframe::wgpu::CommandEncoderDescriptor {
            label: Some("probe"),
        });
    shared.queue.submit([enc.finish()]);
    let _ = shared.device.poll(eframe::wgpu::PollType::wait_indefinitely());
    println!("trivial submit+poll OK");

    // Prove the C++ renderer still works on the same device: build a
    // figure, draw a sine, render to RGBA.
    let mut fig = VolcanoFigure::new(800, 500).expect("figure create");
    let mut xy = Vec::with_capacity(2000);
    for i in 0..2000 {
        let x = i as f64 * 0.01;
        xy.push([x, x.sin()]);
    }
    let id = fig.add_trace();
    fig.set_trace(
        id,
        &xy,
        0x00FF00FF,
        1.0,
        volcano::VolcanoLineStyle::Solid,
    );
    let rgba = fig.render_rgba();
    assert!(
        rgba.is_some(),
        "vp_render_rgba returned None on shared device"
    );
    let px = rgba.unwrap().to_vec();
    let lit = px.chunks_exact(4).filter(|p| p[1] > 200).count();
    println!("vp figure rendered: {} bytes, {} lit px", px.len(), lit);
    assert!(lit > 100, "rendered image looks empty");

    // Finally: boot eframe on the shared device — the real proof that
    // presentation (swapchain on VP's VkInstance/VkDevice) works.
    let device = shared.device.clone();
    let queue = shared.queue.clone();
    let opts = eframe::NativeOptions {
        wgpu_options: eframe::WgpuConfiguration {
            wgpu_setup: eframe::egui_wgpu::WgpuSetup::Existing(
                eframe::egui_wgpu::WgpuSetupExisting {
                    instance: shared.instance.clone(),
                    adapter: shared.adapter.clone(),
                    device,
                    queue,
                },
            ),
            ..Default::default()
        },
        ..Default::default()
    };
    let _fig = fig; // keep the figure (and its buffers on the shared
                    // device) alive across the eframe run.
    eframe::run_native(
        "hal-probe: eframe on VolcanoPlot's VkDevice",
        opts,
        Box::new(|_cc| Ok(Box::new(ProbeApp))),
    )
    .expect("eframe on shared device failed");
}

struct ProbeApp;
impl eframe::App for ProbeApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ui, |ui| {
            ui.heading("eframe is running on VolcanoPlot's VkDevice");
            ui.label("shared-device Phase-0 spike OK");
        });
    }
}
