# Native Agent Debug Harness

## 目标

Agent Debug 为正式 NanaUI/WGPU 桌面提供开发态结构化观察与操作入口。它验证真实窗口、GPU 渲染和应用行为，不是生产功能，也不依赖浏览器自动化。

## 协议边界

- Desktop Debug 二进制在 `LILIA_AGENT_DEBUG=1` 时开启本地 TCP 调试协议。
- `observe` 返回窗口、稳定目标、可见状态、运行时摘要与最近错误。
- `act` 只接受协议定义的稳定目标和类型化动作，不按坐标猜测控件。
- 截图来自真实 WGPU surface；截图与业务快照分别判断，避免图像存在却未证明状态正确。
- secret canary 确认凭据、环境变量和敏感正文不会进入日志、回放或截图元数据。
- Release 构建不包含调试监听、固定调试标记或测试 fixture。

## 运行

从仓库根目录执行：

```bash
cargo xtask agent-debug
```

xtask 构建并启动正式 Native Debug target，等待协议 ready，按固定 corpus 执行 observe/act，最终关闭进程并生成证据。构建前后通过继承当前环境的完整 `cargo metadata --locked --format-version 1` 确认实际 NanaUI 来源；本地依赖按其 manifest 路径向上定位 workspace 并记录源码指纹，包括 CARGO_HOME 配置提供的 patch。依赖缺失、歧义或构建期间来源变化均拒绝验收。外部窗口或 GPU 能力不可用时必须返回结构化 blocker，不能把跳过当作通过。

## 产物

默认验收运行写入 `agent-debug-runs/lilia-agent-debug-<timestamp>/`，包含：

- `summary.json`：协议、证据目录、journal 记录数与总结果；
- `observe.json`、`replay.json`：初始状态与打开设置后的产品状态；
- `action.json`：设置动作结果；
- `task-light.json`、`task-dark.json`：切换主题并返回任务后的产品状态；
- `automation-return.json`、`task-switch.json`：管理页返回与跨任务草稿隔离的状态；
- `task-popup-acceptance.json`、`task-popup-*.json`、`task-popup.png`：独立窗口就绪、同任务草稿同步、跨任务隔离与关闭后保留；截图通过进程及明确窗口标题匹配真实独立窗口，不依赖系统前台焦点；
- `errors.json`：协议、运行时与窗口错误；
- `desktop.png`、`task-light.png`、`task-dark.png`：与观察匹配的真实窗口截图；
- `journal.jsonl`：有序 Kernel 事件记录；
- `secret-canary.json`：敏感数据泄漏检查结果；
- `browser-acceptance.json`：本地 HTTP fixture 的实际 WebView2 场景范围与未覆盖项；
- `browser-*.json/png`：原生导航、接管/恢复、新窗口拒绝及批准后的匹配观察和窗口截图。

产物不得记录凭据原文、用户 home 绝对路径或未脱敏的环境变量。

## 场景与通过标准

当前默认固定场景覆盖项目/任务打开、Composer 输入、设置和自动化页面返回、明暗主题、跨任务草稿隔离，并在 Windows 上覆盖任务浏览器导航、人工接管、显式恢复 Agent 控制、新窗口请求拒绝和批准。浏览器使用随机本地端口的受控页面；要求地址、任务/项目/标签身份、页面版本与生命周期符合实际动作。发送、完整待审批交互、多窗口焦点/窗格生命周期和单实例仍需额外场景，不能以默认命令通过替代这些验收。每个场景必须同时满足：

1. 稳定目标可观察且动作命中唯一目标；
2. 动作后的产品状态符合合同；
3. 截图非空且属于预期 Native 窗口；
4. 没有未归类错误；
5. secret canary 未泄漏。

默认错误检查只允许 fixture 刻意注入的 `mcp:native-debug-invalid` 来源；其他运行错误导致失败。凭据标记检查覆盖产品观察、动作结果、回放、错误、Kernel journal 和桌面 stdout/stderr。截图仍须人工查看实际排版，协议状态正确不等于视觉通过。

真实 provider 回复、远端凭据或设备能力不属于无凭据默认 corpus；需要时作为独立系统验收报告。

## 使用要求

涉及 UI 主路径、Agent runtime、持久化、权限、构建配置、跨端契约或用户关键路径的大型改动，最终确认必须包含 `cargo xtask agent-debug` 的结果，或具体 blocker、产物路径和剩余风险。普通 Markdown、注释或无运行时影响的整理不运行该门禁。

### 普通组合键回放

