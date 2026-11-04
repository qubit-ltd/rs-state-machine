// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================

//! Ensures fuzz entry points cannot silently reject every input.
// The shared harness also serves fuzz targets; this loads helpers, not tests.
#[cfg(any(feature = "fast", feature = "standard"))]
#[path = "../support/mod.rs"]
mod support;

#[cfg(feature = "fast")]
#[test]
fn test_fast_builder_seed_reaches_checks() {
    assert_eq!(
        support::run_fast_builder(&[0, 0]),
        support::BuilderOutcome::Built { checked_pairs: 1 }
    );
}
#[cfg(feature = "standard")]
#[test]
fn test_standard_builder_seed_reaches_checks() {
    assert_eq!(
        support::run_standard_builder(&[0, 0]),
        support::BuilderOutcome::Built { checked_pairs: 1 }
    );
}

#[cfg(any(feature = "fast", feature = "standard"))]
#[test]
fn test_extended_builder_initial_terminal_and_order() {
    for order in [0_u8, 1] {
        // Two states, one event, initial state 1, terminal state 0, 1 --0--> 0.
        let valid = [0xff, 1, 0, 1, 1, 0, order, 1, 0, 0];
        let expected = support::BuilderOutcome::Built { checked_pairs: 2 };
        #[cfg(feature = "fast")]
        assert_eq!(support::run_fast_builder(&valid), expected);
        #[cfg(feature = "standard")]
        assert_eq!(support::run_standard_builder(&valid), expected);

        // Initial state 1 is terminal and has a self-loop, so construction must fail.
        let outgoing = [0xff, 1, 0, 1, 2, 0, order, 1, 0, 1];
        #[cfg(feature = "fast")]
        assert_eq!(support::run_fast_builder(&outgoing), support::BuilderOutcome::Rejected);
        #[cfg(feature = "standard")]
        assert_eq!(
            support::run_standard_builder(&outgoing),
            support::BuilderOutcome::Rejected
        );
    }
}

#[cfg(any(feature = "fast", feature = "standard"))]
#[test]
fn test_extended_builder_boundary_and_terminal_cases() {
    for order in [0_u8, 1] {
        let built: &[&[u8]] = &[
            &[0xff, 1, 0, 1, 2, 0, order],          // initial is terminal; no transitions
            &[0xff, 1, 0, 1, 3, 0, order],          // multiple terminal states
            &[0xff, 1, 0, 1, 0, 0, order, 1, 0, 0], // valid edge and no terminal states
        ];
        for input in built {
            #[cfg(feature = "fast")]
            assert!(
                matches!(support::run_fast_builder(input), support::BuilderOutcome::Built { .. }),
                "expected an accepted boundary definition: {input:?}"
            );
            #[cfg(feature = "standard")]
            assert!(
                matches!(
                    support::run_standard_builder(input),
                    support::BuilderOutcome::Built { .. }
                ),
                "expected an accepted boundary definition: {input:?}"
            );
        }
        let rejected = [
            &[0xff, 1, 0, 2, 0, 0, order], // initial out of range
            &[0xff, 1, 0, 0, 4, 0, order], // terminal out of range
        ];
        for input in rejected {
            #[cfg(feature = "fast")]
            assert_eq!(
                support::run_fast_builder(input),
                support::BuilderOutcome::Rejected,
                "unexpected fast outcome for input {input:?}"
            );
            #[cfg(feature = "standard")]
            assert_eq!(
                support::run_standard_builder(input),
                support::BuilderOutcome::Rejected,
                "unexpected standard outcome for input {input:?}"
            );
        }
    }
    for length in 1..7 {
        let input = vec![0xff; length];
        #[cfg(feature = "fast")]
        assert_eq!(
            support::run_fast_builder(&input),
            support::BuilderOutcome::Skipped,
            "truncated extended input of length {length} must be skipped"
        );
        #[cfg(feature = "standard")]
        assert_eq!(
            support::run_standard_builder(&input),
            support::BuilderOutcome::Skipped,
            "truncated extended input of length {length} must be skipped"
        );
    }
    let too_long = [0xff; 513];
    #[cfg(feature = "fast")]
    assert_eq!(support::run_fast_builder(&too_long), support::BuilderOutcome::Skipped);
    #[cfg(feature = "standard")]
    assert_eq!(
        support::run_standard_builder(&too_long),
        support::BuilderOutcome::Skipped
    );
}

#[cfg(all(feature = "fast", feature = "standard"))]
#[test]
fn test_extended_differential_initial_terminal_and_order() {
    for order in [0_u8, 1] {
        let valid = [0xff, 1, 0, 1, 1, 0, order, 0xff, 0, 0, 0];
        assert_eq!(
            support::run_differential(&valid),
            support::BuilderOutcome::Built { checked_pairs: 2 },
        );
        let outgoing = [0xff, 1, 0, 1, 2, 0, order, 1, 0, 0];
        assert_eq!(support::run_differential(&outgoing), support::BuilderOutcome::Rejected);
    }
    for length in 1..7 {
        assert_eq!(
            support::run_differential(&vec![0xff; length]),
            support::BuilderOutcome::Skipped
        );
    }
    assert_eq!(
        support::run_differential(&[0xff; 513]),
        support::BuilderOutcome::Skipped
    );
}

