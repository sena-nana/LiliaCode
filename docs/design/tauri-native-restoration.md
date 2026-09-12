# Tauri 桌面等价还原

以下为原任务的实施与验收历史。2026-09-10 已将缺失的框架组件恢复并发布到 NanaUI `1100b020b9384f58fb0e744d50f0b2857973d28c`，完成当前真实 Git 来源的工作区、原生矩阵和性能复验。最新结果及未验收边界见 [NanaUI 能力恢复与依赖升级记录](nanaui-upgrade-blockers.md)；旧候选哈希和待发布描述仅属于原任务检查点，不表示当前依赖仍未发布，也不代表整个 Tauri 等价计划已签署。

## 历史验收状态（原任务）

本计划仍在实施，尚未整体签署。下文各历史回放记录只覆盖当时版本和所列场景。
最新冻结 NanaUI 候选为 `a07bc3e1`，含普通文本事务历史、前置图标、实时按键视图及语义混色。富粘贴的权威附件事务与请求过滤已完成；真实窗口回放发现并修复了异步 UI 事件逐条投影清空撤销历史的问题。最终四组矩阵和本批性能结果见文末，不沿用旧候选结果签署新代码。

| 范围 | 当前结果 |
| --- | --- |
| NanaUI候选 | `a07bc3e1`，137文件，干净归档重放及当前逐文件哈希一致 |
| 最新完整工程结果 | workspace970通过、1项既有忽略；Nana runtime832；语义混色3；按键实时视图1；富粘贴应用5和存储8；本结果早于宿主投影时序修复 |
| 本批实窗 | 四组完整矩阵通过，各60PNG，含撤销/重做保存及重启；host `0a5dc1a6`，明细见文末 |
| 本批性能证据 | `1788511254404`绝对阈值通过；历史同条件比较缺失 |
| 当前源码缺口 | 输入器行内附件/对话引用已按引用原文作为编辑原子，并绘制 icon+显示名+关闭 chip 覆盖原文；关闭与退格都整段移除。历史像素签署与正式 NanaUI pin 仍待上游发布 |
| 正式工程门禁 | boundary通过；verify因候选未发布导致Git pin缺失而失败，未绕过 |
| 最终视觉／平台证据 | 历史同条件macOS像素基线、真实Android配对等仍缺；首次路线图保存不可达保留为一次未定位、后续未复现失败 |

## 基准与判定

历史基准为 `2eec58e`；实施起点为 `09ff417`，包含用户已有的
`runtime_shell.rs` live rows 改动。旧界面源码与旧 LiliaUI pin
`5478dab570e5685e753804e16e788e5ad33bc5d9` 是视觉事实源。
当前仓库仍是 Cargo / NanaUI / WGPU；不恢复 Vue 宿主或 legacy SQLite。

本轮先按源码恢复。没有同 revision 的 macOS 历史构建，不能签署历史像素一致。
Windows 宣传图与 `artifacts/shell-restored.png` 不是同语料基线。
旧 `nanaui-feature-equivalence.md` 的 BLOCKED 是当时验收状态，不直接继承为当前结论。

主要断层是应用状态没有进入可见投影，或动作只有调试入口。修复通过正常
ShellIntent / UiModule / application 接线，主窗与弹窗共享对话控制与时间线呈现。
自动化列表已经挂载在导航区；旧调查把它误报成未挂载，实施保留这条现有路径。

## 源码布局标准

| 项目 | 历史来源 | 目标 |
| --- | --- | --- |
| 标题栏 | `styles/shell.css` | 36 逻辑像素 |
| 主内容边距 | `styles/shell.css` | 水平 24、垂直 20 |
| 输入区域 | `styles/chat.css:.chat-controls` | 最大宽度 860 |
| 时间线 | `styles/chat.css:.agent-timeline` | 最大宽度 788，含 28 轨道偏移 |
| 用户消息 | `.agent-timeline__message-row .chat-bubble` | 靠右，最大 min(76%,620) |
| 空态标题 | `.chat-empty` / `__headline` | 24 字号、500 字重；标题与建议组整体中心上移 8vh，建议区最小24、间距14 |
| 发送按钮 | `.chat-composer__send` | 30×30 圆形 |
| 普通／待处理输入 | `.chat-composer__rich-input` / `__input` | 行高 22；最大高度 92／74 |

颜色通过 NanaUI 语义令牌消费，不从截图吸色，不在产品模块另建调色板。
旧 LiliaUI 锁定版本已通过只读 GitHub API 取证，22 项主调色板经色彩空间转换与当前
NanaUI 一致。精确值、别名和平台默认差异见 [主题源码基线](tauri-native-theme-source.md)。
macOS 默认圆角基数是 8；输入器与共享待确认面板已按此调整。通用控件尺寸、字体塑形、
材质仍需逐项核对，不能由调色板一致推断整体视觉等价。

## 领域矩阵

“已接线”表示正常用户控件已接入现有业务，不等于已通过全部平台、异常和像素验收。
每项验收必须同时记录可见控件、操作结果、权威状态和截图。

| ID | 领域／历史入口 | 本轮实现或保留路径 | 关键验收／剩余范围 |
| --- | --- | --- | --- |
| EQ-001 | Shell / router / AppShell | Native 壳；项目 Sessions；设置独立分区；外观页侧栏模式正常入口 | grouped／unified普通控件切换、实际层级与保存值已在四矩阵验证；像素基线待补 |
| EQ-002 | ProjectsOverview / ProjectTreeItem | 复用 dashboard 统计、卡片原地更新 | 项目更新后标题、统计与导航一致 |
| EQ-003 | SessionsView / SidebarSearch | 取消固定80项截断；可搜索分页列表 | 105条会话已通过六页正常控件回放并打开第105条；切换项目不混入旧结果继续分项验证 |
| EQ-004 | AgentTimeline | 主窗／弹窗共用28px过程轨道、状态节点和最终回复层级；过程组与子事件独立折叠，保留原事件动作 | 3项过程行为及真实保留树几何测试通过；分页与虚拟列表保持原路径，真实窗口滚动、状态和跨窗回放待统一验证 |
| EQ-005 | ChatComposer / ComposerToolbar | 共用模型、推理、优化、压缩、引用、review控制 | 实际请求采用选择值；异步优化不覆盖新草稿 |
| EQ-006 | useComposerPaste / ChatBubble / ImageViewer | 既有附件合并；图片像素解码与HostTexture预览；Markdown和历史图片缩略图接异步图片资源；主／弹窗正常条目可回看 | 历史附件投影、消息身份、双窗实际Activate／图片指针及异步资源2项测试通过；加载解码与缓存复用2项测试通过；发送后真实窗口绘制／回看及关闭释放继续统一验收 |
| EQ-007 | MarkdownBlock / MarkdownMath / MarkdownMermaid | NanaUI 原生 RaTeX/Mermaid→SVG→同宿主GPU；行内基线、实际块高度和源码选择复制 | 框架深浅GPU截图与30项定向测试通过；正式pin待上游发布 |
| EQ-008 | AskUser / ToolConsent | 复用既有审批消息；主／弹窗待处理面板替换普通输入器并保留草稿 | 长内容滚动、按钮可达与草稿恢复已测；拒绝、重复提交、取消、MCP字段与重启恢复按场景验收 |
| EQ-009 | TimelinePlanCard | 保留计划确认和继续同一turn；正常批准与要求修改入口 | 四组真实窗口验证approve及revise精确两行反馈、同工具调用及普通草稿恢复；其他审批类型继续分项验收 |
| EQ-010 | TodoFloat / taskGoal | 主窗／弹窗共享TodoPanel；空态隐藏，＋菜单创建；引导多行编辑、优先级、立即插入、删除；Goal编辑／刷新／清除保留预算 | 存储7项及真实挂载控件1项测试通过；queued禁止修改删除，取消回pending，sent不重复调度；四组真实窗口通过创建、优先级、取消编辑与删除 |
| EQ-011 | useTaskComposerController worktree | 既有任务重新接工作树菜单 | 当前／新建／附加、运行锁、归档清理与合并 |
| EQ-012 | Provider / Chat runner | 保持单Native AgentKit；模型选择回到输入器 | 本地真实HTTP验证手动／自动模型和推理采用选择结果；外部Provider和复杂子代理场景需凭据 |
| EQ-013 | RoadmapView / MilestoneInsights | 任务关联、完成统计、原地刷新；同目标刷新保留未保存编辑 | 关联及跨窗口变更期间草稿保留已测；关联／解除后读模型及持久化一致 |
| EQ-014 | MemoryView | 作用域分组、条目启停、注入及冷却设置；恢复选中保留草稿，作用域迁移通知新旧可见范围 | 持久共享存储下草稿及作用域刷新已测；多行正文、启停和重启恢复按场景验收 |
| EQ-015 | Plugins skills/packages | 正常设置分区和现有扩展操作 | 安装／启停／移除、作用域与错误恢复 |
| EQ-016 | HookSourceEditor | 正常设置编辑入口 | handler配置保存、信任与启停实际生效 |
| EQ-017 | McpServerEditor | 正常配置和凭据控件 | stdio/HTTP、密钥脱敏、连接失败与删除确认 |
| EQ-018 | AutomationWorkspacePage | 导航列表、画布与分面板检查器；节点编辑进入同一草稿；普通及条件／switch连线；发布、运行、等待回复与取消；服务级Product／Agent事件触发 | 正常画布配置、发布启用、离页Product事件范围过滤及Human运行两次同home重启已通过；外部副作用幂等和其他节点异常仍分项验收 |
| EQ-019 | RemoteControlSection | 电脑名、二维码、配对和可信设备 | 普通保存名称、生成／取消二维码、撤销取消／确认及刷新已由真实控件与只读权威SQL验证；真Android设备首次配对／重连仍需设备 |
| EQ-020 | QuotaUsageSection | NanaUI DonutChart 恢复项目／会话／工具 Top5 圆图、图例和空态；66/74px、62% 内圆、2px 分隔 | 四类堆叠柱、总量曲线、日期轴和底部图例已在非零21条本地权威语料四矩阵绘制；应用悬停／圆图继续验收，外部真实额度另需凭据 |
| EQ-021 | ArchitectureSidebarPanel | 图与节点／关系详情共享权威快照；节点类型、摘要、路径、关系方向；历史理由、时间、版本迁移及变更展开；正常详情入口和回滚 | 架构10项定向测试通过，覆盖正常选择事件、失效选中清理、权威版本回滚、重复操作、Agent审批与布局恢复；真实窗口及跨窗同步待统一回放 |
| EQ-022 | SharedServices / IabSidebarPanel | NanaUI管理WKWebView，应用地址/导航/截图草稿接线 | 独立宿主及应用两页导航/后退/截图草稿均通过；正式pin待发布，窄窗与异常恢复继续验收 |
| EQ-023 | NativeCredential / attachments | 保留系统对话框/Keyring/剪贴板 | macOS权限、取消、路径与图片粘贴 |
| EQ-024 | PopupWindow / tray | 保留托盘、快捷键、状态窗；共享对话控制 | 多窗口焦点、关闭、草稿和窗口几何 |
| EQ-025 | CLI / task_handoff | handoff首次写草稿、重复导入不覆盖 | 真实CLI首次导入可编辑且不自动发送，离开后再次handoff保留编辑后的摘要与revision，四组窗口回放通过 |
| EQ-026 | AppUpdateHost | 保留更新状态机及确认安装 | 签名更新与Windows安装烟测按发布变更触发 |
| EQ-027 | 显式导入 | 保留正式home与显式导入边界 | 不自动合并、不写旧db、不覆盖现有数据 |
| EQ-028 | 异常与并发 | 复用revision、权威事件和持久化机制 | 故障重试、并发草稿、重启和副作用不重复 |
| EQ-029 | Agent Debug | 真实控件ui-observe/ui-click/ui-click-at/ui-hover/ui-input/ui-key/ui-input-frame；macOS采集及同binary/home重启 | 控件未挂载、被遮挡、禁用或不可输入时必须拒绝；悬停有限坐标测试与实际项目入口回放通过；重启不重新播种业务夹具 |
| EQ-030 | 性能 | macOS进程CPU/RSS采样；现有绝对门禁 | 历史同平台基线尚不可比较，不把缺失标成通过 |

Todo / Goal 的普通事件经过窗口绑定的 `ShellIntent::Todo`，编辑器按窗口和任务保存独立草稿。
引导与运行队列共用调度互斥；排队中的正文和优先级不会被过期界面改写，取消执行后可重新编辑。
Agent 原生 Todo 继续只读，已发送引导隐藏，普通消息草稿不受编辑目标或引导影响。

