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
    pub limit: usize,
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

