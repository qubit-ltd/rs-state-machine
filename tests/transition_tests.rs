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
    assert_eq!(transition.source(), 1, "source should retain the supplied state");
    assert_eq!(transition.event(), 2, "event should retain the supplied trigger");
    assert_eq!(transition.target(), 3, "target should retain the supplied state");
}

#[test]
fn test_transition_equality_uses_all_fields() {
    let first = Transition::new(1_u8, 2_u8, 3_u8);
    let same = Transition::new(1_u8, 2_u8, 3_u8);
    let different = Transition::new(3_u8, 2_u8, 1_u8);
    assert_eq!(first, same, "transitions with identical fields should be equal");
    assert_ne!(first, different, "transitions with different endpoints should differ");
}
