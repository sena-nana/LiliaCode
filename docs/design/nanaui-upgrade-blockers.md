# NanaUI 能力恢复与依赖升级记录（2026-09-10）

## 当前：1100b02 已发布来源验证

用户确认后，NanaUI 恢复版本已提交并推送为 `1100b020b9384f58fb0e744d50f0b2857973d28c`。Code 的四处 manifest pin 已统一到该提交；完整 `cargo metadata --all-features --format-version 1` 重新解析后，`cargo metadata --locked --all-features --format-version 1` 通过。8 个 NanaUI 包均来自该真实 Git source，wgpu 仅 `30.0.0` 一个版本，没有本地 path patch。

使用正常 Cargo HOME（显式取消临时 `CARGO_HOME`）、原 target、`CARGO_BUILD_JOBS=1`，正式 `cargo xtask verify` 已退出 0：boundary-check、pin-check（23 个不可变 Git 依赖）、locked metadata、workspace tests 和 workspace check 全部通过。69 个测试 suite 共 **990 通过、0 失败、2 忽略**；忽略项包括只在独占桌面时显式运行的扩展原生验收探针。锁文件运行前后 SHA 相同，没有恢复旧 3ef 或本地联调 lock。

正式来源的 `cargo build --locked -p lilia-desktop` 预构建也已退出 0（正常 HOME、jobs=1、原 target）；只构建二进制，未启动窗口。日志与结果为 `code-desktop-build.log`、`code-desktop-build-result.json`。

完整命令、依赖来源和结果保存于相邻 `nanaui-restoration-recovery-20260910/published-1100b02/`（下文简称 PUB）的 `dependency-proof.json`、`code-xtask-verify.log` 与 `code-xtask-verify-result.json`。本节随恢复消费提交归档；下方历史段落的提交状态仅对应当时检查点。

同一正式 Git 来源下，`cargo xtask agent-debug --matrix` 已退出 0，四组分别为 `1789022290679`（960×600 浅色）、`1789022361143`（960×600 深色）、`1789022414909`（1440×900 浅色）、`1789022466615`（1440×900 深色），本机实际 scale 均为 1×。每组 61 张截图，共 **244 张逐张复审通过**；另外每组有一张剪贴板输入图片，不计入截图矩阵。业务 96 张、设置 76 张、重点交互 48 张、Memory/弹窗等 24 张分别记录于 PUB 的 `code-native-visual-review-business.json`、`code-native-settings-review.json`、`code-native-priority-review.json`、`code-native-visual-review-root.json`，均含逐图 SHA 和窗口证据。时间线末端实际内容、Todo 与 Markdown 分隔、长气泡复制按钮、MCP 完整开关均已查看。窄窗滚动视口以外内容仍按正常裁剪，不声称所有内容同时在屏。

四个原始 run 完整归档于 PUB 的 `code-native-matrix/`，1304 个文件的集合、大小和 SHA 已经独立复核（`code-native-matrix-archive.json`、`code-native-archive-independent-check.json`）。四组二进制 SHA 均为 `1c480aaa4fa852e57893d0fb9acf3af0ed585220acfbfec0d56b1f42ed128433`，宿主库 SHA 均为 `e2875fb3e47b287f245b1783fce2f893a464ab8aa783c4dced9096682cd538e9`；`localPatch=false`，Git 来源由完整 metadata 和锁文件证实，不能把 Git 场景中空的本地源码指纹当作单独的内容校验。原生命令前后锁文件 SHA 均为 `afe8c19ccb4ea8acad901e1f55adbe4723f91a92459fe76f97c6b63dc632b1d8`。

