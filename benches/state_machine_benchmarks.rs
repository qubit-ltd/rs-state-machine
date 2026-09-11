// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//
//! Comparable atomic, integer, typed, and generic lifecycle benchmarks.
//! Race timing includes worker synchronization but excludes thread/cell
//! construction.
use std::hint::black_box;
use std::sync::Barrier;
use std::sync::atomic::AtomicU64;
use std::sync::atomic::Ordering;
use std::time::Duration;
use std::time::Instant;

use criterion::BatchSize;
use criterion::Criterion;
use criterion::criterion_group;
use criterion::criterion_main;
#[cfg(feature = "standard")]
use qubit_atomic::AtomicRef;
#[cfg(feature = "fast")]
use qubit_fast_cas::FastCasState;
#[cfg(feature = "fast")]
use qubit_state_machine::DenseCode;
#[cfg(feature = "fast")]
use qubit_state_machine::FastStateMachine;
#[cfg(feature = "fast")]
use qubit_state_machine::FastStateMachineError;
#[cfg(feature = "standard")]
use qubit_state_machine::StateMachine;
#[cfg(feature = "standard")]
use qubit_state_machine::StateMachineError;
#[cfg(feature = "fast")]
use qubit_state_machine::TypedFastState;
#[cfg(feature = "fast")]
use qubit_state_machine::TypedFastStateMachine;
#[cfg(feature = "fast")]
use qubit_state_machine::TypedFastStateMachineError;

/// Fixture rule selection, fixed outside timed iterations.
#[derive(Clone, Copy)]
enum Mode {
    Alternating,
    SelfLoop,
    Task,
}

/// Outcome categories counted equally by all implementations.
#[derive(Debug)]
enum Failure {
    Rejected,
    Conflict,
    Unexpected,
}

/// Monomorphized benchmark operations; no trait objects enter the hot path.
trait Driver: Sync {
    type Cell: Send + Sync;
    type Event: Copy + Sync;
    type Source: Copy;
    fn event(code: u64) -> Self::Event;
    fn source(code: u64) -> Self::Source;
    fn cell(&self) -> Self::Cell;
    fn load(cell: &Self::Cell) -> u64;
    fn apply(&self, cell: &Self::Cell, event: Self::Event) -> Result<u64, Failure>;
    fn lookup(&self, state: Self::Source, event: Self::Event) -> Option<u64>;
}

/// Direct match plus strong AtomicU64 CAS baseline.
struct AtomicDriver {
    mode: Mode,
}
impl Driver for AtomicDriver {
    type Cell = AtomicU64;
    type Event = u64;
    type Source = u64;
    fn event(code: u64) -> u64 {
        code
    }
    fn source(code: u64) -> u64 {
        code
    }
    fn cell(&self) -> AtomicU64 {
        AtomicU64::new(0)
    }
    fn load(cell: &AtomicU64) -> u64 {
        cell.load(Ordering::Acquire)
    }
    fn lookup(&self, state: u64, event: u64) -> Option<u64> {
        match (self.mode, state, event) {
            (Mode::Alternating, 0, 0) => Some(1),
            (Mode::Alternating, 1, 0) | (Mode::SelfLoop, 0, 0) => Some(0),
            (Mode::Task, 0, 0) => Some(1),
            (Mode::Task, 0, 1) => Some(5),
            (Mode::Task, 1, 2) => Some(2),
            (Mode::Task, 1, 3) => Some(3),
            (Mode::Task, 1, 4) => Some(4),
            (Mode::Task, 0 | 1, 5) => Some(6),
            _ => None,
        }
    }
    fn apply(&self, cell: &AtomicU64, event: u64) -> Result<u64, Failure> {
        for _ in 0..16 {
            let current = cell.load(Ordering::Acquire);
            let target = self.lookup(current, event).ok_or(Failure::Rejected)?;
            if cell
                .compare_exchange(current, target, Ordering::AcqRel, Ordering::Acquire)
                .is_ok()
            {
                return Ok(target);
            }
            std::hint::spin_loop();
        }
        Err(Failure::Conflict)
    }
}

