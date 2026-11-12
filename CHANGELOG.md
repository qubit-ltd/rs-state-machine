# Changelog

## Unreleased

### Changed

- Clarified graph diagnostic ordering and equality semantics in Rustdoc, with a
  sorted comparison example. (current revision)

## 0.11.3

### Changed

- Exposed shared fuzz harness modules for reuse by fuzz targets. (8962794)
- Refreshed dependency lockfiles. (da6cea1)

## 0.11.2

### Changed

- Installed and ran the repository style analyzer in CI. (682e0ce)
- Removed an unused import allowance from the differential fuzz target.
  (8beed3d)
- Refreshed dependency lockfiles. (770c433)

## 0.11.1

### Changed

- Clarified state machine contracts and streamlined implementation paths,
  including graph diagnostics, typed Fast behavior, and related tests.
  (3e297f8)
- Refreshed dependency lockfiles for the 0.11.1 package version. (f6c4b0c)

## 0.11.0

### Changed

- Updated the Standard implementation's `qubit-cas` dependency from 0.11 to
  0.12.

### Compatibility notes

- Applications injecting a Standard `CasExecutor` through `cas_executor(...)`
  should use `qubit-cas = "0.12"` so the executor type matches this crate's
  dependency, and adapt their calls to that dependency's public API.
- Fast-only users do not need the Standard CAS dependency for this upgrade.

## 0.10.0 - 2026-09-13

### Changed

- Typed Fast runtime errors now carry typed state and event values; the former
  `Raw` wrapper and conversion are removed.
- Fast builders enforce a configurable dense table cell limit, defaulting to
  `FAST_STATE_MACHINE_DEFAULT_MAX_TABLE_CELLS`.
- Restored error projection coverage and removed an undiscovered test module.

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
