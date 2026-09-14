# 状态机性能测量

基准比较手写 AtomicU64、Standard、Raw Fast 和 Typed Fast。查表、单线程转换、
任务生命周期和准备好的 cell 批量竞争分别测量。worker 在
计时前通过就绪屏障同步；线程与 cell 构造不计入测量区间。

Standard 使用 16 次立即尝试且无时间预算；Fast 和 Typed Fast 使用显式
`spin(16)`。统计区分调用次数、成功提交、业务拒绝、CAS 预算耗尽和异常失败。

结果依赖机器。每次结果必须记录 Rust 工具链、CPU、操作系统、源码版本和三次
独立运行。批量成功吞吐与总调用吞吐不是同一指标；Fast 的同值 CAS 与 Standard
的 `AtomicRef` 身份更新也不能直接等价比较。

## 复现选定场景

在 crate 根目录、机器尽量空闲时，分别运行以下命令三次。每个场景使用
20 个采样、300 毫秒预热和 1 秒测量时间。`task_lifecycle` 对同一状态单元
依次应用 `Start` 和 `CompleteSucceeded`；状态单元的创建不计入测量。
两个命令均启用全部 feature，以包含四种实现。

```bash
cargo bench --bench state_machine_benchmarks --all-features transition_target -- --noplot
cargo bench --bench state_machine_benchmarks --all-features task_lifecycle -- --noplot
```

## 实测示例

测量日期为 2026-09-14，基准实现来自提交 `f2c4a7d`。环境：Rust 1.94.0、
Ubuntu 24.04.4 LTS、Linux 7.0.0-30-generic、Intel Core i5-9600K
（六核，3.70 GHz）。先做一次额外预热，再分别运行三次；下表记录 Criterion
给出的平均值点估计，单位为纳秒／操作。仅对这些特定负载而言，数值越小越快。

| 实现 | 查表第 1 次 | 第 2 次 | 第 3 次 | 生命周期第 1 次 | 第 2 次 | 第 3 次 |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| 直接 AtomicU64 基线 | 4.783 | 4.780 | 4.816 | 18.016 | 18.101 | 18.065 |
| Raw Fast | 5.146 | 5.186 | 5.193 | 24.336 | 24.224 | 24.233 |
| Typed Fast | 6.371 | 6.369 | 6.424 | 25.184 | 24.839 | 25.601 |
| Standard | 18.659 | 19.621 | 19.426 | 1259.550 | 1234.473 | 1267.393 |

这些数字只比较查表和无竞争的两次转换。Standard 更新的是 `AtomicRef`，
其他实现使用整数 CAS；直接基线也不能代替规则校验和类型化错误。
表中数据不能推断竞争、回调或其他机器上的性能。选择实现前，请运行其余
基准场景，并测量自己的实际负载。
