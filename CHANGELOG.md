# Changelog

## 0.9.0 - 2026-09-12

### Added

- Added `FastStateMachine` for dense integer-coded transitions.
- Added `TypedFastStateMachine<S, E>` and `DenseCode` for typed dense domains.
- Added graph reachability diagnostics through `diagnose_graph()`.
- Added configurable CAS policies, callback semantics, and detailed terminal
  CAS failure reporting.

### Compatibility notes

- The standard implementation remains generic over `Copy + Eq + Hash + Debug`
  state and event types.
- Repeated `initial_state(...)` calls are accepted; the last value wins.
- Rules are immutable after `build`; each machine-created state cell is
  independent, and externally created cells are not bound to a machine.
- Successful callbacks run after the state commit. Concurrent callback order is
  unspecified, callback re-entry is supported, and callback panics do not roll
  back the committed state.
