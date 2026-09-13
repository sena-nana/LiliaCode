# 任务工作台重构与验收记录

日期：2026-09-06。状态：实施中，不能作为全部计划已完成的声明。

依赖状态更新：NanaUI 已将原生内容区域、终端网格、只读编辑、Windows 合成和浮层生命周期优化发布到 `67c5955603c80da241bf1930c632490825f5aaf7`，LiliaCode 已更新 Cargo pin。此前 `cargo check -p lilia-desktop --lib` 与 `cargo xtask verify` 均通过；下方早期关于旧 pin 缺少 API 的记录保留为历史证据。

最新发布提交为 `67c5955603c80da241bf1930c632490825f5aaf7`。该提交下的 `cargo xtask verify` 与 `cargo xtask performance` 均通过；性能产物位于 `agent-debug-runs/lilia-performance-1788670790987`，绝对门禁为冷启动 P95 15 秒、帧 P95 100 毫秒、千条时间线 30 秒、空闲 CPU 10%、RSS 1 GiB。

## 目标和边界

对话负责表达意图、阅读结果和审批；文件、差异、终端和浏览器承载执行过程。保留现有项目、任务、历史及配置数据。Windows 是本次完整运行验收目标；其他桌面平台保持接口与条件编译兼容，不能据此宣称其浏览器后端可用。

保留 Cargo-first、唯一 NanaUI/WGPU 桌面实现和 Product/Agent 权威。通用控件与原生窗口能力属于 NanaUI；产品状态、权限和宿主装配属于 LiliaCode；跨端数据先定义在 lilia-contracts。继续使用 Mutsuki 的 Agent 执行器，不建立第二个运行时。

## 当前所有权盘点

| 子系统 | 当前实现与所有者 | 尚需完成的边界 |
| --- | --- | --- |
| 启动、宿主装配 | apps/desktop 的 launcher、DesktopProgram、DesktopApplication | ApplicationKey 已删除；扩展注册、建议生成、轮次提交、Hook 执行和注册文件监视已由独立 Kernel 服务协调。DesktopApplicationInner 仍同时承担启动装配与若干运行时协调，尚未收敛为纯装配 |
| 服务、事件、后台任务 | lilia-kernel 的服务注册、事件和 Jobs；领域 Feature | 按领域发布最小服务，逐一替代万能应用入口；事件补齐实体、版本和缺口重读 |
| 窗口和导航 | DesktopWorkspaceSession、WorkspaceSessionsKey；DesktopProgram 装配窗口 | 主窗口使用 WindowRoute；设置返回已验证任务、窗格和草稿保留，焦点与独立窗口仍需完整验收 |
| UI 投影 | UiModule、UiModuleContext、PrimaryShellSnapshot；模块负责业务投影 | 领域投影借用限制模块可写字段；Memory/Roadmap/Architecture、Timeline、Pending、Composer、Settings 已拥有模块自有视图，TaskView 统一主/独立任务窗口装配；其余域及总快照仍待迁移 |
| 项目和任务 | workspace/task Feature 与应用服务；Task/Timeline/Composer UI 模块 | 补全跨窗口共享业务状态与独立视图状态；任务切换不能串草稿、审批或结果 |
| 文档 | document Feature 的文档、缓冲区、修订、诊断；DocumentsModule | 接通所有窗格身份、共享文档与独立光标/选区/滚动；完整保存冲突和异步版本校验 |
| 编辑器呈现 | NanaUI TextArea；runtime_shell 产品装配 | 本次接通代码行为、行号和诊断范围；查找替换、定义、差异交互仍需逐项接线验收 |
| 终端 | LiliaCode 持有 PTY、进程、工作目录和权限 | 本地 NanaUI 终端网格及 PTY 输入/尺寸已接线；底部终端工具区和真实完整交互尚未验收 |
| 设置、扩展 | SettingsView 持有管理页与控件生命周期，Settings/Extensions 模块和配置服务提供数据 | 设置呈现已从 Shell 移出；继续收敛业务编辑状态、投影和服务访问范围，补齐全部业务表面 |
| 架构、路线图、记忆 | 三个 Feature 持有 mutation 事件发布，UI 模块使用类型化领域服务；Roadmap 校验里程碑项目归属，Architecture 校验项目与任务归属 | 项目资源导航及按模块挂载尚未完整迁移 |
| 自动化 | 产品自动化服务、桌面管理入口与 `DesktopAutomationOperationService` | 内核作业只注册自动化运行服务；服务直接拥有产品权威、任务、待办、时间线事件与窄 Agent 回合端口。继续将 Agent 回合适配从 DesktopApplication 兼容门面中抽离，并验收管理页返回、运行状态与任务跳转 |
| 浏览器 | lilia-agent 原生浏览器工具与应用 Windows WebView2 平台宿主；保留旧 BrowserGateway snapshot 兼容路径 | 旧 HTTP snapshot 路径不是新页面操作实现；本地 WebView2 合成、任务标签与 Agent 工具已接线，完整运行验收仍未完成 |

## 已有框架能力与本次改动

核对的 NanaUI Git pin 为已发布提交 `bf6c9c657a56f567dfdefc528cbd0ca3d6f9b89f`，Mutsuki pin 为 `bb728d20e89ef65dffda882c70e5c9f70ee48b11`。后续以 Cargo.toml、Cargo.lock 和实际依赖图为准。

NanaUI `TextArea` 已具备代码缩进/括号行为、highlight、line_numbers、diagnostics、match_spans、折叠、补全、hover、签名、inlay 与 git gutter，不需要新造编辑器内核。`AppContext` 提供 `find_next_focused_text_match`、`find_previous_focused_text_match`、`replace_focused_text_match`、`replace_all_focused_text_matches` 和 `select_focused_text_range`。查找替换已接入主及额外文档窗格，默认收起；执行前校验编辑器的实时资源绑定并聚焦该编辑器。替换经正常 TextChanged 和文档修订流程持久化。资源切换清空查询，关闭窗格清理未挂载和已停放控件。原 pin 的查找 API 拒绝不可编辑输入；本地 NanaUI 已新增独立 TextArea.read_only 并允许选择、查找、复制，应用消费者已接线，两窗格运行/行为结果见后续阶段记录。正式 pin 尚不包含此能力。

本次 runtime_shell 改动保留诊断的类型与 UTF-8 字节范围，向各编辑视图提供 `TextDiagnosticSpan`；无效范围不投影下划线，问题列表保留原消息。代码视图按语言设置注释/缩进，预留行号栏，终端日志不启用代码行为。Unicode 范围回归测试已加入；最终通过情况以执行验收产物为准。

编辑与终端动作已显式携带窗口、窗格和资源身份；非活动窗格的有效动作可执行，关闭或重新绑定后的动作拒绝。文档编辑携带基准修订号，拒绝写入后保留该视图的本地草稿；刷新不会用共享内容覆盖冲突草稿。重新载入只取共享权威缓冲，不撤销其他视图已写入的内容；保留并保存仍须通过当前修订校验。清空文档也是有效编辑。冲突草稿编辑不递增共享修订，过期诊断与定义结果不覆盖草稿。

TimelineModule 按任务保留阅读位置，并缓存最近八个任务的分页历史。缓存历史仅与成功重读的权威快照合并，不恢复旧审批或运行状态。超过缓存范围的深分页位置尚无锚点重建，不能声明任意数量任务都可精确恢复。普通对话默认呈现正文，过程事件默认摘要；主窗口和独立窗口使用相同内容投影。时间线次要操作使用横向工具条，空态内容在有任务正文时退出挂载。

主窗口与独立任务窗口的实际输入控件现在发出带窗口、任务和修订的命令，文本修改还携带编辑前内容，并复用权威 Composer 的条件写入。输入队列不得回落到当前其他任务；同任务冲突显示反馈，跨任务旧事件拒绝。附件选择器记录发起窗口与任务，返回时重新校验。独立窗口审批必须属于该窗口当前开放请求，不再对未知动作回落主窗口。仍存在内部同步和调试兼容命令；独立窗口资源工作区已通过 WorkspacePaneView 复用；完整运行生命周期标识与跨窗口真实操作仍待验收。

领域投影类型和类型化导航是渐进替换的第一步；万能应用服务已删除，但总快照和集中业务消息仍在。不得把中间适配层写成最终架构完成记录。

Memory 和 Roadmap 的 mutation 事件已从 DesktopApplication façade 移到各自服务，启动装配与 Feature 挂载绑定同一 EventBus；façade 只转发调用，不重复发布。两个 UI 模块通过 MemoryServiceKey / RoadmapServiceKey 获取领域服务。Roadmap 服务操作使用 ProjectId，并在同一服务锁内完成里程碑项目归属校验与修改；跨项目更新和关联拒绝，跨项目删除保持无操作返回。事件在持久化成功、释放服务锁后发布；失败写入和无操作删除不发布。后续 Settings、Hook 文档与执行、Composer、Documents 已改为显式领域服务；ApplicationKey 及其 Feature/UiModuleContext 入口已删除。DesktopApplication 仍作为启动装配和兼容门面存在，进一步收敛尚未完成。

ArchitectureService 必须注入共享 ServiceAuthority，直接调用服务也会校验项目存在、任务所属项目；UI 使用 ArchitectureServiceKey。apply、reject 和实际 rollback 在持久化后发布事件。reject 通知历史记录变化但不递增图版本；失败修改和无操作 rollback 不发布。9 项 Architecture 测试通过，包括事件回调读取已提交状态及跨项目拒绝。

## 浏览器实施决策与限制