时间线依据 `2eec58e` 的 `AgentTimeline.vue`、`TimelineNodeIcon.vue` 与 `styles/chat.css`：
轨道宽28、节点中心偏移14、图标13，垂线宽1且位于13.5；普通过程标题13、预览12。
折叠过程只呈现单行标题与预览，展开后呈现独立正文；运行、等待与错误使用状态文字及语义颜色。
过程组采用独立的 `process-group:` 行身份，展开后子事件仍保持原始 ID 和独立动作，
作为同一虚拟列表的平面行输出，避免合并 Markdown 后丢失复制、展开与重试对象。
失败或需要重试的事件不隐藏在折叠组内，最终回复保留独立继续、分叉及应用建议操作。
几何测试使用实际保留树布局，并通过正常控件激活验证窗口绑定的展开与复制事件；
组投影测试覆盖运行状态聚合、子事件身份和历史分页标记，未替代真实滚动及历史像素验收。

历史 `ChatBubble.vue` 的附件入口对应同一消息中的附件。当前 `ShellTimelineRow` 保留权威
`TaskTimelineItem.attachments`，主窗与弹窗共用附件条目；点击携带消息 ID 和附件 ID，
从窗口对应任务历史查询，避免发送清空输入器后失效或与新草稿附件混淆。图片复用现有加载、
GPU纹理与预览关闭路径，其他文件及目录复用系统打开入口。安全加载器在线程内加载与解码，
已就绪图片以缓存的data URL及像素尺寸进入主／弹窗投影；data URL只编码一次并由Arc复用。
Markdown正文和图片资源都参与视图刷新条件，异步结果到达无需正文变化即可呈现。
历史图片缩略图复用同一NativeMarkdown图片资源，布局30×30、图片最大边24，点击保持消息身份；
实际保留树布局与正常图片指针事件已验证，最终绘制仍需真实窗口截图验收。
图片应用缓存的64MiB驻留预算同时统计原始编码、RGBA像素和缓存data URL，
解码器先扣除编码与data URL所需空间，单张图片超限在线程中拒绝。LRU驱逐后的资源
进入不自动排队的状态，避免多图超预算反复解码；用户点击可重新请求，已打开预览受到保护。
该预算约束应用图片缓存，不代表WGPU纹理缓存或整个进程的内存上限。
ImageViewer的纹理现在位于舞台背景之后、关闭和说明控件之前，内容裁剪到舞台区域。
主窗与弹窗只在图片来源改变时重置缩放和拖动状态，异步纹理或元数据更新保留当前操作。
固有尺寸直接来自已解码像素；框架按历史`min(1,舞台宽/原宽,舞台高/原高)`等比居中，
小图不放大，缩放后的拖动范围按实际图片尺寸计算。正式指针分派接通关闭、滚轮、拖动和释放。

架构还原依据 `2eec58e:apps/desktop/src/components/chat/ArchitectureSidebarPanel.vue`：历史节点和关系
为信息呈现，变更由 Agent 架构工具／审批产生。当前 `ArchitecturePanel` 将同一数据投影到可选列表，
`ArchitectureModule` 清理图刷新后已失效的选中项，回滚继续调用权威服务并记录递增版本。
历史列表保留最近40条，展示理由、日期时间（显式UTC）、前后版本和展开后的变更项目。
完整架构详情与浏览器状态均参与 inspector 缓存键，避免选择、历史展开或导航状态改变后仍显示旧内容。
此处测试不构成 macOS 历史像素签署，真实多窗口同步仍在统一回放范围内。

设置表单复用 NanaUI `SettingsRow` 的横排/纵排能力；以工作区逻辑视窗宽度为准，
900px 及以下标签与控件纵排，以上横排，普通输入宽360px并受容器最大宽度约束。
自动化检查器仍独立采用纵排，不继承设置页断点。配额圆图使用正数 Top5，按值降序稳定排序；
项目和会话取 Token，工具取调用次数，图例颜色随排序绑定。三个图保留66px直径与10px图例间隔，
860px及以下改74px；空数据保留浅色圆形占位和空态文字。趋势使用同一每日桶投影，
高度248px（860px及以下220px），四类堆叠柱叠加总量曲线、日期轴与底部图例。
圆图与趋势经正常Pointer Move选择数据，提示随离开关闭，图表移除时释放提示；
命中坐标经保留树转换，设置滚动后仍对应实际扇区。

## 验证机制

核心对话约束使用已有权威数据测试验收，不以按钮触发或文本匹配代替：

- `crates/lilia-agent/src/wire_service.rs::task_session_fork_stops_at_the_selected_durable_turn`
  通过本地模型夹具执行两轮并持久化会话，以第一轮为锚点分叉；目标包含第一轮，排除
  第二轮消息和事件，源会话仍保留第二轮。覆盖的是截至锚点轮的完整历史。
- `apps/desktop/src/application/composer.rs::asynchronous_content_replacement_does_not_overwrite_a_newer_draft`
  使用真实草稿存储验证优化成功后清除 workflow；用户继续输入后，携带旧 revision 的
  异步优化结果返回 RevisionConflict，新草稿内容保持不变。该测试已包含在桌面 lib 门禁中。
- `crates/lilia-agent/src/native_runtime.rs::ask_permission_pauses_and_resumes_same_agentkit_session`
  通过真实 Agent 工具调用先暂停写文件、批准后执行，再将文件改为新内容并重复批准；
  过期审批返回 InvalidInput，文件不会再次被工具覆盖，会话事件水位、消息和轮数均不变。
  分叉与增强后的审批测试已分别通过 `cargo test -p lilia-agent --lib <test-name> --locked`。


自动化事件调度位于 `application/automation_dispatch.rs`，执行器安装后启动，与页面是否
打开无关。`automation_inbox.rs` 使用共享 Product domain_db，先持久化信号与每个匹配
工作流的投递记录，再执行。忙碌工作流保留 pending，其他工作流独立推进；失败记录原因
并重试。运行创建成功后投递标为 delivered，进程中断留下的 Running 运行会继续恢复。
工作流被删除、停用或发布范围改变时，尚未开始的投递记录 skipped 及原因。

Product 事件按权威序列读取，信号及目标提交后才推进 Product cursor；重复读取通过稳定
信号 ID 去重。Agent 时间线完整按存储顺序检查，source checkpoint 与信号/投递同事务
提交，初次安装仅建立历史基线，后续启动补偿基线之后的权威事实。实时通知是唤醒信号，
捕获失败的任务保留重试。Agent `TodoUpdated` 同时映射 `todo_changed`。本地 Todo 的
内容增删改通过同库 SQLite 触发器原子写入变更日志，所以创建后立即删除也不会丢失；
内部 guide 状态切换不作为内容变更，自动化创建条目不反向触发，用户编辑可编辑引导仍
会触发。Todo 日志水位与投递同事务推进。

自动化 Agent 在派发前持久化 task/turn → run 来源，完成和队列确认删除后仍保留；升级
时可从运行节点输出补齐旧关联，不依赖线程局部标记判断迟到事件。自动化自身时间线
和来源关联事件被抑制。执行、恢复、用户继续、Agent 完成和取消使用同一 run 级门，
不同运行可并行，同线程 Inline Agent 完成允许重入；同一桌面实例的跨线程操作串行。

节点副作用继续采用既有幂等端口，WaitingUser 保持等待，已完成节点沿图传播保存的
输出。权威 ProductEvent 目前只含 action/revision，不含历史任务快照，因此任务状态
范围按捕获信号时的当前任务状态匹配；不宣称可重建停机期间每次历史状态。初次启用
前历史不补跑；关闭且在停机期间已消失的外部交互不重新发起。

检查器按节点、工作流、范围、运行记录拆分，新增节点直接选中；等待确认问题与回复操作
位于运行回放JSON之前。检查器编辑即时合入本地草稿，普通保存和发布使用同一份数据，
运行事件刷新不会丢掉草稿。普通output/success端口都可推进下游，条件及switch在尚未
连线时提供可执行的分支端口。上述接线与边界仍须结合正常控件回放和权威状态验收，
不能只以编译成功标为完整等价。

`cargo xtask agent-debug` 使用隔离 LILIA_HOME 与本地确定性模型端点，普通回放不接触个人数据
或外部模型。`ui-click` 调用 NanaUI retained control activation；`ui-input` 经正常聚焦、
选择和编辑事件；`ui-key` 经过 RuntimeInputAdapter 键盘分派。回复只表示事件已派发，随后
必须等待业务观察符合预期。定位校验使用窗口范围、祖先挂载与实际 hit-test，焦点控件的
键盘路由保留其自身下拉菜单状态。
旧 `click` / `input` 保留诊断兼容，不用于声明可见控件验收通过。

夹具凭据通过显式 `LILIA_AGENT_DEBUG_EPHEMERAL_CREDENTIALS=1` 使用内存 SecretStore，且
必须同时处于 debug 构建、Agent Debug 和 seed 模式；正常产品继续使用系统凭据。
原因是重新编译的 macOS 调试二进制读取同名假凭据时可能等待钥匙串授权。这条隔离回放
不代表系统钥匙串已验收。性能语料准备与面板尺寸诊断仍使用旧 helper；输入到呈现延迟
已改用真实输入控件，不把性能诊断当作业务闭环证明。

macOS截图先由CoreGraphics查找指定子进程的真实窗口，再用系统screencapture采集。
PNG旁保存窗口ID、进程ID、逻辑尺寸、像素尺寸与scaleFactor，不采集其他应用或整个桌面。
屏幕录制权限或窗口能力不可用时返回阻塞，不以空白PNG代替通过。
`cargo xtask screenshot --matrix` 从隔离 home 的正式窗口状态文件启动四组尺寸／主题，
每次记录源码 revision、工作区差异清单与请求尺寸；输出当前会话列表场景。

提交前运行 `cargo xtask verify`；交互验证运行 `cargo xtask agent-debug`；渲染性能运行
`cargo xtask performance`。源码还原验收覆盖960×600、1440×900与明暗主题；历史像素签署
必须等到取得对应macOS历史构建后另做同语料对比。

## 当前交付的限制

- 已恢复的是 Native 正常入口、共享对话组件、读模型投影和相应业务接线；本轮截图与
  行为测试不覆盖矩阵每项的成功、取消、故障、重启全部组合，不标全产品完成。
- Math/Mermaid 与 WKWebView 已在兄弟 NanaUI 实现并通过本地 path patch 集成编译。
  正式 manifest 仍锁定旧 revision；新能力需要将本次上游增量提交、发布后再固定新 SHA。
  临时联调配置与 lock 不代表正式 Git pin 验收。NanaUI 同时存在其他任务的动画等修改，
  提交范围必须单独审查，不能夹带。
- 真实 Provider 请求、GitHub 授权、Android 配对与撤销、系统钥匙串、托盘与更新等仍有
  外部或系统场景待验；调试夹具的预置设备不是实际远程配对证据。
- 通用侧栏／卡片令牌、部分设置页面结构、用量图形与历史仍有源码差异。尚未证明
  关键几何在 1 逻辑像素内，也未签署“源码还原通过”或“历史像素等价”。

## 本轮验证记录

- 自动化可靠性定向测试 `cargo test -p lilia-desktop --lib automation --locked`：21 项通过，
  含忙碌运行完成后的待处理信号、SQLite 重开、启动补偿、已确认 Agent 的迟到事件、
  Todo 创建/删除中间状态与可编辑自动化引导、run 级串行与 Inline 重入。

- `cargo xtask verify` 通过：边界与 23 项 immutable Git pin 检查、工作区测试与 Cargo check；
  桌面 lib 共 440 项测试通过。保留既有 macOS launcher 的 `home_hash` / test `Splash`
  警告，未为本轮改动无关的启动器作清理。
- `cargo xtask agent-debug` 通过，证据目录
  `agent-debug-runs/lilia-agent-debug-1788434094599/`。覆盖七个设置分区、设置返回、新对话、
  `#Composer` 搜索及选择引用、中文多行草稿、键盘选择推理强度。`composer.json` 中引用数
  为 1、草稿字节数为 22，并记录模型与推理的业务状态。
- 四组真实 macOS 会话列表截图均验证窗口逻辑尺寸与像素尺寸，缩放为 1：
  `lilia-screenshot-1788433513409`（960×600 浅）、`1788433531599`（960×600 深）、
  `1788433536458`（1440×900 浅）、`1788433540595`（1440×900 深），均位于
  `agent-debug-runs/` 下，对应 PNG 旁有 `window.json`。其他页面、状态仍按矩阵继续验收。
