# 桌面交互重设计

对照当前 Native 装配（`apps/desktop/src/runtime_shell.rs`、`runtime_windows.rs`、`module/*`）与 NanaUI pin `d3cdc1c63`。场景规则：任务对话走聊天桌面；文件 / 终端 / 架构 / 路线图走 IDE 工作台。

本文件覆盖全部表面。**第一批落地**只改主窗口：侧栏、任务聊天、Composer、待处理带、检查器分区。其余表面按批次实施。

## 当前壳结构

主窗口用 NanaUI `DesktopShell`：标题栏槽、`SidebarFrame` 导航、primary 内容、可选 inspector、可选 bottom 诊断。产品自己用 `Stack` 预设拼出对话列、Composer 卡、项目页和工作区页。

进出规则：`settings_open` / `automations_open` / 非 Files 的 `project_page`（架构、路线图、记忆）仍整页替换 primary。打开文档、终端或 Files 时，`primary` 是 `SplitPane(conversation, workspace_page)`，对话列留在左侧。

## 表面清单

| 表面 | 进出 | 当前问题 | 场景 | 重设计 |
| --- | --- | --- | --- | --- |
| 标题栏 | 始终 | 面包屑过宽；检查器/命令面板藏在侧栏「更多」 | 共用 | 保留 `DesktopShell` 标题槽。检查器关闭放到检查器自身。命令面板仍由快捷键 + 菜单打开。 |
| 侧栏：会话 / 项目 / 收集箱 | 折叠、搜索、新建 | 行内停止/菜单必须用 `SidebarRow`，不能用 `ReorderList` | 聊天 | 保持任务树 + 项目分组。拖拽依赖 NanaUI 可嵌工具的重排行（见缺口文档）。 |
| 任务聊天 | 选任务、空标题 | 空状态与时间线争同一 `fill_column`；待处理塞进 Composer 卡 | 聊天 | 时间线占满剩余高度；待处理是 Composer **上方的独立卡片**；输入卡始终贴底。 |
| Composer | 对话 primary | pending 与输入同卡；发送被 blocking pending 禁用 | 聊天 | 输入、附件、权限、worktree、发送/中断留在 Composer 卡。阻塞类 pending 仍禁发送；非阻塞（标题更新）不抢输入。 |
| 待处理 | Agent 请求 | 全部控件竖排进 Composer；没有独立视觉组 | 聊天 | `Stack` 描边卡：标题、说明、字段、选项；动作横排在卡片底。Composer 仍可见。 |
| 检查器 | 更多菜单 / 编码工具 / IAB | 打开后没有区内关闭；标题+正文+待办/IAB/搜索挤一列 | 聊天+IDE | 顶栏标题 + 关闭。一次只显示一种表面（任务 / 编码工具 / IAB）。关闭走已有 `CloseInspectorDock`。 |
| 项目 Overview / Clone | 侧栏项目头、添加项目 | 卡片列表，不是工作台 | 聊天导航 | 保持总览卡；Clone 用 `Settings`/`FormField` 而不是对话输入框。 |
| Roadmap | 项目页 | 只挂了标题字段和卡片；`target_ids` 里的描述/日期/删除等未装配 | IDE | 把模块已有动作接到卡片和检查器，不要只给 Agent Debug 留 ID。 |
| Memory | 项目页 | 只显示标题拼接正文；编辑、范围、注入控件未装配 | IDE | 列表 + 检查器详情；未接线的 `MEMORY_*` 目标要么装配要么删掉。 |
| Architecture | 项目页 | `GraphCanvas` 与标题正文叠在同一列 | IDE | 图占满 primary；刷新/回滚在图上方；选中节点与历史进检查器。 |
| Files | 项目页 / pane | `TreeView` 装在 `workspace_page`，但 `primary_content_id` 在 `project_page` 存在时永不选它；Files 页目前几乎只画标题 | IDE | 文件树必须成为可见 primary 或资源区。打开文件再进文档 pane，不要被项目页规则吃掉。 |
| 文档编辑 | pane tab | `TextArea` 冒充编辑器 | IDE | 继续用 `TextArea` 直到 NanaUI 提供编辑器；诊断走 bottom。与对话并排：`SplitPane(conversation, workspace_page)`。 |
| 终端 | pane tab | `TextArea` 日志 + 底栏输入 | IDE | 与文档同一分栏。输入贴 pane 底。 |
| Settings | 侧栏底 | 已用 `SettingsSidebar`/`SettingsPage` | 共用 | 保持整页替换；返回用框架 `SettingsBack`。 |
| 自动化 | 侧栏底 | 独立侧栏+画布，像另一个应用 | IDE | 保留独立侧栏列表；画布是 primary。节点属性进检查器，不要再开第三套页。 |
| 命令面板 | 快捷键 / 更多 | `CommandPalette` 已接 `OverlayHost` | 共用 | 保持。 |
| 确认框 | 归档/删除/更新 | `ConfirmDialog` | 共用 | 保持锚定浮层。 |
| 图片查看 | markdown / 附件 | `ImageViewer` | 共用 | 保持 overlay。 |
| IAB 面板 | 检查器或新窗 | 检查器只有地址框；`IabPanelState.browser_ready()` 恒为 false；次级窗没有 NanaUI document | IDE | 检查器改为空状态，去掉「打开/新窗口」。真浏览宿主接入前不再造空白窗。 |
| 任务弹窗 | 更多 / 侧栏 | 只有时间线 + 单行 Composer；**没有待处理卡** | 聊天 | 复用主壳对话列（含 pending）。阻塞交互在弹窗里目前无处可答。 |
| 会话状态窗 | 更多菜单 | 独立 `DesktopShell` 列表 | 聊天 | 保持独立小窗；打开/停止必须接到真任务。 |
| 分栏 | PaneChrome 按钮 | `SplitPaneController` 只改拓扑；**不画第二块 live body**；多 pane 是切换器 | IDE | 后续用 NanaUI `SplitPane`/`Dock` 同时画两个 pane。现在的「左右分栏」不能假装已经并排。 |
| 启动窗 | 冷启动 | 独立 | 共用 | 不改产品 chrome。 |