全部看图和归档 I/O 结束后，独占运行 `cargo xtask performance`，退出 0；5 个真实进程 run 为 `1789023059488`、`1789023073242`、`1789023077541`、`1789023081875`、`1789023086221`。完整采样与身份记录在 PUB 的 `code-performance-runs/`，140 个文件均复制后比对 SHA，清单为 `code-performance-archive.json`；正式日志和退出记录为 `code-performance.log`、`code-performance-result.json`。本轮启动 P95 为 **2093.75 ms**，30 次输入与 30 次面板调整 P95 分别为 **6.23 ms / 13.37 ms**，加载/分页到 1000 条事件为 **137.08 ms**，主进程 CPU 采样 **0%**、RSS **203,735,040 字节**。非作者已独立重算 5 / 30 / 30 份原始样本、CPU 间隔和 RSS，并逐份核对 fixture/source/build/runtime 身份，全部通过，记录为 PUB 的 `code-performance-independent-review.json`；5 次性能进程与 4 组矩阵的二进制及宿主库 SHA 完全一致。

性能数值遵循 schema 2 的实际测量边界：启动是 spawn 到 debug ready（不是冷缓存或内容首帧）；交互是 handler 开始到主窗口 `window_frame_presented`（不含请求传输/排队，也不等于持续 FPS）；千条指标是加载与分页到 1000 条（不是同时绘制 1000 行）；CPU 是两次累计时间差按实际间隔和 10 个逻辑处理器归一化，RSS 为主进程单次采样，均不覆盖子进程/GPU 或峰值内存。缺少匹配历史语料，因此没有宣称相对历史版本性能提升。正式来源性能窗口已实际查看，末端事件 986–1000 与输入/详情区域正常。以下本地联调记录只作为问题追溯，未替代本次真实 Git 来源验收。


## 历史：恢复联调状态

用户已授权恢复缺失组件。现已从原任务及其 secondary_parity / business_parity / project_restore 子任务提取真实实现与后续修正，适配当前 NanaUI 单 Runtime 树、共享 Scene 和宿主生命周期。持久来源归档位于 `../nanaui-restoration-recovery-20260910/`；Markdown、文本和预览原始记录保存在其 `markdown/` 目录，没有重建或覆盖原空临时目录。

使用正常 CARGO_HOME、原 target、`CARGO_BUILD_JOBS=2`，仅显式传入 `--config /tmp/nanaui-consumer-upgrade-20260909/local-framework.toml` 进行本地框架联调：`cargo check -p lilia-desktop --all-targets` 已通过，原 71 / 80 个缺失 API 编译错误已清零。日志为 `/tmp/nanaui-consumer-upgrade-20260909/restoration-code-check.log`。桌面 lib 首轮测试 552 通过、4 失败；已修复包括 Workspace 借用表面被 ScrollView hover 投影覆盖、ImageViewer Escape 共享关闭路由在内的行为根因。最终桌面 **557 个测试全部通过**，日志 `restoration-code-tests-final.log`。两项定向日志为 `restoration-code-resize-test.log` 与 `restoration-code-viewer-test.log`。

恢复初批绘制与工作区借入组件修正后，workspace 测试和文档测试再次 **974 通过、0 失败、1 项既有忽略**（69 个 suite），workspace/all-targets check 通过。日志 `restoration-code-workspace-tests-verified.log`、`restoration-code-workspace-check-verified.log`。

本地联调重新执行 `cargo xtask verify`：boundary-check 通过，pin gate 按合同拒绝 path lock，须等恢复版本发布后验证真实 Git 来源。桌面锁定是首次尝试的环境状态，用户解锁后已实际重试三轮。首组 `lilia-agent-debug-1789008162279` 和诊断组 `lilia-agent-debug-1789009603617` 各生成 34 张图，确认模型选择失败来自旧硬编码断言；现从实际候选推导下一项并按连续键盘路径验证。框架 Fixed 滚动提示框、键盘录制徽标及应用 UTC 日期已由实窗图复验通过。

第三轮 `lilia-agent-debug-1789010992612` 生成 56 张 960×600 浅色、1× 截图，构建前后 NanaUI 源码指纹一致；有效引用实际发送、Markdown、图片查看和审批草稿恢复通过。正文删除引用后外置幽灵标签的问题已修复，保留撤销索引并按有效引用投影/发送。当前在 Memory 标题撤销边界发现首次输入与再次编辑错误合并，撤销回空使保存禁用；正在修复 Runtime 焦点和选择历史边界。完整四组矩阵、最终 workspace 复验与性能仍未通过。以上使用显式临时 CARGO_HOME 的本地 patch 联调，不能合并为已发布 Git pin 验收。三轮失败证据已持久归档相邻 recovery 的 `code-native-failures/`。

