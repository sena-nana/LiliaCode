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

UI 把同 turn 内非 message、非 reasoning 的事件折叠到「该 turn 最后一条 assistant message」下方时，只在 turn 已经收到终结信号后才生效。终结信号 = runner emit 的 `kind: "turn"` 事件且 `status ∈ {success, completed, done, error, failed, cancelled}`——对应 Claude SDK 的 `result` 消息那一帧。流式期间没有这个事件，所有过程事件按 `(turnSeq, intraTurnOrder)` inline 显示，避免「最后一条 assistant message」随新 text block 漂移导致折叠抖动。

reasoning 始终 inline 展示（思考是模型的公开叙事），但不阻断折叠。Claude 的典型流程是「思考→工具→再思考→回复」，把收尾 reasoning 当成分隔点会让真实世界几乎所有 turn 都拿不到折叠。
