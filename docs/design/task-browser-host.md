# 任务浏览器宿主

`lilia-contracts/contracts/browser-contract.json` 与 `lilia-contracts::BrowserRequest` 定义产品边界；窗口句柄和 COM 类型只存在于桌面平台实现。项目身份决定 profile 目录（项目 ID 的 SHA-256），任务和 tab 身份共同决定页面归属。

`BrowserSessions` 管理 tab 生命周期、页面版本、人工接管和取消。操作先验证 scope、lifecycle、pageVersion 与已观察的目标，再释放锁调用 `TaskBrowserHost`。宿主必须在真正执行输入前检查 `BrowserCancellation`。页面变更、关闭和接管使排队操作失效；完成结果再次验证版本，不能覆盖新页面。人工接管后，只有明确的 `resume` 才能允许 Agent 操作。

`NativeAgentKitRuntime::set_browser_sessions` 将 `task_browser` 插件加入已有 Mutsuki Host 和 ToolRegistry。`bind_browser_session` 校验 Agent session 的产品任务绑定；模型输入没有项目或任务授权字段。工具使用现有审批执行信封，验证会话、动作、轮次、版本和批准结果；任务取消同步撤销该 session/turn 并取消浏览器操作，迟到的首次 Observe 也不能绕过已取消轮次。没有第二套 Agent 执行器。

Windows `UiBrowserHost` 在应用 STA UI 线程创建 WebView2 composition controller，使用应用提供的 HWND 和 `IDCompositionVisual`。`BrowserBridge` 通过容量 64 的队列向 UI 线程派发命令；等待发生在 Agent worker，UI 线程只启动异步 COM 调用。30 秒超时使 token 失效，迟到调用不能继续输入。`poll` 由应用帧入口调用，COM 页面事件通过提供的 wake 唤醒应用。

宿主持有 STA apartment RAII，异步回调和 tab 都共享其生命周期；初始化失败由 create 返回，初始化成功包括 S_FALSE 均匹配释放。原生键盘/IME 在 `MoveFocus` 后交由 WebView2 controller HWND；不将键盘仿成文本输入。隐藏已聚焦的 tab 时归还父 HWND 焦点，CDP 仍可操作隐藏页面。窗口迁移使用 SetParentWindow 与 RootVisualTarget，失败时尝试恢复原父窗口及 visual。

页面观察使用 CDP Accessibility，目标使用 backend DOM node ID；点击和输入使用 DOM/Input 协议，不向网页注入本机命令桥。DOM 与导航事件使观察过的目标失效；导航完成后才返回观察。`BrowserOperation` 的 Debug 隐藏输入文本，但这不能保护 Mutsuki 执行前持久化的 ToolCallStarted.input。模型适配层因此在结果进入 AgentLoop 前，将 Type 文本及 Navigate 原始 URL 替换为短期内存票据，并同步清理模型结果 raw 与工具调用 metadata。票据绑定 session、call、scope、lifecycle 和完整公开参数，单次使用、容量 128、五分钟过期；拒绝、取消、接管、关闭和绑定切换清理票据。审批执行后才解出原操作交宿主；工具结果 URL 去除用户信息、查询和片段，宿主错误不透出原值。原始用户提示、上游网络日志和手工 wire 注入不属于该适配层保护范围，不能据此声称凭据全链路不会记录。浏览器 cookie 和 profile 数据不进入工具结果。

下载、上传与新窗口通过 `BrowserHostRequest` 票据交可信 UI 处理。`respond` 不进入 Agent 工具注册表，网页不能回传本地路径。下载与新窗口保留 COM deferral；拒绝、导航失效、关闭、取消与十分钟过期均释放。批准下载只接受现存目录内的绝对目标路径，并追踪原生 DownloadOperation 的完成、失败与任务取消。上传使用 CDP file chooser interception；仅接受 UI 明确选中的现存文件，按原 backend node 调用 DOM.setFileInputFiles，异步结果返回原票据。票据 pageVersion 是发起时记录，效力由 scope/lifecycle 及宿主内部文档代数判断：新的观察结果不会误使下载请求失效，上传目标 DOM 变化则会失效。

新窗口需要先 `create_for_request` 准备同项目、同任务的目标 tab，再 `respond(NewWindow)`。目标复用 opener 的 CoreWebView2Environment，必须尚未导航；成功后由 WebView2 原生请求导航，保留 window.open 的 opener/WindowProxy 语义。参见 [WebView2 NewWindowRequested 合同](https://learn.microsoft.com/en-us/microsoft-edge/webview2/reference/win32/icorewebview2newwindowrequestedeventargs)；文件选择拦截使用 [CDP Page.setInterceptFileChooserDialog](https://chromedevtools.github.io/devtools-protocol/tot/Page/#method-setInterceptFileChooserDialog)。

权限卡、文件选择器、目标窗格与宿主票据已接线，完整实际上传/下载仍待真实窗口验收。截图目前保存在项目 profile 下的 screenshots 目录，后续应接入 Product artifact 保留策略。macOS/Linux 没有 WebView2 实现，不能宣称浏览后端可用。全量 DOM 观察及复杂页面的输出成本仍需性能实测。


运行期间的 Product 权威由 `BrowserScopeAuthority` 注入，桌面适配器只依赖 ProjectTaskService。注册、绑定、恢复控制、实际执行前及异步结果提交前重新验证任务存在、未归档、仍属于原项目且项目处于 Active；失效关闭生命周期并撤销绑定与私密票据。无权威的 BrowserSessions 不允许执行。人工导航、文件请求批准也执行相同校验；拒绝保持可用以释放原生 deferral。UI 更新停用失效宿主，但保留原工作区记录，不能仅靠 UI 的更新时机阻止跨项目操作。


2026-09-05 真实 Agent 检查点：`agent-debug-runs/lilia-browser-agent-1788618014509/browser-agent-acceptance.json` 记录五次工具操作、逐次审批、拒绝和待审批期间取消通过；页面回报和 PNG 均由实际 WebView2 生成。选中范围进入 BrowserSessions，Native runtime 在提交新 turn 前按 Product session binding 绑定，避免新会话等待下一次 UI 通知。此检查点仍不覆盖真实 WebView2 进行中 CDP 取消和上传/下载完成；域层取消现已由后续测试覆盖。

2026-09-06：浏览器工作区新增显式“停止操作”按钮。`BrowserSessions::is_busy` 只报告瞬时宿主状态，不写入页面事实；按钮仅在当前 tab 有进行中的 Agent 操作时启用，触发既有 `cancel(scope)`，继续执行 scope、生命周期和页面版本失效保护。应用侧本地 NanaUI 联调检查记录在 `E:/codex-build/lilia-workbench/browser-cancel-check.log`；正式 lock 已随后恢复为 Git pin。该路径仍需真实 WebView2 窗口回放验证。
同日补充 `lilia-agent` 域行为测试：显式取消期间 busy 状态为真，取消后释放 busy，迟到宿主结果不会覆盖原页面（`E:/codex-build/lilia-workbench/browser-cancel-domain-test.log`）。浏览器域 8 项回归测试全部通过（`E:/codex-build/lilia-workbench/browser-domain-regression.log`）。
浏览器工具栏取消按钮的窄窗布局和 busy 启用状态测试通过（`E:/codex-build/lilia-workbench/browser-cancel-ui-test.log`）；正式 lock 随后恢复为 Git pin。
`lilia-agent` 全量回归 83 项通过、1 项忽略（`E:/codex-build/lilia-workbench/lilia-agent-full-regression.log`），确认取消动作没有改变 Agent runtime、权限和共享服务行为。
