# Qubit State Machine（`rs-state-machine`）

[![Rust CI](https://github.com/qubit-ltd/rs-state-machine/actions/workflows/ci.yml/badge.svg)](https://github.com/qubit-ltd/rs-state-machine/actions/workflows/ci.yml)
[![Coverage Status](https://coveralls.io/repos/github/qubit-ltd/rs-state-machine/badge.svg?branch=main)](https://coveralls.io/github/qubit-ltd/rs-state-machine?branch=main)
[![Crates.io](https://img.shields.io/crates/v/qubit-state-machine.svg?color=blue)](https://crates.io/crates/qubit-state-machine)
[![Docs.rs](https://docs.rs/qubit-state-machine/badge.svg)](https://docs.rs/qubit-state-machine)
[![Rust](https://img.shields.io/badge/rust-1.94+-blue.svg?logo=rust)](https://www.rust-lang.org)
[![License](https://img.shields.io/badge/license-Apache%202.0-blue.svg)](LICENSE)
[![English Document](https://img.shields.io/badge/Document-English-blue.svg)](README.md)

文档：[API 文档](https://docs.rs/qubit-state-machine)

`qubit-state-machine` 是一个小型 Rust 有限状态机库，适用于生命周期、工作流和任务状态跟踪代码。

它提供不可变的状态转换规则、构建阶段校验，以及用于对共享状态应用事件的
CAS 支持 `AtomicRef`。

库内同时提供两种实现方式：

- `StateMachine`：适合可读性优先、以枚举语义建模状态/事件的场景。
- `FastStateMachine`：适合高吞吐、热点路径对延迟要求更严格的场景。

这两种实现都在构建后冻结转换规则，并通过 CAS 机制更新共享状态。

## 为什么使用

当你需要以下能力时，可以使用 `qubit-state-machine`：

- 用枚举风格的状态和事件类型显式定义有限状态机规则
- 在线程之间共享不可变的状态转换表
- 在构建阶段校验未知状态和冲突转换
- 通过 `trigger` 和 `try_trigger` 执行事件驱动的状态更新
- 在状态更新成功后通过回调观察旧状态和新状态
- 为服务、任务、设备或 UI 逻辑提供简单、轻量的状态跟踪能力
- 在高频触发场景中使用 `FastStateMachine` 获取更紧凑的转移性能

## 安装

```toml
[dependencies]
qubit-state-machine = "0.2"
```

## 快速开始：任务处理

```rust
use qubit_state_machine::{AtomicRef, StateMachine};

#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
enum JobState {
    Queued,
    Running,
    Succeeded,
    Failed,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
enum JobEvent {
    Start,
    Complete,
    Fail,
}

fn create_job_machine() -> Result<StateMachine<JobState, JobEvent>, Box<dyn std::error::Error>> {
    Ok(StateMachine::builder()
        .add_states(&[
            JobState::Queued,
            JobState::Running,
            JobState::Succeeded,
            JobState::Failed,
        ])
        .set_initial_state(JobState::Queued)
        .set_final_states(&[JobState::Succeeded, JobState::Failed])
        .add_transition(JobState::Queued, JobEvent::Start, JobState::Running)
        .add_transition(JobState::Running, JobEvent::Complete, JobState::Succeeded)
        .add_transition(JobState::Running, JobEvent::Fail, JobState::Failed)
        .build()?)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let machine = create_job_machine()?;

    assert!(machine.contains_state(JobState::Running));
    assert!(machine.is_initial_state(JobState::Queued));
    assert!(machine.is_final_state(JobState::Succeeded));
    assert_eq!(
        machine.transition_target(JobState::Queued, JobEvent::Start),
        Some(JobState::Running),
    );

    let state = AtomicRef::from_value(JobState::Queued);
    let running = machine.trigger(&state, JobEvent::Start)?;
    assert_eq!(running, JobState::Running);
    assert_eq!(*state.load(), JobState::Running);

    let mut audit_log = Vec::new();
    let finished = machine.trigger_with(&state, JobEvent::Complete, |old_state, new_state| {
        audit_log.push((old_state, new_state));
    })?;
    assert_eq!(finished, JobState::Succeeded);
    assert_eq!(audit_log, vec![(JobState::Running, JobState::Succeeded)]);

    assert!(!machine.try_trigger(&state, JobEvent::Fail));
    assert_eq!(*state.load(), JobState::Succeeded);

    Ok(())
}
```

## 标准版与高性能版如何选

如果你的模型天然适合枚举表达，且优先考虑代码可读性和业务语义清晰度，
优先使用 `StateMachine`。

如果你面对的是高频触发路径、并且状态和事件可以表达为稠密 `usize` 编码，
优先使用 `FastStateMachine`。它通过可计算下标的扁平转移表换取更稳定的热点路径
性能。

## Fast State Machine（高性能模式）

`FastStateMachine` 使用稠密的整数编码。它要求你显式声明状态数和事件数，
并在构建时一次性校验完整转移表，适合高频状态转换场景。运行时转换查找采用
行优先（row-major）布局，索引计算为：
`index = source * event_count + event`。

```rust
use qubit_state_machine::{
    FAST_STATE_MACHINE_DEFAULT_CAS_POLICY,
    FastCasPolicy,
    FastStateMachine,
};

const QUEUED: usize = 0;
const RUNNING: usize = 1;
const SUCCEEDED: usize = 2;
const FAILED: usize = 3;
const START: usize = 0;
const COMPLETE: usize = 1;
const FAIL: usize = 2;

let machine = FastStateMachine::builder()
    .state_count(4)
    .event_count(3)
    .initial_state(QUEUED)
    .final_states(&[SUCCEEDED, FAILED])
    .transition(QUEUED, START, RUNNING)
    .transition(RUNNING, COMPLETE, SUCCEEDED)
    .transition(RUNNING, FAIL, FAILED)
    .build()?;

let tuned = FastStateMachine::builder()
    .state_count(4)
    .event_count(3)
    .initial_state(QUEUED)
    .final_states(&[SUCCEEDED, FAILED])
    .transition(QUEUED, START, RUNNING)
    .transition(RUNNING, COMPLETE, SUCCEEDED)
    .transition(RUNNING, FAIL, FAILED)
    .cas_policy(FastCasPolicy::spin(8))
    .build()?;

let state = qubit_cas::FastCasState::new(QUEUED);
assert_eq!(machine.trigger(&state, START)?, RUNNING);
let tuned_state = qubit_cas::FastCasState::new(RUNNING);
assert_eq!(tuned.trigger(&tuned_state, COMPLETE)?, SUCCEEDED);
assert_eq!(machine.transition_target(QUEUED, START), Some(RUNNING));
assert_eq!(machine.state_count(), 4);
assert_eq!(machine.event_count(), 3);
assert!(machine.is_initial_state(QUEUED));
assert!(machine.is_final_state(SUCCEEDED));
assert_eq!(machine.cas_policy(), FAST_STATE_MACHINE_DEFAULT_CAS_POLICY);
assert_eq!(tuned.cas_policy(), FastCasPolicy::spin(8));
```

默认不显式设置时会使用 `FAST_STATE_MACHINE_DEFAULT_CAS_POLICY`，如需调优可通过
`.cas_policy(...)` 自定义。

## 构建阶段校验

无效规则会在创建 `StateMachine` 前被拒绝。

```rust
use qubit_state_machine::{StateMachine, StateMachineBuildError};

#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
enum JobState {
    Queued,
    Running,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
enum JobEvent {
    Start,
}

let error = StateMachine::builder()
    .add_state(JobState::Queued)
    .add_transition(JobState::Queued, JobEvent::Start, JobState::Running)
    .build()
    .expect_err("transition target must be registered");

assert_eq!(
    error,
    StateMachineBuildError::TransitionTargetNotRegistered {
        source_state: JobState::Queued,
        event: JobEvent::Start,
        target: JobState::Running,
    },
);
```

## 不关心错误详情时应用事件

当非法转换只需要返回 `false` 时，可以使用 `try_trigger` 或
`try_trigger_with`。

```rust
use qubit_state_machine::{AtomicRef, StateMachine};

#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
enum DoorState {
    Open,
    Closed,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
enum DoorEvent {
    Close,
    Reopen,
}

let machine = StateMachine::builder()
    .add_states(&[DoorState::Open, DoorState::Closed])
    .add_transition(DoorState::Open, DoorEvent::Close, DoorState::Closed)
    .build()
    .expect("rules should build");
let state = AtomicRef::from_value(DoorState::Open);

assert!(machine.try_trigger(&state, DoorEvent::Close));
assert!(!machine.try_trigger_with(&state, DoorEvent::Reopen, |_, _| {
    unreachable!("callback is skipped when transition fails");
}));

assert_eq!(*state.load(), DoorState::Closed);
```

## 后续阅读

| 任务 | API |
| --- | --- |
| 定义状态和转换 | `StateMachine::builder`、`StateMachineBuilder` |
| 定义高性能状态机 | `FastStateMachine::builder`、`FastStateMachineBuilder` |
| 添加一个或多个状态 | `StateMachineBuilder::add_state`、`StateMachineBuilder::add_states` |
| 配置高性能状态/事件空间 | `FastStateMachineBuilder::state_count`、`FastStateMachineBuilder::event_count` |
| 标记初始状态和最终状态 | `set_initial_state`、`set_initial_states`、`set_final_state`、`set_final_states`、`initial_state`、`initial_states`、`final_state`、`final_states` |
| 添加状态转换规则 | `add_transition`、`add_transition_value`、`Transition` |
| 只查询转换目标，不修改当前状态 | `transition_target` |
| 应用事件并获取详细错误 | `trigger`、`trigger_with`、`StateMachineError` |
| 应用事件但不处理错误详情 | `try_trigger`、`try_trigger_with` |
| 存储共享可变状态 | `AtomicRef` |

## 核心 API 概览

| 类型 | 用途 |
| --- | --- |
| `Transition` | 描述 `source --event--> target` 的不可变值。 |
| `FastStateMachine` | 针对整数编码场景的高吞吐状态机。 |
| `FastStateMachineBuilder` | 用于声明状态数、事件数、转移表和 CAS 策略。 |
| `FastStateMachineError` | `FastStateMachine` 的运行时错误。 |
| `FastStateMachineBuildError` | 构建 `FastStateMachine` 时的配置校验错误。 |
| `FastCasPolicy` | 控制 `FastStateMachine` 并发冲突时重试行为的策略。 |
| `StateMachineBuilder` | 用于定义状态、初始状态、最终状态和转换规则的可变构建器。 |
| `StateMachine` | 已校验的不可变转换表，用于查询和触发事件。 |
| `AtomicRef` | 重新导出的原子引用，用作 CAS 支持的当前状态存储。 |
| `StateMachineBuildError` | 构建无效规则集时返回的校验错误。 |
| `StateMachineError` | 事件无法应用到当前状态时返回的运行时错误。 |

## 项目范围

- `qubit-state-machine` 面向简单有限状态机，不是完整工作流引擎。
- 状态和事件类型应是小型枚举风格值，并实现 `Copy + Eq + Hash + Debug`。
- Fast 版本要求状态码/事件码是连续的 `usize`，且位于
  `[0, state_count)`、`[0, event_count)`，转移表容量固定为
  `state_count * event_count`。
- 规则定义在 `StateMachineBuilder::build` 之后变为不可变。
- 标准版转换通过 `AtomicRef<S>` 与 `qubit-cas` CAS 机制执行更新。
- 回调只会在 CAS 更新成功后执行。
- `FastStateMachine` 采用紧凑整数编码和平铺转移表，适合性能敏感路径。

## 贡献

欢迎提交 issue 和 pull request。

为了让维护和评审更顺畅，请尽量遵循以下约定：

- bug 报告、设计问题或较大的功能建议，先提交 issue 讨论
- pull request 尽量聚焦一个行为变更、问题修复或文档更新
- 提交前运行 `./ci-check.sh`
- 修改运行时行为时，请补充相应测试
- 公共 API 行为变化时，请同步更新 README

向本项目提交贡献，即表示你同意该贡献使用与本项目相同的许可证。

## 许可证

本项目使用 [Apache License, Version 2.0](LICENSE) 许可证。