第三轮发现的撤销、Checkbox标签、Markdown换行与保留裁剪问题已修复并通过非作者复审。最终显式本地联调的 workspace/文档测试 **980 通过、0 失败、1 项既有忽略**，workspace/all-targets check 与 fmt 通过（`restoration-code-*-after-matrix3.log`）。完整四组原生矩阵及性能继续验证，不能把局部回归算作完整实窗通过。

恢复改动尚未再次发布，应用 manifest 仍 pin 已发布的 `3efde14e6ec78fd23951e643ef178bba58b3ff47`。完成联合验证后已将默认 Cargo.lock 恢复为已发布的 3ef Git 来源；本地恢复 lock 单独归档到相邻 recovery 目录。正常 CARGO_HOME、无 --config 的完整 cargo metadata --locked --all-features 再次通过，8 个 NanaUI 包均为 3ef 来源。该发布版仍不包含新恢复 API；新恢复版本的真实 Git 来源、正式门禁、交互与性能复验须在恢复发布后完成。下面的 3ef 发布来源和能力缺失结果是恢复前历史记录，不代表当前本地恢复树仍缺少这些 API。

## 历史：3ef 已发布来源验证

使用正常 CARGO_HOME、原 target 与 `CARGO_BUILD_JOBS=2`，没有传入临时 `--config`，没有默认 NanaUI patch。完整 `cargo metadata --all-features --format-version 1` 重新解析锁文件，随后 `cargo metadata --locked --all-features --format-version 1` 通过。依赖图中的 8 个 NanaUI 包全部指向同一个远端 git source 和上述完整提交，wgpu 仅 `30.0.0` 一个版本。来源证据为 `/tmp/nanaui-consumer-upgrade-20260909/published/code-source-verification.json`，完整依赖图为同目录 `code-metadata-locked.json`。

应用仓库没有提交或推送；现有 `.pin` 备份、用户业务改动和原隔离目录保持原样。

已发布依赖的正式门禁（正常 CARGO_HOME、无临时 patch、`CARGO_BUILD_JOBS=2`）：

| 命令 | 2026-09-10 发布来源复验结果 |
| --- | --- |
| `cargo xtask verify` | boundary-check 通过；pin-check 通过（23 个 immutable git 依赖）。随后 workspace Rust tests 的桌面编译失败：`lilia-desktop` lib 71、lib test 80 个缺失 API 错误，返回 `command_failed`。因此不是 pin 或依赖来源失败。 |
| `cargo xtask agent-debug` | 正式入口已执行，在原生验收前置检查返回 `desktop_session_locked`；本次未进入桌面交互与截图。 |
| `cargo xtask performance` | 正式入口已执行，同样返回 `desktop_session_locked`；本次没有性能采样。 |

日志分别为 `/tmp/nanaui-consumer-upgrade-20260909/published/code-verify.log`、`code-agent-debug.log`、`code-performance.log`。当前真实 git pin 与锁文件来源验证已完成；应用仍因下表既有能力缺失无法编译。原生与性能验收本次另受桌面锁定阻塞，不能把历史本地 patch 构建失败或旧截图算作本次发布验收。没有为相同的 71 个源码错误重建独立 target 或反复冷编。


## 原隔离源码缺失

原 `.cargo/config.toml` 将八个 NanaUI crate 指向 `/tmp/lilia-nanaui-parity/isolated-source-d0f3ccf`。本轮只读检查发现该目录有 150 个目录、零个文件、无 `.git`；整个 `/tmp/lilia-nanaui-parity` 也没有文件。未删除或重建这个目录。原配置备份保存在 `/var/folders/4j/ctycg4hn3j316xrff34fxkfh0000gp/T/nanaui-consumer-migration-9dve1lad/LiliaCode-patch.toml`，仓库现有 `.pin` 文件原样保留。

