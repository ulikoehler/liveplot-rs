use liveplot::persistence::AppStateSerde;
use liveplot::{LivePlotUndoEntry, LivePlotUndoStack};

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
