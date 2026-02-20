//! Utility methods for finding and toggling specific sub-panels.
//!
//! These helpers iterate over all panel lists (left, right, bottom, detached,
//! empty) to locate a panel by its concrete type and modify its visibility state.
//! They are used by the controller modules and the layout/rendering code.

use crate::panels::hotkeys_ui::HotkeysPanel;
use crate::panels::panel_trait::Panel;
use crate::panels::thresholds_ui::ThresholdsPanel;
use crate::panels::traces_ui::TracesPanel;

use super::LivePlotPanel;

impl LivePlotPanel {
    /// Return a mutable reference to the [`ThresholdsPanel`], if one exists in any panel list.
    ///
    /// Searches left → right → bottom → detached → empty panels in order.
    pub(crate) fn thresholds_panel_mut(&mut self) -> Option<&mut ThresholdsPanel> {
        for p in self
            .left_side_panels
            .iter_mut()
            .chain(self.right_side_panels.iter_mut())
            .chain(self.bottom_panels.iter_mut())
            .chain(self.detached_panels.iter_mut())
            .chain(self.empty_panels.iter_mut())
        {
            if let Some(tp) = p.downcast_mut::<ThresholdsPanel>() {
                return Some(tp);
            }
        }
        None
    }

    /// Return a mutable reference to the [`TracesPanel`], if one exists in any panel list.
    ///
    /// Searches left → right → bottom → detached → empty panels in order.
    pub(crate) fn traces_panel_mut(&mut self) -> Option<&mut TracesPanel> {
        for p in self
            .left_side_panels
            .iter_mut()
            .chain(self.right_side_panels.iter_mut())
            .chain(self.bottom_panels.iter_mut())
            .chain(self.detached_panels.iter_mut())
            .chain(self.empty_panels.iter_mut())
        {
            if let Some(tp) = p.downcast_mut::<TracesPanel>() {
                return Some(tp);
            }
        }
        None
    }

    /// Toggle the visibility of the first panel of type `T` found in any list.
    ///
    /// If the panel is currently visible and attached (not detached), it becomes hidden.
    /// Otherwise it becomes visible and attached (un-detached).
    ///
    /// Returns `true` if a matching panel was found (regardless of whether its
    /// state actually changed).
    pub(crate) fn toggle_panel_visibility<T: 'static + Panel>(&mut self) -> bool {
        for p in self
            .left_side_panels
            .iter_mut()
            .chain(self.right_side_panels.iter_mut())
            .chain(self.bottom_panels.iter_mut())
            .chain(self.detached_panels.iter_mut())
            .chain(self.empty_panels.iter_mut())
        {
            if p.downcast_ref::<T>().is_some() {
                let st = p.state_mut();
                let currently_shown = st.visible && !st.detached;
                st.visible = !currently_shown;
                st.detached = false;
                return true;
            }
        }
        false
    }

    /// Hide the Hotkeys panel (useful when focus switches away via hotkeys).
    pub fn hide_hotkeys_panel(&mut self) {
        for p in self
            .left_side_panels
            .iter_mut()
            .chain(self.right_side_panels.iter_mut())
            .chain(self.bottom_panels.iter_mut())
            .chain(self.detached_panels.iter_mut())
            .chain(self.empty_panels.iter_mut())
        {
            if p.downcast_ref::<HotkeysPanel>().is_some() {
                p.state_mut().visible = false;
            }
        }
    }
}