检查 `~/.cargo/git/checkouts/nanaui-9f21799893cf41ff` 中约 130 份本地检出，包括 `5fd7d66`、`d0f3ccf` 与目标版本，未找到以下关键缺失类型的定义：`BrowserView`、`NativeBrowserRequest`、`DonutChart`、`TimeSeriesLayer`。隔离目录名称不能证明缺失补丁与该 git commit 内容相同。`cargo metadata --no-deps` 不验证这些依赖源码或 Rust 消费 API，不能代替编译门禁。

2026-09-10 追加只读恢复检查：NanaUI 没有 stash；现存干净 Claude worktree
`82515cef` 及 `origin/codex/live-panels` 的 `c6ae2851` 也没有这四个关键类型定义。
未修改其他 worktree 或据此构造替代实现。

2026-09-10 进一步追溯确认：Code 当前 `HEAD:apps/desktop/src/iab_panel.rs` 仍是 `browser_attached()` / `browser_ready()` 返回 false 的占位状态；工作树中的 `BrowserView` 接入已经出现在本轮开始保存的 `/tmp/nanaui-consumer-upgrade-20260909/LiliaCode.diff`（约第 6864 行），临时 NanaUI patch 也已经在该初始 diff 的第 1–18 行。因此这两部分都是本轮之前存在、尚未进入 Code HEAD 的改动，不能归因于本轮 pin 升级新增。

NanaUI 的 `git log --all -G 'struct (BrowserView|DonutChart|TimeSeriesLayer)|enum NativeBrowserRequest' -- crates` 没有命中；Code 的 `git log --all -G 'BrowserView|DonutChart|TimeSeriesLayer' -- apps/desktop/src` 也没有命中。只读日志分别保存在 `published/missing-api-history.log` 与 `published/code-missing-api-consumer-history.log`（完整前缀同上述 published 目录）。已确认的是：业务接入尚未进入当前 Code HEAD，所查框架定义不在本地当前可达的 Git 历史中。现有证据不能说明这些实现从未存在，也不能确定临时目录为何变空、何时变化或由谁处理；此前依赖的隔离工作树内容缺失，尚无可恢复的已提交实现。


## 原任务追溯补充（2026-09-10）

用户提供原任务 `01a066c5-4b6a-7b81-958c-1825eb3330e7`（「对比并规划还原Tauri版本」）。已通过原任务记录确认，其中包含实际框架实现、验证结果与 fileChange 记录，并提到 `/tmp/lilia-nanaui-parity/nanaui-restoration.patch` 及尚未发布的候选改动。这纠正了仅根据当前文件系统和 Git 历史无法判断原实现是否存在的证据边界：这些能力确实曾在原任务中实现和验证，尚未进入当前可达的框架 Git 发布历史。当前正从记录提取原补丁，以恢复源码并适配已发布基线；提取和新基线复验尚未完成，不能把历史验证当作本轮恢复验证。

## 历史：恢复前缺失能力清单

| 能力 | 当前消费入口 | 不可直接删改的行为 |
| --- | --- | --- |
| 原生内嵌浏览器 | `apps/desktop/src/iab_panel.rs`；`desktop.rs` 的 `native_browser_requests/native_browser_event` | `BrowserView`、浏览器请求/事件/策略与 RuntimeProgram hook 均缺失。现有应用支持地址导航、前进后退、刷新停止、截图附件；`application/iab.rs` 只负责截图/元数据保存，没有 WebKit 宿主后端，不能用空请求或静态组件替代。 |
| 环形图与多层时序图 | `runtime_surface.rs`、`runtime_shell/quota.rs`、`runtime_shell/debug.rs` | 缺失 `DonutChart/DonutSlice/TimeSeriesLayer` 与 chart `axis_labels/layers/active`，涉及配额组成、趋势与命中交互。 |
| 语义色混合 | `runtime_shell/memory_page.rs` | 缺失 `SemanticColorMix`、样式 `surface_mix/outline_mix`、`InteractionStyle.base`；须保持主题和交互状态语义。 |
| 文本输入会话与高度调整 | `runtime_surface.rs`、`runtime_shell.rs`、`runtime_shell/memory_page.rs`、`runtime_conversation/keyboard.rs` | 缺失 `clear_text_history`、`on_view_key/on_key`、`Textarea.resize_vertical` 和 `TextInput.resize_grip`；涉及切换内容后的 undo 隔离、编辑快捷键和拖动高度。 |
| Markdown 图片与预览 | `runtime_conversation.rs`、`runtime_conversation/links.rs`、`runtime_shell.rs` | 缺失 `NativeMarkdown.resolve_image`、`MarkdownSpan.image_resource`、图片激活事件、Markdown 图片绘制几何以及 `ImageViewer.intrinsic_size`；涉及异步图片、点击打开和图片固有尺寸。 |
| 已投影弹出菜单内容根 | `runtime_shell.rs::sync_action_menu_items` | 缺失 `popover_content_root`。不能直接把打开菜单的子项移回触发节点而绕过 Overlay 生命周期。 |
| 调试与输入验收坐标 | `runtime_shell/debug.rs`、`runtime_windows_debug.rs`、`runtime_conversation.rs` 测试 | 缺失 `UiWorld.pointer_layout_position/layout_pointer_position` 与富文本 pointer begin/end 接口。须保留真实滚动、缩放、命中语义。 |

