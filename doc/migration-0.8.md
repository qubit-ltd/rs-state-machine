# Migrating to qubit-state-machine 0.8

[中文版](migration-0.8.zh_CN.md).

The standard implementation now depends on qubit-cas 0.13 and accepts a complete
executor through StateMachineBuilder::cas_executor. Direct CAS consumers must
upgrade qubit-cas and rename ContentionAdaptive to ContentionBackoff. No old-name
aliases remain. cas_strategy still works; the last executor/strategy setter wins.

StateMachineError::CasFailure keeps kind and attempts. Business-error and
post-commit callback semantics remain unchanged. Fast-only mode still uses
qubit-fast-cas without enabling qubit-cas. See the [guide](user_guide.md).
