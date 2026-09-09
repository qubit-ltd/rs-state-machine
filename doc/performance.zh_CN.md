# 状态机性能测量

基准比较手写 AtomicU64、Standard、Raw Fast 和 Typed Fast。查表、单线程转换、
任务生命周期和准备好的 cell 批量竞争分别测量。worker 在
计时前通过就绪屏障同步；线程与 cell 构造不计入测量区间。

Standard 使用 16 次立即尝试且无时间预算；Fast 和 Typed Fast 使用显式
`spin(16)`。统计区分调用次数、成功提交、业务拒绝、CAS 预算耗尽和异常失败。

结果依赖机器。每次结果必须记录 Rust 工具链、CPU、操作系统、源码版本和三次
独立运行。批量成功吞吐与总调用吞吐不是同一指标；Fast 的同值 CAS 与 Standard
的 `AtomicRef` 身份更新也不能直接等价比较。