/// Enumerates fixture rules outside measured sections.
#[cfg(any(feature = "fast", feature = "standard"))]
fn edges(mode: Mode) -> Vec<(u64, u64, u64)> {
    let baseline = AtomicDriver { mode };
    (0..7)
        .flat_map(|state| {
            (0..6).filter_map(move |event| {
                AtomicDriver { mode }
                    .lookup(state, event)
                    .map(|target| (state, event, target))
            })
        })
        .collect::<Vec<_>>()
        .into_iter()
        .inspect(|&(state, event, target)| {
            assert_eq!(baseline.lookup(state, event), Some(target));
        })
        .collect()
}

#[cfg(feature = "fast")]
impl Driver for FastStateMachine {
    type Cell = FastCasState;
    type Event = u64;
    type Source = u64;
    fn event(code: u64) -> u64 {
        code
    }
    fn source(code: u64) -> u64 {
        code
    }
    fn cell(&self) -> Self::Cell {
        self.create_state()
    }
    fn load(cell: &Self::Cell) -> u64 {
        cell.load()
    }
    fn lookup(&self, state: u64, event: u64) -> Option<u64> {
        self.transition_target(state, event)
    }
    fn apply(&self, cell: &Self::Cell, event: u64) -> Result<u64, Failure> {
        self.trigger(cell, event).map_err(|error| match error {
            FastStateMachineError::UnknownTransition { .. } => Failure::Rejected,
            FastStateMachineError::CasConflict { .. } => Failure::Conflict,
            FastStateMachineError::UnknownState { .. } => Failure::Unexpected,
        })
    }
}

/// Builds a raw machine with either a loop or the actual task lifecycle.
#[cfg(feature = "fast")]
fn raw_machine(mode: Mode) -> FastStateMachine {
    let mut builder = FastStateMachine::builder()
        .state_count(7)
        .event_count(6)
        .initial_state(0);
    if matches!(mode, Mode::Task) {
        builder = builder.terminal_states(&[2, 3, 4, 5, 6]);
    }
    for (source, event, target) in edges(mode) {
        builder = builder.transition(source, event, target);
    }
    builder.build().expect("benchmark rules are valid")
}

#[cfg(feature = "fast")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BenchState {
    Pending,
    Running,
    Succeeded,
    Failed,
    Panicked,
    Cancelled,
    Dropped,
}
#[cfg(feature = "fast")]
impl DenseCode for BenchState {
    const VALUES: &'static [Self] = &[
        Self::Pending,
        Self::Running,
        Self::Succeeded,
        Self::Failed,
        Self::Panicked,
        Self::Cancelled,
        Self::Dropped,
    ];
    fn code(self) -> u64 {
        self as u64
    }
}
#[cfg(feature = "fast")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BenchEvent {
    Start,
    CancelPending,
    CompleteSucceeded,
    CompleteFailed,
    CompletePanicked,
    DropUnfinished,
}
#[cfg(feature = "fast")]
impl DenseCode for BenchEvent {
    const VALUES: &'static [Self] = &[
        Self::Start,
        Self::CancelPending,
        Self::CompleteSucceeded,
        Self::CompleteFailed,
        Self::CompletePanicked,
        Self::DropUnfinished,
    ];
    fn code(self) -> u64 {
        self as u64
    }
}

#[cfg(feature = "fast")]
impl Driver for TypedFastStateMachine<BenchState, BenchEvent> {
    type Cell = TypedFastState<BenchState>;
    type Event = BenchEvent;
    type Source = BenchState;
    fn event(code: u64) -> BenchEvent {
        BenchEvent::VALUES[code as usize]
    }
    fn source(code: u64) -> BenchState {
        BenchState::VALUES[code as usize]
    }
    fn cell(&self) -> Self::Cell {
        self.create_state()
    }
    fn load(cell: &Self::Cell) -> u64 {
        cell.load().code()
    }
    fn lookup(&self, state: BenchState, event: BenchEvent) -> Option<u64> {
        self.transition_target(state, event).map(DenseCode::code)
    }
    fn apply(&self, cell: &Self::Cell, event: BenchEvent) -> Result<u64, Failure> {
        self.trigger(cell, event)
            .map(DenseCode::code)
            .map_err(|error| match error {
                TypedFastStateMachineError::Raw(FastStateMachineError::UnknownTransition { .. }) => Failure::Rejected,
                TypedFastStateMachineError::Raw(FastStateMachineError::CasConflict { .. }) => Failure::Conflict,
                _ => Failure::Unexpected,
            })
    }
}