另有独立的应用未完成接线：`module/composer.rs` 写入 `PrimaryShellSnapshot.composer_atom_spans`，但当前 snapshot 没有该字段。NanaUI 已有 `TextAtomSpan` 公共能力，可后续按业务 ContentAtomSpan 语义完整接线；这并不能解决上表缺失框架实现。

## 已完成的等价迁移与验证边界

重复子节点重排改为 `AppContext::reconcile_children`，保留稳定节点、park 和现有事务语义；设置叶行与 FormField 控制关联改用公共接口；文件选择生产路径改为 WindowCommand / WindowEvent，DesktopHost mock、调试 fixture 和非 UI 的 platform/CLI 分层保留。另修正公开的 RoutedInput、ActionDescriptor、键盘修饰键和窗口事件签名兼容。

## 历史本地 patch 联调记录（不作为已发布来源验收）

2026-09-09 使用 `CARGO_BUILD_JOBS=2 cargo --config /tmp/nanaui-consumer-test-patch.toml check -p lilia-desktop --all-targets`，仍因上述基线能力缺失失败（lib 71 个错误、lib test 80 个错误）；日志 `/tmp/liliacode-consumer-check-final.log`。这不是全量门禁通过，也没有通过 feature 屏蔽、删除浏览器或空实现使编译变绿。恢复缺失能力后必须重新运行桌面 all-targets check、对应功能测试与真实离屏验收，再完成最终无默认 patch 的 git lock 检查。

正式入口已使用共享临时 `CARGO_HOME=/tmp/nanaui-consumer-upgrade-20260909/cargo-home` 重新尝试；该目录配置本地框架 patch，registry/git 指向正常缓存，xtask 内部子 Cargo 也继承配置，未改仓库默认配置。先执行完整 `cargo metadata --format-version 1` 成功解析本地依赖，再执行下列命令：

| 命令（均设置上述 CARGO_HOME 与 CARGO_BUILD_JOBS=2） | 实际结果 |
| --- | --- |
| `cargo xtask verify` | xtask 成功构建并运行；boundary-check 通过，随后 pin-check 返回 `git_pin_missing_from_lockfile`，因为本地联合开发的 path lock 不包含 manifest 声明的 git source。此处是发布来源检查，不是配置未继承。 |
| `cargo xtask performance` | 桌面解锁后重试，在构建 `equivalence_fixture` 原生语料时因 `lilia-desktop` 的 71 个缺失 API 错误失败，返回 `command_failed: seed native performance corpus exited with exit status: 101`。 |
| `cargo xtask agent-debug` | 桌面解锁后重试，实际执行内部 `cargo build --locked -p lilia-desktop --message-format=json-render-diagnostics`，同样报 71 个缺失 API 错误，返回 `command_failed: build native desktop failed`。 |

