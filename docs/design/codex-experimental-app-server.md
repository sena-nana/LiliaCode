# Codex experimental app-server 接口核对

> 状态：本文是 Lilia 对 Codex experimental app-server / Codex MCP interface 的本地核对清单。
> 核对时间：2026-06-04。
> 核对版本：`codex-cli 0.136.0`。

## 核对方式

Codex app-server 的 experimental 字段不会出现在普通 generated TS 里。本文用同一版本 CLI 生成普通 schema 与 experimental schema 后做差异：

```powershell
codex app-server generate-ts --out <base>
codex app-server generate-ts --out <experimental> --experimental
```

判定口径：

- “experimental 接口”指 `--experimental` 相比普通 schema 新增的 JSON-RPC 方法，以及同名参数 / 响应类型新增的字段。
- 当前核对结果里，`ClientRequest` 新增 27 个方法；`ClientNotification`、`ServerRequest`、`ServerNotification` 没有新增方法。
- Lilia 已在 `initialize` 里声明 `capabilities: { experimentalApi: true }`，所以协议协商上具备调用 experimental 方法和字段的前置条件。
- 官方来源：openai/codex 的 [Codex MCP Server Interface](https://raw.githubusercontent.com/openai/codex/main/codex-rs/docs/codex_mcp_interface.md)。该接口标注为 experimental，方法、字段和事件形状可能变化。

## 状态标记

| 状态 | 含义 |
|---|---|
| 已实现 | Lilia 已调用该方法 / 字段，并把语义接到现有产品行为。 |
| 部分实现 | Lilia 已处理相邻事件或通用回调，但没有完整覆盖该接口的字段语义。 |
| 未实现 | Lilia 当前没有调用或消费该接口。 |
| 不接入 | 测试专用或与 Lilia 当前边界不匹配，默认不纳入产品路线。 |

## Experimental 方法

| 方法 | 协议语义 | Lilia 状态 | 说明 |
|---|---|---|---|
| `thread/increment_elicitation` | 增加 thread 的 out-of-band elicitation 计数。参数：`threadId`。 | 未实现 | Lilia 目前用自己的 AskUser / pending interaction 状态，不回写 Codex 线程计数。 |
| `thread/decrement_elicitation` | 减少 thread 的 out-of-band elicitation 计数。参数：`threadId`。 | 未实现 | 同上；若未来要让 Codex 原生线程知道 Lilia 正在等待外部输入，再接入。 |
| `thread/settings/update` | 更新后续 turns 的 sticky 设置：`cwd`、approval、sandbox、permissions、model、effort、summary、`collaborationMode`、personality 等。 | 未实现 | Lilia 当前在 `thread/start` / `turn/start` 传本轮运行参数；Codex plan 更适合先用 `turn/start.collaborationMode`，不把 plan mode 写成 sticky 默认。 |
| `thread/memoryMode/set` | 设置线程 memory mode。参数：`threadId`、`mode`。 | 未实现 | Lilia Memory 设计是旁路系统，不直接切 Codex 原生 memory mode。 |
| `memory/reset` | 重置 Codex memory。无参数。 | 未实现 | 这是 Codex 自身全局/账户语义，Lilia 不应在没有明确用户动作时调用。 |
| `thread/backgroundTerminals/clean` | 清理指定 thread 的 background terminals。参数：`threadId`。 | 未实现 | Lilia 目前只展示 Codex 命令事件，没有接管 Codex background terminal 生命周期。 |
| `thread/search` | 搜索 threads。支持分页、排序、sourceKinds、archived、`searchTerm`。 | 未实现 | Lilia 使用自己的 task/session 存储与搜索；可作为“导入 / 继续 Codex 历史”增强入口。 |
| `thread/turns/list` | 分页读取指定 thread 的 turns，可选择 item detail。 | 未实现 | Lilia 当前依赖运行期事件持久化，不从 Codex thread 回补完整 turn 页。 |
| `thread/turns/items/list` | 分页读取指定 turn 的 items。 | 未实现 | 同上；适合未来做 Codex 历史恢复和时间线重建。 |
| `thread/realtime/start` | 启动 thread-scoped realtime session。参数包括输出模态、prompt、transport、voice。 | 未实现 | Lilia 当前没有 Codex realtime 语音 / 文本会话 UI。 |
| `thread/realtime/appendAudio` | 向 realtime session 追加音频输入。 | 未实现 | 依赖 realtime UI 和音频采集。 |
| `thread/realtime/appendText` | 向 realtime session 追加文本输入。 | 未实现 | 依赖 realtime 会话状态管理。 |
| `thread/realtime/stop` | 停止 thread realtime session。 | 未实现 | 同上。 |
| `thread/realtime/listVoices` | 列出 realtime 支持的 voices。 | 未实现 | 同上。 |
| `remoteControl/enable` | 启用 remote control。无参数。 | 未实现 | Lilia 当前不暴露 Codex remote control。 |
| `remoteControl/disable` | 关闭 remote control。无参数。 | 未实现 | 同上。 |
| `remoteControl/status/read` | 读取 remote control 状态，返回连接状态、serverName、installationId、environmentId。 | 未实现 | 同上；若未来支持远程环境可接入设置页诊断。 |
| `collaborationMode/list` | 返回内置 collaboration mode presets / masks。Plan preset 不设置 model，设置 `reasoning_effort: "medium"`。 | 已实现 | Lilia 在 Codex plan mode 首轮前读取 `mode === "plan"` 的 preset；读取失败或 preset 缺失时使用本地 fallback。 |
| `mock/experimentalMethod` | 测试 experimental gating 的 mock 方法。参数：`value?`。 | 不接入 | 测试专用，不应进入产品行为。 |
| `environment/add` | 注册外部 execution environment。参数：`environmentId`、`execServerUrl`。 | 未实现 | Lilia 当前不向 Codex app-server 注册外部 exec environment。 |
| `process/spawn` | 在 app-server 所在 host 上启动非 Codex sandbox 的独立进程；输出通过 `process/outputDelta`、`process/exited` 通知返回。 | 未实现 | 与 Lilia 自己的 runner / Tauri 执行边界不同；安全上不能默认接入。 |
| `process/writeStdin` | 向 `process/spawn` 创建的进程写 stdin 或关闭 stdin。 | 未实现 | 依赖 `process/spawn`。 |
| `process/kill` | 终止 `process/spawn` 创建的进程。 | 未实现 | 依赖 `process/spawn`。 |
| `process/resizePty` | 调整 `process/spawn` PTY 尺寸。 | 未实现 | 依赖 `process/spawn`。 |
| `fuzzyFileSearch/sessionStart` | 启动 legacy fuzzy file search session。参数：`sessionId`、`roots`。 | 未实现 | Lilia 已有自己的上下文搜索 / 附件入口，不使用 Codex legacy fuzzy search。 |
| `fuzzyFileSearch/sessionUpdate` | 更新 fuzzy file search query。参数：`sessionId`、`query`。 | 未实现 | 同上。 |
| `fuzzyFileSearch/sessionStop` | 停止 fuzzy file search session。参数：`sessionId`。 | 未实现 | 同上。 |

## Existing 方法里的 experimental 字段

| 位置 | 新增字段 | Lilia 状态 | 说明 |
|---|---|---|---|
| `InitializeCapabilities` | `experimentalApi` | 已实现 | `initializeCodexAppServer` 已发送 `capabilities: { experimentalApi: true }`。这是使用 experimental 方法 / 字段的协议前置条件。 |
| `thread/start` | `dynamicTools` | 已实现 | Lilia 用它注册 `AskUserQuestion`，将 Codex 的提问接入统一 AskUser。 |
| `thread/start` | `runtimeWorkspaceRoots`、`permissions`、`environments`、`mockExperimentalField`、`experimentalRawEvents`、`persistExtendedHistory` | 未实现 / 不接入 | Lilia 目前只传 model、cwd、sandbox、approvalPolicy、dynamicTools。`mockExperimentalField` 不接入。 |
| `thread/resume` | `history`、`path`、`runtimeWorkspaceRoots`、`permissions`、`excludeTurns`、`initialTurnsPage`、`persistExtendedHistory` | 未实现 | Lilia resume 只传 threadId 和运行参数，没有注入 Codex history 或分页恢复。 |
| `thread/fork` | `path`、`runtimeWorkspaceRoots`、`permissions`、`excludeTurns`、`persistExtendedHistory` | 未实现 | Lilia 当前没有通过 Codex app-server fork thread。 |
| `turn/start` | `collaborationMode` | 已实现 | Lilia 在计划轮传 `mode: "plan"`，确认后执行轮显式传 `mode: "default"`，避免 plan mode 泄漏。 |
| `turn/start` | `responsesapiClientMetadata`、`additionalContext`、`environments`、`runtimeWorkspaceRoots`、`permissions` | 未实现 | Lilia 当前只传 `threadId`、文本 input、cwd、approvalPolicy。 |
| `turn/steer` | `responsesapiClientMetadata`、`additionalContext` | 未实现 | Lilia 当前没有使用 `turn/steer`。 |
| `command/exec` | `permissionProfile` | 未实现 | Lilia 不调用 Codex `command/exec`；Codex agent 命令由 app-server 内部事件上报。 |
| `item/commandExecution/requestApproval` | `additionalPermissions`、`availableDecisions` | 部分实现 | Lilia 用通用 `/requestApproval` 桥接到工具确认，但当前只回 `{ decision: "accept" | "decline" }`，没有展示或利用新增权限请求和可选决策集合。 |
| `Config` | `apps` | 未实现 | Lilia 不读取 Codex app-server `config/read` 的 apps 配置。 |
| `ConfigRequirements` | `allowedApprovalsReviewers`、`hooks`、`network` | 未实现 | Lilia 设置页只做当前连接 / app-server 可用性探测，没有消费这些要求。 |

## Codex Plan 语义落点

Codex Plan 不应优先用自定义工具模拟。目标实现顺序：

1. `initialize` 保持 `experimentalApi: true`。
2. 启动 Codex app-server 后调用 `collaborationMode/list`，寻找 `mode === "plan"` 的 preset / mask。
3. 发送首轮 `turn/start` 时，如果 composer 的 `planMode` 为 true，传入：

```js
collaborationMode: {
  mode: "plan",
  settings: {
    model: cmd.model || currentModel,
    reasoning_effort: "medium",
    developer_instructions: null,
  },
}
```

4. `turn/plan/updated`、`item/plan/delta` 等计划事件只作为原生 plan / todo 镜像展示。
5. 等 Codex plan turn 自然 `turn/completed` 后，Lilia 展示 `plan_approval`。
6. 用户同意后，Lilia 发起下一轮 default mode 执行已确认计划；用户要求修改时，下一轮继续 plan mode 并带上修改要求。

当前 Lilia 已实现的相邻能力：

- 能初始化 experimental API。
- 能通过 `dynamicTools` 接入 Codex AskUser。
- 能把 Codex plan 事件映射到 Lilia 时间线与 `plan_approval` 展示。
- 能处理 Codex command / file change approval 的通用确认请求。

当前已接入：

- 已调用 `collaborationMode/list` 并读取 `mode === "plan"` 的 preset。
- 已在 `turn/start` 传 `collaborationMode`。
- `turn/plan/updated`、`item/plan/delta` 只作为 plan / todo 镜像；plan turn 自然完成后再等待用户确认。
- 确认后执行轮显式回到 default mode，避免 sticky plan mode 泄漏到执行阶段。

## 接入优先级

1. **Codex Plan collaboration mode**：`collaborationMode/list` + `turn/start.collaborationMode`。这是与 Codex Desktop 最接近的计划模式入口。
2. **审批字段增强**：展示 `additionalPermissions`，并按 `availableDecisions` 限制确认按钮或响应值。
3. **历史恢复**：根据需要接入 `thread/turns/list` 和 `thread/turns/items/list`，用于 Codex 历史线程的时间线回补。
4. **环境 / remote / process / realtime**：暂不默认接入；这些接口涉及更大的权限、UI 和生命周期边界，需要单独设计。

## 复核脚本片段

升级 Codex CLI 后，可以用以下方式重新核对新增方法：

```js
const methods = (text) =>
  [...text.matchAll(/"method":\s*"([^"]+)"/g)].map((match) => match[1]);

const baseMethods = new Set(methods(baseClientRequestTs));
const experimentalOnly = methods(experimentalClientRequestTs).filter(
  (method) => !baseMethods.has(method),
);
```

并额外比较同名 TS 文件内容，找出 `TurnStartParams`、`ThreadStartParams` 等已有类型里的字段级变化。
