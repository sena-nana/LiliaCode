# Codex experimental app-server 记录

> 状态：已废弃。
> 核对时间：2026-06-19。

Codex provider 能力不再单独按 app-server 方法维护文档。相关内容已经并入 [Lilia Agent 三层协议](./lilia-agent-interface.md)，并按 `ChatWorkflow`、`ChatRuntimeCommand`、runtime extensions 和 interaction 的 Lilia 协议层表达。

后续升级 Codex CLI 或 Claude SDK 时，先在 Lilia 协议文档中确认能力落点；没有对应落点时，先定义 Lilia 协议名、层级和 fallback，再实现 provider adapter 映射。本文后续会与 Claude provider 专属接口文档一起删除。