三个完整日志分别为 `/tmp/liliacode-xtask-verify.log`、`/tmp/liliacode-xtask-performance.log`、`/tmp/liliacode-xtask-agent-debug.log`。没有生成本轮性能数据、截图、交互观察或 secret-canary 证据，不能引用先前产物宣称本轮通过。完成 git lock 后，应用编译仍有上表通过直接 all-targets check 已实测的源码能力阻塞。首次仅传 CLI 配置的 lock 拒绝已由本次继承配置的正式重试取代，不再作为最终阻塞原因。

解锁后重试分别创建 `agent-debug-runs/lilia-performance-1788968511819` 与 `agent-debug-runs/lilia-agent-debug-1788968511816`，仅生成准备目录和 source.json；两次构建都失败于缺失 BrowserView、TimeSeriesLayer、DonutChart、clear_text_history 等能力，未启动桌面产品，没有可验收的性能或交互结果。该轮 `source.json` 的 `localPatch=false` 不能作为依赖来源证据：当时的 `xtask/src/agent_debug_source.rs::local_nana_path` 只读取仓库 `.cargo/config.toml`，未识别 CARGO_HOME 中的临时配置；该历史产物仍须以同期完整 Cargo metadata 判断来源。现已修复为读取当前环境的完整 `cargo metadata --locked --format-version 1`，从实际 resolved NanaUI package 的 source / manifest_path 识别本地路径并指纹其 workspace，缺失或多份 NanaUI 均拒绝；真实 Git → 仅 CARGO_HOME patch 的行为回归已通过（`restoration-xtask-source-tests.log`，8 项 source 测试通过）。最终真实 git pin 验收不沿用这些临时联合开发产物。首次锁屏阻塞已经解锁后的编译失败结果取代，不作为最终环境阻塞。

## 历史：第四轮恢复验收（当轮本地源码，未发布）

第四轮三组（960×600 浅/深、1440×900 浅色）完整通过实窗回放；深色宽窗组在第 52 张截图后再次锁屏，未完成最终收尾。235 张已产生的图片全部逐张检查，Memory 编辑/撤销/保存与真实独立任务窗口在前三组完成。全部组实际 scale 1，源码指纹均为 `9028d808f205a323ab49cd77320320c648a8dd371f9872c8eefc01fd1ac3e469`。

复审发现宽窗长消息复制按钮侵入下一行。根因是框架在最大宽度限制前测量子树高度；现修正自然宽度约束的应用顺序，保留原百分比、box-sizing 和滚动高度合同，没有添加业务间距或虚拟行高补丁。原有实际 TimelineContent 测试强化后修前失败、修后通过，83 项框架布局测试与非作者复审通过。最新完整 workspace 测试 980 通过、1 忽略，desktop 559 项全绿；workspace/all-targets check 通过（`restoration-code-workspace-{tests,check}-after-width-fix.log`）。

上述 235 张是行高修复和外观提示文案调整之前的图；最新源码仍待完整四组重拍、性能和最终发布来源门禁。macOS 当前再次锁定，不能把前三组通过算作四组或性能通过。原始产物与逐图记录持久保存于相邻 `nanaui-restoration-recovery-20260910/code-native-matrix4/` 与各 `matrix4-*-review/`。

## 最终本地验收：第六轮与性能

第五轮两个窄窗组共 122 张图完成，但第三组启动返回 `agent_debug_ready_timeout`，不计完整通过。逐图复审发现任务菜单背后的时间线为空；现使用框架保留式虚拟列表和同一 `VirtualListLayout` 管理范围、定位和内容高度，在实际呈现后批量回填已安装行的测量值。缓存按任务、业务 key、内容和实际宽度隔离；保留可见 key/inset 或明确尾锚，用户滚动优先，稳定反馈不请求重绘。主窗口尾滚动意图在成功同步相应任务后才交付，scroll-only 事件保留实测视口。原加载更早记录页脚保持在滚动区外。

实际负向回归还发现虚拟行销毁后遗留的已 park 可选子节点；TimelineContent 现在显式清理自己拥有的剩余实体，主时间线和任务弹窗均调用。普通离屏行仍沿用框架销毁合同，焦点/IME 等保留行维持身份，并非永久保留全部离屏组件。20 项定向回归、三轮实体数量稳定验证和非作者复审通过；最终 workspace 测试 986 通过、1 既有忽略，desktop 565/565，workspace/all-targets check、fmt、桌面构建通过。日志与负/正对照保存相邻 recovery 的 `timeline-measurement/`。

