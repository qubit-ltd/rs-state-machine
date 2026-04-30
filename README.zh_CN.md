# Qubit State Machine（`rs-state-machine`）

[![CircleCI](https://circleci.com/gh/qubit-ltd/rs-state-machine.svg?style=shield)](https://circleci.com/gh/qubit-ltd/rs-state-machine)
[![Coverage Status](https://coveralls.io/repos/github/qubit-ltd/rs-state-machine/badge.svg?branch=main)](https://coveralls.io/github/qubit-ltd/rs-state-machine?branch=main)
[![Crates.io](https://img.shields.io/crates/v/qubit-state-machine.svg?color=blue)](https://crates.io/crates/qubit-state-machine)
[![Docs.rs](https://docs.rs/qubit-state-machine/badge.svg)](https://docs.rs/qubit-state-machine)
[![Rust](https://img.shields.io/badge/rust-1.94+-blue.svg?logo=rust)](https://www.rust-lang.org)
[![License](https://img.shields.io/badge/license-Apache%202.0-blue.svg)](LICENSE)
[![English Document](https://img.shields.io/badge/Document-English-blue.svg)](README.md)

文档：[API 文档](https://docs.rs/qubit-state-machine)

`qubit-state-machine` 是一个小型 Rust 有限状态机库，适用于生命周期、工作流和任务状态跟踪代码。

它提供不可变的状态转换规则、构建阶段校验，以及用于对共享状态应用事件的 CAS 支持 `AtomicRef`。

## 为什么使用

当你需要以下能力时，可以使用 `qubit-state-machine`：

- 用枚举风格的状态和事件类型显式定义有限状态机规则
- 在线程之间共享不可变的状态转换表
- 在构建阶段校验未知状态和冲突转换
- 通过 `trigger` 和 `try_trigger` 执行事件驱动的状态更新
- 在状态更新成功后通过回调观察旧状态和新状态
- 为服务、任务、设备或 UI 逻辑提供简单、无依赖的状态跟踪能力

## 安装

```toml
[dependencies]
qubit-state-machine = "0.1.0"
```

## 快速开始

```rust
use qubit_state_machine::{AtomicRef, StateMachine};

#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
enum JobState {
    New,
    Running,
    Done,
    Failed,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
enum JobEvent {
    Start,
    Finish,
    Fail,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut builder = StateMachine::builder();
    builder.add_states(&[
        JobState::New,
        JobState::Running,
        JobState::Done,
        JobState::Failed,
    ]);
    builder.set_initial_state(JobState::New);
    builder.set_final_states(&[JobState::Done, JobState::Failed]);
    builder.add_transition(JobState::New, JobEvent::Start, JobState::Running);
    builder.add_transition(JobState::Running, JobEvent::Finish, JobState::Done);
    builder.add_transition(JobState::Running, JobEvent::Fail, JobState::Failed);

    let machine = builder.build()?;
    let state = AtomicRef::from_value(JobState::New);

    let running = machine.trigger(&state, JobEvent::Start)?;
    assert_eq!(running, JobState::Running);
    assert_eq!(*state.load(), JobState::Running);

    let finished = machine.trigger_with(&state, JobEvent::Finish, |old_state, new_state| {
        println!("state changed: {old_state:?} -> {new_state:?}");
    })?;
    assert_eq!(finished, JobState::Done);

    Ok(())
}
```

## 后续阅读

| 任务 | API |
| --- | --- |
| 定义状态和转换 | `StateMachine::builder`、`StateMachineBuilder` |
| 添加一个或多个状态 | `add_state`、`add_states` |
| 标记初始状态和最终状态 | `set_initial_state`、`set_initial_states`、`set_final_state`、`set_final_states` |
| 添加状态转换规则 | `add_transition`、`add_transition_value`、`Transition` |
| 只查询转换目标，不修改当前状态 | `transition_target` |
| 应用事件并获取详细错误 | `trigger`、`trigger_with`、`StateMachineError` |
| 应用事件但不处理错误详情 | `try_trigger`、`try_trigger_with` |
| 存储共享可变状态 | `AtomicRef` |

## 核心 API 概览

| 类型 | 用途 |
| --- | --- |
| `Transition` | 描述 `source --event--> target` 的不可变值。 |
| `StateMachineBuilder` | 用于定义状态、初始状态、最终状态和转换规则的可变构建器。 |
| `StateMachine` | 已校验的不可变转换表，用于查询和触发事件。 |
| `AtomicRef` | 重新导出的原子引用，用作 CAS 支持的当前状态存储。 |
| `StateMachineBuildError` | 构建无效规则集时返回的校验错误。 |
| `StateMachineError` | 事件无法应用到当前状态时返回的运行时错误。 |

## 项目范围

- `qubit-state-machine` 面向简单有限状态机，不是完整工作流引擎。
- 状态和事件类型应是小型枚举风格值，并实现 `Copy + Eq + Hash + Debug`。
- 规则定义在 `StateMachineBuilder::build` 之后变为不可变。
- `trigger` 直接接受 `AtomicRef<S>`。
- 事件驱动的转换通过 `qubit-cas` 安装。
- 回调会在 CAS 更新成功后执行。

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
