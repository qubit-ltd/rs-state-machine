# Qubit State Machine（`rs-state-machine`）

[![Rust CI](https://github.com/qubit-ltd/rs-state-machine/actions/workflows/ci.yml/badge.svg)](https://github.com/qubit-ltd/rs-state-machine/actions/workflows/ci.yml)
[![Coverage](https://img.shields.io/endpoint?url=https://qubit-ltd.github.io/rs-state-machine/coverage-badge.json)](https://qubit-ltd.github.io/rs-state-machine/coverage/)
[![Crates.io](https://img.shields.io/crates/v/qubit-state-machine.svg?color=blue)](https://crates.io/crates/qubit-state-machine)
[![Rust](https://img.shields.io/badge/rust-1.94+-blue.svg?logo=rust)](https://www.rust-lang.org)
[![License](https://img.shields.io/badge/license-Apache%202.0-blue.svg)](LICENSE)
[![English Document](https://img.shields.io/badge/Document-English-blue.svg)](README.md)

标准状态机使用 `qubit-cas` 0.11，通过 `cas_executor` 注入次数、预算及退避配置；
也可用 builder 上的 `cas_strategy` 选择预设。实际配置通过
executor 的 `max_attempts()`、`max_operation_elapsed()` 和 `max_total_elapsed()` 查询。CAS 终止类型保留在
`StateMachineError::CasFailure` 中。

`qubit-state-machine` 是一个小型 Rust 有限状态机库，适用于生命周期、工作流和任务状态跟踪代码。

0.10 版本要求配置有效初态；如果多次调用 `initial_state(...)`，最后一次设置生效。终态不能有出边。`create_state()` 创建独立的当前状态单元，外部创建的单元不绑定到某个 machine。回调在成功提交后执行一次；并发回调顺序不保证，回调可能观察到已经超出 `new_state` 参数的后续状态。

标准版默认使用 16 次立即 CAS 尝试，不设置墙钟时间预算。`CasStrategy::LatencyFirst` 使用较小的立即重试预算和软性耗时限制；同步执行一旦开始某次尝试，实际耗时仍可能超过软性预算。CAS 终止失败仍与业务上的未定义转换区分。

它提供不可变的状态转换规则和构建阶段校验。标准版通过 `qubit-cas` 更新
`qubit_atomic::AtomicRef`，Fast 版则直接更新
`qubit_fast_cas::FastCasState`。

库内同时提供三个公开入口：

- `StateMachine`：适合可读性优先、以枚举语义建模状态/事件的场景。
- `FastStateMachine`：适合高吞吐、热点路径对延迟要求更严格的场景。
- `TypedFastStateMachine<S, E>`：在 Fast 内核上提供编译期区分的状态与事件类型。

这三个入口都在构建后冻结转换规则，并通过 CAS 机制更新共享状态；均提供
`diagnose_graph()` 做离线可达性分析。标准版的迭代顺序不保证，Fast 版本按 code 顺序返回。

## 为什么使用

当你需要以下能力时，可以使用 `qubit-state-machine`：

- 用枚举风格的状态和事件类型显式定义有限状态机规则
- 在线程之间共享不可变的状态转换表
- 在构建阶段校验未知状态和冲突转换
- 通过 `trigger` 和 `try_trigger` 执行事件驱动的状态更新
- 在状态更新成功后通过回调观察旧状态和新状态
- 为服务、任务、设备或 UI 逻辑提供简单、轻量的状态跟踪能力
- 在高频触发场景中使用 `FastStateMachine` 获取更紧凑的转移性能

## 强类型 Fast 状态机

任务生命周期使用 `TypedFastStateMachine<S, E>` 时，状态与事件在编译期区分，数量由 `DenseCode::VALUES` 推导。`code()` 必须稳定返回值在数组中的索引；每个可用值列出一次。构建器检查编码表，运行时也检查输入是否属于该表；反向转换使用安全索引，无需实现 `from_code` 或使用 `transmute`。

```rust
use qubit_state_machine::{DenseCode, TypedFastStateMachine};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum State { Pending, Running, Done }
impl DenseCode for State {
    const VALUES: &'static [Self] = &[Self::Pending, Self::Running, Self::Done];
    fn code(self) -> u64 { self as u64 }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Event { Start, Finish }
impl DenseCode for Event {
    const VALUES: &'static [Self] = &[Self::Start, Self::Finish];
    fn code(self) -> u64 { self as u64 }
}

let machine = TypedFastStateMachine::<State, Event>::builder()
    .initial_state(State::Pending)
    .terminal_state(State::Done)
    .transition(State::Pending, Event::Start, State::Running)
    .transition(State::Running, Event::Finish, State::Done)
    .build().expect("valid job definition");
let state = machine.create_state();
assert_eq!(machine.trigger(&state, Event::Start).expect("start"), State::Running);
assert!(machine.try_trigger(&state, Event::Finish));
assert!(machine.is_terminal_state(state.load()));
```

| 入口 | 适用场景与成本 |
| --- | --- |
| `StateMachine` | 泛型 `Copy + Eq + Hash + Debug` 状态；适合稀疏值、已有 `AtomicRef` 或需要完整 CAS 配置的场景。 |
| `FastStateMachine` | 原始整数协议；调用方提供连续 code 范围，稠密查表与整数 CAS。 |
| `TypedFastStateMachine` | 枚举生命周期；复用同一 Fast 内核并检查类型和值表，不额外分配每次转换的堆对象。实际性能以 benchmark 为准。 |

三种入口均保留不可变规则与独立状态单元。原始 Fast 是正式的整数入口，不是兼容适配层。强类型状态单元只能由机器创建，公开读而不公开 setter/raw 单元；同一种状态类型的机器可共用单元，没有机器身份绑定。

## 并发与错误边界

`trigger` 返回详细错误；`try_trigger`/`try_trigger_with` 将未知状态、未定义转换和 CAS 耗尽都压成 `false`。对必须区分业务拒绝与执行失败的路径，使用 `trigger` 并匹配错误。强类型 Fast 的运行时错误直接区分事件编码无效、未定义转换、CAS 冲突和状态编码无效；只有构建错误通过 `TypedFastStateMachineBuildError::Raw` 保留底层 Fast 构建错误。

CAS 只原子提交状态；冲突重试会针对新观察状态重新计算该事件的后继，不保证仍从最早读到的状态出发，也不保证检测循环中的 ABA。回调在提交后执行一次，允许重入，并发顺序不保证；回调可能看到更晚的状态，但参数和返回值仍描述本次提交。回调 panic 会传播，状态不会回滚。

结果发送、hook 排序和业务 payload 的同步由调用方维护。例如 executor 的 `is_done()` 表示已经进入终态，结果仍可能正在发布；它不等价于此刻非阻塞取结果一定成功。需要联合维护队列、计数和生命周期的 monitor 状态，不宜拆成一个独立原子状态机。

## 安装

默认 feature 集同时包含标准版和 Fast 版。原子状态类型必须从其所属 crate
直接导入：

```toml
[dependencies]
qubit-state-machine = "0.10"
qubit-atomic = "0.17"
qubit-fast-cas = "0.3"
```

仅使用标准版：

```toml
[dependencies]
qubit-state-machine = { version = "0.10", default-features = false, features = ["standard"] }
qubit-atomic = "0.17"
```

仅使用 Fast 版，并避免引入 `qubit-cas`：

```toml
[dependencies]
qubit-state-machine = { version = "0.10", default-features = false, features = ["fast"] }
qubit-fast-cas = "0.3"
```

从全新克隆运行仓库本地 CI 包装脚本前，先初始化已固定版本的 CI 子模块：

```bash
git submodule update --init --recursive
./ci-check.sh
```

## 快速开始：任务处理

```rust
use qubit_atomic::AtomicRef;
use qubit_state_machine::StateMachine;

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
        .initial_state(JobState::Queued)
        .terminal_states(&[JobState::Succeeded, JobState::Failed])
        .transition(JobState::Queued, JobEvent::Start, JobState::Running)
        .transition(JobState::Running, JobEvent::Complete, JobState::Succeeded)
        .transition(JobState::Running, JobEvent::Fail, JobState::Failed)
        .build()?)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let machine = create_job_machine()?;

    assert!(machine.contains_state(JobState::Running));
    assert!(machine.is_initial_state(JobState::Queued));
    assert!(machine.is_terminal_state(JobState::Succeeded));
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

如果是小型、固定的枚举生命周期，优先使用 `TypedFastStateMachine`；它保留枚举类型安全，
同时使用紧凑整数状态。

如果状态空间稀疏、已有 `AtomicRef`，或需要完整的 `qubit-cas` 配置，使用 `StateMachine`。

如果你面对的是高频触发路径、并且状态和事件可以表达为稠密 `u64` 编码，
使用 `FastStateMachine`。它通过可计算下标的扁平转移表换取更稳定的热点路径性能。

三种入口都支持 `diagnose_graph()`。该方法只做离线可达性分析，不改变构建合法性，
并报告不可达状态和无法到达显式终态的状态；无终态模型仍可构建。

## Fast State Machine（高性能模式）

`FastStateMachine` 使用稠密的整数编码。它要求你显式声明状态数和事件数，
并在构建时一次性校验完整转移表，适合高频状态转换场景。运行时转换查找采用
行优先（row-major）布局，索引计算为：
`index = source * event_count + event`。

```rust
# fn main() -> Result<(), Box<dyn std::error::Error>> {
use qubit_fast_cas::{FastCasPolicy, FastCasState};
use qubit_state_machine::{
    FAST_STATE_MACHINE_DEFAULT_CAS_POLICY,
    FastStateMachine,
};

const QUEUED: u64 = 0;
const RUNNING: u64 = 1;
const SUCCEEDED: u64 = 2;
const FAILED: u64 = 3;
const START: u64 = 0;
const COMPLETE: u64 = 1;
const FAIL: u64 = 2;

let machine = FastStateMachine::builder()
    .state_count(4)
    .event_count(3)
    .initial_state(QUEUED)
    .terminal_states(&[SUCCEEDED, FAILED])
    .transition(QUEUED, START, RUNNING)
    .transition(RUNNING, COMPLETE, SUCCEEDED)
    .transition(RUNNING, FAIL, FAILED)
    .build()?;

let tuned = FastStateMachine::builder()
    .state_count(4)
    .event_count(3)
    .initial_state(QUEUED)
    .terminal_states(&[SUCCEEDED, FAILED])
    .transition(QUEUED, START, RUNNING)
    .transition(RUNNING, COMPLETE, SUCCEEDED)
    .transition(RUNNING, FAIL, FAILED)
    .cas_policy(FastCasPolicy::spin(8))
    .build()?;

let state = FastCasState::new(QUEUED);
assert_eq!(machine.trigger(&state, START)?, RUNNING);
let tuned_state = FastCasState::new(RUNNING);
assert_eq!(tuned.trigger(&tuned_state, COMPLETE)?, SUCCEEDED);
assert_eq!(machine.transition_target(QUEUED, START), Some(RUNNING));
assert_eq!(machine.state_count(), 4);
assert_eq!(machine.event_count(), 3);
assert!(machine.is_initial_state(QUEUED));
assert!(machine.is_terminal_state(SUCCEEDED));
assert_eq!(machine.cas_policy(), FAST_STATE_MACHINE_DEFAULT_CAS_POLICY);
assert_eq!(tuned.cas_policy(), FastCasPolicy::spin(8));
# Ok(())
# }
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
    .initial_state(JobState::Queued)
    .transition(JobState::Queued, JobEvent::Start, JobState::Running)
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
use qubit_atomic::AtomicRef;
use qubit_state_machine::StateMachine;

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
    .initial_state(DoorState::Open)
    .transition(DoorState::Open, DoorEvent::Close, DoorState::Closed)
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
| 标记唯一初态和终态 | `initial_state`、`terminal_state`、`terminal_states` |
| 添加状态转换规则 | `transition`、`transition_value`、`Transition` |
| 只查询转换目标，不修改当前状态 | `transition_target` |
| 应用事件并获取详细错误 | `trigger`、`trigger_with`、`StateMachineError` |
| 应用事件但不处理错误详情 | `try_trigger`、`try_trigger_with` |
| 存储共享可变状态 | `qubit_atomic::AtomicRef` 或 `qubit_fast_cas::FastCasState` |

需要完整了解任务生命周期、重试策略、错误处理和排障步骤时，请阅读[中文用户手册](doc/user_guide.zh_CN.md)
或 [English user guide](doc/user_guide.md)。完整的公共 API 说明请参阅生成的 [API 文档](https://docs.rs/qubit-state-machine)。

## 核心 API 概览

| 类型 | 用途 |
| --- | --- |
| `Transition` | 描述 `source --event--> target` 的不可变值。 |
| `FastStateMachine` | 使用稠密整数编码和平铺转换表；默认容量上限为 `1_048_576` 个单元格。 |
| `FastStateMachineBuilder` | 用于声明状态数、事件数、转移表和 CAS 策略。 |
| `FastStateMachineError` | `FastStateMachine` 的运行时错误。 |
| `FastStateMachineBuildError` | 构建 `FastStateMachine` 时的配置校验错误。 |
| `StateMachineBuilder` | 用于定义状态、初始状态、最终状态和转换规则的可变构建器。 |
| `StateMachine` | 已校验的不可变转换表，用于查询和触发事件。 |
| `StateMachineBuildError` | 构建无效规则集时返回的校验错误。 |
| `StateMachineError` | 事件无法应用到当前状态时返回的运行时错误。 |

## 项目范围

- `qubit-state-machine` 面向简单有限状态机，不是完整工作流引擎。
- 状态和事件类型应是小型枚举风格值，并实现 `Copy + Eq + Hash + Debug`。
- Fast 版本要求状态码/事件码是连续的 `u64`，且位于
  `[0, state_count)`、`[0, event_count)`，转移表容量固定为
  `state_count * event_count`。
- 规则定义在 `StateMachineBuilder::build` 之后变为不可变。
- 标准版转换通过 `AtomicRef<S>` 与 `qubit-cas` CAS 机制执行更新。
- 回调只会在 CAS 更新成功后执行。
- `FastStateMachine` 采用紧凑整数编码和平铺转移表，适合性能敏感路径。

## Rust 版本

本 crate 使用 Rust 2024 edition，要求 Rust 1.94 或更新版本。

## 依赖项

运行时依赖按 feature 隔离：

- `thiserror` 用于实现具体错误类型。
- `standard` 启用 `qubit-atomic` 和 `qubit-cas`。
- `fast` 仅启用 `qubit-fast-cas`。

默认 feature 集同时启用 `standard` 和 `fast`。

## 测试

```bash
# 使用默认 feature 集运行测试
cargo test

# 使用项目声明的全部 feature 运行测试
cargo test --all-features

# 运行项目 CI 检查
./ci-check.sh

# 检查代码覆盖率
./coverage.sh
```

## 许可证

Copyright (c) 2025 - 2026. Haixing Hu. All rights reserved.

本项目基于 Apache License 2.0 授权。完整许可证文本请参阅
[LICENSE](LICENSE)。

## 贡献

欢迎贡献。请遵循 Rust API 指南，及时更新公共 API 文档与测试，并在提交
Pull Request 前运行 `./align-ci.sh`格式化代码，运行`./ci-check.sh`对齐CI要求。

## 作者

**Haixing Hu** - *Qubit Co. Ltd.*

仓库地址：[https://github.com/qubit-ltd/rs-state-machine](https://github.com/qubit-ltd/rs-state-machine)
