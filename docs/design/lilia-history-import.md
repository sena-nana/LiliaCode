# Lilia History Import 协议

> 状态：本文定义 Lilia 桌面应用导入 provider 历史会话的应用层协议边界。
> 核对时间：2026-06-14。

## 协议定位

History Import 是桌面应用的数据管理协议，用于统一 Codex thread 和 Claude session 的搜索、预览、导入和运行态展示。它不属于 agent turn，因此不进入 `ChatWorkflow`、`ChatRuntimeCommand` 或 runner stdin payload。

`docs/design/lilia-agent-interface.md` 定义的是 UI 到 agent runner / provider adapter 的三层协议。History Import 的执行发生在 Tauri command 层，结果直接返回 UI；只有导入完成后的 task / timeline 数据才进入 Lilia 常规任务视图。

## 公共入口

前端只调用 Lilia 命名的服务入口：

| service | Tauri command | 含义 |
|---|---|---|
| `searchHistoryImports` | `history_import_search` | 按 provider 搜索可导入历史项。 |
| `previewHistoryImport` | `history_import_preview` | 读取轻量或完整 timeline 预览。 |
| `attachHistoryImport` | `history_import_attach` | 把历史项导入到当前或新 task。 |
| `listHistoryImportRuntimeStates` | `history_import_runtime_states` | 读取 Lilia 当前管理的运行态历史项。 |
| `cleanHistoryImportBackgroundTerminals` | `history_import_clean_background_terminals` | 清理导入项关联的后台终端。 |

公共契约源是 `packages/contracts/src/history-import.ts`。UI 可以选择 `provider: "codex" | "claude"` 作为来源，但不得直接调用 `codex_thread_*` 或 `claude_session_*` command。

## Adapter 边界

`apps/desktop/src-tauri/src/history_import.rs` 是唯一 facade。它把公共输入转换成 provider adapter 输入：

| Lilia 操作 | Codex adapter | Claude adapter |
|---|---|---|
| search | `codex_thread_search_blocking` | `claude_session_search_blocking` |
| preview | `codex_thread_preview_blocking` | `claude_session_preview_blocking` |
| attach | `codex_thread_attach_blocking` | `claude_session_attach_blocking` |
| runtime states | `query_codex_thread_runtime_states` | 暂无等价能力，返回空。 |
| clean background terminals | `clean_codex_thread_background_terminals_blocking` | 暂无等价能力，UI 不展示入口。 |

provider 专属字段只允许出现在 facade 内部的转换结构里。返回给 UI 的数据必须归一到 `HistoryImportItem`、`HistoryImportPreview` 和 `HistoryImportAttachResult`。

## UI 约束

导入页可以在展示文案中使用 `Codex thread` / `Claude session` 帮助用户识别来源；业务函数、状态变量、测试断言和服务调用应使用 `HistoryImport*` 语义。

如果后续需要让 agent runner 主动执行历史查询或导入，必须新增独立协议设计，不能直接把现有 History Import command 塞进 `lilia-agent-protocol.json`。