- 430×760 弹窗布局测试验证两行工具栏、工作树、权限、模型、推理、优化和发送控件位于
  窗口内且不相互重叠；800px 主窗工具栏自动换行测试验证打开侧栏时控件不重叠；长时间线测试验证滚动视口止于输入器上方。会话分页测试覆盖
  105 条，handoff 测试覆盖重复导入保留用户草稿，卡片测试验证正常激活与内容更新。
- 性能绝对门禁通过，首轮完整报告
  `agent-debug-runs/lilia-performance-1788433940024/performance.json`：dev 构建冷启动
  P95 2651ms、输入至呈现 P95 5.87ms、面板调整 P95 5.89ms、空闲 RSS 183MiB。
  历史同平台基线状态明确为 `not_compared`，不代表与 Tauri 的相对性能已验收。


### 锁屏期间的环境记录

后续窄主窗换行、透明图片线性色彩预乘和启动兜底后，代码门禁继续通过。末轮 GUI
验证遇到锁屏，读取 CoreGraphics 状态确认为 `displayAsleep=true`、
`CGSSessionScreenIsLocked=1`、`CGPreflightScreenCaptureAccess=true`。没有修改锁屏或
系统权限设置。`agent-debug-runs/lilia-agent-debug-1788434719433/` 的截图失败属于该环境
阻塞；此前通过的图片不能当作这些末轮改动的最新视觉证据。后续截图能力已恢复，新增真实回放证据列于下文；该失败轮不作为视觉证据。

重复启动采样还确认，旧启动路径仅在首帧呈现后初始化桌面业务，锁屏时会无限等待。
`LiliaShell` 现在保留首帧优先路径，并用一次性的 250ms wake 兜底进入同一初始化函数；
Ready/Failed 状态不会二次初始化。末轮已能到达正常 UI 回放，停止于系统截图操作。

### 并行实施的新增证据

- 真实菜单、滚轮和键盘回放统一经过 RuntimeInputAdapter；菜单坐标来自公开的
  ComponentGeometry，点击前验证正常命中与挂载，不直接调用菜单业务。
- 模型回放使用仅绑定 loopback 的夹具端点，检查实际HTTP JSON：界面选择的模型、
  reasoning_effort 和结构化对话引用必须出现在请求。端点只保存请求正文，不记录认证头。
  最新旧pin完整回放目录为 `agent-debug-runs/lilia-agent-debug-1788436078892/`，
  已包含模型请求、最终回复、分叉入口与清除锚点；不把引用元数据误称为完整历史注入。
- 时间线缓存补入运行/审批锁和历史分页入口状态，修复回复不再增量变化时仍隐藏分叉的
  问题；同一轮旧运行状态不再作为当前状态显示，权威历史仍保留。侧栏刷新保留更多入口。
- 更多菜单复用现有窗口内定位，修复底部入口的菜单向窗口外溢出；主窗及弹窗共享时间线
  788px外宽与右侧28px留白。用户气泡使用历史AccentSoft令牌、10×14内边距、右下角4px。
- 本地NanaUI联调下桌面lib 481项通过（独立构建目录，图片预览末轮修复之前；日志 `/tmp/lilia-native-desktop-final-tests.log`）。框架数学/图表 Runtime 13、rich_text 14、
  Scene 2、中文SVG像素1项通过，深浅真实GPU图位于兄弟NanaUI的
  `target/markdown-render/{dark,light}.png`。支持范围和OFL字体notice见其
  `docs/markdown-rendering.md` 与 `docs/third-party/katex-fonts/`。
- 锁屏期间可显式运行 `cargo xtask agent-debug --no-capture` 继续真实控件行为诊断，
  summary必须记录 `visualEvidence=not_collected`；默认门禁仍强制真实窗口截图，
  无截图模式不替代视觉验收或历史像素签署。
- `agent-debug-runs/lilia-agent-debug-1788439418463/` 已完成真实Todo创建、优先级调整、
  取消编辑和删除；`browser.png`与`browser-captured.json`验证菜单入口、两页导航、
  后退和原生截图进入当前草稿（附件1、草稿142字节，未自动发送）。该轮在后续自动化
  检查器滚动诊断停止，不能算整项agent-debug通过。
- `cargo xtask agent-debug --matrix` 将同一正常控件流程扩展至960×600、1440×900的
  明暗主题四组条件；区别于只采初始页面的`screenshot --matrix`，结果仍以实际产物为准。


### 末轮入口审计与验证隔离

末轮审计补齐了原来仅保留数据或 helper 的日常操作：

- 主窗与弹窗共用可消费的键盘策略；Enter 发送、Shift+Enter 换行，候选上下选择与
  Enter/Tab 确认先于发送；IME、禁用状态、其他焦点和重复提交分别受控。
- 时间线保留原消息附件，预览通过窗口、消息和附件三者定位，不从新草稿误取同名附件；
  历史图片使用30×30缩略图，加载完成后可以在正文不变化时更新资源。
- NativeMarkdown 由应用安全加载资源，框架消费已解析URL和固有尺寸，在同一Scene绘制；
  图片、超链接及拖选统一经过 RuntimeInputAdapter。选区动作继续使用既有复制、引用、
  追问业务，不另建对话状态。

NanaUI 的键盘和富文本输入能力已加入更新的审核范围。原基于560448c的补丁
`0e0225d0…`不再用于发布。并发工作已在应用中接入OverlayClosing；为保留该工作，
新候选以已可从远端获取的d0f3ccf705e59f30f05642ad761d4d4c08cab27a为基准，
保留并行工作已接入的全屏与窗口动效修复，只纳入本轮新增的框架增量。
本任务仍未提交或推送，正式Git pin尚待新的具体审核包确认。

默认target曾被另一个普通验证源码副本共用，导致可执行文件及同名crate产物被覆盖。
因此此前相关定向测试保留作诊断，末轮统一验证采用独立
`CARGO_TARGET_DIR=/tmp/lilia-native-parity-target`；框架审核包使用独立的
`/tmp/lilia-nanaui-parity/target-validation-d0`。xtask通过Cargo compiler-artifact选择实际
桌面可执行文件，并将路径和源信息写入每轮产物，不再硬编码target/debug路径。
最终完整通过状态仍以这一轮的实际报告为准。


最新真实窗口回放 `agent-debug-runs/lilia-agent-debug-1788442279768/` 已完成17个设置分区、
模型和推理选择、引用搜索，并通过正常Enter发送验证实际模型请求；Markdown图片产生
实际Scene图片绘制对象，普通点击能打开预览。该轮发现预览背景遮盖内容及Esc被吞，
所以整轮失败，不能当作完整agent-debug通过。修复调整图片在舞台背景和关闭控件之间
的绘制顺序，接入正常指针的缩放、拖动和关闭；普通业务投影保留同一图片的浏览状态，
图片身份变化才重置。后续结果以新的完整回放为准。


### 固定候选的四组真实窗口验收

最终联调候选基于 NanaUI `d0f3ccf705e59f30f05642ad761d4d4c08cab27a`，
95文件补丁SHA-256为 `07a485ba32da5d6730e6bccf6c22bda2aaac3a6924752ed8c107ae149b06918d`。
候选源位于 `/tmp/lilia-nanaui-parity/isolated-source-d0f3ccf`，
审查包、逐文件清单和独立框架检查见该目录上层的 `review.md`、`manifest.json`。
候选已包含图片固有尺寸与历史min(1,…)适配：72×40小图保持原尺寸，大图等比缩小；
实际舞台绘制、缩放与拖动限制共14项框架行为检查通过。

- 应用独占构建目录的 `cargo test --locked -p lilia-desktop --lib`：485项全部通过。
- `cargo xtask agent-debug --matrix`：四组完整回放全部通过，各25张实际窗口截图；
  窗口标识、逻辑／像素尺寸、缩放和源码信息均记录在各目录中。

| 窗口与主题 | `agent-debug-runs/` 下的证据目录 | 结果 |
| --- | --- | --- |
| 960×600 浅色 | `lilia-agent-debug-1788443531088` | passed，scale=1 |
| 960×600 深色 | `lilia-agent-debug-1788443592146` | passed，scale=1 |
| 1440×900 浅色 | `lilia-agent-debug-1788443607422` | passed，scale=1 |
| 1440×900 深色 | `lilia-agent-debug-1788443621616` | passed，scale=1 |

每组通过正常控件完成17个设置分区、模型／推理及引用进入实际HTTP请求、Enter发送、
Markdown图片打开与Esc关闭、回复分支锚点与清除、Todo创建／优先级／取消编辑／删除、
WKWebView两页导航／后退／截图进入草稿、自动化发布／等待／恢复完成／再次运行取消。
源码回放发现并修复了980px以下显式打开的侧栏仍自动隐藏的问题，现在使用框架已有
覆盖式窄窗布局。用量工具栏三个按钮的完整文字与换行也已通过实际字体测量和截图核对。
这些证据覆盖上述场景，不意味着所有领域的异常、跨窗、外部设备和历史像素都已签署。

当前联调仍使用临时Cargo path patch。`cargo xtask verify` 的boundary-check通过，
pin-check因NanaUI正式Git revision没有对应本地增量而停止；没有跳过或修改pin门禁。
正式收口需要先获得明确的NanaUI提交／推送许可，发布上述固定补丁，更新应用Git pin
并移除path patch，再运行正式verify。此前旧pin的verify通过不能替代这一步。


同一固定候选下，`cargo test --locked --workspace`通过880项，失败0；
既有 `shared_lsp_surfaces_unsaved_rust_diagnostics` 因需要rust-analyzer保持忽略。
`cargo check --locked --workspace`通过，仅保留既有launcher `home_hash` dead_code警告。
日志分别为 `/tmp/lilia-native-candidate-workspace-tests.log`、
`/tmp/lilia-native-candidate-workspace-check.log`。两条命令是pin发布前的独立代码检查，
不将它们合并标记成正式 `cargo xtask verify` 已通过。


最终候选的 `cargo xtask performance` 绝对门禁通过，报告为
`agent-debug-runs/lilia-performance-1788443797836/performance.json`：
冷启动P95 3261.80ms，输入至呈现P95 5.74ms，面板调整P95 8.28ms，
千条时间线就绪79.54ms，空闲CPU 0%，RSS 199426048字节（约190MiB）。
`historicalBaseline`仍为matching historical platform corpus unavailable，
只证明现有绝对阈值通过，不表示性能相对Tauri通过。

发布审批对象仅为上述95文件NanaUI增量，基准d0f3ccf，补丁hash07a485ba…；
拟发布分支为 `codex/tauri-native-parity`。本次尚未创建分支、提交或推送，
未创建PR；用户与并行任务的其他修改不进入该补丁。获得明确许可后再发布并接回
正式不可变Git pin。本仓库业务实现、设计记录和临时联调配置当前保持未提交。

### 发布前的业务复核与新增验证

本轮继续复核后补齐了以下实际缺口，NanaUI 95文件候选及07a485ba…哈希保持不变：

- 主窗和任务弹窗在待审批期间仅挂载待处理区域，普通输入器与多行草稿保留在原实体中；
  审批结束后恢复。长问题和计划进入最大260逻辑像素的滚动区域，底部操作保持可达，
  窄窗按钮可换行。真实布局和事件测试覆盖主窗、430px弹窗及长计划。
- 自动化正常检查器使用的选中节点方法不再仅限debug构建；连线身份使用UUID，领域图
  验证拒绝重复边ID，删除后重连不会与剩余边碰撞。启动恢复在运行互斥下重读等待任务，
  用持久Agent终态推进一次；未知或未完成任务继续等待，不重复启动Agent。
- 路线图统计／关联刷新与记忆弹窗恢复选中保留当前目标的未保存编辑；记忆作用域迁移
  同时通知原范围和新范围，涉及用户记忆时触发全局刷新。
- 四类扩展页均可搜索；只读技能／插件隐藏不可用修改操作，删除确认再次校验权限。
  编辑中的MCP ID明确显示为只读，仍由既有reducer保护身份；此前不是已证实的重复插入。
  Hooks详情显示原配置与信任／支持状态，GitHub设置显示已绑定身份及授权错误。

新增回归首次执行时，3项项目测试误用各自独立的内存领域存储，1项Agent恢复测试未写入
生产流程要求的Product会话绑定。修正为共享持久home及正常会话绑定后全部通过；没有修改
生产存储或放宽恢复条件来迁就夹具。

- `cargo test --locked --workspace`：893项通过，0失败，1项既有rust-analyzer测试忽略；
  其中桌面lib 497项通过。日志 `/tmp/lilia-native-audit-workspace-tests.log`。
- `cargo check --locked --release -p lilia-desktop`通过，日志
  `/tmp/lilia-native-release-check.log`，仅保留既有launcher警告。
