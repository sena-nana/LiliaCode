# 工作台功能与所有权清单

2026-09-06 实施中清点。此表记录当前入口和迁移验收单位，不是改动前截图或性能基线，也不代表每项已通过运行验收。源头为 desktop 的 Message/各子消息、module 清单、KernelServices/Feature 清单、application 服务及 contracts；后续新增入口必须同步对应行。

## 桌面入口与行为

| 表面 | 必须保留的操作 | 权威事实与当前入口 | 迁移验收 |
| --- | --- | --- | --- |
| 启动与项目入口 | 打开目录、创建分类、克隆、CLI 打开/显式导入/handoff、单实例转发 | launcher、application/cli/handoff；Product 项目 | 已有目录/无效目录、重复启动、导入预览失败不写原库 |
| 左侧项目与任务 | 搜索、新任务、重命名、置顶、排序、跨项目移动、归档/恢复、打开独立任务窗口 | Project/Task/SidebarMessage；ProjectTaskService | 活跃/等待处理/失败可见；跨项目移动后旧资源权限失效；返回恢复当前窗口 |
| 项目设置与资源 | 名称/工作目录、外部文件管理器/终端/编辑器、项目资料与树目录 | ProjectMessage；Product + ProjectSettingsService | 项目不存在/归档/目录变化；无效配置不覆盖已存数据 |
| 对话与历史 | 正文、过程展开、复制、重试、加载更早、滚动到末尾、链接与图片查看 | TimelineModule、TaskSessions；TimelineService | 主/独立窗口功能相同；旧任务消息不作用于新任务；每窗口阅读位置独立 |
| 输入与上下文 | 草稿、发送、停止、计划/目标、附件/粘贴、斜杠命令、对话引用、补全、提示词优化 | ComposerModule/ComposerView、共用 TaskView、InputService；ComposerStore 与 Agent turn | 条件修订冲突保留草稿；文件对话框返回检查原任务；停止携带 task/turn |
| 审批与提问 | 工具同意/拒绝、用户问题、MCP 表单/URL elicitation、取消 | AgentInteraction/ToolConsent/McpElicitation；Product pending/Agent await | 每个 pending 的 task/turn/request 归属一致；已结束请求不能重放 |
| 任务属性与状态窗口 | Todo/guide、目标、用量、任务状态、置顶/透明度、跳转/新对话 | TaskModule、TodoService、ConversationStatusMessage | 非活动窗口不会误操作当前任务；状态窗口停止也核对 turn |
| 工作树 | 当前目录/新建/关联、清除关联、合并后归档、清理后归档 | WorktreeMessage、DesktopWorktreeService/WorktreeFeature | 同仓库并发；真实 Git 失败不伪造成功；取消与已经发生的副作用一致 |
| 文件与编辑 | 文件树、搜索、打开多个文档/同文档多视图、保存、查找替换、诊断、定义、冲突处理 | DocumentsModule、DocumentService、共享 LSP | 未保存编辑不被外部更新覆盖；版本过期结果拒绝；选区/光标/滚动按视图隔离 |
| 代码工具与差异 | 代码/文本搜索、范围切换、Git 状态/差异范围、审阅/修复请求 | CodingMessage；共享 Agent coding services | 查询结果绑定项目；差异实际可读与导航；不创建第二套代码服务 |
| 底部终端/问题 | 输入、选择、复制粘贴、中断、尺寸同步、退出状态、诊断跳转 | DesktopTerminalService/PTY；NanaUI TerminalGrid | 真实 ConPTY、小窗/大输出/退出后复制；两个终端不会串输入 |
| 浏览器 | 导航/前进后退/刷新、观察/点击/输入/滚动/截图、人工接管/显式恢复、进行中操作取消、文件/新窗口审批、失败重试 | BrowserSessions + ProductBrowserScopeAuthority；Windows WebView2 | 真实页面副作用；权限/取消/页面版本；项目资料隔离；IME/DPI/浮层/重启恢复 |
| 项目记忆 | 列表/选择、标题/正文/标签/范围、保存/删除 | MemoryService + MemoryView | 跨项目写拒绝；返回保留编辑焦点；删除后的选择合法 |
| 路线图 | 里程碑创建/编辑/状态/顺序/关联/删除 | RoadmapService + RoadmapView | 关联与里程碑均属于目标项目；边界移动无动作；返回保留选择 |
| 架构 | 图选择/移动/编辑、变更历史、应用/拒绝/回滚、检查器 | ArchitectureService + ArchitectureView | 主图/检查器共用选中身份；回滚校验当前已应用版本和项目 |
| 全局自动化 | 列表/创建/编辑/保存/图操作/发布/运行历史/人工确认/取消/任务跳转 | AutomationView 拥有页面与运行反馈；AutomationMessage、AutomationFeature 和共享自动化服务持有业务，节点草稿和表单已拆分，范围选择、每窗口状态与 Jobs 迁移仍待完成 | 管理页往返保留任务；重复触发/失败/取消和运行记录一致；回复和继续绑定运行及等待节点 |
| 设置：外观/快捷键 | 明暗/系统主题、侧栏模式、快捷键捕获/保存/清除 | SettingsModule/SettingsView；NanaUI settings | 实际主题截图、冲突按键、窄窗和高 DPI，不丢返回焦点 |
| 设置：Agent/模型 | 交互开关、自定义 Agent CRUD、模型和端点、凭据、助手/功能模型映射 | AgentInteractionService、ProviderRuntimeSettingsService、ProviderCredentialService | 配置失败回滚；凭据不显示或写普通日志；运行中变更的生效边界明确 |
| 设置：扩展 | Skill/Plugin 安装启用删除、Hooks 编辑保存、MCP 注册/凭据/资源/提示词 | ExtensionsModule、HookDocumentsService、共享 Agent registry | 无效注册不覆盖旧值；请求结果归属原服务；删除有真实确认语义 |
| 设置：配对与用量 | 远程主机/设备/配对/撤销、保持唤醒、用量筛选 | RemoteFeature/UsageFeature；共享权限与用量记录 | 撤销后请求拒绝；用量按筛选权威重读；配对信息不泄漏 |
| 设置：更新/导入 | 检查/安装更新、发布页面、数据源选择/预览/执行/重置 | DesktopUpdateService + Jobs、ImportFeature | 状态机/进度、重复安装拒绝、失败恢复、导入失败不修改原数据 |
| 窗口/壳 | 命令面板、菜单、分栏/调整、关闭/移动到新窗、状态恢复 | WindowRoute、WorkspaceSessions、NativeWorkspaceTopologyState | 窗口与窗格身份校验；窄窗切换保留资源；坏布局保留原字节再回落 |