第六轮 `1789017421571`、`1789017487276`、`1789017540530`、`1789017591909` 对应 960×600 / 1440×900 × 浅/深色，四组均完成 61 张截图、`summary.ok=true`，全部 244 张已逐张复审；其中两张 MCP 宽窗图的部分可见问题已通过下述脚本修正和定向补拍修闭。源码构建前后均为 528 个文件、指纹 `38b10f6f4e3f8d6a6cb7e69e488e23ebf8c8a002594a4f7d96d2f84ee3d452a2`，截图均为实际 1×；独立任务窗为 430×760。原任务菜单空白、长气泡复制按钮越界、Memory 撤销/保存、Todo 裁剪、外观提示、原生浏览器与 MCP/审批回流在本轮范围内通过。完整产物为 `code-native-matrix6/`，逐图复核分为 `matrix6-visual-review/`、`matrix6-priority-review/` 与 `native-settings/matrix6-settings-review.json`。

最终内容离屏探针 20 张和图表 14 张已在宽度修正后重新生成、逐张复审，包括 2× 内容、真实长消息、指针调整 TextArea 与滚动图表工具提示。证据为 `content-after-width-fix/` 与 `charts/evidence-after-width-fix/`。它们与原生 1× 截图分别记录，不宣称原生 2× 验收。

MCP 补拍已完成：原脚本将命中目标部分露出判为已展开；现使用完整布局边界和实际 scroll viewport 验证包含，并按真实 Wheel 方向滚动。首个修正候选的滚轮符号错误已由原生负对照捕获并修正；不改变产品滚动或聚焦策略。`1789019155044`（宽浅）与 `1789019255885`（宽深）两组定向扩展回放均通过，开关、标签与完整焦点圈可见。两组共 14 张扩展图逐张复核；MCP 实际 UI 几何、截图、窗口/源码/产物身份与原始失败证据保存 `extensions-reveal-final/`。原 244 张的两张问题图保留为历史负证据，由同主题补拍修闭，不改写旧图。xtask 库 28 项通过、1 项原生探针默认忽略；定向滚轮行为测试及 all-targets 严格 Clippy 通过，忽略式原生探针随后显式执行并通过。

最终 `cargo xtask performance` schema 2 在无并发构建/GPU 测试时通过，主 run `lilia-performance-1789018767913`。五个隔离目录均使用相同 performance-v1 千条语料；语料 SHA、NanaUI 源码指纹、可执行文件和宿主库身份全部一致。启动指标为 spawn 到 debug ready（不含构建/语料准备/产物核验，不代表首帧或 OS 缓存冷启动），5 次 P95 2148.76 ms。每类 30 次操作从 handler 开始到主窗口 `window_frame_presented`，输入 P95 6.20 ms、面板缩放 P95 14.23 ms，均不含传输/排队时间，也不代表持续 FPS。千条时间线加载观察就绪 91.07 ms，不代表同时绘制千行。

CPU 按两次进程采样完成时刻的实际 1.01361175 秒和 10 核归一化；本次累计 CPU 计数均为 1.04 秒，报告 0%，仅表示短采样窗未超出计数精度。主进程 RSS 单次采样 203554816 字节（194.125 MiB），不含子进程/GPU，也不是峰值。5 次启动和 30+30 次原始帧耗时、采样边界和产物身份均持久保存；非作者从原数组重新计算 P95/CPU 与报告一致。既有门槛和环境变量保持，全部通过。实际 `performance.png` 已逐张查看。历史同平台语料不可获取，未作历史性能比较。完整新证据为相邻 recovery 的 `code-performance-schema2/`；旧口径报告保存 `code-performance-final/`，不替代新结果。


上述本地联调历史记录均使用显式本地 NanaUI 来源，当时验证后恢复四应用已发布 3ef 锁文件。它们先于 1100b02 发布，不能代替本文顶部记录的正式 Git 来源门禁。此前各轮失败仅用于追溯，不作为当前阻断或最终通过证据。