- 四组`cargo xtask agent-debug --matrix`完整通过，每组27张真实窗口截图：

| 窗口与主题 | `agent-debug-runs/` 下的最新证据目录 | 结果 |
| --- | --- | --- |
| 960×600 浅色 | `lilia-agent-debug-1788444713353` | passed |
| 960×600 深色 | `lilia-agent-debug-1788444747920` | passed |
| 1440×900 浅色 | `lilia-agent-debug-1788444771587` | passed |
| 1440×900 深色 | `lilia-agent-debug-1788444787333` | passed |

此次回放在之前完整场景上增加真实模型工具审批：通过普通输入和Enter发送请求，等待期间
编辑两行草稿，模型返回`confirm_plan`后验证普通输入器未挂载；正常点击“执行计划”后，
实际HTTP请求包含相同`tool_call_id`的工具结果，原草稿完整恢复。
`pending-approval.png`、`approval-draft-restored.png`、`approval-resumed-request.json`
及`approval-completed.json`分别记录界面、请求和权威状态，不能据此替代全部审批异常场景。

上述代码和回放检查使用独占`/tmp/lilia-native-parity-target`，仍基于本地候选path patch。
正式verify的不可变Git pin收口继续等待之前提出的明确发布许可；自动继续执行不构成
提交、推送或创建分支的授权。历史macOS像素基线、真实外部账号和设备场景仍按矩阵保留。

此次补充实现后的性能绝对门禁通过，报告
`agent-debug-runs/lilia-performance-1788445226009/performance.json`：冷启动P95
3447.52ms、输入呈现P95 5.42ms、面板调整P95 8.11ms、千条时间线就绪96.52ms、
空闲CPU 0%、RSS 197869568字节（约189MiB）。历史基线仍为`not_compared`。


### 完整页面与并发复核（继续实施，尚未视觉签署）

本轮进一步修复了空态与完整扩展页的缺口，并扩充正常控件回放；NanaUI候选哈希保持
`07a485ba…`，没有提交、推送、创建分支或修改正式pin规则。

- 主窗和任务弹窗的空态仅挂载标题／建议组，时间线在有内容时恢复。依据历史源码，
  标题24/500、行高1.25、字距0.2，组中心相对正文中心上移8vh；隐藏建议仍保留24px
  高度与14px间距。实际字体与布局检查覆盖960×600、1440×900和430px弹窗。
- 空态建议由同一组件投影，主窗与弹窗保持各自草稿及加载／错误状态；按建议ID从当前
  状态获取提示词，避免旧界面覆盖新草稿。来源说明按历史GitHub／本地Git／对话顺序
  显示，标题和来源使用独立文本节点。
- Composer CAS与持久化失败现在返回失败结果；优化和review后续路由只在保存成功后
  执行。SQLite快照、revision检查和保存位于同一IMMEDIATE事务，跨连接写入也不能
  绕过revision保护。并发、回滚和失败后重试行为测试通过。
- 已有会话工作树变更与消息提交共用准入锁，变更期间阻止新turn，结束或失败后释放；
  运行／排队任务拒绝变更。合并确认携带任务身份，重复弹层已移除。
- 设置／自动化替换侧栏保留原折叠偏好；项目默认目录、工作树模式、清理和指令按修改
  即时保存。技能、插件、Hooks、MCP恢复独立列表—详情页面及编辑对话框；宽窗独立
  滚动，760px及以下按窗口宽度转为纵向。编辑取消不写权威状态，失去写权限保留草稿
  与错误反馈。对话框动画完成后释放控件，反复重开仍能操作。
- 扩展回放通过真实输入／点击执行技能创建、Hook编辑、MCP创建的取消与保存，以及
  插件安装／启停；新增分叉实际HTTP历史边界、三个review目标、105会话分页和真实
  CLI单实例handoff回放。普通草稿同时核对长度、SHA-256和revision，不输出原文。

当前工程检查：桌面lib 518项通过；workspace 917项通过、0失败、1项既有rust-analyzer
测试忽略，日志`/tmp/lilia-native-restoration-workspace-tests.log`。这些结果不替代以下
仍未完成的窗口回放和正式pin验证。

最新诊断记录：

- `lilia-agent-debug-1788450512258`启动失败：测试Hook使用旧事件名。改为权威
  `HookEvent::UserPromptSubmit`并保持禁用；插件夹具改为合法的纯文档技能包，未放宽
  生产校验。
- `lilia-agent-debug-1788450790194`成功启动但macOS锁屏，原生窗口截图失败。本轮
  无视觉证据，不能继承旧四组截图为当前源代码通过。
- `lilia-agent-debug-1788450856716`使用既有`--no-capture`诊断模式，正常控件回放在
  插件安装按钮未曝光处停止；来源输入框占满工具栏并挤出按钮，正在修复布局。该模式
  仍走可见性与正常事件分派，但不构成视觉验收。


该次无截图回放的隔离权威文件已复核：Skill registry revision1含新技能及对应SKILL.md，
取消条目和目录不存在；Hook revision1保存新command且enabled=false；MCP revision2
保存新stdio条目且enabled=false，取消条目不存在。插件尚未安装，符合按钮不可达的
失败位置。插件回放按正式安装默认禁用策略改为安装后验证禁用→启用→停用，并核对
每次UI registry revision和busy状态；未改变产品默认策略。

Release复核曾发现新扩展页的debug_nodes未隔离到debug构建，已修正cfg归属。
`cargo check --locked --release -p lilia-desktop`随后通过，日志
`/tmp/lilia-native-restoration-release-check.log`。正式`cargo xtask verify`再次运行：
boundary-check通过，pin-check因本地NanaUI patch仍报git_pin_missing_from_lockfile，
日志`/tmp/lilia-native-restoration-verify.log`。没有跳过或放宽门禁。


插件工具栏改为专用单行输入与保留内容宽的动作组，760px以下分两行。
`extensions_plugin_toolbar_keeps_install_visible_at_window_breakpoints`经真实字体塑形
验证1180/960/700及再次960窗口下完整文字与按钮边界，1项通过，日志
`/tmp/lilia-extension-toolbar-shaped-test.log`。

解除macOS锁屏后，带截图诊断`lilia-agent-debug-1788451267683`确认工具栏几何已修复，
但仍发现两项新问题，不能标记完整回放通过：插件页刷新期间丢弃可编辑路径草稿，安装
按钮维持禁用；扩展编辑对话框上方出现背景文字，需修正宿主／绘制层序。这轮截图作为
失败定位证据保留，后续须在修复后的同源代码重跑。


背景文字穿透已定位到NanaUI通用scene painter：纯2D路径先画全部几何，再画全部文字，
会把底层文字移到后来的模态背景上方。正于隔离候选中恢复绘制顺序并增加GPU遮挡回归。
因此此前07a485ba…只代表已验证的旧候选，**不再作为包含本次修复的最终发布对象**；
新补丁、逐文件清单与哈希将在框架修复冻结后重新生成并复验。未进行发布操作。


### 最新框架候选与业务回放状态

最终冻结候选：基准仍为d0f3ccf；100文件、5678新增／219删除，补丁SHA-256
`938f892ce11ea5854fa8a7817fc73aff655d29b5cd197e5422218551f0ec897c`。
旧候选移入审查包的`baseline-candidate-*`目录作为历史记录；最终候选在普通archive目录
`patch-replay-final-938f892c-v2/`从干净d0归档完整重放，100个文件逐项SHA-256一致，
无分支或worktree。NanaUI Runtime 823项、3项真实GPU像素回归及full feature check通过，
manifest记录命令／日志。32组文字与不透明图形交错保持1个color pass；安全顺序仍
4×MSAA。最终候选还包含宽度约束向子树测量传播、flex分配后auto高度重测及宽度缓存键
修正，覆盖窄Inspector输入器、fixed/max/percentage/content-box与宽窄往返布局。

应用补充通过两项正常事件测试：路径草稿在刷新期间输入、模块接受、快照更新且刷新后
安装命令使用正确路径；编辑对话框登记shell.overlays后，Shell重装配仍能输入、取消、
退出动画完成及重开。日志`/tmp/lilia-plugin-refresh-input-test.log`与
`/tmp/lilia-extension-shell-lifecycle-test.log`。

`lilia-agent-debug-1788451827157`已完整通过四类扩展正常入口与权威状态回放，
`extensions-replay.json`记录安装后禁用、启用、再次停用及各项取消／保存。
实际编辑对话框截图确认背景文字不再穿透。整轮随后在分叉场景的自动策略模型夹具响应
处失败，不标记完整agent-debug通过；自动模型路径单独保留诊断，分叉／review回放改用
正常控件显式选择模型和推理强度。其后还在复核输入提交时序，待新完整回放覆盖。


`lilia-agent-debug-1788452161688`通过正常控件完成分叉A/B/C三轮实际模型请求，
`fork-history-verified.json`确认锚点第一轮保留、第二轮排除，sessionBranch.mode=fork。
此前对空请求的初步归因已纠正：这轮失败发生在后续`/review`提交，目标截图存在但模型
输入被判为空，正在检查workflow到输入的编译。不会用分叉通过替代review验收。
另，`lilia-agent-debug-1788451964033/fork-a-request.json`证明自动策略A轮已采用
辅助模型gpt-4.1-mini并选中gpt-5.4/medium；该轮后续辅助请求传输失败仍保留诊断，
不能泛称默认辅助路由未配置，也不将连续自动策略执行标为通过。


Review根因已确认：ApplySlashWorkflow清空触发文本后保留typed workflow，提交层允许
workflow-only，但AgentMessage仍为空；wire非空校验正确拒绝。正在产品请求准备层恢复
实际审查指令编译并保留typed metadata，不能用放松wire校验或空占位文本解决。
辅助模型的偶发传输失败也定位到测试fixture：Darwin accept继承listener非阻塞标志，
body稍晚到达即EAGAIN并关闭连接。修复只恢复accepted stream的阻塞读取与既有超时，
以延迟body真实socket回归验证，不改变产品模型路由。


`lilia-agent-debug-1788452909186`完整正常控件回放通过，visualEvidence=not_collected，
使用既有`--no-capture`诊断模式。覆盖自动策略与实际模型／推理一致、#引用、Enter、
Markdown图片打开关闭、分叉A/B/C实际HTTP历史、Review三target的完整指令与typed metadata、
审批与草稿SHA-256恢复、Todo、WKWebView导航截图进入草稿、105会话六页访问、真实CLI
handoff首次草稿与再次打开不覆盖、自动化发布／等待／恢复／取消，以及四类扩展管理。
这次不签署视觉通过，四组带截图回放另行进行。

产品workflow修复位于`application/workflow.rs`及统一请求准备入口：Review/Fix/BatchApply/
TaskWorkflow编译有意义的执行指令，重复准备不叠加；Compact保留原路径。其他typed
兼容workflow的空内容在准备层明确拒绝，不用泛化prompt冒充本地设置操作。正常Goal、
Memory、Slash入口继续直连本地服务。7个真实application→wire→loopback请求测试通过，
日志`/tmp/lilia-workflow-tests.log`；工作流标题显示具体审查范围，不再回退为“附件”。
fixture延迟body回归修复前BrokenPipe、修复后通过，日志
`/tmp/lilia-fixture-delayed-body-before.log`与`/tmp/lilia-fixture-delayed-body-after.log`。


布局修复前的workspace全测924项通过、0失败、1项既有rust-analyzer测试忽略，日志
`/tmp/lilia-native-post-workflow-workspace-tests.log`。在随后纯布局修复前，四组完整窗口
操作回放全部通过，每组43张截图：960浅1788452958425、960深1788452983384、1440浅
1788453008468、1440深1788453033534（均位于agent-debug-runs/lilia-agent-debug-*）。

这些旧自动回放结果仍需截图复核，不能直接记作最终源码视觉还原通过。人工复核发现：
空态Text使用了行内布局text_align而非普通Text字形text_horizontal_alignment，字形偏左；
960扩展详情末尾按钮截断，长路径使列表行和header撑高；打开浏览面板后约305px输入器
工具栏出现裁切。空态字形对齐、扩展详情换行与长路径投影、主窗约305px和弹窗430px
输入工具栏均已修复，并由真实字体塑形与实际dock宽度测试覆盖。

### 最终本地验证与待签署边界

