# 时间线显示派生

时间线事件的展示语义由前端 / `@lilia/contracts` 在渲染时从事实事件派生。runner 和 Rust 端只持久化事实字段：`kind`、`status`、`title`、`summary`、`payload`、`sourceId`。

display 是白名单派生结果，不允许工具或扩展注入任意 Vue 组件。新增工具节点时，应在 contracts 的 timeline display 派生表中声明：

- `icon`：lucide 图标的 kebab-case 名（例如 `terminal`、`file-pen`、`book-open`）。前端按名查 `lucide-vue-next` 命名导出，未声明或解析不到都不渲染图标节点——每个工具/事件 case 自己决定是否带图标。
- `label` 或 `action + object`：标题。`action` 会由前端按通用 status 生成"正在/已/失败"等状态文案。
- `preview`：折叠态单行预览。
- `details`：通用详情块，只支持 `line`、`fields`、`code`、`markdown`、`list`。
- `group`：相邻折叠和最终回复过程摘要使用的 `key`、`bucket`、`unit`、`count`。

开发阶段不保留旧事件兼容；写入和 emit 的 timeline event 不带 `display`，历史数据展示规则可随 contracts 派生逻辑即时更新。

## 「过程折叠到最终回复」的触发时机

UI 把同 turn 内所有事件折叠到「该 turn 最后一条 assistant message」下方时，只在 turn 已经收到终结信号后才生效。终结信号 = runner emit 的 `kind: "turn"` 事件且 `status ∈ {success, completed, done, error, failed, cancelled}`——对应 Claude SDK 的 `result` 消息那一帧。流式期间没有这个事件，所有事件按 `(turnSeq, intraTurnOrder)` inline 显示，避免「最后一条 assistant message」随新 text block 漂移导致折叠抖动。

折叠范围：用户消息（锚点）和最终回复（卡片）保留在外，**之间**的可见过程事件（工具 / 计划 / 中间 text block 等）全部进 processEvents。`reasoning` 和 `turn` 仍可持久化供调试/恢复使用，但默认 UI 不渲染，也不计入「展开过程 N 项」。

## Plan Mode 与权限

### Claude Plan

`planMode` 是本轮先进入 Claude 原生计划模式的工作流开关，`permission` 是计划确认后的执行权限，二者正交。计划待确认时，runner 镜像 `ExitPlanMode` 为 `kind: "plan"` / `status: "requires_action"`，通过现有 AskUser 通道请求用户确认；用户确认后，runner 恢复发送时已经选择的执行权限（`full` / `ask` / `readonly`），不改 composer 默认值，也不把只读伪装成 Claude plan mode。

Claude 仍拥有原生 Plan 内容，Lilia 只负责镜像、确认、恢复权限和记录时间线事实。只读权限在执行阶段由 Lilia 的 `canUseTool` 门禁拒绝可写或无法判定的工具，并把拒绝原因写入时间线。

### Codex Plan

Codex 的首选入口是 experimental app-server / Codex MCP interface 的 collaboration mode，而不是自定义 `SubmitPlanForApproval` 类工具。Lilia 初始化 Codex app-server 时已经声明 `capabilities: { experimentalApi: true }`；协议字段在普通 `codex app-server generate-ts` 结果里不可见，需要 `codex app-server generate-ts --experimental --out <dir>` 才会生成。

Codex experimental app-server 的完整接口核对与实现状态见 [codex-experimental-app-server.md](./codex-experimental-app-server.md)。

已确认的 experimental 入口：

- `collaborationMode/list`：返回内置 collaboration mode presets / masks。内置 preset 不设置 `model`；Plan preset 将 `reasoning_effort` 设为 `medium`，client 保留或覆盖当前 model。
- `turn/start.collaborationMode`：本轮设置 preset，优先级高于 turn 级 `model`、`reasoning_effort` 和 developer instructions。`settings.developer_instructions: null` 表示使用所选 mode 的内置 instructions。
- `thread/settings/update.collaborationMode`：设置后续 turns 的 collaboration mode；它更适合 sticky 偏好，不作为 Lilia “本轮先计划、确认后执行”流程的默认入口。

Lilia 的 Codex plan 流程：

```js
await server.request("turn/start", {
  threadId,
  input: [{ type: "text", text: prompt }],
  cwd,
  approvalPolicy,
  collaborationMode: cmd.planMode
    ? {
        mode: "plan",
        settings: {
          model: cmd.model || currentModel,
          reasoning_effort: "medium",
          developer_instructions: null,
        },
      }
    : null,
});
```

实现先调用 `collaborationMode/list` 找到 `mode === "plan"` 的 preset / mask，再合并当前 model 后传给 `turn/start.collaborationMode`。`turn/plan/updated`、`item/plan/delta` 等计划事件只作为原生 plan / todo 镜像展示；plan turn 自然 `turn/completed` 后，Lilia 再展示 `plan_approval`。用户同意后，Lilia 发起下一轮 default mode：`Implement the approved plan...`；用户要求修改时，下一轮仍用 plan mode 带修改要求重新规划。

由于该入口仍是 experimental，文档和实现都必须把它当作可能变化的协议面：Codex CLI 版本探测、schema 生成和失败 fallback 要显式区分“experimental 字段不可用”与“plan preset 不存在”。协议来源见 [openai/codex Codex MCP Server Interface](https://raw.githubusercontent.com/openai/codex/main/codex-rs/docs/codex_mcp_interface.md)。

### UI 边界

计划模式的 UI 边界：

- 计划正文只出现在时间线 `kind: "plan"` 事件里。计划事件走专用灰色卡片：待确认状态默认展开，用户同意、取消或发送修改要求后默认折叠，用户可从卡片头部再次展开/折叠。
- composer 上方的 inline AskUser 卡片只保留标题和确认动作；卡片不复制计划正文，也不展示执行权限提示。
- 计划确认挂起时，用户从 composer 输入并发送的文本被视为计划修改要求，回写到当前 `plan_approval` ask-user 的 `AskUserAnswer.notes`，不创建新 turn，也不进入调度队列。
- 带 `revisionRequest` 的计划事件显示为“要求修改计划”，详情保留原计划正文和用户修改要求。Claude 返回 deny 但不设置 interrupt，在同一轮重新给出计划并再次调用 `ExitPlanMode`；Codex 发起下一轮 plan collaboration mode，并把修改要求作为重规划输入。
