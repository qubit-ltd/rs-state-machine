# 迁移到 qubit-state-machine 0.9

0.9 改变了通用 `StateMachine` 的默认 CAS 执行器：默认允许 16 次立即尝试，
不设置操作时间预算或总墙钟预算。这样调度器延迟重试时不会让本来合法的转换
因为时钟预算而失败。强竞争仍可能耗尽尝试次数，并报告为 CAS 失败。

如果应用需要 0.8 的低延迟时间上限，请显式选择：

```rust
use qubit_cas::CasStrategy;

let machine = StateMachine::<u8, u8>::builder()
    .add_states(&[0, 1])
    .initial_state(0)
    .transition(0, 0, 1)
    .cas_strategy(CasStrategy::LatencyFirst)
    .build()?;
```

`trigger` 系列仍区分未定义转换和 CAS 终止失败；`try_trigger` 仍将两者折叠为
`false`。Typed Fast、Raw Fast、状态单元所有权、回调和终态规则不变。下游执行器
中的 `is_done()` 仍表示终态已经安装，结果可能在随后才可读取。