最终应用源码全工作区测试929项通过、0失败、1项因环境缺少`rust-analyzer`按既有规则
忽略，日志`/tmp/lilia-native-final-exact-workspace-tests.log`；`cargo check --locked
-p lilia-desktop --all-targets`通过且无警告。最终正常控件业务回放
`lilia-agent-debug-1788455600621`通过，覆盖前述完整业务链。`source.json`记录构建前后
NanaUI源码指纹均为`e1f3e056a5a2f4288c8097008d5086d6dab2df05deaf4a0b0270af59af5e56ab`
（421个输入文件），`build-artifact.json`记录桌面二进制SHA-256为
`81fcb1bd6dd4b11be77c67809b1b241df271ac27c5336c7f0c01592a0a857d44`。

最终四矩阵在`lilia-agent-debug-1788454799332`和`1788454833843`两次尝试中均由macOS
`screencapture`在首张截图拒绝，错误为“could not create image from window”；窗口选择、
源码指纹和构建产物均已成功。新增截图内容门禁会拒绝仅窗口铬、原生子视图、透明或纯色
帧，未为通过门禁而降级。性能门禁`lilia-performance-1788454897864`装载1000条语料后，
因当前桌面会话无法提交下一WGPU帧而使首个`ui-input-frame`超时；与截图失败一起作为
锁屏／Screen Recording会话环境阻塞记录，未产出或声称性能阈值通过。历史同版本、同平台、
同数据像素基线仍缺失，因此最终状态不宣称历史像素等价。

xtask现会在截图、四矩阵和性能门禁启动前读取macOS会话状态；当前状态为console仍在线、
用户图形会话锁定，三个入口均立即返回`desktop_session_locked`，避免将锁屏误报为截图权限
或把无法提交呈现帧误判为性能回归。锁屏解析与截图内容门禁2项测试通过；锁屏下
`agent-debug --no-capture`仍可完成业务回放。证据日志为
`/tmp/lilia-native-locked-matrix-preflight.log`、`/tmp/lilia-native-locked-performance-preflight.log`、
`/tmp/lilia-native-locked-screenshot-preflight.log`与`/tmp/lilia-native-locked-session-no-capture.log`。

正式`cargo xtask verify`的boundary-check通过；pin-check按设计拒绝本地path候选，错误为
`git_pin_missing_from_lockfile`，日志`/tmp/lilia-native-final-verify.log`。要完成正式门禁，
须先经用户明确批准把100文件候选发布到NanaUI远端不可变提交，更新LiliaCode git pin并
移除本地patch，再重跑verify、四矩阵与performance。当前未commit、push或创建PR。

### 解锁后的真实窗口复验

macOS解锁后，四组正常控件回放均通过：`1788488274996`、`1788488302397`、
`1788488327767`、`1788488353894`。截图内容门禁通过，空态居中与扩展详情修复得到实际
图像支持，但960宽窗口的`handoff-imported.png`暴露新缺陷：Inspector在980以下按Overlay
覆盖Primary，对话标题和输入器仍按完整宽度布局。旧窄宽测试使用默认Shrink，未复用
产品`initial_workspace`，因此不能证明该真实场景。正在统一应用Inspector策略及测试配置；
这四组证据只能签署业务回放，不签署最终视觉还原，也撤回“只剩外部签署”的旧判断。

同源性能回放`lilia-performance-1788488386873`绝对门禁通过：冷启动P95 2755.36ms、
输入到呈现P95 6.48ms、面板缩放P95 8.47ms、千条时间线55.88ms、空闲CPU 0%、
RSS 195837952字节。历史平台基线仍不可比较。Inspector策略修复后须重新核对相关窗口。

### Inspector修复后的最终本地回放

应用的`initial_workspace`已将Inspector窄宽策略统一为Shrink，移除980以下覆盖Primary
的配置。窄宽输入器测试复用产品实际workspace，覆盖960、1035、1160及再次960宽窗口，
检查标题居中、Primary与Inspector不重叠、工具栏完整可见及正常发送事件。

修复后四组带截图的正常控件回放全部通过，日志`/tmp/lilia-native-shrink-matrix.log`：

| 逻辑窗口 | 主题 | agent-debug-runs目录 |
| --- | --- | --- |
| 960×600 | 浅色 | `lilia-agent-debug-1788488590994` |
| 960×600 | 深色 | `lilia-agent-debug-1788488628733` |
| 1440×900 | 浅色 | `lilia-agent-debug-1788488655944` |
| 1440×900 | 深色 | `lilia-agent-debug-1788488683477` |

人工复核960浅／深色及1440深色的`handoff-imported.png`：标题在实际Primary内居中，
输入器和工具栏随剩余宽度换行，浏览器不再遮挡标题或发送操作。此结论针对该缺陷，
四组回放及截图内容门禁不替代全部历史状态的逐像素签署。
最终桌面二进制SHA-256为`432ca5f608b76d97dc083fb5739191d5411e3d455b04c4b129d355afca236927`；
NanaUI构建前后指纹仍为`e1f3e056a5a2f4288c8097008d5086d6dab2df05deaf4a0b0270af59af5e56ab`。

同一应用修复后的性能证据为`lilia-performance-1788488837160/performance.json`，绝对门禁
通过：冷启动P95 2504.09ms、输入到呈现P95 6.11ms、面板缩放P95 8.64ms、千条时间线
90.19ms、空闲CPU 0.1%、RSS 201539584字节。历史平台基线仍未比较。

NanaUI候选保持100文件、补丁SHA-256 `938f892ce11ea5854fa8a7817fc73aff655d29b5cd197e5422218551f0ec897c`，
没有提交或推送。正式依赖发布及pin更新后仍需重跑正式门禁；历史macOS像素基线、真实设备
远程配对和依赖外部账号的验收按矩阵保留待验状态，不宣称整个计划已完成最终验收。

### 完成度复核后的补漏

继续逐项复核发现，依赖发布并非唯一剩余工作：历史外观页的侧栏显示模式虽有存储和
调试命令，但未挂载普通设置控件。现已在应用外观页组合语言、侧栏样式及非打断／Debug
运行配置，保留NanaUI通用外观选项。正常下拉事件与模式持久化定向测试通过，日志
`/tmp/lilia-appearance-controls-test.log`；当前桌面all-targets检查无警告通过，日志
`/tmp/lilia-restoration-followup-check.log`。完整窗口回放另记，不以定向测试替代主窗侧栏变化。

新增本地用量夹具使用隔离home的权威timeline投影：两项目、三会话、七天共21条usage、
21条工具记录，涵盖输入、输出、缓存命中和缓存写入，总Token 268800。普通设置回放必须
等待quota加载完成且记录数至少21后再截图；不再用全零图证明非空绘制，也不把本地图表
验收笼统归为需要Provider凭据。

计划审批回放增加“要求修改”：通过普通备注输入和按钮提交两行反馈，以独立tool_call_id
核对HTTP工具结果并检查原普通草稿SHA-256。该新增回放需运行结果才能签署。自动化事件
触发与同home重启、扩展实际执行／连接、路线图／记忆／架构的跨窗回放和图表悬停仍按
独立场景继续验证，之前四矩阵通过不覆盖这些缺口。

上述补漏的四矩阵已通过：960浅`1788489610128`、960深`1788489659337`、1440浅
`1788489688074`、1440深`1788489716146`（均为`agent-debug-runs/lilia-agent-debug-*`）。
日志`/tmp/lilia-restoration-followup-matrix.log`。每组`appearance-sidebar-modes.json`证明
普通下拉切换后实际侧栏层级改变，保存值分别为unified、grouped；`settings-quota-state.json`
记录21条及268800 Token，明暗截图实际显示四类堆叠柱和总量曲线；
`plan-revise-verified.json`证明两行feedback逐字一致、同tool_call_id及普通草稿恢复。
这些新增证据不扩展为图表悬停、所有远程操作或自动事件／重启场景已通过。

远程本地操作回放`lilia-agent-debug-1788490110794`通过，证据`remote-local-verified.json`：
普通输入保存电脑名；生成二维码后权威有效ticket数为1，取消后为0；取消设备撤销保留
trusted，确认撤销后trusted=false且revoked_at存在，刷新仍无可信设备。只读SQL仅访问
该回放隔离home的Product数据库。真实Android首次配对／重连仍未验证。

自动化事件与重启回放`lilia-agent-debug-1788490758183`通过，日志
`/tmp/lilia-automation-restart-replay-3.log`。通过真实画布点击选择Trigger、检查器选择
task_changed、限定项目与task_created、发布启用，然后离开自动化页创建任务。
不匹配收件箱任务的创建事件已被Product游标处理，未生成该工作流运行；匹配项目仅生成
一个waiting_user运行。首次同home重启后完整run/node事实保持一致；普通输入及继续操作
使同一run成功，第二次重启仍保持完成事实且没有重复运行。

证据为`automation-event-unmatched.json`、`automation-event-waiting.json`、
`automation-event-restart-verified.json`及两个`restart-*.json`。两次均确认旧进程已退出、
新PID不同，使用相同二进制SHA-256
`fff21493ed5dc841db361512955577d2b6835500d48959dedea9330eca908aa7`；重启只恢复隔离model
凭据，不重放业务夹具。此为Human工作流恢复／去重证据，不等价于外部工具副作用幂等。

回放同时补齐真实返回项目按钮、项目悬停工具按钮和画布节点几何的调试曝光；`ui-hover`
经正常挂载、禁用、视口与命中校验派发指针移动，不直接变更应用hover状态。前两次失败
分别记录在`1788490309053`（回放误用已卸载导航）和`1788490509463`（未发送悬停事件），
均保留现场；没有通过业务helper绕过正常页面切换或悬停。

### 图表与窄画布的进一步视觉缺陷

`1788491102077/quota-hover-verified.json`证明四类图表的普通指针输入、真实Tooltip文字和
移出清除，但人工复核`quota-hover-projects.png`发现Tooltip在滚动后移到窗口顶部并截断，
圆环有密集放射状背景缝。前者是Fixed节点虽按viewport布局却仍继承祖先scroll的绘制／
命中偏移，后者是圆弧离散Butt线段未填补连接处。通用修复属于NanaUI；在修复并重新采集
图像前不签署图表视觉通过，旧938f候选将保留为修复前快照。

同组随后在960新建自动化时失败：初始化视图硬编码按900×560拟合，而实际画布更窄，
导致初始Trigger不可见。应用已改为复用实际canvas尺寸的适配方法；尚待修复后回放。
再次发现路线图／记忆仅有旧debug打开动作，普通项目菜单入口正在补齐，因此此前业务
接线与存储测试不构成这两页的正常入口验收。

### Fixed／圆图修复后的复核

当前NanaUI候选为103文件，SHA-256 `3f4a11c313058f158815daecd63c46d5d882575ba1022ad39f7254db0ca3182a`。普通干净归档重放 `/tmp/lilia-nanaui-parity/patch-replay-3f4a11c3/` 与manifest逐文件哈希一致；未提交或发布。Fixed使用视口绘制与命中、保留透明度组；Donut连续三角环带消除径向缝隙，保留扇区间隔和悬停位移。824项runtime、183项painter、定向Scene、8组Donut GPU及full编译通过。旧重绘测试改为验证实际像素覆盖和下一帧缓存复用，不固定内部MSAA路径。

四组业务回放 `1788492081693`、`1788492173101`、`1788492217673`、`1788492262290` 全部通过，每组57张截图；日志 `/tmp/lilia-fixed-knowledge-matrix.log`。已通过真实项目菜单进入路线图／记忆，里程碑多行说明保存、关联／取消／重新关联、记忆三行正文与标签保存、条目启停和冷却轮数保存，同home重启后完整权威记录一致。图表悬停截图已人工核对：Tooltip靠近图表，环带无旧裂缝。实际窄画布初始Trigger可见且操作通过。

上述回放人工复核另发现验收脚本重启后沿用了此前窗口操作产生的尺寸，因此重启后的路线图／记忆截图不能签署指定窗口矩阵。Session重启现从本组requestedViewport恢复验收尺寸及主题并记录到restart证据；只调整隔离home窗口配置，不重放业务夹具。修正后的矩阵另行采集，不将旧图冒充指定尺寸。记忆实际注入／冷却执行、完成任务统计、跨窗口更新和外部真实设备仍需各自证据。

修正重启尺寸与页面重建后的正常指针移入后，四矩阵全部通过：960浅`1788492447215`、960深`1788492502042`、1440浅`1788492556653`、1440深`1788492601477`。日志 `/tmp/lilia-fixed-viewport-matrix-2.log`，每组57张PNG。路线图／记忆的window.json尺寸逐项与requestedViewport一致，Nana源码构建前后指纹一致。正式verify本次仍仅boundary-check通过，pin-check报`git_pin_missing_from_lockfile`，见 `/tmp/lilia-current-verify.log`；原因是当前候选尚未正式发布，未绕过门禁。

