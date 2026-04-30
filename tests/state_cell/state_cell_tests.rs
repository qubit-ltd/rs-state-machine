/*******************************************************************************
 *
 *    Copyright (c) 2026.
 *    Haixing Hu, Qubit Co. Ltd.
 *
 *    All rights reserved.
 *
 ******************************************************************************/
//! Tests for the `StateCell` alias.

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
fn test_state_cell_alias_can_create_atomic_ref() {
    let state = StateCell::from_value(TestState::Ready);

    assert_eq!(*state.load(), TestState::Ready);
}

#[test]
fn test_state_cell_alias_exposes_atomic_swap() {
    let state = StateCell::from_value(TestState::Ready);

    let old_state = state.swap(Arc::new(TestState::Running));

    assert_eq!(*old_state, TestState::Ready);
    assert_eq!(*state.load(), TestState::Running);
}

#[test]
fn test_state_cell_get_is_thread_safe() {
    let state = Arc::new(StateCell::from_value(TestState::Stopped));
    let mut handles = Vec::new();

    for _ in 0..8 {
        let state = Arc::clone(&state);
        handles.push(thread::spawn(move || *state.load()));
    }

    for handle in handles {
        assert_eq!(
            handle.join().expect("thread should read state"),
            TestState::Stopped
        );
    }
}