#[cfg(all(feature = "fast", feature = "standard"))]
#[test]
fn test_extended_differential_bounded_tables_and_traces() {
    for initial in 0..2_u8 {
        for mask in 0..4_u8 {
            for order in 0..2_u8 {
                for encoding in 0..81 {
                    let mut code = encoding;
                    let mut input = vec![0xff, 1, 1, initial, mask, 0, order];
                    let mut terminal_outgoing = false;
                    for source in 0..2 {
                        for _event in 0..2 {
                            let cell = match code % 3 {
                                0 => u8::MAX,
                                1 => 0,
                                _ => 1,
                            };
                            terminal_outgoing |= mask & (1 << source) != 0 && cell != u8::MAX;
                            input.push(cell);
                            code /= 3;
                        }
                    }
                    if terminal_outgoing {
                        assert_eq!(
                            support::run_differential(&input),
                            support::BuilderOutcome::Rejected,
                            "terminal outgoing edge must be rejected: {input:?}"
                        );
                    } else {
                        for length in 0..=4 {
                            for sequence in 0..3_usize.pow(length) {
                                let mut sequence_code = sequence;
                                let mut trace = input.clone();
                                for _ in 0..length {
                                    trace.push((sequence_code % 3) as u8);
                                    sequence_code /= 3;
                                }
                                assert_eq!(
                                    support::run_differential(&trace),
                                    support::BuilderOutcome::Built { checked_pairs: 4 },
                                    "bounded table and trace must be accepted: {trace:?}"
                                );
                            }
                        }
                    }
                }
            }
        }
    }
}

#[cfg(any(feature = "fast", feature = "standard"))]
#[test]
fn test_builder_reference_acceptance_and_error_branches() {
    let cases: &[(&[u8], bool)] = &[
        (&[0, 0], true),
        (&[1, 0, 0, 0, 1], true),
        (&[1, 0, 0, 0, 1, 0, 0, 1], true),
        (&[1, 0, 0, 0, 1, 0, 0, 0], false),
        (&[1, 0, 2, 0, 0], false),
        (&[1, 0, 0, 0, 2], false),
    ];
    for &(input, accepted) in cases {
        #[cfg(feature = "fast")]
        assert_eq!(
            matches!(support::run_fast_builder(input), support::BuilderOutcome::Built { .. }),
            accepted,
            "reference acceptance differs for input {input:?}"
        );
        #[cfg(feature = "standard")]
        assert_eq!(
            matches!(
                support::run_standard_builder(input),
                support::BuilderOutcome::Built { .. }
            ),
            accepted,
            "reference acceptance differs for input {input:?}"
        );
    }
    #[cfg(feature = "fast")]
    assert_eq!(
        support::run_fast_builder(&[1, 0, 0, 1, 1]),
        support::BuilderOutcome::Rejected
    );
    #[cfg(feature = "standard")]
    assert!(matches!(
        support::run_standard_builder(&[1, 0, 0, 1, 1]),
        support::BuilderOutcome::Built { .. }
    ));
    for input in [&[][..], &[0][..], &[0; 513][..]] {
        #[cfg(feature = "fast")]
        assert_eq!(
            support::run_fast_builder(input),
            support::BuilderOutcome::Skipped,
            "unexpected fast outcome for input {input:?}"
        );
        #[cfg(feature = "standard")]
        assert_eq!(
            support::run_standard_builder(input),
            support::BuilderOutcome::Skipped,
            "unexpected standard outcome for input {input:?}"
        );
    }
}

#[cfg(all(feature = "fast", feature = "standard"))]
#[test]
fn test_differential_queries_unreachable_rows_without_trace() {
    // Two states/events; state 0 cannot reach state 1. No trailing trace.
    // Row 0: self-loop, unset. Row 1: target 0, self-loop.
    assert_eq!(
        support::run_differential(&[1, 1, 0, 255, 0, 1]),
        support::BuilderOutcome::Built { checked_pairs: 4 }
    );
}

#[cfg(all(feature = "fast", feature = "standard"))]
#[test]
fn test_differential_invalid_events_preserve_trace() {
    assert_eq!(
        support::run_differential(&[1, 0, 1, 0, 0, 254, 255, 0]),
        support::BuilderOutcome::Built { checked_pairs: 2 }
    );
}

#[cfg(all(feature = "fast", feature = "standard"))]
#[test]
fn test_differential_all_two_by_two_tables_and_short_traces() {
    for encoding in 0..81 {
        let mut code = encoding;
        let mut input = vec![1, 1];
        for _ in 0..4 {
            input.push(match code % 3 {
                0 => u8::MAX,
                1 => 0,
                _ => 1,
            });
            code /= 3;
        }
        for length in 0..=4 {
            for sequence in 0..3_usize.pow(length) {
                let mut sequence_code = sequence;
                let mut trace = input.clone();
                for _ in 0..length {
                    trace.push((sequence_code % 3) as u8);
                    sequence_code /= 3;
                }
                support::run_differential(&trace);
            }
        }
    }
    for invalid in [&[][..], &[0][..], &[0, 0][..], &[0; 513][..]] {
        support::run_differential(invalid);
    }
}