/// Builds the typed form of the exact same benchmark rules.
#[cfg(feature = "fast")]
fn typed_machine(mode: Mode) -> TypedFastStateMachine<BenchState, BenchEvent> {
    let mut builder = TypedFastStateMachine::builder().initial_state(BenchState::Pending);
    if matches!(mode, Mode::Task) {
        builder = builder.terminal_states(&BenchState::VALUES[2..]);
    }
    for (source, event, target) in edges(mode) {
        builder = builder.transition(
            BenchState::VALUES[source as usize],
            BenchEvent::VALUES[event as usize],
            BenchState::VALUES[target as usize],
        );
    }
    builder.build().expect("typed benchmark rules are valid")
}

#[cfg(feature = "standard")]
impl Driver for StateMachine<u64, u64> {
    type Cell = AtomicRef<u64>;
    type Event = u64;
    type Source = u64;
    fn event(code: u64) -> u64 {
        code
    }
    fn source(code: u64) -> u64 {
        code
    }
    fn cell(&self) -> Self::Cell {
        self.create_state()
    }
    fn load(cell: &Self::Cell) -> u64 {
        *cell.load()
    }
    fn lookup(&self, state: u64, event: u64) -> Option<u64> {
        self.transition_target(state, event)
    }
    fn apply(&self, cell: &Self::Cell, event: u64) -> Result<u64, Failure> {
        self.trigger(cell, event).map_err(|error| match error {
            StateMachineError::UnknownTransition { .. } => Failure::Rejected,
            // Preserve all terminal CAS failures as failures, never as business rejection.
            StateMachineError::CasFailure { .. } => Failure::Conflict,
            StateMachineError::UnknownState { .. } => Failure::Unexpected,
        })
    }
}

/// Builds the generic form of the benchmark rules.
#[cfg(feature = "standard")]
fn standard_machine(mode: Mode) -> StateMachine<u64, u64> {
    let mut builder = StateMachine::builder()
        .add_states(&[0, 1, 2, 3, 4, 5, 6])
        .initial_state(0);
    if matches!(mode, Mode::Task) {
        builder = builder.terminal_states(&[2, 3, 4, 5, 6]);
    }
    for (source, event, target) in edges(mode) {
        builder = builder.transition(source, event, target);
    }
    builder.build().expect("generic benchmark rules are valid")
}

/// Local worker counters, read only after each worker joins.
#[derive(Debug, Default)]
struct Counts {
    committed: usize,
    rejected: usize,
    conflicts: usize,
    unexpected: usize,
}

/// Times a two-worker batch after spawning, then returns both worker counts.
fn measure_race<D: Driver>(driver: &D, cells: &[D::Cell], events: [D::Event; 2]) -> (Duration, [Counts; 2]) {
    let ready = Barrier::new(3);
    let start = Barrier::new(3);
    let finish = Barrier::new(3);
    let run = |worker: usize| {
        ready.wait();
        start.wait();
        let mut counts = Counts::default();
        for cell in cells {
            match driver.apply(cell, events[worker]) {
                Ok(_) => counts.committed += 1,
                Err(Failure::Rejected) => counts.rejected += 1,
                Err(Failure::Conflict) => counts.conflicts += 1,
                Err(Failure::Unexpected) => counts.unexpected += 1,
            }
        }
        finish.wait();
        counts
    };
    std::thread::scope(|scope| {
        let a = scope.spawn(|| run(0));
        let b = scope.spawn(|| run(1));
        ready.wait();
        let began = Instant::now();
        start.wait();
        finish.wait();
        let elapsed = began.elapsed();
        (
            elapsed,
            [
                a.join().expect("worker A finishes"),
                b.join().expect("worker B finishes"),
            ],
        )
    })
}

