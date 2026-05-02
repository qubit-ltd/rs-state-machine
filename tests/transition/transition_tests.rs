/*******************************************************************************
 *
 *    Copyright (c) 2026 Haixing Hu.
 *
 *    SPDX-License-Identifier: Apache-2.0
 *
 *    Licensed under the Apache License, Version 2.0.
 *
 ******************************************************************************/
//! Tests for the `Transition` type.

use std::collections::HashSet;

use qubit_state_machine::Transition;

#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
enum TestState {
    Ready,
    Running,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
enum TestEvent {
    Start,
}

#[test]
fn test_transition_new_stores_source_event_and_target() {
    let transition = Transition::new(TestState::Ready, TestEvent::Start, TestState::Running);

    assert_eq!(transition.source(), TestState::Ready);
    assert_eq!(transition.event(), TestEvent::Start);
    assert_eq!(transition.target(), TestState::Running);
}

#[test]
fn test_transition_hash_and_equality_use_all_fields() {
    let transition = Transition::new(TestState::Ready, TestEvent::Start, TestState::Running);
    let same = Transition::new(TestState::Ready, TestEvent::Start, TestState::Running);
    let different = Transition::new(TestState::Running, TestEvent::Start, TestState::Ready);

    let mut transitions = HashSet::new();
    transitions.insert(transition);
    transitions.insert(same);
    transitions.insert(different);

    assert_eq!(transitions.len(), 2);
    assert!(transitions.contains(&transition));
    assert!(transitions.contains(&different));
}
