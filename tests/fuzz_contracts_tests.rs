// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================

//! Ensures fuzz entry points cannot silently reject every input.
#[cfg(any(feature = "fast", feature = "standard"))]
#[path = "../fuzz/support/mod.rs"]
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
            accepted
        );
        #[cfg(feature = "standard")]
        assert_eq!(
            matches!(
                support::run_standard_builder(input),
                support::BuilderOutcome::Built { .. }
            ),
            accepted
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
        assert_eq!(support::run_fast_builder(input), support::BuilderOutcome::Skipped);
        #[cfg(feature = "standard")]
        assert_eq!(support::run_standard_builder(input), support::BuilderOutcome::Skipped);
    }
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
