# 时间线显示声明

时间线事件的展示语义由事件生产方声明在 `AgentTimelineEvent.display` 中。前端时间线只消费这份受控 schema，负责排序、折叠、状态色、分组和通用详情块渲染。

`display` 是白名单数据契约，不允许工具或扩展注入任意 Vue 组件。新增工具节点时，生产方应声明：

- `icon`：lucide 图标的 kebab-case 名（例如 `terminal`、`file-pen`、`book-open`）。前端按名查 `lucide-vue-next` 命名导出，未声明或解析不到都不渲染图标节点——每个工具/事件 case 自己决定是否带图标。
- `label` 或 `action + object`：标题。`action` 会由前端按通用 status 生成“正在/已/失败”等状态文案。
- `preview`：折叠态单行预览。
- `details`：通用详情块，只支持 `line`、`fields`、`code`、`markdown`、`list`。
- `group`：相邻折叠和最终回复过程摘要使用的 `key`、`bucket`、`unit`、`count`。

开发阶段不保留旧事件兼容；写入和 emit 的 timeline event 必须带 `display`。
