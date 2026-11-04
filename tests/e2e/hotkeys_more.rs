//! CAT-HK — remaining default hotkeys through the input pipeline.

use liveplot::data::hotkeys::{Hotkey, HotkeyName, Modifier};
use liveplot::panels::{ThresholdsPanel, TriggersPanel};

use crate::harness::*;

/// Resolve the configured binding for `name`, then simulate it.
fn press_hotkey_for(bed: &mut TestBed, name: HotkeyName) -> bool {
    let binding: Option<Hotkey> = {
        let hk = bed.harness.state().hotkeys.borrow();
        match name {
            HotkeyName::Thresholds => hk.thresholds,
            HotkeyName::Measurements => hk.measurements,
            HotkeyName::Triggers => hk.triggers,
            HotkeyName::Math => hk.math,
            HotkeyName::Fft => hk.fft,
            _ => None,
        }
    };
    let Some(h) = binding else { return false };
    let c = h.key;
    let key = match c.to_ascii_uppercase() {
        'A' => egui::Key::A,
        'B' => egui::Key::B,
        'C' => egui::Key::C,
        'D' => egui::Key::D,
        'E' => egui::Key::E,
        'F' => egui::Key::F,
        'G' => egui::Key::G,
        'H' => egui::Key::H,
        'M' => egui::Key::M,
        'P' => egui::Key::P,
        'R' => egui::Key::R,
        'S' => egui::Key::S,
        'T' => egui::Key::T,
        'X' => egui::Key::X,
        'Y' => egui::Key::Y,
        _ => return false,
    };
    let modifiers = match h.modifier {
        Modifier::None => egui::Modifiers::NONE,
        Modifier::Ctrl => egui::Modifiers::CTRL,
        Modifier::Alt => egui::Modifiers::ALT,
        Modifier::Shift => egui::Modifiers::SHIFT,
        Modifier::CtrlShift => egui::Modifiers::CTRL | egui::Modifiers::SHIFT,
        _ => return false,
    };
    key_press_mod(bed, modifiers, key);
    steps(bed, 2);
    true
}

#[test]
fn thresholds_hotkey_toggles_panel() {
    let mut bed = make_test_bed();
    feed_sine(&bed, "a", 10, 1.0, 10.0, 0.0);
    steps(&mut bed, 2);
    let before = panel_visible::<ThresholdsPanel>(&bed);
    assert!(
        press_hotkey_for(&mut bed, HotkeyName::Thresholds),
        "no thresholds binding configured"
    );
    assert_ne!(
        panel_visible::<ThresholdsPanel>(&bed),
        before,
        "thresholds hotkey should toggle the panel"
    );
}

#[test]
fn triggers_hotkey_toggles_panel() {
    let mut bed = make_test_bed();
    feed_sine(&bed, "a", 10, 1.0, 10.0, 0.0);
    steps(&mut bed, 2);
    let before = panel_visible::<TriggersPanel>(&bed);
    assert!(
        press_hotkey_for(&mut bed, HotkeyName::Triggers),
        "no triggers binding configured"
    );
    assert_ne!(
        panel_visible::<TriggersPanel>(&bed),
        before,
        "triggers hotkey should toggle the panel"
    );
}

#[test]
fn math_hotkey_toggles_panel() {
    let mut bed = make_test_bed();
    feed_sine(&bed, "a", 10, 1.0, 10.0, 0.0);
    steps(&mut bed, 2);
    let before = panel_visible::<liveplot::panels::MathPanel>(&bed);
    assert!(
        press_hotkey_for(&mut bed, HotkeyName::Math),
        "no math binding configured"
    );
    assert_ne!(
        panel_visible::<liveplot::panels::MathPanel>(&bed),
        before,
        "math hotkey should toggle the panel"
    );
}
