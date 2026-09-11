// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//
//! Tests for the feature-independent transition value.

use qubit_state_machine::Transition;

#[test]
fn test_transition_is_available_without_machine_features() {
    let transition = Transition::new(1_u8, 2_u8, 3_u8);
    assert_eq!(transition.source(), 1);
    assert_eq!(transition.event(), 2);
    assert_eq!(transition.target(), 3);
}

#[test]
fn test_transition_hash_and_equality_use_all_fields() {
    let first = Transition::new(1_u8, 2_u8, 3_u8);
    let same = Transition::new(1_u8, 2_u8, 3_u8);
    let different = Transition::new(3_u8, 2_u8, 1_u8);
    assert_eq!(first, same);
    assert_ne!(first, different);
}
