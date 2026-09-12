# Qubit State Machine 用户手册

[English](user_guide.md) · 适用于 0.9 版本

## 手册目标与读者

本手册面向需要为任务、服务、设备或其他有限生命周期明确规定状态转换的 Rust 开发者。
内容覆盖公开的 `StateMachine`、`FastStateMachine` 和 `TypedFastStateMachine` API。
本库是有限状态机，不负责工作流调度，也不提供跨资源事务。

## 概念模型

构建完成的 machine 保存已注册状态、唯一初态、终态标记和转换规则，之后规则不可变。
每个任务单独持有一个当前状态单元；多个任务可以共享同一份 machine，而不会共享当前状态。

标准版把枚举风格的值存放在 `qubit_atomic::AtomicRef` 中，通过同步的 `qubit-cas` 执行更新。
Fast 版把连续的 `u64` 编码存放在 `qubit_fast_cas::FastCasState` 中；强类型 Fast 版还会根据
每种类型的 `DenseCode::VALUES` 校验编码。

## 贯穿场景：启动任务并记录审计

假设 worker 需要把任务从 `Queued` 转为 `Running`，并且只有状态提交成功后才能写入审计记录。
如果再次收到 `Start`，则应将其作为不允许的转换处理。

## 安装与最小配置

只使用标准版时，可以采用下面的依赖配置：

```toml
[dependencies]
qubit-state-machine = { version = "0.10", default-features = false, features = ["standard"] }
qubit-atomic = "0.13"
qubit-cas = "0.9"
```

项目要求 Rust 1.94 或更新版本。默认 feature 集同时启用 `standard` 和 `fast`。

## 核心工作流

先定义实现 `Copy + Eq + Hash + Debug` 的小型状态和事件类型，再注册状态、指定唯一初态、添加转换规则，
最后构建不可变规则表。每个任务创建自己的状态单元；需要在成功转换后记录审计时使用 `trigger_with`：

```rust
use qubit_atomic::AtomicRef;
use qubit_cas::CasExecutor;
use qubit_state_machine::StateMachine;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum JobState { Queued, Running }
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum JobEvent { Start }

let executor = CasExecutor::builder().max_attempts(1).no_delay()
    .build().expect("valid retry settings");
let machine = StateMachine::builder()
    .add_states(&[JobState::Queued, JobState::Running])
    .initial_state(JobState::Queued)
    .transition(JobState::Queued, JobEvent::Start, JobState::Running)
    .cas_executor(executor)
    .build().expect("valid transition table");
let state = AtomicRef::from_value(JobState::Queued);
let mut audit = Vec::new();

machine.trigger_with(&state, JobEvent::Start, |old, next| {
    audit.push((old, next));
}).expect("start transition commits");
assert_eq!(*state.load(), JobState::Running);
assert_eq!(audit, vec![(JobState::Queued, JobState::Running)]);
assert!(!machine.try_trigger(&state, JobEvent::Start));
```

运行结果是一次成功的状态提交和一条审计记录。调用方需要区分业务规则拒绝与 CAS 执行失败时，
使用 `trigger` 或 `trigger_with` 并检查错误；不需要区分时，使用 `try_trigger` 或 `try_trigger_with`，
它们会把失败压缩为 `false`。

## 进阶用法

`cas_executor` 用于注入已校验的同步 executor；`cas_strategy` 可选择 `LatencyFirst`、
`ContentionBackoff` 或 `ReliabilityFirst`。最后一次设置 executor 或 strategy 的调用生效。
默认配置为 16 次立即尝试，不设置操作或总墙钟预算。构建后可通过 `machine.cas_executor()` 查看实际配置，
包括 `max_attempts()`、`max_operation_elapsed()` 和 `max_total_elapsed()`。

当状态和事件可以编码为连续的 `u64`，且适合使用平铺转换表时，选择 `FastStateMachine`。
需要编译期区分状态和事件类型时，选择 `TypedFastStateMachine<S, E>`；其 `DenseCode::VALUES` 必须完整且不重复。
三种 machine 都提供 `diagnose_graph()` 做离线可达性分析，但该分析不会改变构建是否合法。

## 错误与诊断

构建错误涵盖缺少定义、目标不同的冲突重复转换、初态无效，以及违反状态注册或终态规则的转换。
标准版运行时会返回 `UnknownState`、`UnknownTransition` 或 `CasFailure`；后者保留 CAS 失败类别和尝试次数，
包括冲突或软性耗时预算耗尽的情况。Fast 版返回对应的 `FastStateMachineError` 变体，重试策略耗尽时为
`CasConflict`。强类型 Fast 版还会报告值不属于编码表的错误。

`trigger_with` 仅在提交成功后调用一次回调，自转换也不例外。回调 panic 会向上传播，状态不会回滚。
并发回调没有全局顺序，回调中重新读取状态还可能看到后续提交；记录审计时应使用回调参数中的 `old` 和 `new`。

## 排障

- `build` 失败：确认初态已注册；重复调用 `initial_state(...)` 是允许的，最后一次设置生效；终态没有出边，所有转换端点都已注册。
- 触发失败：先匹配详细错误，再判断是 CAS 冲突还是业务拒绝，以及是否适合重试。
- 延迟升高：检查同步重试延迟和竞争情况，使用有界策略并基于实际负载测量，不要无限增加重试次数。
- Fast 版本：确认所有编码都在声明的状态/事件范围内；强类型 Fast 版本还要确认 `code()` 与
  `DenseCode::VALUES` 中的索引一致。

## 限制与最佳实践

`build` 后规则不可变，但每个任务仍需独立的状态单元。CAS 只负责原子提交状态；结果投递、hook 顺序和业务负载的同步
由调用方负责。同步触发路径不提供异步执行或外部事务协调。本库适合有限状态转换，不是完整工作流引擎。

## 延伸阅读

- [README](../README.zh_CN.md)
- [English user guide](user_guide.md)
- [API 文档](https://docs.rs/qubit-state-machine)