使用 Windows WebView2 的原生合成承载页面，Agent 操作宿主内部 DevTools 适配层；不以连续截图回读呈现网页。`ICoreWebView2CompositionController::RootVisualTarget` 可接 DirectComposition visual，鼠标和指针由宿主转发，尺寸、可见性和焦点走其 controller 接口。参见 [Microsoft composition controller 文档](https://learn.microsoft.com/en-us/microsoft-edge/webview2/reference/win32/icorewebview2compositioncontroller?view=webview2-1.0.4129.50)。

NanaUI 本地 checkout 已新增 NativeContent 合同、布局后 NativeContentRegion 与 WindowsComposition 宿主模式。框架管理窗口、DirectComposition 根、原生 visual、WGPU 透明开口和文档绘制顺序；应用平台层管理 WebView2 controller。普通控件不取得 HWND。这些接口尚未发布到正式 Git pin。当前原生区域只支持矩形裁剪和纯平移，不支持透明度/滤镜离屏组或曲线裁剪；实际工作台布局必须验证兼容这些限制。

必须先解决 NanaUI 浮层与浏览器的合成顺序。直接覆盖 HWND 的浏览器会遮住菜单和审批框；不能用隐藏浏览器或假截图冒充完整浮层支持。需要统一合成树中的 UI 层次或正确的遮挡裁剪，并真机验证复杂交叠、输入法、高 DPI 和窗口缩放。旧 `docs/gpu.md` 的 WebView HostTexture 部分只是未实现提案，不能据此声称 WebView2 已能提供 WGPU 可采样纹理。

项目隔离浏览资料；任务绑定独立标签页。人工接管暂停 Agent，恢复显式触发。导航变化后旧页面目标失效；上传、下载、新窗口、审批、取消和关闭期间的操作均经宿主处理。页面不获得本机命令桥，凭据不写入任务日志。

Mutsuki 现有 `BrowserGateway` 仅有 `snapshot(&BrowserNavigateRequest)` 和 `cancel(handle_id)`；请求只有 URL 与 limits，不能表达任务标签页、目标、输入、滚动或观察版本。不能通过滥用 URL 补足这些语义。Lilia 现有 `AgentToolDescriptor` + `ToolRegistry` + `PluginBuilder` + `TaskAwaitRunnerAdapter` 原生工具路径可以承载应用浏览器工具，无需新增 Mutsuki 执行器。当前已定义 lilia-contracts 浏览器请求/结果，并经原生工具路由及受控消息桥派发到 Windows UI 线程。工具注册不自动保证权限、取消或任务隔离，这些仍需按真实行为接入和测试。

## 剩余实施里程碑

以下清点基于第二阶段当前源码，状态中的“已实施”仅指代码路径存在，“未验收”表示尚无覆盖该项的完整运行证据。混合状态必须逐项完成，不能按整行认定完成。CodeGraph 索引仍指向旧 Web 文件，本次以现行 Rust 源码核对。

| 计划项 | 当前实施状态 | 剩余工作及验收条件 |
| --- | --- | --- |
| 1. 功能、数据与性能基线 | 部分实施：已有阶段差异、行为测试、截图和性能记录 | 已有[功能与所有权清单](workbench-functional-inventory.md)，按入口、命令组、后台生命周期和跨端消费者列出验收单位；仍需逐控件补运行证据。已有实施后结果不能补称无改动基线。保留第一阶段性能原记录 |
| 2. 领域服务与万能入口 | 已移除 ApplicationKey/Feature 和 UiModuleContext::application；Memory、Roadmap、Architecture、Settings、Hook 文档与执行、Composer、Documents、扩展注册、建议生成、轮次提交、注册文件监视及自动化作业使用显式服务 | 未完成 DesktopApplication 的纯装配收敛。自动化 Feature 已注册 `DesktopAutomationOperationService`，不再将整个 DesktopApplication 作为 `AutomationOperationPort`；服务直接拥有产品权威、任务查询、待办服务和时间线事件，Agent 回合通过窄端口适配。轮次提交栅栏、耐久队列及提交存储已收敛在 TurnSubmissionService；Hook 源解析、插件 Hook 装载、每轮执行栅栏及完成/失败持久化已收敛在 HookExecutionService；注册文件监视持有路径、项目查询、事件发布及自身线程生命周期。Todo/guide dispatch、Update、Provider runtime settings、ProjectSettings、Worktree、ProjectFiles、ProjectCommandRun、SessionSearch 已归服务；将剩余校验、存储和协调迁入真实领域服务及显式 ports，不能仅改字段名 |
| 3. 模块自有视图 | 部分实施：模块拥有类型化消息、编辑状态及受限 Projection 借用 | Memory/Roadmap/Architecture、Timeline、Pending 与 Composer 已由模块自有 view 管理生命周期，TaskView 组合任务正文、审批和输入区，Shell 挂载其区域。UiModule 注册表仍未统一视图生命周期，ErasedUiModule::project 仍写 PrimaryShellSnapshot，其他域仍集中持有控件并 sync；逐个迁移后删除平铺总快照、重复桥接和 shell 业务路由 |
| 4. 权威数据与每窗口状态 | 已有共享文档服务、条件修订写入和窗口/窗格/资源目标；主/辅助窗口复用 WorkspacePaneView | 未验收同一文档双视图光标、选区、滚动和焦点独立；任务关闭期间异步取消及生命周期校验尚需全链核对。业务事实继续由共享 Product/Agent 权威持有 |
| 5. 统一导航与恢复 | WindowRoute 已区分工作区/设置/自动化/项目，管理页面返回记录已接线；PrimaryShellSnapshot 直接携带该路由，设置和自动化不再有独立布尔写入来源 | 逐验各管理页、多窗口、项目资源返回与焦点恢复；不能只以任务/草稿恢复测试代替 |
| 6. 局部事件与持续输出 | 多个领域已在持久化后发布类型化事件；Composer 修订写入事务化；终端已有有界输出路径 | 未完成所有事件的实体/版本/缺口重读统一审计；多数 UI 仍通过总快照同步。需要验证大量输出合并、落后订阅恢复和任务切换期间旧结果拒绝 |
| 7. 任务对话主路径 | 已有对话、底部输入、审批、跨任务草稿和有限历史缓存；主/辅助窗口共享业务路径 | 未完成全部固定搜索/新建入口、等待处理/失败可见性、模型/权限选择器及焦点可用性的统一验收；八任务之外的历史位置恢复仍有限制 |
| 8. 右侧资源与底部工具 | 已有真实 SplitPane 和 WorkspacePaneView；文件、文档、终端、浏览器可作为资源窗格；问题列表接入 shell.bottom | 终端专用窗格已进入底部工具区域；新建/显示终端将其与文档、浏览器分离，旧混合窗格保留直到显式显示。问题列表保留。分类与窗格交互测试通过；完整实际尺寸/焦点/关闭恢复仍待验收 |
| 9. 窄窗与宽窗交互 | 宽窗真实分栏及独立任务窗口已接线 | 已实现每窗口 CompactWorkbench 对话/工作区切换，保留原视图、资源、草稿及选区；NanaUI 焦点域支持切换返回原控件，行为测试通过。辅助抽屉、实际窄窗、移至新窗口和高 DPI 仍待完整运行验收 |
| 10. 编辑体验 | 现有 NanaUI 代码文本能力接入行号、诊断、查找替换、定义与冲突处理；文档服务共享内容 | 本地 NanaUI 只读属性与查找已补齐，消费者验收中。语法高亮覆盖、定义多目标、差异审阅实际操作、外部修改冲突、关闭重开和双视图都需完整运行验收；不能将普通文本或差异数据投影视为完整审阅通过 |
| 11. 终端 | 本地 NanaUI 网格、光标、选择、IME、键盘及尺寸事件已有实现，应用连接 PTY | 未验收真实输入/复制粘贴/中断/退出后复制、频繁 resize 和长期大输出；底部工具布局另见第 8 项。不扩展调试器或运行配置系统 |
| 12. 设置、自动化与项目资源迁移 | 原有功能入口保留；设置/扩展服务边界已拆；架构/路线图/记忆有领域服务 | 原项目页面和 inspector 投影仍在，尚未完成所有项目资料作为同一资源体系打开；逐项验证设置/自动化管理页返回、任务跳转、架构编辑/回滚、路线图和记忆操作，无功能遗漏 |
| 13. 原生浏览器与 Agent | 已有契约、WebView2/DirectComposition、任务标签、项目资料隔离、页面版本、人工接管、工具操作和宿主请求路径；Agent Debug 已验证导航、接管/恢复、子窗口批准与拒绝，以及经五次审批的观察、输入、点击、滚动和截图 | 仍需真实 IME、缩放、高 DPI、分栏和 NanaUI 浮层遮挡验证；上传下载完成、活动 CDP 操作中的取消和远程模型质量未覆盖。空 native visual probe 不证明网页合成通过 |
| 14. 浏览器失败恢复 | 已有失败卡“重试”和“安装浏览组件”：重试关闭旧宿主实例并清理 visual/error，后续布局重新创建；安装入口打开微软 WebView2 页面 | **运行时失败入口已接线、未完整验收**：不是应用内安装器。**标签页持久恢复已通过编译与领域测试，实际进程重启验收待完成**：BrowserTabRestoration 仅保存 scope/url，workspace 校验任务/项目存在、未归档及 tab/item/resource 身份；主窗口拓扑新增可选 primary，浏览器管理器重新绑定实例。正常修订变化不归档；无法解析或结构裁剪/迁移保存原字节，归档失败禁止覆盖，恢复期间暂停拓扑写入。缺失 Runtime、初始化失败、安装后重试、进程退出及取消也需验收。现有 surface recovery probe 不等于 WebView 崩溃恢复 |
| 15. 数据迁移与兼容 | 保留现有权威数据路径与显式导入约束；未以新执行器替代 Mutsuki | 旧项目/任务/历史/配置、旧布局失败回落并保留原记录的完整验收尚未完成；macOS/Linux 条件编译及不可用提示须验证，不能宣称三端浏览运行通过 |
| 16. 清理与最终交付 | 万能注册入口已删除；设计记录持续更新；本地 NanaUI overlay 与正式 pin 分开记录 | 总快照、集中装配与剩余兼容适配器尚未清理完。完成全部现有功能回归、明暗/窄窗/DPI/GPU 截图及第二阶段 verify/agent-debug/performance；分别报告绝对门禁和历史差异，正式 pin 尚不能提供本地新增接口 |

建议继续顺序：先完成当前新增边界的行为测试与整应用运行，随后实现模块自有视图接口、底部工具和窄窗模式；同步补齐剩余服务所有权与项目资源迁移，再做数据恢复及全量最终验收。浏览器真实交互验证应尽早进行，以便发现原生合成限制；不等待文档标记全部完成后才验证。

## 验收矩阵与证据

| 路径 | 必须观察的行为 | 当前证据状态 |
| --- | --- | --- |
| Memory/Roadmap 服务 | 直接与挂载服务发布一次事件；失败修改不发布；跨项目修改不落库；事件回调可读取已提交状态 | `cargo test --locked -p lilia-feature-memory -p lilia-feature-roadmap` 通过：20 + 11 项；桌面交互仍待完整运行验收 |
| Architecture 服务 | 项目与任务归属校验；成功修改发布一次；拒绝历史可观察；失败无事件 | `cargo test -p lilia-feature-architecture` 通过：9 项 |
| 任务与审批 | 切换任务保留草稿/阅读位置；审批和异步结果不串任务 | 草稿切换真实场景通过；输入身份与冲突单测通过；审批、长历史恢复和完整异步生命周期仍待验收 |
| 管理页返回 | 设置/自动化返回原任务、窗格及焦点 | 真实场景验证任务、活动窗格与草稿保留；焦点仍待完整验收 |
| 双窗格/多窗口 | 同类资源同时可编辑；来源身份正确，光标/滚动独立 | 窗格来源、过期控件拒绝、清理行为单测通过；多窗口资源 UI 已接线，完整真实操作待验收 |
| 文档 | 诊断 UTF-8 范围准确；查找/替换/定义正确；外部修改不覆盖未保存内容 | UTF-8、替换焦点、清空、冲突草稿与过期语义结果行为测试通过；完整实际编辑交互验收尚未覆盖 |
| 终端 | 直接输入、复制粘贴、中断、尺寸同步、退出状态；大输出有界 | 本地 NanaUI 网格/IME/选区/只读控件测试及应用宽字符与 ANSI 映射测试通过；真实 PTY 整体交互仍待验收 |
| 浏览器 | 原生合成、IME/DPI/遮挡、页面版本、接管、审批拒绝、取消和关闭 | 本地 Windows WebView2 合成后端与 Agent 操作已接线；5 项 Agent 行为测试通过；真实整应用浏览、IME/DPI/遮挡和文件请求仍待验收 |
| 迁移 | 原有项目/任务/历史/配置可读，失败不破坏原数据 | 待完整验收 |
| 视觉/性能 | 明暗主题、窄窗、高 DPI、长时间线、大输出 | 明暗任务/设置截图及千条时间线性能已采集；窄窗、长期大输出、完整 DPI/焦点矩阵未验收 |

执行 `cargo xtask verify`、`cargo xtask agent-debug`、`cargo xtask performance`。UI 证据保存在 `agent-debug-runs/lilia-*`；性能分别报告绝对阈值与历史基线。NanaUI 变更另做通用控件、真实窗口/GPU与截图验证。编译通过不代替界面验收。

NanaUI 调查期间存在持续变化的未提交修改，涉及 Runtime 组件、事件、几何、场景合成、GPU 纹理/绘制、宿主窗口与调度，并有未跟踪的 application.rs。不得覆盖或把其已有改动归为本次成果；修改前重新检查差异并协调所有权。本地联调与正式 Git pin 验证分别记录，未提交/发布的变更不能伪装为可获取依赖版本。

本记录不包含提交、推送、PR 或正式发布授权。

## 阶段运行证据

以下是实施中的阶段证据，晚于这些运行的改动仍须重验，不是完整计划的最终验收。

- `cargo xtask verify`：通过，桌面 452 项测试及工作区测试、依赖 pin、边界和 workspace check 均通过。日志：`target/refactor-baseline/verify-final.log`。
- `cargo xtask agent-debug`：`agent-debug-runs/lilia-agent-debug-1788598164624` 通过设置、自动化往返与跨任务草稿隔离，并要求首次输入实际保留 19 个字符。`task-light.json/png`、`task-dark.json/png` 为匹配的产品观察与真实窗口截图，已复查正文默认展开、工具条横排、空态不与正文并存。`secret-canary.json` 通过，允许的唯一已知错误来自刻意注入的无效 MCP fixture。临时草稿标记和输入模块在导航时统一清理，避免出现有草稿标记却没有输入状态的情况。日志：`target/refactor-baseline/agent-debug-final.log`。
- `cargo xtask performance`：`agent-debug-runs/lilia-performance-1788598213929/performance.json` 的绝对门禁全部通过。运行使用未优化 dev 构建、正式 NanaUI Git pin；没有 NanaUI 本地路径覆盖验证，也未完成浏览器/终端网格框架验收。日志：`target/refactor-baseline/performance-final.log`。

| 指标 | 本次阶段值 | 最近可用历史值 | 绝对门禁 |
| --- | ---: | ---: | ---: |
| 冷启动 P95 | 5864.63 ms | 1601.42 ms | 15000 ms |
| 输入帧 P95 | 14.88 ms | 32.41 ms | 100 ms |
| 分栏调整帧 P95 | 20.37 ms | 118.98 ms | 100 ms |
| 千条时间线就绪 | 612.70 ms | 1103.73 ms | 30000 ms |
| 空闲 CPU | 0.0977% | 0.1953% | 10% |
| 空闲 RSS | 253407232 bytes | 271142912 bytes | 1073741824 bytes |

历史来源：`agent-debug-runs/lilia-performance-1787663826614/performance.json`，其分栏调整门禁失败。本次五次冷启动中四次为 1.42–1.53 秒，一次为 5.86 秒；冷启动 P95 约为历史的 3.66 倍，不能声明历史基线无退化。其他延迟和 RSS 较低。历史环境和构建未完全对齐，冷启动长尾尚未归因，不能将差异解释为本次改动的因果性能收益。


## 第二阶段集成状态（本地 NanaUI）

- Settings、Hooks、Composer、Documents 的校验、状态及事件进入显式服务；UI 模块注册表不再暴露整个 DesktopApplication。Composer reducer 与持久化使用同一锁，拒绝同 revision 的并发覆盖。
- 主窗口和任务独立窗口复用 WorkspacePaneView；窗格事件携带窗口、窗格和资源身份。终端网格直接连接 PTY 输入/尺寸事件，退出后保留选区复制。
- WindowsComposition 的真实主/辅助窗口 present、surface recovery 和健康设备重建探针通过；这不等于设备移除故障注入或整应用浏览验收。日志在 NanaUI/target/refactor-baseline/composition-*.log。
- 本轮构建缓存使用 E:/codex-build/lilia-workbench（NanaUI 使用 nanaui-workbench），CARGO_INCREMENTAL=0。原 D 盘 incremental 缓存已完整搬到 E:/codex-build/lilia-incremental-archive-20260905 保留，未删除用户数据。
- 本地联调通过 target/refactor-baseline/nanaui-local.toml 显式 patch sibling checkout。正式 NanaUI Git revision 保留 e90867d8972b11a42febda586986ef1abbbfcb98，不能据本地测试声明该远端版本已包含新接口。
- 本轮全量应用测试与真实窗口回归进行中。上方第一阶段 verify/debug/performance 数字只属于当时版本，不能替代本轮验收。


## 2026-09-05 后续回归记录

- 本地 NanaUI 联调 `cargo test -p lilia-desktop --lib`：473/473，通过日志 `E:/codex-build/lilia-workbench/workbench-integration-tests-5.log`。包括主/辅助浏览器真实布局几何、窄窗切换焦点及恢复数据保护；不代表此后服务和视图迁移已验收。
- `agent-debug-runs/lilia-agent-debug-1788606790501` 的原生 WebView2 导航、人工接管、显式恢复及新窗口拒绝协议场景通过，但截图复查发现右侧标签栏分走大量高度、地址工具条溢出，以及导航期间地址被错误置为 about:blank。本次验收明确不通过视觉检查。
- 已修复 PaneChrome header/tabs/body 归属、地址输入可收缩及控制按钮独立行，并以 NavigationStarting URI / SourceChanged 更新权威地址。主/辅助窗格尺寸测试通过，增强后的真实窗口场景重新运行中；新增批准新窗口后独立标签身份断言。
- 真实 Windows PTY 行为测试 3/3 通过（`E:/codex-build/lilia-workbench/terminal-reentrant-tests.log`）：ConPTY 启动光标查询响应、跨读取边界协议处理、输出/退出及同步事件订阅可重入。完整用户复制/粘贴/中断与长时间大输出验收仍待完成。
- Update 状态、操作串行化、版本校验及进度事件已进入 DesktopUpdateService，宿主通过 UpdateHostPort 注入，应用兼容函数仅转发；该后续改动等待统一回归。
- 本地 patch 下 `cargo xtask verify` 已运行：边界检查通过，随后在 `git_pin_missing_from_lockfile` 停止，日志 `E:/codex-build/lilia-workbench/verify-local-overlay.log`。这是本地路径锁文件与正式 pin 门禁的区别，不记为 verify 通过。正式 pin 未更新，仍不能提供新增 NanaUI API。


### 后续边界迁移（待统一回归）

- Memory 控件进入模块自有 MemoryView；typed snapshot 替代原 memory 平铺字段。挂起保留节点及选区，返回恢复焦点，项目身份变化卸载旧视图。Roadmap 按此方式继续迁移；其他域尚未因此获得自有视图，PrimaryShellSnapshot 仍未全部删除。
- ProviderRuntimeSettingsService 接管 SQLite 配置、修订校验、运行时应用、失败回滚和提交事件，使用窄 ProviderModelRuntimePort；与凭据路径共享 provider revision。
- Update 服务首轮真实组合启动发现 Feature ID 与既有 Jobs Feature 冲突，`agent-debug-runs/lilia-agent-debug-1788607758757` 未能启动，不能记作通过。现已区分原 Jobs owner 与 update-operations 服务注册，新增完整 KernelHost 启动行为测试，等待重跑。
- 用户停止操作携带 task/turn 身份，领域 request_cancel_at 在同一锁内比较当前 turn 后才取消。旧按钮、排队事件及待审批卡不能取消该任务后来启动的 turn。无活动 turn 的停止入口禁用或不挂载；远端兼容取消入口不等于 UI 已失去身份保护。
- 浏览器私密输入与运行期 Product scope 校验的实现、覆盖边界和限制见 task-browser-host.md；包含产品移项目/归档与异步撤销的新增测试，最终结果待统一记录。


### 本地工作区统一测试

`cargo test --locked --workspace` 本轮通过：69 个测试套件共 907 项通过、0 项失败、1 项默认忽略，桌面库为 485/485。日志：`E:/codex-build/lilia-workbench/workspace-integration-tests-2.log`。包含 Provider/Update 完整装配、任务停止轮次、Pending 投影身份、Memory/Roadmap 视图生命周期及浏览器 Product 权威适配器；此前 PendingActionView 丢失 task/turn 的编译问题已修复。

默认忽略项为 `shared_lsp_surfaces_unsaved_rust_diagnostics`。本机原先无 rust-analyzer，已通过 rustup 安装当前工具链组件，版本 `1.96.0 (ac68faa2 2026-05-25)`，该测试将单独执行。此测试记录使用本地 NanaUI 覆盖，不能替代正式 Git pin 检查或真实界面验收。


### 后续服务与原生控件回归

- `cargo test --locked -p lilia-desktop --lib`：490/490 通过，日志 `E:/codex-build/lilia-workbench/workbench-integration-tests-6.log`。新增 Todo 直接服务、Architecture 双区域视图、浏览器窄工具条与按钮可见性验证；晚于此运行的 Timeline/ProjectSettings 迁移不在该结果内。
- `cargo test --locked -p lilia-feature-agent-session -p lilia-xtask`：41/41 与 13/13 通过，日志 `E:/codex-build/lilia-workbench/todo-and-browser-harness-tests.log`。Todo 更新和删除使用 SQLite Immediate 事务，两个独立 Store 句柄并发修改不同字段不互相丢失；事件在提交后发出，回调可以重读。
- NanaUI runtime 测试 804/804 通过，日志 `E:/codex-build/nanaui-workbench/runtime-workbench-tests.log`。PaneChrome 图标按钮切换会清理旧文字节点；Button 的单行布局与单行绘制一致，避免中文“恢复 Agent”按可换行最小宽度计算后被截断。
- `agent-debug-runs/lilia-agent-debug-1788610788494` 原有浏览器场景通过，包括批准新窗口后的独立标签及继承人工控制状态。匹配的 `browser-navigation.png` 已人工复查：地址框及打开按钮在窗格内、控制按钮完整、标题图标无残留文字重叠、原生页面占据正文区域。
- 同一次默认 `cargo xtask agent-debug` 的新增真实 Agent profile `lilia-browser-agent-1788610962310` 失败：harness 寻找不存在的 `.resume`，真实视图只提供随控制状态变化的 `.control`。因此整个命令仍记失败，不能用第一 profile 通过代替。修复后使用同一入口 `cargo xtask agent-debug --profile browser-agent` 定向复验，默认入口仍覆盖两个 profile。


### 实际语言服务器验证

安装 rust-analyzer 后，直接执行工作区测试二进制中的 `shared_lsp_surfaces_unsaved_rust_diagnostics --ignored` 首次失败：3.60 秒内未收到诊断。原测试只有 100 次、20 ms 间隔轮询，没有为首次工作区索引留足时间。改为 45 秒有界截止时间，要求真正的 error severity 诊断，并断言磁盘源码仍是未修改内容。`cargo test --locked -p lilia-agent --lib shared_lsp_surfaces_unsaved_rust_diagnostics -- --ignored --nocapture` 已通过 1/1，测试实际耗时 10.14 秒，日志 `E:/codex-build/lilia-workbench/lsp-real-diagnostics-2.log`。没有以固定睡眠、忽略错误或写入文件触发保存检查替代未保存诊断；该外部工具测试仍保持显式 ignored 入口。


### 文件、工作树与时间线迁移（统一回归中）

ProjectFilesService 拥有目录读取、文件打开校验、项目监听和修订号，使用 ProjectTaskService 与共享 DocumentService，不捕获整个 DesktopApplication。监听通知改为容量 1 的合并失效通知，消费者重读目录权威；ProjectFilesChanged 携带项目、根目录和修订。目录改绑/归档后旧监听拒绝发布，监听替换和停止在服务锁外 join，线程内回调停止自身不 self-join。文件内容读取有硬上限，读取结束再次核对项目目录归属。新增实际文件变更、回调重入停止、服务释放和目录改绑行为测试，结果待下轮记录。

DesktopWorktreeService 使用显式任务、偏好、Git ports，Feature 与应用共享唯一 Store。预约先按任务、再按 Git common-dir 串行化；同步回调不能再次申请工作树操作。首个副作用之前可取消，副作用开始后继续保存已经发生的事实，不能将取消写成回滚成功。Git 期间任务改绑或归档导致旧操作返回失败，已经创建的旧绑定保留为可核查事实，后续不能被新任务范围操作。合并尝试使用原工作树表的兼容新增列记录，解决 merge 已完成而 remove 失败后的重试；旧表数据保留测试已补。此阶段不宣称跨进程 Git 事务或任意外部工具并发具有原子性。

共享 TimelineView 已同时接入主窗口和独立任务窗口，复用虚拟化、展开、复制、重试与加载更早。用户动作捕获窗口和任务，拒绝关闭或切换后的旧目标；滚动使用 NanaUI 布局后 anchor 恢复，替代被主窗口提前消费的辅助窗口相对滚动命令。Pending 已迁移到模块自有共用视图，Composer 控件尚未共享；TimelineRow 已进入时间线模块，主总快照仍未全部迁移完成。

真实浏览器 Agent profile 第二次运行 `lilia-browser-agent-1788611772418` 暴露资源导航缺陷：WebView 已恢复 Agent 控制，但选中任务被 WorkspaceState::sync_selection 清空，中心退回新任务空态。匹配 PNG 与观察 JSON 均证实此状态。资源项激活/刷新/关闭现改为保留任务上下文，显式任务/项目/管理页导航继续决定任务；feature-workspace 的 15 项测试通过（含浏览器、文档、终端），日志 `E:/codex-build/lilia-workbench/workspace-resource-context.log`。真实流程仍须重新运行。


### 本批领域回归结果与剩余失败

`cargo test --locked -p lilia-desktop --lib` 第 8 轮完成：503 通过、2 失败，日志 `E:/codex-build/lilia-workbench/workbench-integration-tests-8.log`。ProjectFiles 5/5、ProjectSettings 5/5、Worktree 12/12 全部通过，包含真实 watcher、Git 同 common-dir 并发、任务中途改绑、部分合并重试及旧表迁移。

失败为：共享 TimelineView 的实际滚动 offset 恢复断言，以及资源上下文修复影响了跨窗移走最后一个任务视图后的选择清理。两项均按实际用户行为修复后重验，不能因领域测试通过记整个命令通过。第 7 轮编译错误（Project.archive 字段与测试读取框架私有 pending_anchor）已修；行为测试继续使用实际布局/滚动结果，不公开框架私有字段。


### 只读编辑与后续服务

本地 NanaUI 的 read_only_tests 3/3、framework::text_edit 回归 34/34 已通过，日志 `NanaUI/target/refactor-baseline/read-only-runtime-tests.log` 与 `read-only-text-edit-regression.log`。只读是可聚焦、可选取的阅读状态，禁用是不可交互状态；IME、进行中拖放与替换不能绕过只读。Lilia 的两个文档窗格已保留查找、禁用替换，并补实际双窗格行为测试，等待整批应用回归。

ProjectCommandRunService 已接管运行中项目命令与启动预约，复用现有 TerminalService/PTY 和 Product 项目。进程启动在预约锁外执行；非并发命令的同时启动被拒绝，失败释放预约。允许并发的命令保留所有活跃 session，最新进程退出不会掩盖仍在运行的较早进程。命令启动前后检查项目目录，发生改绑则停止旧目录新启动的进程并报告失败。新增实际进程并发、目录回读与失败重试测试待运行。未引入新运行配置格式。

SessionSearchService 持有从 Product 事实派生的搜索语料，应用门面仅转发。语料仍每次按权威项目/任务指纹校验，不能因事件缺口长期返回旧名称或已归档任务；新增直接服务在重命名与归档后的行为测试待运行。


### 共享审批与进程退出回归

第 10 轮桌面库测试 510 通过、3 失败（`E:/codex-build/lilia-workbench/workbench-integration-tests-10.log`）。共享 TimelineView 的真实布局/滚动测试 2/2、PendingView 七类审批及双宿主生命周期测试 3/3、只读双窗格、SessionSearch 3/3 均通过。PendingView 已统一主/任务窗的输入、自由答案、工具许可及停止动作；事件携带 window/task/request 或 turn，切换请求会释放旧控件的焦点和编辑态。完整 ComposerView/TaskView 仍未完成。

失败的 3 项均为 ProjectCommandRunService 的真实进程终止测试。根因是 portable-pty 0.9.0 Windows WinChildKiller 将 TerminateProcess 的非零成功返回解释为错误，暴露线程上残留的系统错误码。Terminal feature 增加 Windows ProcessKiller，复制 Child 的原生进程句柄并持有 OwnedHandle，按系统返回正确判断，不按 PID 重新打开进程。自然退出竞态通过等待句柄的已退出状态确认；其他真实失败仍返回错误。未修改 Cargo registry 缓存。系统约定见 [TerminateProcess 官方文档](https://learn.microsoft.com/en-us/windows/win32/api/processthreadsapi/nf-processthreadsapi-terminateprocess)。

修复后的 Terminal 库测试 5/5（`terminal-process-tests.log`），项目命令服务测试 4/4（`project-command-process-tests.log`），日志均在 `E:/codex-build/lilia-workbench/`。真实并发进程预约、较新进程退出仍保留较早进程、启动失败后重试均通过。

浏览器 Agent profile 第 7 轮 `lilia-browser-agent-1788615154900` 已通过真实 Settings provider 保存、工具注册与审批，但批准 Observe 后返回 WrongScope。任务会话是在此前浏览器 UI 通知之后创建，原绑定仅在 BrowserPoll 更新，缺少下一次通知就无法取得范围。当前将选中范围保存在 BrowserSessions，在 Agent 提交轮次、读取 Product session binding 后绑定对应任务浏览器；UI 通知仍负责显式更换标签。批准、页面版本、人工控制和 Product 权威校验保留。修复后的真实流程正在重跑，不记作已通过。


终端修复后的桌面测试二进制完整运行 513/513 通过（`workbench-integration-tests-11.log`），构建来源为同轮 `project-command-process-tests.log`，早于随后 browser selected-scope 修复。NanaUI 当前库测试 815/815 通过（`E:/codex-build/nanaui-workbench/runtime-workbench-tests-2.log`），覆盖本地只读选择/输入拦截。两个结果均为本地路径依赖，不代表正式 Git pin 或浏览器全链路通过。


### 真实浏览器批准、拒绝与取消的后续缺陷

第 8 轮 `lilia-browser-agent-1788616687108` 在真实 Agent 审批后完成五次工具操作，DOM 回报确认输入、点击结果和 scrollY=480，截图文件真实生成。后续拒绝步骤遇到旧审批卡：Product projection 已是 resolved，但 `checkpoint_from_session` 从事件重新生成了所有曾经请求过的 permission_approval。现按同 turn/call 的 ToolCallStarted/Completed 移除已执行的权限请求，只在等待阶段输出待处理项；事件序列行为测试 2/2 通过（`approval-checkpoint-tests.log`）。harness 同时严格匹配本轮 call ID。

第 9 轮 `lilia-browser-agent-1788617374627` 五次操作与拒绝通过，取消后审批消失但任务仍占据活动轮次。根因是应用取消后才重读“是否等待审批”，此时 Native runtime 已写入 cancelled，应用无法收尾。现使用 Native runtime 返回的 TurnCancellationDisposition::PausedAction 完成任务与持久 claim 收尾；ActiveRun 保留由执行 worker 收尾，避免提前启动下一轮。用户停止与自动化取消共用此判断，等待默认全量 debug 重验。

截图复查发现审批卡的独立标题/正文没有绘制。NanaUI 的父控件文字去重将无标题 Card 也视为已绘制所有 Text 子节点。现在 Card 只抑制与其实际标题重复的文字，独立正文仍生成 scene text primitive。目标绘制测试 3/3 通过（`E:/codex-build/nanaui-workbench/card-text-scene-tests.log`）；真实 GPU 截图复验仍在进行。


### 本地集成验收检查点

- 默认 `cargo xtask agent-debug` 已整体通过，日志 `E:/codex-build/lilia-workbench/agent-debug-integration-final.log`。原有场景在 `agent-debug-runs/lilia-agent-debug-1788617793557`，真实 Agent 场景在 `agent-debug-runs/lilia-browser-agent-1788618014509`。五次实际工具执行、5 张审批、拒绝与待审批期间取消通过；取消后待处理卡与停止按钮消失，输入区可发送。批准和取消截图已人工复查，Card 正文重新绘制。
- 此结果未覆盖进行中 CDP 取消、上传/下载完成、全部 IME/缩放/高 DPI 与三端运行验收。长权限说明的正文与动作间距仍在补窄卡片验证；时间线仍展示部分原始状态码及重复事件，产品视觉收敛尚未完成。
- `cargo test --locked --workspace` 完成 69 个套件，945 通过、0 失败、1 默认忽略（已另行通过真实 LSP 测试），日志 `E:/codex-build/lilia-workbench/workspace-integration-tests-3.log`。覆盖新会话浏览器绑定、批准后 checkpoint 不重开旧审批及当前服务迁移。晚于此构建的窄卡片布局测试将另记。
- NanaUI scene 当前全库 95/95 通过，日志 `E:/codex-build/nanaui-workbench/scene-workbench-tests.log`；runtime 815/815 的记录仍有效，后续只改 scene Card 文本过滤。


窄卡片使用明确的横向拉伸约束，使 Text 的测量宽度与实际卡片内容宽度一致，长说明的高度参与布局，动作行保持在正文之后。初次测试误把内边距写成固定 12px（当前实际为 10px 加边框），改为按实际卡片边界验证后，桌面全库 514/514 通过，日志 `E:/codex-build/lilia-workbench/workbench-integration-tests-12.log`。该测试包含真实 NanaTextShaper、场景布局和多行正文与动作行的间距断言。

Agent Debug 的 desktop-binary.json 现同时记录 launcher 与其旁实际加载的 liliacode_host 动态库指纹。此前只有启动器指纹，不能独立证明 DLL 内容；旧运行仍保留原构建日志与截图，不事后伪造旧 DLL 指纹。后续真实运行与性能报告使用完整两份指纹。


### 当前性能结果与历史记录

修正后的 `cargo xtask performance` 全部绝对阈值通过，原始结果在 `agent-debug-runs/lilia-performance-1788620883814/performance.json`，日志 `E:/codex-build/lilia-workbench/performance-integration-final-2.log`。指纹读取在启动计时结束之后，不预热本轮启动。

| 指标 | 本轮 | 第一阶段 1788598213929 | 较早记录 1787663826614 | 绝对门禁 |
| --- | ---: | ---: | ---: | ---: |
| 冷启动 P95 / ms | 4677.78 | 5864.63 | 1601.42 | 15000 |
| 输入帧 P95 / ms | 15.19 | 14.88 | 32.41 | 100 |
| 分栏调整帧 P95 / ms | 15.09 | 20.37 | 118.98 | 100 |
| 千条时间线就绪 / ms | 165.40 | 612.70 | 1103.73 | 30000 |
| 空闲 CPU / % | 0.20 | 0.10 | 0.20 | 10 |
| 空闲 RSS / MiB | 191.55 | 241.67 | 258.58 | 1024 |

本轮五次冷启动原始值为 4677.78, 1527.81, 1522.88, 1523.55, 1520.21 ms；P95 在五个样本中等于最慢一次，不能用后四次替代报告。空闲 CPU 是短时进程采样。历史记录未锁定同一构建、缓存及后台负载，因此表格是历史数值对照，不宣称改动因果或统计显著性。较早记录本身未通过 100 ms 分栏阈值。

第一次性能运行 `lilia-performance-1788620287702` 也通过绝对阈值，但其启动前读取动态库指纹会预热文件，因此不选作本轮冷启动对比；原记录完整保留。验证期间曾观察到无关项目的 Rust 构建，没有停止或修改其进程。


最后的真实浏览器视觉复验 `cargo xtask agent-debug --profile browser-agent` 通过，日志 `E:/codex-build/lilia-workbench/browser-agent-visual-final.log`，产物 `agent-debug-runs/lilia-browser-agent-1788621109950`。`browser-agent-approval-0.png` 已人工查看：审批标题、说明与动作行完整绘制，正文不再挤入按钮；该运行再次通过五次操作、拒绝和取消。desktop-binary.json 包含实际宿主 DLL 指纹，sourceRebuilt=true。该截图仅验收本次修复，不代表整套工作台视觉已完成。


本地 `cargo check --locked --workspace` 已通过（`E:/codex-build/lilia-workbench/workspace-integration-check.log`）。此后已移除 `.cargo/config.toml` 中临时 patch 段，该文件与原记录相同；`cargo metadata --offline` 将 Cargo.lock 的 NanaUI 来源恢复为实际 Git revision e90867d8972b11a42febda586986ef1abbbfcb98，保留 WebView2、终端 Windows API 等实际依赖变化。本地联调锁文件备份在 `target/refactor-baseline/Cargo.local-validation.lock`，覆盖配置在 `target/refactor-baseline/nanaui-local.toml`。正式 pin 的 `cargo xtask verify` 正在独立执行，不沿用本地联调通过状态。


### 正式 Git pin 检查与尚未交付项

正式 `cargo xtask verify` 的 boundary-check 和 pin-check 均通过（21 个不可变 Git 依赖）。随后桌面编译失败：正式 NanaUI e90867d8 缺少 TerminalView/TerminalScreen、NativeContent/NativeContentRegion、WindowsComposition/HostedSurfaceMode 及 RuntimeProgram 原生内容生命周期接口。完整日志为 `E:/codex-build/lilia-workbench/verify-formal-pin.log`。因此正式 verify 未通过，不能发布或宣称已完成正式依赖验收；没有改变 pin、提交或推送 NanaUI。

重复本地联调时，应先使用保存的本地覆盖解析锁文件；xtask 会启动子 Cargo，仅给外层 Cargo 传 --config 不会把该覆盖传给子进程，所以本轮是临时向 `.cargo/config.toml` 注入标记 patch、完成后精确移除，再用 Cargo 重新解析正式锁文件。不要把本地锁文件或路径覆盖称为远端 pin 验证通过。

本批剩余工作仍包括完整 ComposerView/TaskView 共享（权限与工作树菜单按窗口归属、附件/补全/建议齐备）、Settings/Automation 和项目资源视图迁移、剩余领域服务与 PrimaryShellSnapshot 清理；浏览器上传/下载完成、进行中取消、IME/高 DPI/主题/窄窗及长时间输出的完整验收也未完成。已通过的检查点仅覆盖上述实际记录。未将整个重构计划标为完成。


## 共用任务视图阶段

- ComposerView 完整拥有输入、原子附件、附加操作、权限、工作树、建议、斜杠/文件/对话引用补全及发送/停止控件。主窗口和任务独立窗口复用同一实现；独立窗口保留原生窗口关闭入口，移除输入工具条重复的“关闭窗口”按钮。
- TaskView 统一时间线、空态、错误、独立审批区及底部 Composer 的挂载和更新。时间线更新缓存归 TaskView，每窗口各有实例；独立窗口有正文时不重复显示任务标题。
- PrimaryShellSnapshot 的 Composer 平铺字段已替换为模块自有 ComposerViewSnapshot。其他领域总快照及 UiModule 总投影仍未删除，不能把本阶段记为所有 UI 边界完成。
- 权限、工作树、附加操作、附件移除、建议和补全控件均发送窗口、任务、草稿版本和基准内容，应用沿已有条件写入入口校验。菜单状态归每窗口 ComposerModule，同任务权威重读保持当前菜单，切换任务清除旧菜单。
- 相同建议 ID 的提示内容变化会重建动作绑定；任务切换销毁上一任务的动态控件，包括停放菜单。原先未投影的对话引用结果接入补全列表。聚焦编辑器遇到未变化投影时保留排队编辑的目标版本；待处理请求阻止发送时仍显示有效停止操作。
- 本地 NanaUI 全量桌面行为测试：`cargo test --locked -p lilia-desktop --lib`，518/518 通过，日志 `E:/codex-build/lilia-workbench/task-view-tests.log`。覆盖实际控件事件的双窗口操作归属、建议回调更替、旧任务节点销毁、菜单隔离、排队编辑及停止按钮。随后清理未使用桥接，并恢复无 Composer 状态的独立窗口输入禁用条件；这部分随当前真实桌面调试构建验收。
- 本阶段真实运行、性能和正式依赖结果见下方最终记录；此前其他阶段结果不能替代本阶段视觉验收。


共用 TaskView 的真实运行 checkpoint：`cargo xtask agent-debug` 已通过，包含主窗口明暗主题、管理页返回、跨任务草稿、Windows WebView2 及真实 Agent 五项浏览操作/五次审批、拒绝与取消。产物 `agent-debug-runs/lilia-agent-debug-1788624877485` 和 `agent-debug-runs/lilia-browser-agent-1788624945102`。新增 `task-popup-acceptance.json` 证明同任务双窗口共享草稿、主窗口切换到其他任务后输入隔离、关闭独立窗口后草稿保留。已查看真实 645×1140 独立窗口与主窗口明暗截图。早先前台窗口捕获失败记录保留于 `task-view-popup-agent-debug.log`；使用 PID + 精确标题的实际窗口捕获后通过。截图发现空资源时仍显示无用切换按钮，已改为仅有资源时挂载；该布局修正及凭据服务行为测试已纳入下方最终验证。

ProviderCredentialService 已接管凭据校验、登录、显式导入、撤销、运行时刷新、共享 Provider revision 和领域事件；通过独立 Feature 注册，DesktopApplication 仅保留兼容转发。服务使用同一个 ServiceAuthority/ProductCredentialBridge，并以显式 refresh port 调用 Agent runtime。持久提交后即使刷新失败也发布 CredentialChanged/ProviderChanged，读取端可恢复真实已提交状态；失败验证不得写入或发事实事件。新增直接服务存活、拒绝无写入、刷新失败后的登录/撤销事实测试，已纳入 520 项桌面测试。


本阶段最终行为测试：`E:/codex-build/lilia-workbench/task-view-services-tests.log`，520/520 通过，包含 ProviderCredentialService 直接调用/提交后刷新失败事实事件，以及空资源入口收起与焦点恢复。

本阶段性能：`cargo xtask performance` 通过，30 帧样本、5 次冷启动，产物 `agent-debug-runs/lilia-performance-1788625425769/performance.json`，完整日志 `E:/codex-build/lilia-workbench/task-view-performance.log`。五次冷启动为 4937.2086、1470.4874、1469.7744、1523.5734、1479.6567 ms；P95 保留第一次长尾，不排除样本。

| 指标 | 本阶段 | 上阶段 1788620883814 | 绝对门禁 |
| --- | ---: | ---: | ---: |
| 冷启动 P95 / ms | 4937.2086 | 4677.7777 | 15000 |
| 输入 P95 / ms | 15.106 | 15.1878 | 100 |
| 分栏调整 P95 / ms | 15.377 | 15.0914 | 100 |
| 千条时间线就绪 / ms | 141.0164 | 165.3982 | 30000 |
| 空闲 CPU 采样 / % | 0 | 0.1953125 | 10 |
| 空闲 RSS / bytes | 200740864 | 200855552 | 1073741824 |

全部绝对门禁通过。历史对照不是控制环境下的因果对照；本机另有其他项目构建，未停止无关进程。不得据此声称冷启动回归或时间线提升由当前改动单独导致，也不把 CPU 采样值 0 解释为零资源消耗。


最终 Windows 回归：`E:/codex-build/lilia-workbench/task-view-final-agent-debug.log` 通过，完整主流程和独立窗口结果位于 `agent-debug-runs/lilia-agent-debug-1788625806231/summary.json`，Agent 五项操作/审批/拒绝/取消位于 `agent-debug-runs/lilia-browser-agent-1788625859104/browser-agent-acceptance.json`。已查看最终 `task-popup.png`，确认空资源切换入口退出布局，输入区保持贴底。实际宿主 DLL SHA256 为 `3b7820d1978019b770165d7e2a62f3ca5cba0a30602c7f308b394d8b0c38cc2f`，大小 109118464 bytes；启动器与 DLL 的记录同时保留。

正式配置复核：已移除临时 `.cargo/config.toml` overlay，恢复正式 NanaUI Git 依赖并核对该配置文件与原记录无差异；本地 lock 证据单独保存在 `target/refactor-baseline/Cargo.task-view-local.lock`。`cargo xtask verify` 失败，日志 `E:/codex-build/lilia-workbench/task-view-formal-verify.log`：boundary-check 通过，21 项 immutable Git pin 检查通过，桌面 lib/lib-test 分别因正式 NanaUI 缺少 TerminalScreen/NativeContent/WindowsComposition/read_only 等接口报告 63/71 个编译错误。该结果是正式 pin 的失败记录，不影响对本地 checkout 运行产物的识别，也不能写成默认正式依赖已可构建。

剩余完整计划继续包括 Settings/Automation 及其他界面的模块生命周期、总快照/集中业务路由和剩余领域锁清理、编辑/终端/浏览器全部验收矩阵（包括活跃 CDP 中取消、上传下载完成、IME/DPI 与恢复）。本阶段完成共用任务视图和凭据服务，不代表完整计划交付。

Composer 的附件、建议、斜杠/引用条目及权限/工作树选项进一步移入 `module/composer/presentation.rs`；模块投影不再从 runtime_shell 导入这些数据类型，调用方直接使用模块 API，不保留旧 Shell 类型别名。520/520 桌面行为测试再次通过，日志 `E:/codex-build/lilia-workbench/composer-presentation-tests.log`。本次仅移动类型及调用路径，不改变控件或运行行为，沿用上方真实窗口和性能验收；未重复系统门禁。本地 lock 另存 `target/refactor-baseline/Cargo.composer-presentation-local.lock`，随后恢复正式 Git 依赖解析，`.cargo/config.toml` 未修改。

## 设置视图阶段

SettingsView 现在拥有设置导航、页面、外观与关于、项目字段、凭据/Agent/扩展动作、用量图和快捷键状态的挂载及更新；只接受 SettingsSnapshot、主题和可见性。Shell 保留区域装配，不再持有这些控件缓存或设置页业务组装函数。设置条目类型位于 `module/settings/presentation.rs`，扩展模块和凭据投影直接使用该呈现接口。总 UiModule 投影和 SettingsModule 之外的业务编辑状态仍待继续收敛。

ProductFields 只封装产品字段名称、ShellIntent 输入绑定及每个视图的控件缓存，使用 NanaUI 现有 FormField/TextArea。项目页和设置页分别持有实例，离开页签时释放不再呈现的字段；不再通过 project-/pending- 字符串前缀保护另一个页面的缓存。设置页动作以可更新绑定持有当前命令，凭据 revision 更新后复用的按钮提交新 revision；删除条目同时销毁控件和绑定。Provider 行缓存使用独立键空间，首次挂载即可呈现正确选中状态。不可见设置页及非桌面页签停止 KeyCaptureLayer 录制。

首轮 `cargo test --locked -p lilia-desktop --lib` 本地联调 523/523 通过，日志 `E:/codex-build/lilia-workbench/settings-view-tests.log`，涵盖实际按钮事件携带新凭据版本、条目移除、双视图表单隔离和录制层生命周期。随后仅移动设置行的数据类型；当前真实 Windows 回归包含该最终类型归属。

Agent Debug 新增全部 10 个设置页签的明暗主题遍历，每页保存 `settings-<tab>-<theme>.json` 与同名真实窗口 PNG，核对当前页签/主题并检查观察结果不含凭据 canary。运行结果待当前进程完成后记录；截图遍历不等同于所有设置业务操作已验收。

首轮扩展后的 Windows 脚本通过，产物 `agent-debug-runs/lilia-agent-debug-1788627557397`。实际查看模型服务和扩展截图后未接受其视觉结果：TextArea 默认最小高度让单行字段占据过大空间，操作按钮逐项竖排。继续修正为单行 TextInput（凭据 secure）、必要的多行 TextArea、可搜索模型服务选择、支持换行的顶部操作区和 760 logical px 表单内容上限。NanaUI Stack 新增 wrap(bool)，只映射已有布局合同，并验证窗口收窄后子控件真实换行。修正后的测试和截图待以下最终记录，不以首轮脚本通过代替最终视觉验收。

修正后 `cargo test --locked -p lilia-desktop --lib` 524/524 通过，日志 `E:/codex-build/lilia-workbench/settings-view-compact-tests-2.log`。NanaUI `cargo test --locked -p nana-ui-runtime --lib wrapping_stack_reflows_controls_when_the_viewport_narrows` 1/1 通过（831 项过滤），日志 `E:/codex-build/nanaui-workbench/settings-stack-wrap-tests-2.log`。该测试实际计算两个视口的布局，断言控件由同行变为下一行，不是对 wrap 字段的重复断言。

最终清理移除了设置视图中重复的 KeyCaptureLayer 节点：按键捕获已经由 DesktopProgram 的宿主输入路径处理，并在离开设置/桌面页签时关闭；旧视图节点没有事件绑定，只产生空白控件。同步删除针对该重复节点的测试，保留实际宿主逻辑。GitHub bound/unbound 状态在呈现层转为已绑定/未绑定。最新桌面测试 `settings-view-final-tests.log` 523/523 通过。

第二轮 Windows 运行 `lilia-agent-debug-1788628482431` 已生成完整设置主题截图、返回任务及独立窗口证据；已查看修正后的模型服务明暗截图，单行字段和搜索选择器布局正常。该运行不能记为整体通过：第二个 browser-agent profile 构建时，其他工作在 NanaUI runtime 增加 hashbrown 依赖，旧本地 lock 与实时 manifest 不符，--locked 正确拒绝继续。本地 metadata 已按当前 checkout 刷新；未撤销其他工作。默认 Agent Debug 改为两个 profile 复用同一次构建，并比较启动器及 DLL SHA256；产物不一致时拒绝合并结果，独立 browser-agent 入口的构建策略不变。

最终 Windows 回归通过：`E:/codex-build/lilia-workbench/settings-view-final-agent-debug-2.log`；主流程/20 份设置主题截图位于 `agent-debug-runs/lilia-agent-debug-1788629151333`，真实 Agent 浏览器操作位于 `agent-debug-runs/lilia-browser-agent-1788629364748`。已查看最终桌面浅色及扩展深色截图，空白录制节点消失，单行输入尺寸正常；模型服务明暗截图已在前一轮同布局修正产物检查。主流程与浏览器 profile 的启动器和 DLL 哈希一致，后者 reused=true。宿主 DLL SHA256 `f8287461e60959d8ce8a9ee292df418548cb6029a310d1d69d833ab9ac4fcde1`，109279232 bytes。

本阶段不代表设置业务全量完成：模型 ID 仍是手动输入，扩展列表仍需按实体组织，快捷键等编辑状态还需完成每窗口归属；Automation、剩余领域服务、总快照以及编辑/终端/浏览器全矩阵仍属完整目标。

性能首轮 `settings-view-performance.log` 失败于后续冷启动的重复构建：首个性能桌面还持有宿主 DLL，Windows 拒绝覆盖正在使用的产物。构建原本不计入 startup_ms，且每个冷启动均启动独立进程，因此改为首次正常构建、后续 Session::start_reusing，并逐次比较启动器和宿主 DLL SHA256。哈希校验与 Agent Debug 共用 verify_same_binary；不同构建不能混入同一组样本。此修正不改采样数、启动计时范围、阈值或夹具。

性能复测通过：`E:/codex-build/lilia-workbench/settings-view-performance-2.log`，报告 `agent-debug-runs/lilia-performance-1788629907010/performance.json`。30 帧、5 次进程启动；所有冷启动产物与首次 Session 通过 SHA256 一致性检查。原始冷启动依次为 4068.166、1453.2494、1451.9882、1393.9673、1392.8599 ms。

| 指标 | SettingsView 阶段 | 前阶段 1788625425769 | 绝对门禁 |
| --- | ---: | ---: | ---: |
| 冷启动 P95 / ms | 4068.166 | 4937.2086 | 15000 |
| 输入 P95 / ms | 15.3479 | 15.106 | 100 |
| 分栏调整 P95 / ms | 15.0865 | 15.377 | 100 |
| 千条时间线就绪 / ms | 456.4522 | 141.0164 | 30000 |
| 空闲 CPU 采样 / % | 0.1953125 | 0 | 10 |
| 空闲 RSS / bytes | 204038144 | 200740864 | 1073741824 |

全部绝对门禁通过；历史千条时间线指标上升约 315.4 ms，原始结果保留。存在 NanaUI 本地代码并行更新和其他项目测试负载，历史数值不是控制环境下的因果对照，不把差异单独归因于 SettingsView。首次性能运行因 DLL 占用失败的记录保留，不排除或改写成性能样本。

本地联调 lock 已另存 `target/refactor-baseline/Cargo.settings-view-local.lock`；临时 `.cargo/config.toml` 覆盖已删除，正式依赖重新解析。正式 Git 依赖验证结果见下方最终记录。

正式依赖最终验证失败，日志 `E:/codex-build/lilia-workbench/settings-view-formal-verify.log`：boundary-check 和 21 项 immutable Git pin 检查通过，桌面 lib/lib-test 分别报告 64/72 个缺失接口错误，包含正式 NanaUI 尚无 TerminalScreen、NativeContent、WindowsComposition、read_only 及本阶段 Stack::wrap。该失败与本地 checkout 的已验证产物分开记录，未更新或伪造远程 Git pin。`.cargo/config.toml` 与原配置无差异；两仓库 diff --check 均通过。没有提交、推送或创建 PR。

## 自动化视图阶段（实施中）

AutomationView 拥有工作流侧栏、名称、发布与启用操作、节点工具栏、画布及运行反馈的挂载更新，Shell 只装配页面和侧栏，自动化行不再共享工作区按钮缓存。隐藏时清除动作绑定并关闭运行选择器。PrimaryShellSnapshot 仅聚合 AutomationViewSnapshot，领域编辑状态和总投影注册机制尚未迁移完成。

工作流动作携带 WindowId、工作流 ID 和 updated_at；该时间戳只是当前客户端的新鲜度检查，不是持久化 CAS revision。运行操作额外携带运行 ID，人工回复和继续额外携带等待节点 ID；切换运行、完成、隐藏页面后旧控件事件不能操作新记录或下一个确认节点。工作流发布先保存草稿，失败不继续发布。运行历史用搜索选择器显示状态与 UTC 时间，等待确认时提供回复、继续和取消，错误直接呈现。

首轮视图抽取测试 `automation-view-tests.log` 526/526 通过。该结果只覆盖视图抽取，后续运行历史与节点身份保护以以下测试和 Windows 记录为准。

运行闭环补入后的桌面测试 `E:/codex-build/lilia-workbench/automation-run-tests.log` 527/527 通过；首轮完整 Windows 回归通过，主流程 `agent-debug-runs/lilia-agent-debug-1788632077067`，Agent 浏览器 `agent-debug-runs/lilia-browser-agent-1788632367428`。人工查看自动化等待确认与取消截图后，发现分组直接作为 SidebarFrame.body 的布局在分组刷新时被覆盖，底部刷新/新建入口不可见；运行状态也出现重复。该轮保留为问题证据，不作为最终视觉验收。

修正使用 SidebarFrame.vertical_body_scroll 承载分组，列表按内容增长，固定底部动作与滚动内容分离；运行详情只显示错误或确认提示。`E:/codex-build/lilia-workbench/automation-view-layout-tests.log` 5/5 通过（523 项过滤），包含在 260×320 视口内依次刷新 1、60、2 个工作流，实际计算布局并断言底部按钮区域始终可见。

最终 `cargo xtask agent-debug` 通过，日志 `E:/codex-build/lilia-workbench/automation-view-final-agent-debug.log`。主流程与自动化四组状态/截图位于 `agent-debug-runs/lilia-agent-debug-1788632736115`，Agent 浏览器位于 `agent-debug-runs/lilia-browser-agent-1788632987102`。已查看最终等待确认和取消截图：底部刷新/新建入口可见，回复和继续/取消操作完整显示，重复状态消失。自动化截图为深色主题；自动化浅色、窄窗和完整配置交互仍待后续矩阵覆盖。浏览器五个动作、五次审批、拒绝及审批期间取消均通过，不扩大为活跃 CDP 中取消或上传下载完成验收。

主流程与浏览器宿主 DLL SHA256 同为 `a3e3ab9e41061fbfa7f914bc87d0d7d50c67d800fa152f8dfe4e5a9c32778bce`，109412352 bytes，浏览器 profile reused=true。NanaUI 本地 lock 已保存为 `target/refactor-baseline/Cargo.automation-view-local.lock`；临时配置块删除后 `.cargo/config.toml` 无 diff，已恢复正式 Git 依赖解析。本阶段没有修改 NanaUI 源码或发布依赖；未再次采样 performance，上一阶段数据继续作为历史记录，不作为当前自动化页面的性能验收。

正式 `cargo xtask verify` 失败，日志 `E:/codex-build/lilia-workbench/automation-view-formal-verify.log`：boundary-check 与 21 项 Git pin 检查通过，桌面 lib/lib-test 分别报告 67/75 个缺失 API 错误，仍是正式 NanaUI `e90867d8` 缺少 NativeContent、WindowsComposition、TerminalScreen/TerminalView、read_only、Stack::wrap 等本地能力。未更改远程 pin。两仓库 `git diff --check` 通过，未提交、推送或创建 PR。

仍待完成：节点配置与范围编辑的产品视图、运行任务关联、长操作通过现有 Jobs 执行、每窗口自动化编辑状态及领域版本事件；完整任务目标不因该页面抽取而完成。

## 自动化节点编辑与连线语义（实施中）

节点草稿类型、字段规则、配置校验位于 `module/automation/editor.rs`，表单挂载与控件缓存位于 `editor_view.rs`，画布投影与连接校验位于 `graph.rs`。DesktopProgram 继续持有自动化选中记录与领域协调，尚未完成整个 Automation UiModule/Jobs 迁移；本阶段不把文件拆分等同于完整状态所有权收敛。

节点表单提供名称、指令、明确的权限/触发方式/工具动作选择，以及对应字段。宽窗在画布旁编辑，窄窗切换为节点编辑并提供返回画布操作；输入和保存携带工作流与节点身份，隐藏视图清除绑定，移除失效字段控件。新增节点自动选中并打开配置。新 Agent 节点不再写死模型版本，使用既有执行器的默认模型选择。

配置解析失败时保留原稿并报告错误，不再用空对象覆盖；保存失败保留草稿和错误，不立即重读并清空。普通输出连线现在写入默认出口（source_handle=None），领域执行器兼容旧记录的 output 别名。连接在保存前检查端点、重复边和循环，生成未占用的边 ID。条件节点暴露成立/不成立端口，多分支节点暴露声明值与默认端口，显示名称与持久化身份分离。

共享 `automation-contract.json` 定义默认出口别名与分支 cases 数据；`lilia-contracts::AutomationSwitchCases` 接受旧逐行文本和新字符串数组，保存时写规范数组。画布与执行器复用同一端口转换函数，拒绝无法区分或占用保留名称的声明值。编辑中的换行保留在草稿，保存时才转为分支列表。该字段声明可连接的分支出口，执行选择仍由现有 AutomationExecutionEngine 和实际连线决定，不新增执行器。

回归脚本新增图节点稳定目标，实际选择人工确认节点、编辑名称和指令、保存后核对持久化投影，再发布运行；节点编辑截图与观察配对。验证结果待当前进程结束后记录。

一致版本测试 `cargo test --locked -p lilia-contracts -p lilia-feature-automation -p lilia-desktop --lib` 全部通过：19 项契约、534 项桌面、24 项自动化领域测试，日志 `E:/codex-build/lilia-workbench/automation-editor-final-tests-2.log`。覆盖分支旧格式转换、无效草稿保留、未修改字段和类型保留、输入换行、编辑事件身份与失效控件移除，以及由实际画布端口创建的连线可执行、循环拒绝和默认分支匹配。前一轮 `automation-editor-final-tests.log` 的 533 项桌面结果不含最后一项普通连线兼容测试，以上新结果取代该轮作为本阶段行为证据。


首轮节点编辑原生回归在 `agent-debug-runs/lilia-agent-debug-1788634938605` 完成保存、发布和运行状态验证，但编辑器截图中固定 900×560 自动适配将小流程放大，打开侧表单后所选节点被裁切。初始视口改为原始比例和内容起点留白，保留用户后续缩放/平移。旧 `output`/`input` 连线与空句柄按同一默认端口判重。

该轮后续浏览器 profile `agent-debug-runs/lilia-browser-agent-1788635304595` 失败；观察中标签已激活、浏览器状态为空且没有宿主错误，1180×760 截图仍显示窄窗对话。原因是资源打开未改变 CompactWorkbench 的对话/工作区选择，网页区未挂载。打开或重新激活资源成功后，现在请求对应窗口显示工作区；日常投影刷新不改变用户主动返回对话的选择。测试覆盖显式显示、返回后刷新保留选择，以及原有草稿/选区/焦点恢复。


修复后的桌面库测试 535/535 通过，日志 `E:/codex-build/lilia-workbench/automation-editor-viewport-tests.log`。其中默认连线测试也覆盖旧 `output`/`input` 双别名判重；初始视口测试覆盖偏移图坐标和表单旁的完整显示。随后将资源显示触发限定为文档、终端、浏览器和项目文件，防止打开设置等管理页改变原对话/工作区选择。

完整 `cargo xtask agent-debug` 通过，日志 `E:/codex-build/lilia-workbench/automation-editor-agent-debug-2.log`；主流程 `agent-debug-runs/lilia-agent-debug-1788636211216`，浏览器 profile `agent-debug-runs/lilia-browser-agent-1788636581191`。已查看本轮 1180×760 的节点编辑截图：两个节点与右侧表单完整可见，保存、发布、等待确认、继续和取消均成功。已查看浏览器真实窗口截图并核对接受报告：五个页面动作、五次审批、拒绝与待审批取消通过。此轮窗口实际呈现分栏，不能只凭截图像素尺寸宣称完整窄窗或 DPI 矩阵通过；窄窗显式显示及返回保留选择目前另有控件行为测试。

两 profile 的宿主 DLL SHA256 均为 `babc68300826ded62b023d480bf9421df5aa0160a8103be7085243f6ed0ec0b4`，109625344 bytes；浏览器 profile reused=true。未覆盖活跃 CDP 操作中取消、上传/下载完成或远程模型质量。性能与正式 Git 依赖结果待记录。


尺寸事件修复前的性能门禁通过，报告 `agent-debug-runs/lilia-performance-1788636646562/performance.json`，日志 `E:/codex-build/lilia-workbench/automation-editor-performance.log`。5 次启动、30 帧输入/尺寸采样。绝对门禁分别为启动 P95≤15000ms、帧 P95≤100ms、千条时间线≤30000ms、空闲 CPU≤10%、RSS≤1GiB，全部通过。

| 指标 | 上次 Settings 阶段 | 本阶段 |
| --- | ---: | ---: |
| 冷启动 P95 (ms) | 4068.166 | 3161.2634 |
| 输入帧 P95 (ms) | 15.3479 | 4.4292 |
| 调整尺寸帧 P95 (ms) | 15.0865 | 11.232 |
| 千条时间线就绪 (ms) | 456.4522 | 431.7938 |
| 空闲 CPU (%) | 0.1953125 | 0 |
| 空闲 RSS (bytes) | 204038144 | 204836864 |

历史对照来自 `lilia-performance-1788629907010`，不是受控 A/B；共享机器同期存在其他构建，NanaUI 工作区也有独立变更，不能将上述差异归因于此次编辑区调整。固定窄窗脚本在该性能命令启动后加入；性能结果不作为新窄窗交互的截图验收。


固定尺寸第三轮 `agent-debug` 进程成功（主流程 `lilia-agent-debug-1788637378906`，浏览器 `lilia-browser-agent-1788637698844`），浏览器 780 逻辑像素宽度下实际显示工作区并就绪。但人工查看自动化窄窗截图发现宽版分栏仍在，尽管截图后的 observe 已报告窄版目标；该轮因此不计为完整视觉通过。根因是 Ready/Resized 只更新工作区模型并请求重绘，没有同步产品视图，下一条消息才使组件树追上模型。

修复在主窗口尺寸事件后同步主壳，在任务窗口尺寸事件后同步各任务视图。任务窗口的窄版判定使用窗口会话提供的当前逻辑宽度，避免读取上一帧布局宽度。工作区行为测试增加无新布局帧时按新窗口宽度切换窄/宽版并保留编辑器实例的断言。管理页面的返回操作同时保留原对话/工作区选择，与主动打开资源区分。修复后的检查另行记录。


尺寸同步修复后，工作区 3 项针对性行为测试全部通过（其余 532 项过滤），日志 `E:/codex-build/lilia-workbench/automation-editor-resize-tests-2.log`。完整 `cargo xtask agent-debug` 通过，主流程 `agent-debug-runs/lilia-agent-debug-1788638395153`，浏览器 profile `agent-debug-runs/lilia-browser-agent-1788638671496`，日志 `E:/codex-build/lilia-workbench/automation-editor-agent-debug-4.log`。已查看本轮 `automation-node-editor-narrow.png`：780×760 下只保留完整节点表单，旧画布不再残留。调试图节点目标改为依据实际挂载视图，避免只读取模型导致漏检。

随后运行 `cargo xtask agent-debug --profile browser-agent --no-build`，在窄窗加载真实测试网页后才恢复宽窗，结果通过；证据 `agent-debug-runs/lilia-browser-agent-1788638775452`，日志 `E:/codex-build/lilia-workbench/automation-editor-browser-narrow.log`。已查看 `browser-agent-page-narrow.png`，地址栏、原生页面、滚动区及人工接管状态均完整显示。恢复宽窗后完成五个页面动作、五次审批、拒绝和待审批取消。主流程与两次浏览器验证宿主 DLL SHA256 均为 `9e03e773d9b9dd1ef061164d0b82a11076855abd998adada0c0691400e24c3ca`，109630976 bytes；浏览器两轮 reused=true。固定窄窗已覆盖，不能扩大为输入法、多档 DPI、上传下载完成或活跃 CDP 中取消的验收。


尺寸事件修复后的 `cargo xtask performance` 绝对门禁通过，报告 `agent-debug-runs/lilia-performance-1788638829651/performance.json`，日志 `E:/codex-build/lilia-workbench/automation-editor-performance-final.log`。仍为 5 次启动、30 帧采样，门禁阈值未改动。

| 指标 | Settings 历史基线 | 最终复测 |
| --- | ---: | ---: |
| 冷启动 P95 (ms) | 4068.166 | 6157.6292 |
| 输入帧 P95 (ms) | 15.3479 | 5.51 |
| 调整尺寸帧 P95 (ms) | 15.0865 | 10.817 |
| 千条时间线就绪 (ms) | 456.4522 | 820.2697 |
| 空闲 CPU (%) | 0.1953125 | 0 |
| 空闲 RSS (bytes) | 204038144 | 206979072 |

启动和时间线历史指标变慢，尚未完成受控归因，不能将绝对阈值通过解释为历史无回退。五次启动原值为 6157.6292、1577.2304、1575.7673、1580.8241、1489.1612ms，按现有五样本 P95 规则如实保留首次高值。最终性能宿主 SHA256 为 `25b5f32dcad40ab6bb661feaed67bda299c09dd1aeba46b0dbca9a3908d39d10`，109733376 bytes；本地 NanaUI 同期另有变更，二进制与上一轮原生截图不同，不将两轮合并为同一依赖快照的验收。需要稳定依赖快照后复核历史差异。

本地 lock 保存为 `target/refactor-baseline/Cargo.automation-editor-local.lock`，临时配置块已删除，`.cargo/config.toml` 无 diff，离线 metadata 已恢复正式 Git 依赖解析。正式 pin 没有修改，正式门禁结果待记录。


正式 `cargo xtask verify` 失败，日志 `E:/codex-build/lilia-workbench/automation-editor-formal-verify.log`。boundary-check 与 21 项不可变 Git pin 检查通过；桌面 lib/lib-test 分别报 68/76 个编译错误，当前正式 NanaUI `e90867d8972b11a42febda586986ef1abbbfcb98` 缺少已在本地联调中使用的终端网格、原生内容区域、Windows 合成、只读输入与换行布局等接口。与上一阶段正式错误集合相比，仅多了一处 `Stack::wrap` 缺失，未引入新的非依赖类失败。保留正式解析和本地 lock 的独立证据，不修改远程 pin，不提交、推送或创建 PR。

本轮仍未完成完整计划。后续重点是 Automation UiModule/每窗口状态与 Jobs 迁移、显式实体版本和局部事件投影、剩余管理表面与任务窗口一致性，以及浏览器 IME/多档 DPI/文件完成/活跃操作取消、迁移恢复和稳定快照下的历史性能复核。

### 自动化操作 Jobs 迁移

自动化 Feature 注册 `lilia.automation/operate@1`，复用 Kernel/Mutsuki Jobs；桌面提交启动、继续和取消请求，执行引擎与 Product/Agent 权威保持不变。契约先定义请求与结果身份，启动绑定点击时的发布版本，存储在创建运行的同一事务中核验版本。旧 begin 请求可以省略版本；不迁移或重写历史数据。

继续请求绑定工作流、运行和等待节点，取消使用独立 Job，避免排在执行操作后面。运行创建后通过进度报告身份，界面即可展示该运行和取消入口。终态结果只携带运行身份与错误，界面重读权威详情；晚到结果不能切走用户已选择的另一条历史运行。运行事件只刷新历史与详情，不重载正在编辑的流程草稿。

这一步尚未完成 Automation UiModule 的每窗口状态归属；执行协调端口仍由 DesktopApplication 实现。Job 取消检查覆盖开始前与创建运行后，显式取消依赖已有引擎状态事务保护，不代表任意阻塞外部端口可以立即中断。后续验证结果独立记录，不沿用上一轮截图或性能作为本轮通过证据。

本阶段契约 20 项、自动化领域初轮 26 项通过（`E:/codex-build/lilia-workbench/automation-jobs-domain-tests-2.log`）。本地 NanaUI 桌面 537 项通过（`automation-jobs-desktop-tests.log`），涵盖真实应用端口的启动、错误工作流拒绝、错误等待节点拒绝及正确确认输出持久化。追加执行中取消行为后，领域 27 项通过（`automation-jobs-cancel-tests.log`）：工具调用返回前取消，运行与未完成节点保持取消状态，晚到输出不写入，后续工具不执行。该测试验证状态竞争，不等同于撤销已发生的外部副作用。

完整 `cargo xtask agent-debug` 通过（`E:/codex-build/lilia-workbench/automation-jobs-agent-debug.log`）。主流程证据 `agent-debug-runs/lilia-agent-debug-1788641586213`，自动化依次等待确认、确认成功、第二次运行、取消；运行计数最终为 2。已查看 `automation-waiting.png` 和 `automation-node-editor-narrow.png`，确认卡及实际窄版表单完整显示。`journal.jsonl` 包含本轮 `lilia.automation/operate@1` Job 状态记录，非同步入口冒充异步验证。

浏览器 profile 证据 `agent-debug-runs/lilia-browser-agent-1788642027576`，五个动作、五次审批、拒绝与待审批取消通过。两者宿主 DLL SHA256 同为 `3565777b15f4330fedb17aa7cc9258a3889b5349e3944dcd4a134b82bae0ff6f`（110118912 bytes）；浏览器复用该宿主。仍不覆盖活跃 CDP 中取消、文件传输完成和输入法等未验收范围。

本轮 `cargo xtask performance` 绝对门禁通过，报告 `agent-debug-runs/lilia-performance-1788642112553/performance.json`，日志 `E:/codex-build/lilia-workbench/automation-jobs-performance.log`。5 次启动与 30 帧采样，原有阈值不变。

| 指标 | Settings 历史基线 | Automation Jobs 本轮 |
| --- | ---: | ---: |
| 冷启动 P95 (ms) | 4068.166 | 3202.7845 |
| 输入帧 P95 (ms) | 15.3479 | 4.4753 |
| 调整尺寸帧 P95 (ms) | 15.0865 | 9.4547 |
| 千条时间线就绪 (ms) | 456.4522 | 698.8205 |
| 空闲 CPU (%) | 0.1953125 | 0.09765625 |
| 空闲 RSS (bytes) | 204038144 | 206704640 |

时间线历史指标仍变慢，不能声明历史无回退或把差异归因于 Jobs。共享机器仍有独立构建及 NanaUI 变更；性能宿主 SHA256 `912091f66f644d902b4362db461fd3b5f007ab1ee6ec727fbeecc56c9ecd840a`（110139904 bytes）与本轮截图宿主不同，两组证据分别保留。需要稳定依赖快照后的受控历史复核。

本地解析 lock 保存为 `target/refactor-baseline/Cargo.automation-jobs-local.lock`。临时 NanaUI patch 块已移除，离线 metadata 已恢复正式 Git pin，正式门禁另记。

正式 `cargo xtask verify` 已结束并失败，日志 `E:/codex-build/lilia-workbench/automation-jobs-formal-verify.log`。boundary-check、21 项 Git pin 检查通过；桌面 lib/lib-test 分别为 68/76 个错误，诊断类别与上一轮 `automation-editor-formal-verify.log` 完全一致，仍为正式 NanaUI pin 缺少本地已使用的 API。没有把本地未发布接口伪装成可获取版本。`.cargo/config.toml` 无 diff，两仓库 `git diff --check` 通过；未提交、推送或创建 PR。

完整计划保持未完成：本轮关闭自动化 UI 同步执行路径，下一步仍需迁移 Automation UiModule 的每窗口状态、消息与投影所有权，继续收敛应用服务边界；浏览器未覆盖项和稳定依赖下的历史性能复核继续保留。

### 自动化 UI 控制器边界

将流程列表与选择、运行历史与选择、确认草稿、图与视口、节点编辑草稿、错误及 Jobs 跟踪收敛至 `module/automation/controller.rs`。字段私有，主壳只能只读查询或发送操作；图编辑、存储读写、运行操作与回调处理从 DesktopProgram 迁出。控制器持有窗口身份、AutomationServiceKey 提供的领域服务和现有 Jobs，不持有 DesktopApplication、KernelHost 或整份主壳快照。

带身份的 AutomationAction 在控制器内核验窗口、流程版本、运行与等待节点。投影方法只返回 AutomationViewSnapshot，主壳提供是否显示和可用宽度。任务上下文作为运行请求参数传入，项目范围编辑只传入项目身份是否有效，不把项目服务或主壳对象暴露给自动化。

此阶段先替换主窗口的实际调用路径，不代表所有任务窗口已经挂载自动化管理页，也尚未完成 UiModuleRegistry 的挂载更新协议迁移。双实例状态与消息行为测试、实际窗口回归结果另行记录。

本地 NanaUI `cargo test --locked -p lilia-desktop --lib` 538 项通过，日志 `E:/codex-build/lilia-workbench/automation-controller-tests.log`。新增测试在共享权威服务上创建两个独立控制器，验证跨窗口和过期目标被拒绝、各自节点草稿与视口隔离、另一个窗口保存后权威节点更新而本窗口编辑草稿保留，并检查不同窗口宽度下的投影。它不替代两个真实窗口同时挂载自动化页面的验收。

完整 `cargo xtask agent-debug` 通过，日志 `E:/codex-build/lilia-workbench/automation-controller-agent-debug.log`，主流程 `agent-debug-runs/lilia-agent-debug-1788643884940`，浏览器 Agent profile `lilia-browser-agent-1788644159844`。已查看本轮 `automation-waiting.png`，确认区与运行图正常显示；运行状态经历等待、成功、第二次运行和取消，最终历史数为 2。宿主 DLL SHA256 为 `0ab65e3d633a2d20c7e547b4219603b7760a7670c4889c14e261a5cfebf14d2d`（110145024 bytes）。本轮是所有权迁移，没有修改渲染、缓冲或性能路径，不重复性能采样，也不把上一轮性能报告标记为本轮结果。

临时 patch 已移除，本地 lock 保存为 `target/refactor-baseline/Cargo.automation-controller-local.lock`；离线 metadata 已恢复正式 pin。正式门禁及最终 diff 检查另记。

正式 `cargo xtask verify` 失败，日志 `E:/codex-build/lilia-workbench/automation-controller-formal-verify.log`。边界与 21 项 Git pin 检查通过；lib/lib-test 仍为 68/76 个错误，诊断类别与 Jobs 阶段完全一致，均来自正式 NanaUI pin 尚无已使用接口。本轮本地 538 项测试和原生回放通过不替代正式依赖通过。配置无残留 patch，两仓库 diff 检查通过，未提交或推送。

后续需让 UI 模块工厂接收明确的窗口上下文，将该控制器纳入真实模块注册和窗口挂载生命周期，继续完成全计划中的所有权与多窗口要求；当前不声明 Automation UiModule 已全部完成。

### 自动化正式模块挂载

UI 模块工厂现在接收 `UiModuleContext`，模块宿主向每个窗口实例分发 Jobs 更新；自动化控制器通过 `AutomationServiceKey` 与窗口身份创建，纳入 `UiModuleRegistry`。主壳不再保存自动化控制器字段，也不直接处理自动化 Job；自动化投影由控制器写入其受限投影字段。窗口消息、领域事件和 Jobs 都经过模块宿主，避免跨窗口状态串写。

本地 NanaUI 桌面测试 538 项通过，日志 `E:/codex-build/lilia-workbench/automation-module-tests.log`；完整原生 `agent-debug` 通过，日志 `E:/codex-build/lilia-workbench/automation-controller-agent-debug.log`。正式 pin 仍缺少本地联调使用的接口，未宣称正式构建通过。控制器已可多窗口实例化，但任务弹窗尚未展示完整自动化管理页，仍属后续工作。

模块注册接线后的本地 `cargo check --locked -p lilia-desktop --lib` 通过，日志 `E:/codex-build/lilia-workbench/automation-current-check.log`；随后桌面完整测试 538 项仍通过，日志 `automation-module-tests.log`。`UiModuleFactory` 现为带 `UiModuleContext` 的构造函数，主窗口、任务窗口和自动化控制器均按窗口身份创建，Jobs 通过模块宿主分发。临时 patch 已移除，正式 pin 保持未改。

当前模块工厂与宿主接线后的本地检查仍通过，桌面测试 538 项通过，日志 `E:/codex-build/lilia-workbench/automation-current-check.log` 与 `automation-module-tests.log`。重新运行的原生 `cargo xtask agent-debug` 通过，日志 `E:/codex-build/lilia-workbench/automation-module-current-agent-debug.log`，主流程 `agent-debug-runs/lilia-agent-debug-1788659491413`；自动化等待、继续、重复运行、取消和浏览器 profile 均通过。该轮宿主在 1770×1140 及窄窗下完成真实截图回放。

本轮临时 NanaUI patch 已移除，正式 metadata 已恢复，新增本地 lock 保存为 `target/refactor-baseline/Cargo.automation-module-current-local.lock`。`git diff --check`（Lilia、NanaUI）通过，`.cargo/config.toml` 无差异。正式 Git pin 的缺口仍保留为依赖发布阻塞，未提交或推送。

### 窗口上下文修正

模块上下文现在按窗口解析活动应用表面、设置页签和逻辑宽度。主窗口读取自己的导航与工作区；任务窗口读取其活动工作区项及序列化状态，不再复用主窗口的表面状态。模块宿主仍为每个窗口创建独立实例，Jobs 和领域事件按各窗口上下文调用。

本地 `cargo check --locked -p lilia-desktop --lib` 通过，日志 `E:/codex-build/lilia-workbench/window-context-check.log`；临时 NanaUI patch 已移除，本地 lock 保存为 `target/refactor-baseline/Cargo.window-context-local.lock`。该修复尚未让任务弹窗呈现完整自动化管理页面，只修正了模块生命周期与上下文判定。

UI 模块宿主定向测试 6 项通过，日志 `E:/codex-build/lilia-workbench/ui-module-focused-tests.log`，覆盖模块工厂窗口上下文、独立实例、消息路由与各自投影；本地 lock 保存为 `target/refactor-baseline/Cargo.ui-module-focused-local.lock`，正式配置已恢复。

正式 `cargo xtask verify` 复核日志为 `E:/codex-build/lilia-workbench/window-context-formal-verify.log`：boundary-check 与 21 项 immutable Git pin 检查通过，随后仍因正式 NanaUI pin 缺少终端、原生内容和窗口事件 API 而失败（桌面 lib/lib-test 68/76 项）；与前轮诊断类别相同。正式 `.cargo/config.toml` 已恢复，无临时 patch；两仓库 `git diff --check` 通过。

### 窗口工作区快照边界

`UiModuleContext` 新增 `workspace_snapshot()`，模块需要多个工作区字段时通过一次窗口会话读取获得一致快照；`selected_project`、`selected_task`、`first_task` 以及任务和扩展模块的投影已统一使用该入口。这样减少同一归约周期内重复采样可变会话的机会，也让模块不会绕过窗口上下文取得默认工作区。使用本地 NanaUI 联调配置执行 `cargo check --offline --config target/refactor-baseline/nanaui-local.toml -p lilia-desktop --lib` 通过；UI 模块 6 项定向测试也通过（`E:/codex-build/lilia-workbench/ui-module-workspace-snapshot-tests.log`）。该命令生成的本地解析结果保留在 `target/refactor-baseline/Cargo.window-context-local.lock`，正式 pin 与 `.cargo/config.toml` 未改动。

### 管理页面的窗口归属

任务弹窗的工作区只挂载任务对话、文档、终端和浏览器资源。此前从弹窗触发设置或自动化入口时，应用级工作区项可能被插入弹窗，形成没有对应 `WorkspacePaneView` 的空白标签。现在管理页面入口始终通过主工作区装配，并在打开后将焦点切回主窗口；同步旧布局时还会移除弹窗内遗留的管理项，下一次打开会重新落到主工作区。弹窗保持独立任务会话和资源状态，避免出现可交互入口与实际视图不一致。变更后的本地联调检查记录在 `E:/codex-build/lilia-workbench/popup-management-migration-check.log`；runtime shell 相关 59 项行为测试通过，日志为 `E:/codex-build/lilia-workbench/management-surface-routing-tests.log`。

正式门禁复核记录在 `E:/codex-build/lilia-workbench/continued-formal-verify-2.log`。先用正式配置重新生成 Git pin 锁文件，确认 `--locked` 能正常进入编译；随后 `cargo xtask verify` 仍稳定失败于 68/76 个正式 NanaUI API 缺口，未出现管理页面路由引入的新错误。正式 `.cargo/config.toml` 未写入本地 patch。

### 轮次提交服务边界

`DesktopTurnSubmissionService` 现在拥有轮次提交串行栅栏、耐久队列和 Composer/Guide 提交存储，以 `TurnSubmissionServiceFeature` 注册到 Kernel。Agent 启动、恢复、领取、完成和取消，以及 Composer 发送和 Guide 派发均只经该服务取得队列或提交事务；`DesktopApplicationInner` 不再公开 `pending_turns`、`turn_submission` 或 `submissions` 原始锁。常规路径将中毒锁报告为状态不可用，原有完成/取消恢复路径继续在服务内部恢复锁值。

本轮聚焦验证通过：`application::submission` 6 项原子提交与回滚测试、`application::agent` 11 项轮次上下文与交互测试、`kernel_host::tests` 7 项组合根测试（含 `TurnSubmissionServiceKey` 注册）。完整 `cargo xtask verify` 和 `cargo xtask agent-debug` 结果另记，不能用该聚焦测试替代端到端 UI 验收。

### Hook 执行服务边界与浏览器运行证据

`HookExecutionService` 现在拥有 Hook 源解析、插件包 Hook 装载、每轮执行栅栏，以及执行完成或失败的持久化；它以 `HookExecutionFeature` 注册为 `HookExecutionServiceKey`。Agent 的 prompt 与 stop Hook 都只通过该服务执行，`DesktopApplicationInner` 只在启动时装配服务实例，不再持有原始 Hook 执行存储或锁。`HookDocumentsService` 保持 Hook 配置读取和编辑的所有权，两者不共享可变执行状态。

本轮聚焦验证通过：`cargo check --locked -p lilia-desktop --lib`、`cargo test --locked -p lilia-desktop application::hooks -- --nocapture`（4 项）以及 `cargo test --locked -p lilia-desktop kernel_host::tests -- --nocapture`（7 项，含 `HookExecutionServiceKey` 注册）。

完整 Agent Debug 已通过，主记录为 `agent-debug-runs/lilia-agent-debug-1788673414644/summary.json`。浏览器路径验证了任务页面导航、人工接管、显式恢复、子窗口拒绝和隔离身份子标签；浏览器 Agent 验证了观察、输入、点击、滚动和截图，包含 5 次审批、拒绝和待执行操作取消。其子产物位于 `agent-debug-runs/lilia-browser-agent-1788673515584`。该运行尚未覆盖活动 CDP 操作取消、上传下载完成、IME/拖选和远程模型质量，不能替代这些端到端验收。

### 注册文件监视服务边界

`RegistryFileWatchService` 现在拥有用户与项目级 Hooks、Skills、MCP 和 Plugin 注册文件的目标计算、去抖动变更合并、事件发布和监视线程生命周期。它只依赖 `LiliaDataPaths`、`ProjectTaskService` 和共享事件总线，后台循环不再持有 `DesktopApplication`。该服务以 `RegistryFileWatchFeature` 注册为 `RegistryFileWatchServiceKey`；原有应用方法仅转发启动、停止和兼容读取。

`cargo test --locked -p lilia-desktop application::registry_watch -- --nocapture` 通过 3 项：外部 Hooks 编辑事件、后建目录的轮询回退以及停止后重启。`cargo test --locked -p lilia-desktop kernel_host::tests -- --nocapture` 通过 7 项并确认该服务已注册，`cargo check --locked -p lilia-desktop --lib` 通过。此变更不改绘制或布局路径，因此未重复运行性能门禁。

### Product 变更订阅服务边界与正式验收

`ProductChangeFeedService` 现在拥有 durable Product event 游标、轮询线程、停止信号和变更合并。它只依赖 `ServiceAuthority` 与 `DesktopEventBus`，后台循环不会再捕获 `DesktopApplication`。服务以 `ProductChangeFeedFeature` 注册为 `ProductChangeFeedServiceKey`；`DesktopApplication` 上的播种、启动、轮询和停止方法保留为兼容转发。外部写入仍按项目、任务、路线图和自动化实体发布范围化失效事件，首次启动仍跳过历史记录。

本轮针对性验证通过：`cargo test --locked -p lilia-desktop application::change_feed -- --nocapture`（3 项）覆盖外部 Product 写入、历史游标播种和后台重启；`cargo test --locked -p lilia-desktop kernel_host::tests -- --nocapture`（7 项）确认服务完成 Kernel 注册；`cargo check --locked -p lilia-desktop --lib` 与范围 `git diff --check` 通过。

当前正式 NanaUI pin `67c5955603c80da241bf1930c632490825f5aaf7` 下，`cargo xtask verify` 通过，包含边界检查、21 项不可变 Git pin 检查、workspace 测试及文档测试。`cargo xtask agent-debug` 通过，主记录为 `agent-debug-runs/lilia-agent-debug-1788675185076/summary.json`，浏览器 Agent 子记录为 `agent-debug-runs/lilia-browser-agent-1788675285480`。任务弹窗草稿隔离、自动化运行和真实 WebView2 页面导航、接管、恢复、子窗口审批均通过；浏览器 Agent 观察、输入、点击、滚动、截图完成 5 次审批，拒绝和待执行取消也通过。活跃 CDP 操作中取消、上传下载完成、IME 和拖选仍未覆盖。

`cargo xtask performance` 报告 `agent-debug-runs/lilia-performance-1788675360913/performance.json` 通过所有绝对门禁（5 次冷启动、30 帧采样）：冷启动 P95 1528.2883ms（门限 15000ms）、输入帧 P95 14.0107ms、调整尺寸帧 P95 14.3129ms（门限各 100ms）、千条时间线 555.3264ms（门限 30000ms）、空闲 CPU 0.09765625%（门限 10%）、RSS 207110144 bytes（门限 1GiB）。相对 `lilia-performance-1788670790987` 的非受控历史样本，冷启动 P95 从 6229.2203ms 降至 1528.2883ms，时间线从 616.907ms 降至 555.3264ms；输入和尺寸帧也略有改善。两轮都在共享机器上运行，历史差异不归因于单一改动，稳定依赖快照后的受控比较仍待完成。

完整计划仍未完成。后续继续处理其余 `DesktopApplication` 运行协调、UiModule 平铺投影和遗留导航状态，并针对浏览器 IME/DPI/文件传输、活动操作取消、文档多视图冲突与终端大输出补齐实际端到端证据。