新一轮闭环复核定位两处业务缺口：记忆注入开关／冷却只保存未进入Agent请求；任务Done只接旧调试动作，普通菜单缺少完成／重新打开。这两处正在实现，上述矩阵不包括尚未实现的能力，不能据此签署整体等价。

上述四组应用二进制并非全部相同：前三组为`7baca74e2d774298534ee37bd15b5b94d4e3ae07dd94449653c4bbd4283dd53c`，最后一组为`b812af387aa754b54bb765b531995992bc64299d255e16806a1adb2228fbf458`（并行实现期间重新构建）；因此属于逐组业务与指定尺寸证据，最终冻结版本仍需统一回放。Nana候选在四组中保持相同。

### 记忆请求接线与任务完成入口

新增跨边界`MemoryTurnInjection`契约，按任务与turn持久化注入决策（包含不注入结果）。全局／baseline／任务开关、条目启停、用户及当前项目范围、冷却控制进入真实Agent context；正文仅存在于typed memoryInjection一次，工作树additionalContext保持原义。同turn复用决策，不二次推进冷却；新turn在事务内取观察序号与已保存最高序号+1的较大值，防止compact后绑定变化使序号回退。

三项存储回归通过（`/tmp/lilia-memory-injection-store-tests.log`）；真实loopback四轮请求验证1注入、2冷却、3再注入、4关闭，并断言每次正文出现0或1次、重试checkpoint不变（`/tmp/lilia-memory-wire-test.log`）。普通UI连续发送回放正在执行，以上不是普通入口验收的替代。

任务菜单新增标为完成／重新打开，按菜单目标task更新，不强制切换当前任务；运行、等待审批或工作树忙时拒绝。忙状态行为测试通过，路线图投影测试仍在修正并重验，真实菜单0/1→1/1→0/1已加入下一轮回放。

`lilia-agent-debug-1788493260459`已通过普通入口闭环：`knowledge-roadmap-completion.json`记录真实投影1/1与0/1及权威任务状态；`knowledge-model-injection.json`记录五轮普通输入发送的实际HTTP context，sequence1/4含唯一记忆、2/3冷却不含、5由正常UI关闭后不含。`knowledge-restart-verified.json`的modelInjectionVerified现为true。回放对默认折叠的任务先点项目更多并正常滚动到目标，不直接调用状态或发送业务helper。其他矩阵仍在执行。

任务完成相关5项测试通过（含busy限制、当前／非当前选择保持），日志`/tmp/lilia-task-completion-tests.log`；Memory完整20项测试通过，日志`/tmp/lilia-memory-full-tests.log`。

后续第二组`1788493362090`在正常里程碑保存返回`target_not_interactive`后停止，未宣称四矩阵通过。正在补失败时的UI/业务快照和真实窗口截屏，先保留现场再定位，不自动重试掩盖交互问题。

历史`2eec58e`的popup路由只有对话，没有路线图／记忆页面；跨窗验收不新增历史不存在的页面，而验证主窗修改记忆后已打开任务弹窗下一轮请求使用新权威内容。当前调试UI命令仅路由主窗口，旧弹窗debug helper不能作证；正在补实际popup retained文档的正常输入路由。

弹窗retained adapter定向测试通过（`/tmp/lilia-popup-adapter-test.log`），真实回放`1788493830872`在记忆任务选择菜单停止：展开后多数条目位于视口上方，只有最后项y=0可见，目标task不可交互。现场`ui-failure.png/json`，通用ActionMenu定位正在核查。没有使用旧popup debug helper或直接设置注入目标绕过。Hook/MCP回放已编译通过，改在该弹窗场景前执行以独立收集证据。

当前桌面lib完整531项测试通过，日志`/tmp/lilia-current-desktop-tests.log`。页面切换后回放用于移出项目行的hover目标改为固定的新对话入口，避免选择正在重新布局的项目滚动区；随后仍移入实际项目／任务行并打开普通菜单，不重复业务动作。

`1788494105935`已完成Hook真实启用执行与停用不执行断言，隔离home的`native-hook-executions.jsonl`仅1条，具有真实task/turn/context。随后MCP管理页连接并发现1个工具，但最新实际模型请求仅含3个内建交互工具，没有MCP；回放以`model_request_missing`停止，现场`request-failure.json/png`及`extensions-mcp-runtime-ready.png`。因此此前“正常保存即时reconcile”不等于Agent实际能调用，正在定位runtime工具目录到模型请求接线。

MCP缺失已定位为两处：无workspace时的工具过滤误删共享MCP descriptor；Host缓存键未包含实际工具目录，连接／停用后继续复用旧工具快照。修复保留ToolAccess／审批，只允许已被共享MCP服务公布的工具进入无folder请求，目录快照变化使Host重建。尚待实现后真实调用验收，不预先签署。

已逐项核对`1788493260459`的57份window元数据，全部为960×600，而非只抽查路线图／记忆两张；这一组的几何证据范围明确，仍不代表历史像素等价。

ActionMenu修复采用框架内部独立可滚surface：trigger保持布局，surface以trigger屏幕几何及真实viewport定位。应用仅在原sync_action_menu_items的raw reconcile处消费框架content-root接口；不会把整个用户树偷偷重排，也不在应用端补偿坐标。正在补边缘／滚动／关闭回归，实时Nana源码此时已不同于3f4a11c3，需重新冻结候选。

MCP定向真实请求测试已通过（`/tmp/lilia-live-mcp-host-test.log`）：无folder连接前无工具，连接后工具进入模型目录，标准ApprovalRequest批准前调用数0，批准后共享MCP执行一次且结果进入下一模型请求，断开后工具移除。固定Mutsuki版本的MCP工具注解camelCase解析有兼容限制，当前保守触发审批；回放遵循实际审批，不降低权限来凑通过。真实UI HTTP场景已加入普通审批按钮，待重跑。

Popover完整runtime回归826通过、1失败：关闭后is_overlay_reachable=false但hit_test仍返回原item，已证明隐藏浮层残留命中，正在修根因；不会删除该真实交互断言。新菜单候选此时仍未冻结签署。

共享Agent完整测试70通过，1项因需要rust-analyzer按既有条件忽略（`/tmp/lilia-current-agent-tests.log`）。新增MCP取消通道仍需单独行为证据。单行标题／标签／冷却字段改用TextInput，正文和说明保持TextArea；此前TextArea默认最小高度使height40的单行字段实际撑到约96，语义与历史input不符，调整后待真实截图核对。

菜单候选现冻结为108文件`c4c2ce1de853eec48ad827d8b03a48b588ca9f7eaabccd0a9565f3e8c4a87806`，干净重放`/tmp/lilia-nanaui-parity/patch-replay-c4c2ce1d/`逐文件一致；828项runtime与full Nana编译通过（`popover-runtime-full.log`、`popover-full-check.log`）。display-hidden祖先现在裁掉完整命中分支，同时保留visibility-hidden允许子级显式visible语义；闭开闭和Fixed完整／增量命中测试通过。未提交或发布；实际应用菜单及性能仍待新候选验证。


### 最终候选编译与取消复核

`/tmp/lilia-final-desktop-check.log`记录当前桌面all-targets编译通过。已审批且等待中的共享MCP取消测试通过（`/tmp/lilia-mcp-cancel-test.log`），验证正常Stop及时取消transport、权威turn为cancelled且没有成功工具结果。独立复查另发现runner开始与取消句柄登记之间的竞态，正在补按执行轮次的取消锁存，前述通过结果不覆盖该窗口。

`/tmp/lilia-final-workspace-tests.log`中Agent为71通过、1项既有rust-analyzer测试忽略；Desktop为530通过、1项旧菜单布局断言失败。旧断言要求整个ActionMenu根节点Fixed，修正为实际960×600布局下每个菜单项在视口内且打开前后工具栏几何相同，保留当前选项高亮检查；定向测试通过（`/tmp/lilia-permission-menu-test.log`）。这不是全workspace通过声明，需在最新取消修复冻结后重跑。

最终`cargo xtask agent-debug --matrix`在任何场景启动前返回`desktop_session_locked`（`/tmp/lilia-final-parity-matrix.log`）。当前macOS桌面锁定，无法采集有效实窗或帧性能证据；未绕过锁屏检查，旧截图不充当本候选验收。正式`verify`再次boundary通过、pin-check报`git_pin_missing_from_lockfile`（`/tmp/lilia-final-verify.log`），本地Nana候选尚未发布，未提交或更新远端。


取消竞态修复现已通过最终工作区测试（`/tmp/lilia-final-workspace-tests-2.log`，退出0）：942通过、0失败、1项既有rust-analyzer测试忽略。Agent72通过、Desktop531通过。`cancelling_before_mcp_registration_prevents_dispatch_and_preserves_next_turn`用正常模型／审批和确定性登记屏障验证Stop先到时不发tools/call，同session下一turn仍实际执行；等待态取消与动态目录执行测试同时通过。取消锁存和active登记使用同一Mutex，key为session＋turn，不清除旧turn来启动新turn。最新普通UI和性能仍被锁屏阻塞，未据此宣称整体计划完成。


取消记录生命周期进一步修正为活跃turn准入表与RAII清理，缺省拒绝；每次执行携带generation，同turn审批恢复不能使旧runner借用新准入。清理可在正常取消后先发生，回归不再假定取消记录必然尚在。子Agent通过原child Host／共享MCP服务进入相同准入机制，父停止先取消child MCP再取消执行；子调用登记时校验父generation，防止父取消扫描之后迟到登记。

`/tmp/lilia-final-agent-lifecycle.log`：完整Agent74通过、1项既有忽略。真实委派只读MCP成功后结果进入child权威事件；第二轮等待中父Stop取消实际transport且无成功child工具结果。删除准入、同turn新generation、迟到旧runner拒绝、下一turn正常执行均通过。上述修复没有新增Agent宿主或独立MCP连接。最终workspace重新验证中；先前942结果属于前一冻结版本。

最终冻结版本workspace测试退出0：944通过、0失败、1项既有忽略，日志`/tmp/lilia-final-workspace-tests-4.log`。本结果替代此前942项测试作为最新源码验证；不替代普通UI／性能及正式pin门禁。

同冻结版本`cargo check --locked --workspace --all-targets`通过，日志`/tmp/lilia-final-workspace-check-2.log`；最终`git diff --check`通过。


### 解锁后的实窗复核

桌面解锁后两次真实矩阵`1788496848850`、`1788497176742`均在记忆页父滚动区不可交互处停止，日志分别`/tmp/lilia-unlocked-final-matrix.log`与`/tmp/lilia-scroll-diagnostic-matrix.log`。只读诊断证明父viewport为(236,36,724,564)，五个坐标转换正常但命中均为空；子控件仍可命中。框架`workspace.rs::project_region`覆盖消费ScrollView的InteractionState为pointer_events=false，真实空白处Wheel也会被scroll_at拒绝，因此属于产品缺陷，正在修框架，不通过debug业务helper或选择巧合位置绕过。

实窗`settings-project.png`另证设置当前项目名称仍走独立TextArea，已改为TextInput。通用应用表单改为基于TextInput默认layout仅覆盖高度，保留其边框、圆角和内边距，避免用空Stack布局清除控件样式；还需修复后截图。历史MemoryView具有输入边框，此处不签署1px几何一致。

技能范围核查确认历史`2eec58e`同样没有自动向模型注入技能目录／正文；不作为等价还原新增模型工具。已修现有管理投影对registered技能忽略catalog.available的问题；真实manifest缺required_tools时不可用且load拒绝，修正manifest并reload后恢复正文加载，定向回归通过（`/tmp/lilia-skill-availability-test-2.log`）。此测试显式给共享SkillRegistry配置ToolRegistry验证器，用于覆盖合法unavailable读模型，不宣称默认运行时已启用依赖验证。


Workspace修复冻结为Nana候选`e68fb0c5567ab63b80e09f028644937565fbe66c6b1b9a4e384a22634dc64333`，109文件；`/tmp/lilia-nanaui-parity/patch-replay-e68fb0c5/`干净重放逐文件哈希一致。project_region只更新自有region样式，不覆盖消费者interaction/accessibility；真实空白滚轮和焦点回归通过。全runtime all-features832项通过（`workspace-runtime-full-2.log`），旧测试改为验证消费者完整无障碍状态保持，不恢复技术区域名覆盖。

实窗`1788497570240`已经通过修复后的记忆编辑、正常滚动、五轮注入及同home重启恢复。设置项目名称截图现在为40px单行框，Memory标题／标签保留输入边框和内边距。随后Hook执行和MCP普通审批后实际调用／结果返回断言完成，但停用MCP返回任务时旧审批卡仍存在，普通输入器不可达，回放失败`ui_target_not_visible`；现场`ui-failure.json/png`与`extensions-mcp-approval.png`。这是待修的业务／投影同步问题，不宣称扩展完整链路或本组矩阵通过。该组运行期间Nana仅补旧测试断言，从28707cc3变为e68fb0c5，生产实现相同，但不是最终冻结矩阵签署。


