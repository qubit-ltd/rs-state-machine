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
依次应用 `Start` 和 `CompleteSucceeded`。状态单元的创建与最终销毁不计入测量；
转换内部的分配与释放仍计入。
两个命令均启用全部 feature，以包含四种实现。

```bash
cargo bench --bench state_machine_benchmarks --all-features transition_target -- --noplot
cargo bench --bench state_machine_benchmarks --all-features task_lifecycle -- --noplot
```

## 共享 toggle 竞争统计

每次 `shared_toggle_t{workers}` Criterion 调用在全部批次结束后，还会向
stderr 写入一条 `rsm_shared_toggle_v1` 原始统计记录。每次运行分别保存记录；
Criterion 校准与预热也会调用同一 benchmark 闭包，因此日志同时包含这些调用
和正式采样调用。它们是每次闭包调用的计数，不是 Criterion 样本或置信区间。

```bash
cargo bench --locked --bench state_machine_benchmarks --all-features shared_toggle -- --noplot 2>target/shared-toggle-run1.log
```

制表符分隔的记录包含 `batches`、调用总数 `calls`、各结果计数及累计
`elapsed_ns`。调用数为 `batches * workers * 4096`；每次调用恰好归入
committed、rejected、conflict、unexpected 之一。速率公式为
`calls_per_second = calls / (elapsed_ns / 1_000_000_000)`、
`commits_per_second = committed / (elapsed_ns / 1_000_000_000)`，以及
`conflict_ratio = conflicts / calls`。耗时为零时两种速率记为
`unavailable`；调用数为零时冲突比例记为 `unavailable`。

返回给 Criterion 的耗时只累计 `measure_shared_toggle` 的 worker 测量区间。
每段区间结束后才汇总计数并格式化 stderr 记录，因此日志 I/O 不计入该耗时；
Criterion 校准和预热仍会产生记录。合并多条记录时，先分别求计数与耗时总和，
再重算比例，不要对输出速率取平均。每次运行同时记录源码版本、工具链、CPU
和操作系统，并保留原始日志。

## 实测示例

以下为 2026-09-14 的历史测量，基准实现来自提交 `f2c4a7d`。环境：Rust 1.94.0、
Ubuntu 24.04.4 LTS、Linux 7.0.0-30-generic、Intel Core i5-9600K
（六核，3.70 GHz）。先做一次额外预热，再分别运行三次；下表记录 Criterion
给出的平均值点估计，单位为纳秒／操作。仅对这些特定负载而言，数值越小越快。
历史 `task_lifecycle` 测量区间包含状态单元的最终销毁；其生命周期数据不能与
采用修正后边界的测量直接比较。查表数据保持原有测量口径。

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

## 修正口径后的 task_lifecycle 测量（2026-11-01 UTC）

使用 Criterion 0.8.2，独立运行三次：
`cargo bench --locked --bench state_machine_benchmarks --all-features task_lifecycle -- --noplot`。
每格为 Criterion 给出的平均值点估计，单位为纳秒／次；每次使用 20 个采样、
300 毫秒预热和 1 秒测量时间。这些数据用于记录测量结果，不是性能门槛。

| 实现 | 第 1 次 | 第 2 次 | 第 3 次 |
| --- | ---: | ---: | ---: |
| 直接 AtomicU64 基线 | 13.704 | 14.849 | 15.104 |
| Raw Fast | 20.960 | 23.509 | 21.063 |
| Typed Fast | 22.127 | 27.574 | 22.290 |
| Standard | 1081.9 | 1277.4 | 1097.5 |

环境：Rust 1.94.0（`4a4ef493e3a1488c6e321570238084b38948f6db`）、Linux
7.0.0-38-generic x86_64、Intel Core i5-9600K（6 核，3.70 GHz）。源码来自基于
`e58629fb8216faa30c2749a194a1541b258ceb3a` 的未提交工作树；测量前差异涉及
10 个文件（增加 421 行、删除 36 行）。`Cargo.lock` 的 SHA-256 为
`e150c902a109cd0be9bfdc470acc8d0c00edd37b9860184c5b389abd7470019f`。
本次运行的三份完整 Criterion 输出保存在
`target/rsm-lifecycle-054a6b95-run{1,2,3}.log`。上方历史生命周期结果使用不同计时边界，
不可直接比较。
