# 时间线显示声明

时间线事件的展示语义由事件生产方声明在 `AgentTimelineEvent.display` 中。前端时间线只消费这份受控 schema，负责排序、折叠、状态色、分组和通用详情块渲染。

`display` 是白名单数据契约，不允许工具或扩展注入任意 Vue 组件。新增工具节点时，生产方应声明：

- `icon`：lucide 图标的 kebab-case 名（例如 `terminal`、`file-pen`、`book-open`）。前端按名查 `lucide-vue-next` 命名导出，未声明或解析不到都不渲染图标节点——每个工具/事件 case 自己决定是否带图标。
- `label` 或 `action + object`：标题。`action` 会由前端按通用 status 生成"正在/已/失败"等状态文案。
- `preview`：折叠态单行预览。
- `details`：通用详情块，只支持 `line`、`fields`、`code`、`markdown`、`list`。
- `group`：相邻折叠和最终回复过程摘要使用的 `key`、`bucket`、`unit`、`count`。

开发阶段不保留旧事件兼容；写入和 emit 的 timeline event 必须带 `display`。

## 「过程折叠到最终回复」的触发时机

UI 把同 turn 内所有事件折叠到「该 turn 最后一条 assistant message」下方时，只在 turn 已经收到终结信号后才生效。终结信号 = runner emit 的 `kind: "turn"` 事件且 `status ∈ {success, completed, done, error, failed, cancelled}`——对应 Claude SDK 的 `result` 消息那一帧。流式期间没有这个事件，所有事件按 `(turnSeq, intraTurnOrder)` inline 显示，避免「最后一条 assistant message」随新 text block 漂移导致折叠抖动。

折叠范围：用户消息（锚点）和最终回复（卡片）保留在外，**之间**的可见过程事件（工具 / 计划 / 中间 text block 等）全部进 processEvents。`reasoning` 和 `turn` 仍可持久化供调试/恢复使用，但默认 UI 不渲染，也不计入「展开过程 N 项」。

## Claude Plan 与权限

`planMode` 是本轮先进入 Claude 原生计划模式的工作流开关，`permission` 是计划确认后的执行权限，二者正交。计划待确认时，runner 镜像 `ExitPlanMode` 为 `kind: "plan"` / `status: "requires_action"`，通过现有 AskUser 通道请求用户确认；用户确认后，runner 恢复发送时已经选择的执行权限（`full` / `ask` / `readonly`），不改 composer 默认值，也不把只读伪装成 Claude plan mode。

Claude 仍拥有原生 Plan 内容，Lilia 只负责镜像、确认、恢复权限和记录时间线事实。只读权限在执行阶段由 Lilia 的 `canUseTool` 门禁拒绝可写或无法判定的工具，并把拒绝原因写入时间线。
