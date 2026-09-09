# Qubit State Machine 用户指南

适用于 0.8。[English](user_guide.md)。本指南面向需要明确任务生命周期规则的 Rust 应用。

## 模型与安装

转换表创建后不可变。标准版在 AtomicRef 中存储 enum 风格状态，Fast 版使用紧凑整数状态。
两者均要求一个初态，终态不能有出边。状态单元与规则表独立；每个任务应有自己的状态单元。

```toml
[dependencies]
qubit-state-machine = { version = "0.8", default-features = false, features = ["standard"] }
qubit-atomic = "0.13"
qubit-cas = "0.13"
```

## 任务启动与审计

初始任务处于 Queued；Start 将其提交为 Running。提交成功后记录审计，重复 Start 则失败。

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
machine.trigger_with(&state, JobEvent::Start, |old, next| audit.push((old, next)))
    .expect("start transition commits");
assert_eq!(*state.load(), JobState::Running);
assert_eq!(audit, vec![(JobState::Queued, JobState::Running)]);
assert!(!machine.try_trigger(&state, JobEvent::Start));
```

## 重试配置

`cas_executor` 注入已校验的 CasExecutor；`cas_strategy` 选择 LatencyFirst、ContentionBackoff 或
ReliabilityFirst。两者都替换整个 executor，最后调用者生效。ContentionBackoff 是固定指数退避加
jitter，不会自动学习竞争率。默认 LatencyFirst 为 100 次尝试、5ms 操作预算、20ms 总预算。

构建后使用 `machine.cas_executor().retry_policy()` 查看实际生效的次数、软预算和退避配置；
标准版不再提供单独的 machine `cas_strategy()` 查询，因为自定义 executor 没有对应的预设名称。

自定义 builder 支持 max_attempts、max_operation_elapsed、max_total_elapsed 和退避。
软预算只决定后续准入，不撤销已成功提交的结果；同步触发忽略 attempt_timeout/flow_timeout，
重试延迟阻塞调用线程。这里不提供异步触发或 hooks，需这些能力时直接使用 rs-cas。
Typed Fast 错误可用 `is_unknown_transition()` 和 `is_cas_conflict()` 分类；
`diagnose_graph()` 只做离线图分析，不改变构建规则。

## 错误与副作用

UnknownState/UnknownTransition 表示业务规则不允许当前操作；CasFailure 保留 CAS 错误类别和尝试数，
可区分冲突耗尽与预算耗尽。try_trigger 将失败压成 false，需排障时使用 trigger。

trigger_with 的回调只在成功提交后执行一次，自转换也算成功提交。回调 panic 不会回滚状态。
并发回调没有全局顺序，回调中重新读取状态可能看到更新后的值；使用传入的旧/新状态做该次审计。

## Fast 模式与排障

仅使用 fast 时配置 default-features=false、features=["fast"] 并依赖 qubit-fast-cas 0.3，
不需要 qubit-cas 或 qubit-atomic。整数状态/事件编码必须落在构建时声明的范围。

- 构建失败：检查初态、状态注册、终态出边和重复转换。
- 触发失败：先分清业务错误与 CasFailure，再选择是否重试。
- 延迟增大：检查同步退避和热点竞争，用实际负载评估策略；不要直接放宽重试到无限。

本库不提供跨资源事务或完整工作流调度。参阅 [README](../README.zh_CN.md)、
[API](https://docs.rs/qubit-state-machine)及[迁移说明](migration-0.8.zh_CN.md)。
