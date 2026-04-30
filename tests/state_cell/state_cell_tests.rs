/*******************************************************************************
 *
 *    Copyright (c) 2026.
 *    Haixing Hu, Qubit Co. Ltd.
 *
 *    All rights reserved.
 *
 ******************************************************************************/
//! Tests for the `StateCell` type.

use std::sync::Arc;
use std::thread;

use qubit_state_machine::StateCell;

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
enum TestState {
    Ready,
    Running,
    Stopped,
}

#[test]
fn test_state_cell_new_stores_initial_state() {
    let state = StateCell::new(TestState::Ready);

    assert_eq!(state.get(), TestState::Ready);
}

#[test]
fn test_state_cell_replace_returns_old_state_and_updates_current_state() {
    let state = StateCell::new(TestState::Ready);

    let old_state = state.replace(TestState::Running);

    assert_eq!(old_state, TestState::Ready);
    assert_eq!(state.get(), TestState::Running);
}

#[test]
fn test_state_cell_get_is_thread_safe() {
    let state = Arc::new(StateCell::new(TestState::Stopped));
    let mut handles = Vec::new();

    for _ in 0..8 {
        let state = Arc::clone(&state);
        handles.push(thread::spawn(move || state.get()));
    }

    for handle in handles {
        assert_eq!(
            handle.join().expect("thread should read state"),
            TestState::Stopped
        );
    }
}
