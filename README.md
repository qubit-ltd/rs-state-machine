# Qubit State Machine

A small, thread-safe finite state machine for Rust.

This crate provides immutable transition rules and a `StateCell` wrapper for
atomic state changes guarded by a mutex. It is intended for simple lifecycle,
workflow, and task-state tracking code.
