# Issue79：多视图代码编辑与冲突验收矩阵

日期：2026-10-03。本文把 GitHub Issue79 的交付要求转换为可复核的行为条目。`implemented` 只表示代码入口存在，`behavior-tested` 表示有自动化行为测试，`native-verified` 才表示真实桌面窗口回放和证据齐全；前两种状态不能写成桌面能力已交付。

Issue79 标题为 **[Native IDE][Desktop] 完成多视图代码编辑体验与冲突验收**，关联 #15、#51。它要求把 NanaUI 编辑能力接入桌面实际工作流，并完成同一文档多视图、未保存编辑、外部变更、保存冲突和版本一致性的验收。

## 需求矩阵

| ID | 需求与可见行为 | 当前证据 | 状态 | 完成条件／未覆盖边界 |
| --- | --- | --- | --- | --- |
| E-01 | 打开多个不同文档；每个文档拥有自己的 tab、缓冲区和 dirty/save/revert 状态 | `DocumentsModule`、`DocumentStore`；文档服务保存/丢弃测试 | behavior-tested | 仍需 Windows 实窗回放，观察 tab、焦点和关闭重开；测试不能代替窗口证据 |
| E-02 | 同一文档可在两个 pane 中打开；两个视图共享权威文本和修订，但视图身份独立 | `document_workspace_item_view`、workspace 单测，以及 `split_workspace_pane_for_view` 的拆分路径 | behavior-tested（API） | 用户入口代码已接到文档 pane 的拆分动作；仍需真实双 pane 回放，确认第二 view 的渲染、编辑和关闭生命周期 |
| E-03 | 选区、光标、滚动位置和焦点按视图隔离；切换视图不串状态 | serialized state 的 workspace 单测只验证保存/恢复字段 | implemented（部分） | runtime 切换目前主要替换文本；需将每个 `WorkspaceItemId` 的 selection/cursor/scroll 回灌 TextArea，并用双视图交互回放证明隔离 |
| E-04 | 代码语法高亮和行号在代码文档中可见，普通文本不误启用代码行为 | `apply_workspace_editor_chrome` 设置 highlight/line numbers/language；代码行为测试 | behavior-tested | 需按多种语言在 Windows 高 DPI 实窗查看覆盖、裁剪、滚动和字体缩放 |
| E-05 | 查找、下一个/上一个、替换、全部替换使用真实 TextArea 交互并写入共享文档修订 | `EditorSearchView` 与 search/replace 行为测试 | behavior-tested | 需真实回放查找替换、空查询、只读文档、Unicode 和跨视图观察 |
| E-06 | 诊断按文档修订投影；过期诊断不能覆盖较新文本或冲突草稿 | LanguageService revision/UTF-16 测试、`DocumentEditorViewState::sync_diagnostics` | behavior-tested | 需实窗显示 Problems/下划线，并回放异步结果晚到、切换文档和冲突草稿 |
| E-07 | 定义跳转支持一个或多个目标；结果与源文档修订绑定，过期结果必须拒绝 | definition offset/target 与 stale-result 测试 | behavior-tested | 需真实交互点击/选择多个定义并在编辑后验证旧结果不可导航 |
| E-08 | 外部更新到达时保留未保存草稿，明确提供“保留并保存”或“重新载入”，绝不静默覆盖 | DocumentStore 外部变更/保存冲突测试；项目文件 watcher 触发 `observe_open_documents`；`EditorReplaced` stale→SaveEditor 真实回归；冲突按钮走 overwrite/reload | behavior-tested（服务与模块） | 仍需 Windows 回放和真实磁盘 watcher 证据；Linux 单测不能替代系统权限/锁语义验收 |
| E-09 | dirty/save/revert 的状态和磁盘结果一致；保存冲突不能伪造成功 | DocumentService 条件修订写入、atomic save、disk fingerprint 测试 | behavior-tested | 需确认保存成功后所有视图清除 dirty，外部磁盘写入时保存显示冲突且不会覆盖外部内容 |
| E-10 | Agent、DocumentStore 和语言服务引用同一个文档版本；过期编辑/诊断/定义结果不可提交 | LSP 绑定使用 `BufferRevision`；turn context 现在捕获 `document_context_snapshots`（含 revision） | behavior-tested（数据） | WorkspaceEdit/Agent 工具尚未展示或校验该 snapshot 的完整应用契约；需加 stale Agent/编辑回归，证明版本失配不会静默覆盖 |
| E-11 | 差异审阅可读、可导航，并能明确区分工作树/磁盘/编辑缓冲区差异 | `DocumentDiff`、NanaUI `DiffView` 分栏/统一视图，以及按 revision CAS 拒绝块测试 | behavior-tested（UI projection） | 仍需 Windows 实窗回放；“接受”只确认当前缓冲区版本，写盘仍需显式保存 |
| E-12 | 关闭、重开、切换项目和窗口时不丢弃未保存草稿，不把旧异步结果写入新视图 | 部分 workspace/document 生命周期测试 | partial | 需补关闭重开、跨窗口同文档、任务/项目切换和异步取消的端到端证据 |
| E-13 | 真实证据满足 Windows 高 DPI、窄/宽窗口和截图身份记录要求 | Issue82 M-06 规定 editor behavior tests + Windows replay；现有矩阵文档要求记录逻辑/像素尺寸、scale、主题、revision、依赖指纹 | not-tested | 未有 Issue79 专属 Windows 回放、截图或高 DPI 证据；不能以编译或单测代替 |

