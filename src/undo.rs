//! Undo/redo stack for the standalone LivePlot application.
//!
//! Stores [`AppStateSerde`](crate::persistence::AppStateSerde) snapshots before
//! and after each frame.  When the state changes, the (old, new) pair is pushed
//! onto the stack so the user can undo or redo the change.

use crate::persistence::AppStateSerde;

/// Maximum number of undo entries retained in memory.
const DEFAULT_UNDO_LIMIT: usize = 100;

/// A single undo/redo entry storing the state before and after a change.
#[derive(Debug, Clone)]
pub struct LivePlotUndoEntry {
    /// State before the change (restored on undo).
    pub old_state: AppStateSerde,
    /// State after the change (restored on redo).
    pub new_state: AppStateSerde,
    /// Human-readable description of the change.
    pub description: String,
}

/// A simple undo/redo stack for LivePlot state snapshots.
pub struct LivePlotUndoStack {
    undo: Vec<LivePlotUndoEntry>,
    redo: Vec<LivePlotUndoEntry>,
    limit: usize,
}

impl Default for LivePlotUndoStack {
    fn default() -> Self {
        Self {
            undo: Vec::new(),
            redo: Vec::new(),
            limit: DEFAULT_UNDO_LIMIT,
        }
    }
}

impl LivePlotUndoStack {
    pub fn new() -> Self {
        Self::default()
    }

    /// Push a new undo entry and clear the redo stack.
    pub fn push(&mut self, entry: LivePlotUndoEntry) {
        self.undo.push(entry);
        self.redo.clear();
        self.enforce_limit();
    }

    /// Returns `true` if there are actions that can be undone.
    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    /// Returns `true` if there are actions that can be redone.
    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    /// Description of the next undo action, if any.
    pub fn undo_description(&self) -> Option<&str> {
        self.undo.last().map(|e| e.description.as_str())
    }

    /// Description of the next redo action, if any.
    pub fn redo_description(&self) -> Option<&str> {
        self.redo.last().map(|e| e.description.as_str())
    }

    /// Pop the last undo entry (caller is responsible for applying `old_state`).
    pub fn pop_undo(&mut self) -> Option<LivePlotUndoEntry> {
        self.undo.pop()
    }

    /// Pop the last redo entry (caller is responsible for applying `new_state`).
    pub fn pop_redo(&mut self) -> Option<LivePlotUndoEntry> {
        self.redo.pop()
    }

    /// Push an entry onto the redo stack.
    pub fn push_redo(&mut self, entry: LivePlotUndoEntry) {
        self.redo.push(entry);
    }

    /// Push an entry onto the undo stack (without clearing redo).
    pub fn push_undo(&mut self, entry: LivePlotUndoEntry) {
        self.undo.push(entry);
        self.enforce_limit();
    }

    /// Clear all undo/redo history.
    pub fn clear(&mut self) {
        self.undo.clear();
        self.redo.clear();
    }

    /// Debug helper: returns the number of undo entries.
    pub fn undo_len(&self) -> usize {
        self.undo.len()
    }

    /// Debug helper: returns the number of redo entries.
    pub fn redo_len(&self) -> usize {
        self.redo.len()
    }

    fn enforce_limit(&mut self) {
        while self.undo.len() > self.limit {
            self.undo.remove(0);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dummy_state(id: u32) -> AppStateSerde {
        let mut s = AppStateSerde::default();
        // Distinguish states by varying a field.
        s.next_scope_idx = Some(id as usize);
        s
    }

    #[test]
    fn push_clears_redo() {
        let mut stack = LivePlotUndoStack::new();
        stack.push(LivePlotUndoEntry {
            old_state: dummy_state(0),
            new_state: dummy_state(1),
            description: "change 1".into(),
        });
        assert!(stack.can_undo());
        assert!(!stack.can_redo());

        // Simulate undo
        let entry = stack.pop_undo().unwrap();
        stack.push_redo(entry);
        assert!(!stack.can_undo());
        assert!(stack.can_redo());

        // New push should clear redo
        stack.push(LivePlotUndoEntry {
            old_state: dummy_state(1),
            new_state: dummy_state(2),
            description: "change 2".into(),
        });
        assert!(stack.can_undo());
        assert!(!stack.can_redo());
    }

    #[test]
    fn undo_redo_cycle() {
        let mut stack = LivePlotUndoStack::new();
        stack.push(LivePlotUndoEntry {
            old_state: dummy_state(0),
            new_state: dummy_state(1),
            description: "change".into(),
        });

        // Undo
        let entry = stack.pop_undo().unwrap();
        assert_eq!(entry.old_state.next_scope_idx, Some(0));
        stack.push_redo(entry);

        // Redo
        let entry = stack.pop_redo().unwrap();
        assert_eq!(entry.new_state.next_scope_idx, Some(1));
        stack.push_undo(entry);

        assert!(stack.can_undo());
        assert!(!stack.can_redo());
    }

    #[test]
    fn clear_wipes_both_stacks() {
        let mut stack = LivePlotUndoStack::new();
        stack.push(LivePlotUndoEntry {
            old_state: dummy_state(0),
            new_state: dummy_state(1),
            description: "change".into(),
        });
        let entry = stack.pop_undo().unwrap();
        stack.push_redo(entry);

        stack.clear();
        assert!(!stack.can_undo());
        assert!(!stack.can_redo());
    }

    #[test]
    fn descriptions() {
        let mut stack = LivePlotUndoStack::new();
        stack.push(LivePlotUndoEntry {
            old_state: dummy_state(0),
            new_state: dummy_state(1),
            description: "first".into(),
        });
        stack.push(LivePlotUndoEntry {
            old_state: dummy_state(1),
            new_state: dummy_state(2),
            description: "second".into(),
        });

        assert_eq!(stack.undo_description(), Some("second"));

        let entry = stack.pop_undo().unwrap();
        stack.push_redo(entry);

        assert_eq!(stack.undo_description(), Some("first"));
        assert_eq!(stack.redo_description(), Some("second"));
    }

    #[test]
    fn limit_enforced() {
        let mut stack = LivePlotUndoStack::new();
        stack.limit = 3;
        for i in 0..10 {
            stack.push(LivePlotUndoEntry {
                old_state: dummy_state(i),
                new_state: dummy_state(i + 1),
                description: format!("change {}", i),
            });
        }
        assert_eq!(stack.undo_len(), 3);
    }
}
