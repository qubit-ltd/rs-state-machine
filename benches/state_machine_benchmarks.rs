// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================

//! Benchmarks transition lookup and uncontended state changes for both state
//! machine implementations.

#[cfg(any(feature = "fast", feature = "standard"))]
use std::hint::black_box;

use criterion::{
    Criterion,
    criterion_group,
    criterion_main,
};
#[cfg(feature = "standard")]
use qubit_atomic::AtomicRef;
#[cfg(feature = "fast")]
use qubit_fast_cas::FastCasState;
#[cfg(feature = "fast")]
use qubit_state_machine::FastStateMachine;
#[cfg(feature = "standard")]
use qubit_state_machine::StateMachine;

/// Benchmarks dense transition lookup and an uncontended alternating update.
#[cfg(feature = "fast")]
fn benchmark_fast_state_machine(c: &mut Criterion) {
    let mut group = c.benchmark_group("fast_state_machine");
    let lookup_machine = create_fast_state_machine();
    group.bench_function("transition_target", |bencher| {
        bencher.iter(|| {
            black_box(
                lookup_machine.transition_target(black_box(0), black_box(0)),
            )
        });
    });

    let trigger_machine = create_fast_state_machine();
    let trigger_state = FastCasState::new(0);
    group.bench_function("try_trigger_alternating", |bencher| {
        bencher.iter(|| {
            black_box(
                trigger_machine
                    .try_trigger(black_box(&trigger_state), black_box(0)),
            )
        });
    });
    group.finish();
}

/// Keeps the benchmark target compilable when the Fast feature is disabled.
#[cfg(not(feature = "fast"))]
fn benchmark_fast_state_machine(_criterion: &mut Criterion) {}

/// Builds the two-state Fast fixture outside each measured iteration.
#[cfg(feature = "fast")]
fn create_fast_state_machine() -> FastStateMachine {
    FastStateMachine::builder()
        .state_count(2)
        .event_count(1)
        .initial_state(0)
        .transition(0, 0, 1)
        .transition(1, 0, 0)
        .build()
        .expect("Fast benchmark fixture should build")
}

/// Benchmarks an uncontended alternating update through the generic machine.
#[cfg(feature = "standard")]
fn benchmark_standard_state_machine(c: &mut Criterion) {
    let mut group = c.benchmark_group("standard_state_machine");
    let machine = create_standard_state_machine();
    let state = AtomicRef::from_value(0_u8);
    group.bench_function("try_trigger_alternating", |bencher| {
        bencher.iter(|| {
            black_box(machine.try_trigger(black_box(&state), black_box(0)))
        });
    });
    group.finish();
}

/// Keeps the benchmark target compilable when the standard feature is
/// disabled.
#[cfg(not(feature = "standard"))]
fn benchmark_standard_state_machine(_criterion: &mut Criterion) {}

/// Builds the two-state generic fixture outside each measured iteration.
#[cfg(feature = "standard")]
fn create_standard_state_machine() -> StateMachine<u8, u8> {
    StateMachine::builder()
        .add_states(&[0, 1])
        .initial_state(0)
        .transition(0, 0, 1)
        .transition(1, 0, 0)
        .build()
        .expect("standard benchmark fixture should build")
}

criterion_group!(
    benches,
    benchmark_fast_state_machine,
    benchmark_standard_state_machine,
);
criterion_main!(benches);