## 后台执行与退出归属

| 工作 | 当前所有者/入口 | 必须满足的生命周期 |
| --- | --- | --- |
| 项目命令运行 | ProjectCommandRunService + 现有 TerminalService | 启动预约与进程生命周期分离；失败释放预约；追踪全部并发 session，项目改绑后停止旧启动 |
| 任务执行、标题、建议、提示词优化、索引/查询、Git/克隆、远程/更新/导入 | Kernel 注册的 Jobs 协议与 Mutsuki runtime；KernelServices 注入窄 port | 阻塞工作不运行在 UI 线程；完成结果核对任务/资源身份；不能将忽略取消的 Git 副作用宣称撤销 |
| Agent turn 队列与自动继续 | AgentSession/runtime 与 application/submission/auto_turn | Composer/Guide/Queue 同一事务；唯一 Mutsuki executor；旧 turn 停止请求不能取消新 turn |
| 自动化启动、继续与取消 | automation Feature `lilia.automation/operate@1`、现有 Kernel/Mutsuki Jobs | 启动事务检查发布版本；继续绑定运行与等待节点；取消独立提交；晚到结果不切换另一条历史运行；每窗口 UiModule 归属仍待迁移 |
| 自动化窗口模块 | `UiModuleRegistry`、`AutomationController`、`UiModuleContext` | 工厂按窗口创建实例；Jobs、领域事件和操作消息按窗口路由；自动化管理页归属主工作区，任务弹窗保持任务对话和资源视图，避免挂载无渲染器的管理标签 |
| Product 变更订阅 | application/change_feed | 保留 sequence，缺口重读；停止订阅后不再把旧投影写入已关闭窗口 |
| 扩展注册文件监听 | application/registry_watch | 保留共享 registry；合并变更；应用关闭释放监听线程 |
| 项目文件监听 | ProjectFilesService | 路径/项目重新绑定时废弃旧 watcher 和旧结果；有界失效通知，服务关闭回收线程 |
| PTY 读写和退出 | terminal Feature/session | 有界网格/输出；锁外回调；关闭不死锁，退出后展示最终屏幕 |
| WebView2/UI 线程桥与私密输入票据 | BrowserWorkbench/platform、BrowserSessions | 队列有界、scope/lifecycle/version 校验、接管/关闭/拒绝清除票据、过期单次消费 |
| 窗口拓扑持久化 | NativeWorkspaceTopologyStateWriter | 恢复完成前不写部分拓扑；坏记录备份失败时禁止覆盖 |

## 跨端消费者

- `crates/lilia-contracts/contracts` 与对应 Rust API 是产品数据契约源，`crates/lilia-service` 提供共享 Product/Agent 权威。`apps/service` 是该服务入口，不直接消费 NanaUI 控件或窗口 ID。
- `apps/android/remote-core`、Android app 与 voice-app 消费远端协议。新增 browser/terminal 数据并不表示 Android 已实现本次桌面控件；接口变化需核对远端 payload/parser，不能把桌面编译当作 Android 验收。
- `agent-debug-contract.json` 对应桌面调试协议及 `xtask agent-debug` 消费者。目标 ID 必须来自当前挂载可操作控件，测试不得绕过真实权限或直接伪造成功页面状态。
- NanaUI 的 NativeContent、布局/焦点/输入事件属于本地 UI 合同；WebView2 登录资料与 Product TaskId 绑定属于应用。未发布本地改动不可写成已获取的 Git pin。

此清单仍需按每个控件/命令的实际运行证据逐项销项。具体阶段结果与失败记录见 [重构记录](task-workbench-refactor.md)；浏览器安全边界见 [浏览器宿主](task-browser-host.md)。