/// Benchmarks one implementation under identical setup and result checks.
fn benchmark_driver<D: Driver>(c: &mut Criterion, name: &str, alternating: D, self_loop: D, task: D) {
    let mut group = c.benchmark_group(name);
    let start_event = D::event(0);
    let success_event = D::event(2);
    let pending_state = D::source(0);
    group.sample_size(20);
    group.warm_up_time(Duration::from_millis(300));
    group.measurement_time(Duration::from_secs(1));
    group.bench_function("transition_target", |b| {
        b.iter(|| black_box(alternating.lookup(black_box(pending_state), black_box(start_event))))
    });
    let state = alternating.cell();
    group.bench_function("try_trigger_alternating", |b| {
        b.iter(|| {
            black_box(
                alternating
                    .apply(black_box(&state), black_box(start_event))
                    .expect("alternating update commits"),
            )
        })
    });
    let state = self_loop.cell();
    group.bench_function("self_transition", |b| {
        b.iter(|| {
            black_box(
                self_loop
                    .apply(black_box(&state), black_box(start_event))
                    .expect("self update commits"),
            )
        })
    });
    group.bench_function("task_lifecycle", |b| {
        let verification = task.cell();
        assert_eq!(task.apply(&verification, start_event).expect("start commits"), 1);
        assert_eq!(task.apply(&verification, success_event).expect("success commits"), 2);
        b.iter_batched(
            || task.cell(),
            |state| {
                let _ = black_box(task.apply(&state, black_box(start_event)));
                let _ = black_box(task.apply(&state, black_box(success_event)));
                black_box(state);
            },
            BatchSize::SmallInput,
        )
    });
    for (scenario, running, events) in [
        ("start_cancel_1024", false, [0, 1]),
        ("complete_drop_1024", true, [2, 5]),
    ] {
        group.bench_function(scenario, |b| {
            b.iter_custom(|iterations| {
                let mut duration = Duration::ZERO;
                for _ in 0..iterations {
                    let cells: Vec<_> = (0..1024)
                        .map(|_| {
                            let cell = task.cell();
                            if running {
                                assert_eq!(task.apply(&cell, start_event).expect("setup starts task"), 1);
                            }
                            cell
                        })
                        .collect();
                    let (elapsed, [a, b]) = measure_race(&task, &cells, events.map(D::event));
                    duration += elapsed;
                    assert_eq!(a.committed + b.committed, cells.len());
                    assert_eq!(a.rejected + b.rejected, cells.len());
                    assert_eq!(
                        a.conflicts + b.conflicts,
                        0,
                        "CAS failures invalidate a task graph sample"
                    );
                    assert_eq!(a.unexpected + b.unexpected, 0);
                    assert!(cells.iter().all(|cell| {
                        let state = D::load(cell);
                        if running {
                            matches!(state, 2 | 6)
                        } else {
                            matches!(state, 1 | 5)
                        }
                    }));
                }
                duration
            })
        });
    }
    group.finish();
}

/// Registers all available implementations without requiring other features.
fn benchmarks(c: &mut Criterion) {
    benchmark_driver(
        c,
        "atomic_baseline",
        AtomicDriver {
            mode: Mode::Alternating,
        },
        AtomicDriver { mode: Mode::SelfLoop },
        AtomicDriver { mode: Mode::Task },
    );
    #[cfg(feature = "fast")]
    {
        benchmark_driver(
            c,
            "fast_state_machine",
            raw_machine(Mode::Alternating),
            raw_machine(Mode::SelfLoop),
            raw_machine(Mode::Task),
        );
        benchmark_driver(
            c,
            "typed_fast_state_machine",
            typed_machine(Mode::Alternating),
            typed_machine(Mode::SelfLoop),
            typed_machine(Mode::Task),
        );
    }
    #[cfg(feature = "standard")]
    benchmark_driver(
        c,
        "standard_state_machine",
        standard_machine(Mode::Alternating),
        standard_machine(Mode::SelfLoop),
        standard_machine(Mode::Task),
    );
}

criterion_group!(benches, benchmarks);
criterion_main!(benches);