## 第一批交互（本会话落地）

### 任务对话列

```
conversation (fill_column, 水平居中)
  conversation_column (max-width 860)
    conversation_body (fill_column)   // 空标题或时间线
    pending_card (optional)           // 待处理，独立卡片
    composer_dock (composer_card)     // 输入 + 工具条，始终在
```

- 待处理不再作为 `composer_dock` 的子节点。
- Composer 始终在对话列底部。`blocking_pending_count > 0` 时仍禁用发送；用户可以看见并编辑草稿。
- 斜杠/提及补全面板仍锚定在 Composer 卡内，不占用待处理卡。

### 待处理卡

- 容器：NanaUI `Stack::column` + `surface` + `outline` + `radius`（与 Composer 卡同一圆角）。
- 内容：标题、说明、该 kind 的字段/选项。
- 动作：`Stack::row` 右对齐（允许 / 拒绝 / 执行计划 / 提交）。
- 不引入假按钮。kind 仍走现有 `ShellIntent`。

### 检查器

- `DesktopShell.inspector` 仍由 `inspector_title` 是否为空决定可见性。
- 区内增加顶栏：标题 + 关闭（`IconButton`）。关闭发出 `CloseInspectorDock`。
- 一次一种内容：任务摘要/待办、编码工具、或 IAB 地址。不把三套控件同时排进同一列。

## 后续批次

已落地：Files 树可见、pane 按 kind 互斥、Roadmap/Memory 编辑器、任务弹窗 pending、待处理卡与检查器顶栏、IAB 检查器 EmptyState 且去掉未接线窗口/打开/导航、架构图占满 primary 且节点/历史进检查器、任务弹窗 MCP 字段、侧栏行挂进 `ReorderList`、时间线 `materialize_virtual_list`、工作区不同 kind 双 live pane（`SplitPane`）、对话与文档/终端/文件并排（S2：`primary` 为 `SplitPane(conversation, workspace_page)`，sash 仅 UI 状态）。