审批残留根因修复在`checkpoint_from_events`：权限审批只在匹配turn/call的ToolCallCompleted后移除，completed/cancelled/failed终态不重新发布开放pending，其他等待审批不受影响。Agent完整75项通过、1项既有忽略（`/tmp/lilia-approval-checkpoint-tests.log`）。后续真实矩阵`1788498100156`在路线图截图时系统`screencapture`返回空图而停止，桌面当时解锁，不能据此断言缺权限；日志`/tmp/lilia-checkpoint-fixed-matrix.log`。截图路径现先持久化窗口ID/bounds/CGPreflightScreenCaptureAccess，失败再记录窗口状态；未重试业务动作或把缺图计为通过。

完整源码复核另外确认Memory界面仍不等价：历史`MemoryView.vue`宽屏列表+320..380编辑器双列、<=920单列且列表在前，当前旧实现是全部控件先纵排、列表最后；历史紧凑toolbar、固定作用域空组、逐项动作和表单enabled草稿也未完整恢复。这是实施缺口，不是单纯缺截图。正在恢复历史结构和draft.enabled，Native会话注入配置保留为独立次级区。回放将编辑enabled后正常保存，冷却数字Enter提交，不保留历史不存在的可见冷却保存按钮。此前CRUD/模型注入结果不能证明这些源码布局要求完成。


### 记忆页结构恢复与完整业务矩阵

已实现历史Memory列表／编辑器双列、920窗口断点、固定作用域空组、逐项编辑启停删除、刷新和数字冷却确认；编辑器启停作为draft随保存提交。Native会话注入独立保留。独立review发现并修复外窗更新后卡片启停推进过期草稿revision的问题，旧revision保留以确保Save冲突，不覆盖外部正文。重复旧启停实现、未用ShellIntent和测试专用DraftEnabled消息已清理。

初版实窗`1788499089644`暴露纵向grow/shrink把grid压短，注入区域覆盖右编辑器。修正Memory横排容器为内容高度、不参与纵向压缩；新增真实字体布局验证880/960/1440、空/12条列表、两列最大高度、tags→enabled→actions顺序及注入区域边界。记忆相关12项测试通过（`/tmp/lilia-memory-layout-tests-3.log`），过期草稿并发测试单独通过（`/tmp/lilia-memory-concurrency-test.log`）。

修复后四组完整普通UI矩阵全部通过（`/tmp/lilia-memory-height-fixed-matrix.log`）：960浅`1788499388012`、960深`1788499455535`、1440浅`1788499506103`、1440深`1788499557946`。四组同binary SHA256 `ffccc2720b501e62d8192f13b155e23c0281504fbd7e45e927e1acac4eacfe02`，每组59张PNG，全部window元数据与该组requestedViewport一致。`extensions-runtime-verified.json`包括Hook启停实际执行、MCP普通审批后真实调用一次及结果回送、停用后目录/调用消失；`memory-popup-sync-verified.json`包括正常弹窗输入发送，主窗保存新正文／停用后的真实请求同步。旧审批复活问题本轮已完成普通入口验证。

但人工图片复核仍发现Memory工具栏两个Shrink Switch没有绘制标签/轨道，仅padding可点。框架缺Checkbox/Switch固有标签测量，geometry控制宽度被压成0；所以四组业务通过不是完整视觉通过。正在修框架固有测量，并将历史checkbox使用现有Checkbox还原，不设置猜测宽度。字体/字段间距、标题maxlength、标签placeholder、正文原生调整高度等仍有明确待还原细节；没有同条件历史macOS像素基线，不签署像素等价。

## 记忆页控件源码复核（继续实施）

历史 MemoryView 的全局、基线及表单启用均为 checkbox；当前已改用 NanaUI Checkbox，继续经 ToggleChanged 分派，保留既有 `lilia.ui.switch.memory-*` 操作标识。Native 的任务注入设置仍使用 Switch。输入框恢复 12px/400、上下8/左右9内边距、6px圆角，标签占位提示为“逗号分隔”；正文按8行×18加内边距/边框组合初始高度。

`/tmp/lilia-memory-controls-tests.log`：12项应用 memory 回归通过，包括正常控件、草稿启停、并发编辑冲突和防重叠几何。独立审查确认 Memory→其他项目页面会重置共享根布局，没有内边距污染。

NanaUI 控件字体测量第一轮真实字体测试通过，但进一步审查发现 Switch 提示仍按固定18/20位置绘制，以及动画不应触发文字布局。修订版本的首次编译失败，记录于 `/tmp/lilia-nanaui-parity/toggle-intrinsic-tests-2.log` 和 `/tmp/lilia-memory-checkbox-matrix.log`；此轮矩阵未进入窗口场景，不能声明新版本通过。正在修正类型与 mutation 阶段归属后重新验证。

后续明确缺口：标题 maxlength120 的通用输入约束（含IME/粘贴/撤销语义）、正文纵向拖动能力；scope radio 可复用已有 SegmentedControl radio_group，正在接回。框架受限宽多行 Checkbox 未单独验收。以上均不计源码还原完成。


## 控件冻结候选 488c6358

最终 Switch 测量链补齐两处遗漏：正常调度不再过滤拥有自身文字的 Switch；显式 `size()` 投影字号，使测量与绘制继续统一使用解析字体。提示几何使用真实两段行高，thumb 动画仅重绘。832项 runtime 与4项真实字体回归通过，完整日志见候选 review。应用13项 memory 回归含正常radio点击、方向键、互斥和焦点通过。

候选：`/tmp/lilia-nanaui-parity/nanaui-restoration.patch`，SHA256 `488c635874e38e8d7e916e9101ff88311c4757a2eeff7074292b8a1c3ce9e971`，115文件；`patch-replay-488c6358` 与 manifest 每项哈希相符。

两次窗口重验未签署：`/tmp/lilia-memory-checkbox-matrix-2.log` 在构建期间检测到测试文件格式变化，源指纹门禁正确拒绝；`/tmp/lilia-memory-checkbox-matrix-3.log` 在远程配对截图调用失败，`1788500854030/remote-pairing.window.json` 证实权限true且前后窗口17377/PID34793/960×600相同。现采集器仅在同PID/窗口/尺寸及授权仍有效时对读取最多重试3次（200ms），逐次保存captureAttempts；业务动作不重放、内容验证不放宽。xtask检查通过，下一轮矩阵待验。


## 488c6358 工程结果及锁屏边界

`/tmp/lilia-controls-workspace-tests.log`：workspace 953通过、1忽略、0失败；`/tmp/lilia-controls-workspace-check.log`：all-targets检查通过。`/tmp/lilia-controls-verify.log`：boundary通过，正式verify仍因NanaUI Git pin未进入lock失败，未绕过。

`/tmp/lilia-memory-checkbox-matrix-4.log`：启动前检测macOS会话锁定，返回desktop_session_locked，未采集新窗口证据。本候选性能也未执行。已请求解锁；工作继续补标题长度约束及Memory表单标签组合。上一版四组业务矩阵通过仍仅对应其原binary，不归属此候选。


## 标题约束、正文拖动与表单源码细节

标题 `maxlength=120` 已通过 NanaUI TextInput 通用属性接回，预算按 UTF-16 code unit 计数，不拆分 emoji；普通键入、粘贴、选区替换和 IME commit 走正常输入分派，preedit 不截断。程序加载超长值保持，后续允许缩短、拒绝增长。相同文本替换仍完成选区收拢。该批未实现 TextInput 原本缺失的 Undo/Redo 历史栈，不将撤销计为等价完成。

冻结候选 `2b215d1062924acbb441abc5a0fcbfdc0b85d18bf202f930d35a494b29acdc05`（118文件）干净重放哈希一致；`maxlength-tests.log` 5项正常 adapter/clipboard/IME 测试和 `maxlength-runtime.log` 832项 runtime 通过。此为 resize 前候选，当前源码已继续推进。

Memory 字段改为历史独立标签加6px间距，保留原输入实体和事件分派；标签与空组文案消费既有 task-statuses 产品契约。作用域 radio 恢复12px字号和8px组间距，正文初始高度162、最小148、纵向拖动接通。浏览器 UA 默认尺寸仍无同平台历史构建，不推断像素一致。

新增 `ui-drag` 调试命令只接受真实挂载、可见、启用且实际命中的起点，经过正常 Pointer Down/Move/Up；失败发送 Cancel。正文 grip 的调试观察来自实际 ComponentGeometry 和窗口变换，不使用固定坐标或业务 resize helper。知识场景新增正常滚动、拖动48px、权威正文未改变及截图取证；真实窗口场景尚未执行。

`/tmp/lilia-memory-resize-tests-final.log`：15项记忆应用回归通过，覆盖真实布局下拖动增高64px、非控件起点拒绝、无正文编辑事件、权威正文刷新后保留用户高度和标题正常粘贴限制。此结果在最后 radio 字号/间距及 ContentBox 支持边界修订前，统一冻结后重验。

独立复查发现 ContentBox 百分比/em padding 不能以近似边框盒反推内容高，且被 min/max 夹紧的声明高度会产生拖动死区。本次新增 API 将 ContentBox 支持限定为无高度约束的声明 Px 高度；默认 BorderBox 支持 min/max，Memory 使用此路径。readonly 允许调整大小但禁止文字编辑，disabled 拒绝；取消、缺失 Cancel 后新 Down、移除和外部文本同步分别验证。最终候选与门禁结果继续记录，不将有限 ContentBox 支持称为通用 CSS resize 完整等价。


## a241bb2c 实窗拖动验收与新发现

候选 SHA256 `a241bb2c02c62f726bcc90b366adbd7e4edc34ed8a4cbca7a5e62980ff4040f2`，121文件，`patch-replay-a241bb2c`逐文件哈希一致。最终冻结框架832项runtime和3项adapter全部通过（`/tmp/lilia-textarea-resize-runtime-final.log`、`/tmp/lilia-textarea-resize-adapter-final.log`），独立复查确认受限ContentBox不显示grip，ui-drag经真实命中和普通指针分派。

macOS解锁后四组矩阵通过（`/tmp/lilia-memory-resize-matrix.log`）：960浅 `1788503295714`、960深 `1788503393716`、1440浅 `1788503447322`、1440深 `1788503499450`。每组60PNG，同binary `7ac5e22d2bf624f8c7ee44e62b251452175c132d4485cf03b2bf5d1a6dc0211f`；全部窗口元数据符合requestedViewport。每组memory-resize-verified证实正文162→210逻辑px，权威记录不变；原有扩展、注入、弹窗同步与恢复链路全部完成。

人工复核knowledge-memory及resized图片：checkbox、radio、字段标签与拖柄实际可见，但拖动后主区原背景消失，卡片层次降低，明暗主题均复现。正在核查Workspace借用ScrollView节点后两侧投影是否覆盖样式；此为明确视觉缺口，240张业务截图不能替代视觉签署。最终性能与workspace门禁待修后统一执行。


## ff3a15d8 内部视觉投影修复及滚动回归

候选 `ff3a15d88e75d790afafcf777787c86c84c1821e4dabffe0b49f15cc9c189f1b`，122文件，干净重放一致。ScrollView内部hover/drag仅更新滚动条视觉，旧实现真实PointerMove后背景/圆角丢失的断言失败（`/tmp/lilia-scroll-surface-before.log`），修复后回归通过（fixed.log）；832 runtime及15项应用Memory通过，独立review未发现该窄范围阻断。

但窗口矩阵 `1788504029396` 停在路线图保存按钮不可达（`/tmp/lilia-scroll-surface-matrix.log`，ui_target_not_visible），尚未进入Memory拖动。白底保持同时暴露region_style把借用ScrollView的Scroll overflow覆盖为Hidden；之前hover完整重投影意外恢复滚动，掩盖此问题。继续修复Workspace与scrollport的overflow归属，并要求正常layout产生滚动范围、普通Wheel到达底部。此候选不计整体回放通过，性能与正式门禁仍待最终冻结。


## 滚动归因更正与诊断矩阵

进一步取证否定了“Hidden使L2普通Wheel拒绝”的初步归因：L2按ScrollView组件axes滚动；正确负方向Wheel在原配置也能跨Workspace刷新滚动（`/tmp/lilia-workspace-wheel-before-correct-sign.log`）。未合入推测性的overflow覆盖修复。自然内容度量也未发现依赖Scroll/Hidden的特殊分支；不以错误方向在顶端不动的测试当产品失败证据。

