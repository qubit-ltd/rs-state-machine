# Migrating to qubit-state-machine 0.8

[中文版](migration-0.8.zh_CN.md).

The standard implementation now depends on qubit-cas 0.13 and accepts a complete
executor through StateMachineBuilder::cas_executor. Direct CAS consumers must
upgrade qubit-cas and rename ContentionAdaptive to ContentionBackoff. No old-name
aliases remain. cas_strategy still works; the last executor/strategy setter wins.

## Unreleased changes after 0.8

`StateMachine::cas_strategy()` is removed. The builder's `cas_strategy(...)` remains,
while the installed executor is inspected through `cas_executor().retry_policy()`.
Typed Fast errors gain classification helpers and all three entries gain
`diagnose_graph()`. These are unreleased source changes, not features of the
published 0.8 package.

StateMachineError::CasFailure keeps kind and attempts. Business-error and
post-commit callback semantics remain unchanged. Fast-only mode still uses
qubit-fast-cas without enabling qubit-cas. See the [guide](user_guide.md).