## 验收规则

1. 每个条目分别记录 `implemented`、`behavior-tested` 或 `native-verified`；未达到 `native-verified` 的条目保持未完成。
2. 测试必须覆盖文档修订、窗口/窗格/资源身份和异步结果的失效条件。测试通过不证明真实绘制、焦点、DPI、文件 watcher 或系统权限。
3. 外部更新的任何处理路径都必须保留用户草稿并给出显式决策。保存调用必须以最新权威 `BufferRevision` 为基准；失败只能显示冲突，不能更新磁盘或清除 dirty。
4. Windows 回放的截图和 JSON 应记录逻辑尺寸、像素尺寸、scale、主题、源码 revision、NanaUI/Mutsuki 依赖来源和二进制指纹。Issue82 矩阵明确说明 #77–#81 不会因矩阵存在而自动关闭。

## 当前阻塞

- 同一文档第二视图的拆分入口已接线，但尚无真实 Windows 双视图回放证据。
- 每视图 selection/cursor/scroll 尚未从持久化状态完整恢复到运行时 TextArea。
- 差异审阅已接入主编辑器的分栏/统一视图，并按 hunk 提供拒绝动作；仍需补多 pane 的实窗回放和接受动作的用户证据。
- Agent turn context 已带文档 revision 快照，但尚未有 stale Agent/WorkspaceEdit 应用回归，统一版本契约仍未完成验收。
- Issue79 要求的 Windows 高 DPI 截图和真实交互回放尚未产出。

## 当前验证记录

配置 `/workspace/.local/sysroot` 的 GUI 依赖后，系统库不再阻塞 Cargo 编译。最近一次回归中 `lilia-feature-document` 23/23、`lilia-feature-automation` 33/33、`lilia-desktop` 637/637 全部通过；`cargo check --locked -p lilia-desktop`、格式检查和差异检查也通过。自动化保存时间戳的同毫秒碰撞已改为严格单调递增，避免过期 revision 冲突被绕过。

该结果只说明已有行为测试可运行，未把 Issue79 的 E-02/E-03/E-10/E-11/E-13 写成完成，也未产生 Windows 高 DPI 截图。

## 相关记录

- [`issue82-acceptance-matrix.md`](issue82-acceptance-matrix.md)：M-06 文件、编辑器、多视图、冲突证据规则。
- [`workbench-functional-inventory.md`](workbench-functional-inventory.md)：文件编辑入口和每视图隔离要求。
- [`task-workbench-refactor.md`](task-workbench-refactor.md)：编辑体验第 10 项及权威数据第 4 项的未验收边界。
- [`desktop-interaction-redesign.md`](desktop-interaction-redesign.md)：同 kind 两个文档编辑器仍待，早期 SplitPane 仅拓扑限制。