候选 `3866b7f1125f80a9a5a6d668e798d754a9c3f0a29b4effee8c8dacf9af947280`（122文件）生产与ff3一致，仅补正确Wheel测试；干净重放通过。应用ui-observe新增ScrollView只读offset、metrics、overflow和实际事件坐标/命中；回放失败时保留逐次scroll轨迹，不改操作行为。

诊断矩阵四组通过（`/tmp/lilia-scroll-surface-diagnostic-matrix.log`）：960浅 `1788504596696`、960深 `1788504670743`、1440浅 `1788504741182`、1440深 `1788504802331`，各60PNG且窗口尺寸全符。记忆正文拖动后主区背景、圆角和卡片层次真实保留；Hidden配置下contentHeight735.8→783.8、viewport564，排除简单无滚动范围假设。首个ff3路线图保存不可达仅记录为一次未复现失败，未声称已找到并修复其独立根因。

## 动态库产物证据与工程阻塞

`/tmp/lilia-restoration-workspace-final.log`在链接desktop lib test时因磁盘空间不足（errno28）失败，不能计完整workspace通过。已清除本任务旧NanaUI target的可再生成依赖缓存，保留源码、补丁、归档、日志、截图与应用产物，等待重跑；没有删除其他任务或用户目录。

重要证据边界：前述`executableSha256=7ac5e22d...`只标识liliacode启动器。应用通过同目录libliliacode_host动态库承载实际业务/渲染；不同修复版本可拥有相同启动器哈希。因此前文“同binary”的证据不等于完整桌面运行产物相同，不能将其当动态库身份签署。正在扩展Cargo产物记录，校验实际同目录host路径及SHA256，并在启动、重启和成功签署前共同验证launcher/host。此前窗口功能及人工图片观察仍为有效现场记录，但完整运行产物身份需新回放证据。


## 完整运行产物身份与当前冻结批次结果

xtask 已记录 Cargo 同 package `liliacode_host` cdylib 的实际 launcher 相邻路径、SHA256和原Cargo artifact；拒绝rlib、其他package/目录及旧launcher-only证据。启动前、重启前后及agent-debug/performance成功签署前复核两文件。3项真实临时文件测试覆盖host变化/缺失而launcher不变、错误产物选择及launcher变化；独立review未发现误签阻断。

最新四矩阵（`/tmp/lilia-restoration-runtime-identity-matrix.log`）全部通过：960浅 `1788505391207`、960深 `1788505445404`、1440浅 `1788505500811`、1440深 `1788505556777`。每组60PNG，全部window尺寸匹配requestedViewport；每组 `runtime-artifact-verified.json` 完成最终校验。完整运行产物对一致：launcher `7ac5e22d2bf624f8c7ee44e62b251452175c132d4485cf03b2bf5d1a6dc0211f`，host `940c0f7bb8ce44fb290628de3888aaa786554b00b9c2a2b880493bfb59650d9f`。各组仍经过实际入口、权威状态、重启恢复与Memory拖动；人工图检确认主区白底/圆角和卡片层次保持。

性能 `agent-debug-runs/lilia-performance-1788505314562/performance.json` 绝对阈值通过，使用同launcher/host并验证所有cold样本和最后产物身份：cold P95 2814.26ms；composer帧P95 5.47ms；panel resize帧P95 9.70ms；1000条时间线75.66ms；idle CPU0%、RSS197836800bytes。dev profile、5次cold与30次帧样本，不把启动首帧当稳态帧；历史同条件平台/语料缺失，未比较历史基线。

磁盘清理后workspace（排除正在修改的xtask）938通过、1项既有忽略，xtask20通过；日志 `/tmp/lilia-restoration-workspace-retry.log`、`/tmp/lilia-restoration-xtask-tests.log`。两部分all-targets check通过。完整统一 `cargo test --workspace --locked` 已再次通过：958项通过、0失败、1项既有忽略，日志 `/tmp/lilia-restoration-workspace-unified.log`，覆盖全workspace依赖特性组合。`/tmp/lilia-restoration-verify.log` boundary通过，正式pin失败 `git_pin_missing_from_lockfile`；未绕过、未提交、未推送。

当前批次未补完的已知源码细节：普通TextInput/TextArea撤销/重做历史；Memory新增/清空/保存按钮的历史前置图标。特殊ContentBox resize只支持无min/max的Px初始高度，默认BorderBox包括Memory支持min/max；外部显式ScrollView样式更新与Workspace借节点的既有归属边界不称彻底解决。首次ff3路线图Save不可达仍作为一次未定位失败保留，后续诊断及完整身份共八组未复现。历史像素和真实外部平台/凭据证据仍按先前边界待补。


## 当前增量：文本历史与记忆按钮图标

计划先补 NanaUI 普通 TextInput/TextArea 事务历史及 Button 前置 Icon，再按产品文档身份接线，最后做正常事件测试、独立review及运行验收。历史明确不跨任务、记忆条目、里程碑、文件、供应商凭据或自动化节点；同一草稿的权威回显、保存及尺寸更新保留历史。旧3866批次不为本增量背书。

Memory 新增、清空采用14px Plus，保存采用15px Save，间距6px。路径来自历史2eec58e yarn.lock的 `@lucide/vue` 1.30.0，保存在 `apps/desktop/assets/icons/lucide/`，含原ISC许可证与来源。按钮继续使用同一 Native Button/Activate/禁用链路，不叠加第二套图标按钮。


本增量候选 SHA256 `cab014971fcbc3c058a0d876a3101b7ba5e8abb3de3e16c9110d35a625c09c5a`，126文件；干净归档重放及逐文件哈希一致。Nana历史7项、完整runtime832项、Button真实字体/键盘3项及Scene1项通过；独立Undo复核未发现阻断。应用10项history测试通过，覆盖同值回显、跨任务/Memory/路线图/文件及Surface对象隔离。第一轮2项失败是测试夹具未启用composer、未把文件挂入pane，修正夹具后通过，未修改生产以绕开失败。

完整 `cargo test --workspace --locked`：963通过、0失败、1项既有忽略，日志 `/tmp/lilia-history-workspace.log`。`git diff --check`通过。`cargo xtask verify` boundary通过，正式依赖pin仍失败，日志 `/tmp/lilia-history-verify.log`。

最新 `cargo xtask agent-debug --matrix` 在构建/采集前报 `desktop_session_locked`，日志 `/tmp/lilia-history-matrix.log`。因此本增量实窗及性能验收未运行，需要用户解锁macOS后继续；不以旧3866矩阵/性能冒充当前结果。其余历史像素、真实外部平台/凭据及正式Nana发布pin边界保持。Undo不持久化到重启后，也未新增操作系统菜单注册。Memory颜色混合令牌等尚未完成的精确源码/像素差异继续保留，不宣称全计划或像素等价完成。


## 普通粘贴、颜色与组合键回放增量

最新审计发现普通Cmd/Ctrl+V仍回落到纯文本输入：长文本没有按历史阈值转附件，图片/文件只有菜单入口；菜单短文本还直接追加尾部。正在把主窗/弹窗普通粘贴与菜单接回统一业务语义，保留选区和现有附件服务。

Memory选中border历史为Accent50%+Border，保存border为Accent58%+Border、背景Accent22%+Surface，悬停为Hover；错误面板border为Danger35%+Border。正在框架语义解析层增加通用混色，避免原始RGBA覆盖正常hover。未以当前近似色声明源码等价。

Agent Debug `ui-key.key` 新增 `Meta+z` / `Meta+Shift+z` 等组合键解析，主窗与弹窗使用相同正常RuntimeInputAdapter。Memory实窗回放增加撤销→保存、重做→保存、再次撤销→保存，并逐次核对权威记录；场景已编写但锁屏期间尚未执行。应用history10项以Meta组合键复测通过，日志 `/tmp/lilia-history-chord-tests.log`。


## a07bc3e1 富粘贴、语义混色与输入事件投影

NanaUI候选SHA256 `a07bc3e1e0cf913806be1bc5b2be258c8097b374e516dcd5d5e552c100a72005`，137文件，`/tmp/lilia-nanaui-parity/patch-replay-a07bc3e1` 干净重放一致。通用按键监听在实际分派时读取当前TextArea；主窗/弹窗Cmd/Ctrl+V与菜单复用文件→图片→文本捕获，2000个UTF16单元起转附件。短文本在当前选区替换，捕获或缓存失败不先删草稿。

富粘贴通过同一SQLite事务核对revision与原正文，再写正文及行内附件来源。普通Undo移除引用后，该附件不再显示或进入实际请求；Redo恢复引用后恰好恢复一次。手工附件与行内来源分开，显式删除同时去掉对应来源和引用；成功发送只清理仍匹配的草稿。`/tmp/lilia-rich-paste-store-final.log` 存储8项通过，覆盖迁移、重载、CAS与失败回滚；`/tmp/lilia-rich-paste-app-final.log` 应用5项通过，含真实adapter撤销/重做和请求不尾补。图文混合输入中的行内附件与对话引用已按引用原文作为编辑原子，并由 NanaUI 在该区间上绘制 icon+显示名+关闭 chip 覆盖原文。关闭命中与退格都通过同一套 TextArea 历史删除整段；草稿 `content_atom_spans` 喂给主窗与弹窗。权威正文仍保存引用原文，发送载荷只包含正文中仍存在的 token。

语义混色在框架解析层采用premultiplied-alpha sRGB，保存动态主题和正常hover/disabled优先级，应用仅消费历史比例。Memory选中边框50%Accent、保存边框58%Accent/背景22%Accent、错误边框35%Danger已接回；错误背景浅色Danger10%、深色14%。`/tmp/lilia-semantic-mix-tests.log` 3项通过，`/tmp/lilia-mix-memory-tests.log` Memory19项通过。完整runtime832项通过（`/tmp/lilia-mix-key-runtime.log`）；workspace970通过、1项既有忽略（`/tmp/lilia-rich-mix-workspace.log`），此完整workspace结果早于下一段宿主修复。

真实窗口四次失败 `1788509075323`、`1788509971464`、`1788510259152`、`1788510409643` 均在Memory首个Undo处停止；不是锁屏。第五次诊断 `1788510747360/restart-1788510798111.stderr.log:196` 证明全选先发送携带旧正文的TextChanged，替换再发送新正文；两条正常MemoryTitleChanged被逐条投影，控件被旧值覆回再覆新，清空事务历史。仅跳过debug输入消息末尾投影的初步修复不足，已删除。

最终修复保留全部普通事件和FIFO业务操作：三处主窗/弹窗sink共同计数已发出但未处理的ShellIntent；每条业务操作完成后递减，只有待处理数归零才投影两类窗口。浏览器直接触发的同步也经过同一门禁，发送失败回滚计数。没有过滤Undo的旧正文、绕过正常控件、合并业务动作或添加debug专用编辑语义。独立review未发现漏计/绕过；临时消息日志、历史深度API和debug延迟分类均已移除。

第一轮最终回放 `/tmp/lilia-history-drain-matrix.log` 因移除最后诊断注释导致构建期间源指纹变化而被正确拒绝，不计通过；冻结后重跑 `/tmp/lilia-history-drain-matrix-2.log`。每组新 `knowledge-memory-history.json` 记录Undo→Save、Redo→Save、Undo→Save三次权威状态；不弱化原断言。


最终 `a07bc3e1` 本批四组矩阵全部通过：960浅 `1788510977893`、960深 `1788511044442`、1440浅 `1788511096756`、1440深 `1788511148579`，各60PNG、窗口元数据全部为对应逻辑尺寸及scale1。三个保存后的Memory标题依次为原题、临时题、原题，四组一致。主窗/弹窗、扩展实际执行和重启链路继续通过。完整运行产物统一：launcher `7ac5e22d2bf624f8c7ee44e62b251452175c132d4485cf03b2bf5d1a6dc0211f`，host `0a5dc1a6aa67b7c1fa8e4ba71c983239dd610f5785cbeaf2308955fd1abea6af`。人工复核浅960与深1440的Memory图：Plus/Save、checkbox/radio、正文拖柄和保存混色实际可见；字体差异正在额外源码核查，不签署历史像素一致。

性能 `agent-debug-runs/lilia-performance-1788511254404/performance.json` 同批绝对阈值通过：5次cold的P95为2424.21ms，30样本composer帧P95为5.37ms、resize帧P95为7.98ms，1000条时间线85.18ms，idleCPU0%、RSS203685888bytes。日志 `/tmp/lilia-rich-mix-performance.log`；历史同平台同语料基线仍缺失，不声称性能历史等价。
