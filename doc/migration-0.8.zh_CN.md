# 迁移到 qubit-state-machine 0.8

[English](migration-0.8.md)。

标准版依赖升级到 qubit-cas 0.13，可通过 StateMachineBuilder::cas_executor 注入完整配置。
直接使用 CAS 的项目需同时升级 qubit-cas，并把 ContentionAdaptive 改为 ContentionBackoff；
没有旧名兼容层。cas_strategy 仍可用，与 cas_executor 最后一次调用生效。

StateMachineError::CasFailure 仍保留 kind 和 attempts，业务错误和成功回调语义不变。
Fast-only 模式继续只使用 qubit-fast-cas，不引入 qubit-cas。完整例子见[指南](user_guide.zh_CN.md)。
