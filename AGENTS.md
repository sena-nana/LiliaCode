# Agent 入口规范

<!-- CODEGRAPH_START -->
## CodeGraph

In repositories indexed by CodeGraph (a `.codegraph/` directory exists at the repo root), reach for it BEFORE grep/find or reading files when you need to understand or locate code:

- MCP tools (when available): `codegraph_explore` answers most code questions in one call, returning relevant source plus call paths. `codegraph_node` returns one symbol's source and callers, or reads a whole file with line numbers. If the tools are deferred, load them by name via tool search.
- Shell fallback: `codegraph explore "<symbol names or question>"` and `codegraph node <symbol-or-file>` print the same output.

If there is no `.codegraph/` directory, skip CodeGraph entirely.
<!-- CODEGRAPH_END -->

## 项目级 Skills

本仓库通过 `.agents/skills` 提供 Agent 能力。处理对应任务时优先使用这些 Skill,不要把细则继续堆进 `AGENTS.md`。

- `$lilia-app-design`: 设计、交互、视觉层级、页面样式、侧边栏、卡片、浮层和状态评审。
- `$lilia-app-coding`: 功能实现、问题修复、重构、路由、命令、业务页面和应用专属 Tauri 代码。
- `$lilia-app-boundary`: 判断改动属于 Lilia 应用、`packages/contracts` 还是 LiliaUI 公共能力。
- `$lilia-app-validation`: 选择功能验证、测试、构建、Tauri 检查和结果汇报方式。
- `$lilia-app-git`: 暂存、提交、推送、合并和依赖更新收口。
- `$lilia-agent-debug`: Agent 调试入口、`data-agent-id`、`window.__liliaAgentDebug`、`yarn verify:agent-debug` 和 `tauri-driver` 调试验证。

## 运行入口

- Lilia 是 monorepo,常用命令从仓库根目录运行:`yarn tauri:dev`、`yarn dev`、`yarn verify:desktop:test`、`yarn verify:agent-debug`。
- `apps/desktop` 是桌面子包;子包脚本只做本地 Vite/Tauri CLI 或转发根目录脚本,不要把 Tauri-Template 的单包 `@lilia/build` 入口原样覆盖到 Lilia 根脚本。
- `@lilia/build`、`@lilia/tools` 和 `@lilia/config` 的单包模板能力需要先在 LiliaUI 支持 monorepo 路径后,再替换 Lilia 现有构建、发布、Android 或 release 脚本。

## 硬约束

- 灵活运用子代理任务分派,并行化执行边界清晰的任务;主 Agent 负责整合、验证和收口。
- 修复问题时先定位根本原因,禁止打补丁式修复。
- 实现前结合上下文判断代码和设计是否有足够价值,优先选择更简洁优雅的方案。
- 禁止在 UI 显示技术说明内容。
- 禁止让 UI 看起来像有功能但实际未接入;所有可见操作必须落地功能或表达真实不可用状态。
- 禁止添加低价值测试和硬匹配日志或字符串的测试;所有测试必须以功能为准,无功能变动则不添加测试。
- 不覆盖用户或其他 Agent 的已有改动。

## 代码边界

- 先读相关模块、数据契约和现有测试,再动手改代码。
- 跨端数据先改 `packages/contracts`,再同步前端、后端和测试。
- 通用 UI、样式、主题、浮层、桌面壳基础能力、构建工具和默认资源优先下落到 `C:\Files\workspace\LiliaUI`;Lilia 消费 `@lilia/ui` 等公共包,不复制维护公共实现。
- `apps/desktop` 只保留 Lilia 业务编排、业务页面、业务 Tauri 命令、provider、timeline、agent runner 和应用专属状态。
- 不加冗余注释;需要长期记录的背景、取舍和未决问题写进 `docs/design/`。
- 新增组件或功能单元时,优先解耦到独立文件/模块,并以异步懒加载方式接入;仅当任务明确是修改现有组件时,才在原组件内调整。

## 样式

- 保持 Lilia 的工程工具气质:克制、清晰、可扫描。
- 视觉分级明确:主内容 > 当前状态 > 过程信息 / 辅助操作。
- 优先使用 `@lilia/ui` 的 CSS 变量、基础类和组件语言;深浅主题都要可读。
- 涉及 UI 设计或交互模式调整时,先询问用户是否需要 IAB 确认交互模式;若需要,实现前先完成确认。
- 不实现 `prefers-reduced-motion` / 减少动效兼容,不要因系统偏好关闭 Lilia 既有动效。

## 验证

- 功能实现后根据任务风险和影响范围选择是否验证;可选验证包括 `yarn verify:desktop:test`、`yarn verify:contracts` 或相关定向测试。
- 文档、注释、配置说明等低风险改动可不跑测试;涉及跨端契约、持久化、调度、权限、构建配置或用户关键路径时,优先运行最小必要验证。
- 涉及大型 UI、Agent runtime、Tauri command、持久化、权限、构建配置、跨端契约或用户关键路径的大型改动,必须运行 `yarn verify:agent-debug` 做 Agent 调试确认;该脚本会自动准备 `tauri-driver` 与 EdgeDriver,若仍因 cargo、网络、Edge 版本探测或 debug binary 缺失阻塞,最终说明必须写清楚 blocker、`agent-debug-runs/` 产物路径和剩余风险。
- Agent 调试确认以开发态为准:设置 `LILIA_AGENT_DEBUG=1` / `VITE_LILIA_AGENT_DEBUG=1`,通过 `data-agent-id` 结构化操作,并用截图确认界面表现;调试层不得进入生产功能面。
- 完整构建或全量验证,如 `yarn verify:desktop:build`、`yarn verify:tauri` 或 `yarn verify`,仅在用户要求、改动范围较大或风险确实需要时运行。
- 若未运行测试、构建或验证,在最终说明里写清楚原因;若验证无法运行,写清楚阻塞原因和剩余风险。

## Git 提交

- 提交标题用中文短句概括结果。
- 提交正文按列表简短写具体改动;无必要不写正文。
- 提交前按改动范围选择是否检查 diff;涉及多人协作、合并冲突或跨模块改动时,确认 diff 只包含本次改动。
- 提交前按任务复杂度选择是否做代码自检;涉及逻辑调整、重构或公共模块时,检查是否存在可删除的冗余逻辑、重复分支、无效辅助函数或代码复述型注释。