仍待：

1. IAB 真浏览宿主。NanaUI 无 WebView；在接入宿主之前不恢复「打开 / 新窗口」。
2. 同 kind 两个文档编辑器（S1 已支持不同 kind 的双 live pane）。

## 视图层重做（2026-10）

壳层框架仍由 `ShellHandles` 命令式装配；新表面按「反应式视图 + 分离挂载」逐个替换旧装配，放在 `apps/desktop/src/ui/`：

- `ui/theme.rs`：页面列、标题、元信息字号和列表行按钮。
- `ui/timeline.rs`：步骤分类和中文标签。
- `ui/timeline_row.rs`：时间线条目。用户气泡、带轨道的回复和步骤，挂一个 `RowChrome` 信号。
- `ui/coding_panel.rs`：右侧编码面板。搜索、改动列表加内联 `DiffView`，以及运行项。

视觉检查以离屏渲染为主：`cargo xtask screenshot [--matrix]` 由 `offscreen_tests.rs` 的夹具逐场景绘制 PNG。真实窗口的 `agent-debug` 只在里程碑跑。

取舍：

- **提示。** 错误和短确认（「已复制」）走 `DesktopShell.status` 的 `Toast`，同一时间只显示一条：错误优先，否则显示最早的通知。错误一直留着，等用户关闭；通知 3 秒后自己消失，计时由 NanaUI `Toast::timeout` 负责。主窗口的对话列不再内嵌错误行，任务弹窗仍内嵌，因为它没有提示层。
- **消息操作在悬停或键盘焦点进入时露出。** 动作行平时 `opacity: 0`，占位不变，悬停时不改行高，虚拟列表的测量不受影响。不用 `VisibilitySpec::Hidden`，因为隐藏的按钮进不了 Tab 顺序；透明的按钮仍可 Tab 到，焦点一进来整行就显出来。悬停来自 NanaUI `PointerHoverChanged`，焦点来自 `FocusWithinChanged`。
- **选区工具条。** `NativeMarkdown` 的 `SelectionChanged` 记进时间线模块，复制、引用、在弹窗中提问都作用于这份记录。工具条锚在选区上：位置取 NanaUI `text_selection_bounds`，默认在选区上方居中，选区贴近顶部时翻到下方，并收在对话列内。滚动会触发时间线同步，工具条随之跟着文字走。它和「回到底部」按钮可以同时出现。
- **补全弹层贴着输入框，不跟随光标。** 聊天输入里的 `/`、`@`、引用补全浮在输入卡正上方，盖住时间线底部而不挤动它，这是聊天产品的通行做法。NanaUI `TextArea` 自带的光标补全是代码编辑器那套：接受时插入文本，接不上提及标签和对话引用这类结构化动作，所以不用。每行显示命令名或文件名，右侧是说明或路径；方向键移动高亮，最多露出 8 行，窗口跟着高亮滚动。
- **滚动地图。** 右侧细刻度同时标出关键消息和失败步骤，失败用 Danger 色。刻度是带可访问名的按钮，键盘能 Tab 到、读屏能读出消息摘要，所以留在应用侧，不改成画在 `ScrollView` 滚动条上的原生标记：那样只剩指针能点。
- **审阅基线。** 文档审阅对比的是编辑器视图状态里「最近一次载入或保存的文本」，视图同步时不读磁盘，同一 revision 不重算 diff。

## Key Decisions

- 不把整个壳一次性换成 `DockWorkspace`。`DesktopShell` 已提供 navigation / primary / inspector / bottom，第一批只修正分区内容。
- 待处理与 Composer 分离是产品「非打断交互」的布局兑现；发送是否锁定仍由 `blocking_pending_count` 决定，不改契约。
- 壳层装配统一用 NanaUI `Stack` 预设 + `reconcile_children`。
- 通用缺口写入 `nanaui-api-gaps.md`，不在 app 里复制 Dock / 终端 / 编辑器。