`ui-key` 保留正常控件挂载、可见性和焦点检查。`key` 可使用普通键名（例如 `Enter`）或组合键（例如 `Meta+z`、`Meta+Shift+z`、`Control+y`）；支持 Control/Ctrl、Meta/Cmd、Shift、Alt 前缀。主窗口与任务弹窗都经 RuntimeInputAdapter 分派，不直接调用撤销或业务helper。

Memory知识回放将临时标题撤销/重做后通过保存按钮写入，逐次查询权威记录，证据为 `knowledge-memory-history.json`。源码或测试通过不能替代该实窗回放通过。

浏览器场景等待真实宿主状态，不以 about:blank 或控制器数量代替页面加载。新窗口拒绝不能增加标签数量，批准必须打开同任务同项目下的独立标签并显示 child 页面。宿主不可用时保存 blocked 观察/截图并返回失败；上传下载完成、完整 Agent 工具审批执行、IME 和拖动选区仍是单独的待验收项。

当设置 CARGO_TARGET_DIR 时，Session 从 Cargo metadata 的 target_directory 定位本次构建二进制，避免运行根目录 target 中的旧程序。NanaUI 本地覆盖必须被子 Cargo 继承，且与正式 Git pin 验证分开记录。


## 真实浏览器 Agent profile

默认 `cargo xtask agent-debug` 在既有原生交互场景之后执行 browser-agent。可用 `cargo xtask agent-debug --profile browser-agent` 单独诊断：启动本机 HTTP 页面与模型 fixture，通过真实 Settings 保存 provider，要求 Mutsuki 模型请求实际声明 task_browser，逐次点击真实审批卡，执行 Observe、Type、Click、Scroll、Screenshot，并检查页面回报和截图产物，再验证拒绝与关闭审批期间取消。浏览器宿主和 Agent 执行器均为产品代码。

`--profile browser-agent --no-build` 仅用于 fixture 调试，复用 Cargo metadata 对应的现有二进制。每次保存 desktop-binary.json 中启动器和实际加载的 host 动态库的路径、SHA256、修改时间、大小和是否重建；复用产物的成功不能替代最新源码的构建验收。正常入口始终构建。运行证据保存于 agent-debug-runs/lilia-browser-agent-*；任一步失败即整个 profile 失败，不以出现审批卡代替执行成功。

SettingsView 回归遍历项目、模型服务、Agent、用量、扩展、远程、桌面、数据、关于、外观 10 个页签，分别记录明暗主题下的 `settings-<tab>-<theme>.json/png`。每份观察核对页签与主题且经过 secret canary 检查，最后回到外观页以继续既有主题/返回任务场景；这里只覆盖页面呈现和导航，不能替代保存/撤销等业务验收。

AutomationView 回归在隔离的 debug home 中新建带人工确认节点的工作流，发布并运行，检查等待确认、回复后完成、第二次运行及取消。`automation-waiting`、`automation-completed`、`automation-second-run`、`automation-cancelled` 的 JSON/PNG 配对保存，核对同一次确认没有切换运行、新运行具有独立身份，并检查观察中的 secret canary。这些场景不覆盖完整节点配置、异步运行调度及所有触发器；真实控件的事件归属另由视图行为测试验证。

节点编辑回归从观察到的图节点目标选择人工确认节点，输入名称与指令，保存后核对已保存节点投影，再继续发布/运行流程。`automation-node-editor.json/png` 记录匹配的编辑器状态与真实窗口。图节点目标包含工作流和节点身份；不能用配置输入目标代替实际节点选择，也不能把人工节点覆盖扩大为所有 Agent/工具/条件表单验收。

默认完整回归只构建一次桌面产物；后续 browser-agent profile 复用该产物，两个 profile 的 desktop-binary.json 中启动器与 hostLibrary SHA256 必须一致。不一致时报告 debug_binary_changed 并拒绝生成成功 summary，避免将不同源码构建混为同一运行。

性能冷启动同样在首次构建后复用产物，每次都创建新进程并与首个 Session 校验启动器/DLL 哈希。构建不计入原有 startup_ms，复用避免测试桌面仍运行时重写 Windows 已加载的 DLL；不改变五次冷启动及 30 次帧采样的定义。


自动化和 Agent 浏览器回归将独立测试窗口调整到 780 逻辑像素宽度，并保存窄窗观察与真实截图，随后恢复到 1180×760。窗口尺寸通过现有 Win32 捕获流程按当前窗口 DPI 换算，保留非客户区边界；不改变用户窗口、不使用坐标猜测内容目标。自动化在窄窗断言节点保存入口仍可达且图节点目标隐藏；浏览器在窄窗显式打开后等待原生宿主就绪，再恢复宽窗执行页面操作。
