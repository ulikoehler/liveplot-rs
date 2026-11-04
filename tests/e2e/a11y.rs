//! CAT-A11Y — accessibility audit.
//!
//! The global harness disables egui_kittest's built-in accessibility check
//! because several widgets lack accessible names today. This audit counts
//! them so the number is tracked — it must never go *up*; lower it as
//! widgets get labelled, and eventually re-enable the global check.

use egui_kittest::kittest::{NodeT, Queryable};

use crate::harness::*;

/// Interactive roles that should have an accessible label.
const INTERACTIVE: &[egui::accesskit::Role] = &[
    egui::accesskit::Role::Button,
    egui::accesskit::Role::CheckBox,
    egui::accesskit::Role::Slider,
    egui::accesskit::Role::SpinButton,
    egui::accesskit::Role::TextInput,
    egui::accesskit::Role::ComboBox,
];

#[test]
fn unnamed_interactive_widgets_do_not_increase() {
    const CURRENT_UNNAMED: usize = 30; // measured baseline — only shrink this!
    let mut bed = make_test_bed();
    feed_sine(&bed, "a", 10, 1.0, 10.0, 0.0);
    steps(&mut bed, 3);

    let mut unnamed = 0usize;
    let mut offenders: Vec<String> = Vec::new();
    for node in bed.harness.query_all_by(|_| true) {
        let a = node.accesskit_node();
        if INTERACTIVE.contains(&a.role()) && a.label().is_none() {
            unnamed += 1;
            offenders.push(format!("{:?}", a.role()));
        }
    }
    assert!(
        unnamed <= CURRENT_UNNAMED,
        "accessible-name regressions! {unnamed} unnamed widgets (was {CURRENT_UNNAMED}):\n{}",
        offenders.join("\n")
    );
    eprintln!("a11y audit: {unnamed} unnamed interactive widgets");
}
