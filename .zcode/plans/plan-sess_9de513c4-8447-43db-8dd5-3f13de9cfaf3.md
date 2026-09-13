## 目标

1. 升级 NanaUI 依赖到最新 main rev。
2. 为 apps/desktop 接入 `nana-ui-devtools`（离线渲染/headless 截图）作为 dev-dependency。
3. 更新 Lilia `AGENTS.md`：要求先读 NanaUI 的 `AGENTS.md`，并写明调试 NanaUI 界面时利用其离线渲染能力。

## 改动内容

### 1. 升级 NanaUI rev（apps/desktop/Cargo.toml:62-63）

- 执行时用 `git ls-remote https://github.com/sena-nana/NanaUI.git main` 确认最新 origin/main rev（本地 checkout 当前在 `86d32eb1e`，比锁定的 `67c5955` 新 4 个提交；以远端最新为准）。
- 将 `nana-ui` 与 `nana-ui-platform` 两行的 `rev` 更新为该 rev，保持两处一致。
- `cargo update -p nana-ui -p nana-ui-platform`（或直接构建）刷新 Cargo.lock。
- 查看 `67c5955..新rev` 的提交范围，确认是否有破坏性 API 变更；若有，修复 apps/desktop 消费端，遵循根本原因优先原则，不绕过。

### 2. 接入 nana-ui-devtools（同仓库、同 rev）

- 在 `apps/desktop/Cargo.toml` 的 `[dev-dependencies]` 中新增：
  `nana-ui-devtools = { git = "https://github.com/sena-nana/NanaUI.git", rev = <同上>, ... }`。
- 执行时先读 NanaUI checkout 的 `crates/nana-ui-devtools/Cargo.toml`，确认默认 feature 是否足够（offscreen/agent 会话），只加必要 features。
- 不在正式产品代码路径使用它——仅 dev-dependency，供离线渲染调试与后续测试使用；不违反 NanaUI "正式路径禁止 CPU 回读" 的渲染合同。

### 3. 更新 Lilia AGENTS.md（「UI 布局与边框参考」小节）

- 在阅读清单最前面加一步：先读 NanaUI 仓库根的 `AGENTS.md`（工作区旁 `NanaUI/AGENTS.md`，或 cargo git checkout 对应路径），了解 NanaUI 的渲染合同与 Agent 使用规范，再写 NanaUI 界面。
- 补充调试指引：需要调试或验证 NanaUI 界面渲染时，使用 NanaUI 的离线渲染能力（`nana-ui-devtools`：`OffscreenSnapshots` 离屏截图、`RuntimeAgentSession` 的 screenshot/accessibility/click、`nana-agent-session` 命令行），已作为 dev-dependency 接入；不要靠日志猜界面状态。
- 措辞与现有 AGENTS.md 风格一致，简洁、不复述 NanaUI 文档细节。

## 验证

- `cargo xtask verify`（依赖 rev 与构建配置变更，属默认门禁必跑项）。
- 编译确认 dev-dependency 可用（verify 覆盖）。
- 本次不做 agent-debug：纯依赖升级 + 文档，未触碰 UI 主路径逻辑；若升级引发消费端代码修改再按风险补跑。
- 不提交、不推送，改动留在工作区由用户确认。