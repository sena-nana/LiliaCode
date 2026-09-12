# Tauri 与 Native 主题源码基线

记录日期：2026-09-03。本文锁定源码和默认值，供还原与截图验收使用；不是像素等价通过声明。首次源码提取阶段只读取 Git / GitHub API，没有安装旧依赖或启动旧前端；后续实施与 Cargo 验证见 `tauri-native-restoration.md`。

## 1. 可复核来源

- 应用基线：LiliaCode `2eec58e` 的 `apps/desktop/package.json` 第 30–33 行与 `yarn.lock`；主题、UI、UI contract、UI foundation 均锁定 `sena-nana/LiliaUI@5478dab570e5685e753804e16e788e5ad33bc5d9`，不是跟随默认分支。
- 首次调查的框架基线：当时 `apps/desktop/Cargo.toml` 锁定 `sena-nana/NanaUI@b86ae1a275e8c20dac5535056733bfe2661ca6b0`，启用 `full`，包含 `bundled-fonts`。下文框架值取该 Git 对象，不取兄弟目录未提交代码。
- LiliaUI 历史对象不在本地 NanaUI Git 对象库；通过 `gh api repos/sena-nana/LiliaUI/contents/<path>?ref=<完整 SHA>` 读取成功，身份验证和网络未阻塞。

历史文件（永久 revision 链接）：

- [tokens](https://github.com/sena-nana/LiliaUI/blob/5478dab570e5685e753804e16e788e5ad33bc5d9/packages/theme/src/styles/tokens.css)
- [基础字体与控件](https://github.com/sena-nana/LiliaUI/blob/5478dab570e5685e753804e16e788e5ad33bc5d9/packages/theme/src/base.css)
- [字体资源](https://github.com/sena-nana/LiliaUI/blob/5478dab570e5685e753804e16e788e5ad33bc5d9/packages/theme/src/styles/fonts.css)
- [页面与卡片](https://github.com/sena-nana/LiliaUI/blob/5478dab570e5685e753804e16e788e5ad33bc5d9/packages/theme/src/styles/page.css)
- [侧栏](https://github.com/sena-nana/LiliaUI/blob/5478dab570e5685e753804e16e788e5ad33bc5d9/packages/theme/src/styles/sidebar.css)
- [标题栏](https://github.com/sena-nana/LiliaUI/blob/5478dab570e5685e753804e16e788e5ad33bc5d9/packages/theme/src/styles/app-shell.css)
- [平台圆角默认值](https://github.com/sena-nana/LiliaUI/blob/5478dab570e5685e753804e16e788e5ad33bc5d9/packages/ui/src/composables/useCornerStyle.ts)
- [启动安装器](https://github.com/sena-nana/LiliaUI/blob/5478dab570e5685e753804e16e788e5ad33bc5d9/packages/ui/src/runtime.ts)

首次调查的 NanaUI 对照对象：[语义调色板](https://github.com/sena-nana/NanaUI/blob/b86ae1a275e8c20dac5535056733bfe2661ca6b0/crates/nana-ui-core/src/style_model.rs)、[尺寸令牌](https://github.com/sena-nana/NanaUI/blob/b86ae1a275e8c20dac5535056733bfe2661ca6b0/crates/nana-ui-core/src/theme.rs)、[侧栏尺寸](https://github.com/sena-nana/NanaUI/blob/b86ae1a275e8c20dac5535056733bfe2661ca6b0/crates/nana-ui-runtime/src/sidebar.rs)、[字体资源](https://github.com/sena-nana/NanaUI/blob/b86ae1a275e8c20dac5535056733bfe2661ca6b0/crates/nana-ui/src/theme.rs)、[字体解析](https://github.com/sena-nana/NanaUI/blob/b86ae1a275e8c20dac5535056733bfe2661ca6b0/crates/nana-ui/src/nana_text.rs)。

## 2. 颜色：历史字面量与 Native 映射

历史颜色空间是 OKLCH；保留下表原始精度和 alpha。Native `SemanticPalette` 使用 sRGB 分量，最后一列是其源码 RGB8 的十六进制表示。不要把 OKLCH 百分数直接当 sRGB 分量，也不要凭截图吸色覆盖主题。

| 历史变量 | 历史深色原值 | 历史浅色原值 | Native 语义角色 | Native 深色 / 浅色 |
| --- | --- | --- | --- | --- |
| `--bg` | `oklch(20.9% 0 89.9)` | `oklch(100% 0 89.9)` | `Background` | `#181818` / `#FFFFFF` |
| `--bg-elev` | `oklch(24.35% 0 89.9)` | `oklch(96.7% 0.0029 264.5)` | `Surface` | `#202020` / `#F3F4F6` |
| `--bg-subtle` | `oklch(22.64% 0 89.9)` | `oklch(97.89% 0.0029 264.5)` | `Subtle` | `#1C1C1C` / `#F7F8FA` |
| `--bg-hover` | `oklch(29.72% 0 89.9)` | `oklch(94.54% 0.0046 258.3)` | `Hover` | `#2D2D2D` / `#EBEDF0` |
| `--bg-active` | `oklch(32.9% 0 89.9)` | `oklch(91.19% 0.0075 260.7)` | `Active` | `#353535` / `#DFE2E7` |
| `--lilia-color-state-selected-solid` | `oklch(32.9% 0 89.9)` | `oklch(91.19% 0 89.9)` | `Selected` | `#353535` / `#E2E2E2` |
| `--lilia-color-state-selected-hover-solid` | `oklch(35.5% 0 89.9)` | `oklch(93.2% 0 89.9)` | `SelectedHover` | `#3C3C3C` / `#E8E8E8` |
| `--lilia-color-state-selected-pressed-solid` | `oklch(30.5% 0 89.9)` | `oklch(90.5% 0 89.9)` | `SelectedPressed` | `#2F2F2F` / `#DFDFDF` |
| `--border` | `oklch(28.5% 0 89.9)` | `oklch(92.12% 0.0046 258.3)` | `Border` | `#2A2A2A` / `#E3E5E8` |
| `--border-soft` | `oklch(25.62% 0 89.9)` | `oklch(95.45% 0.0046 258.3)` | `BorderSoft` | `#232323` / `#EEF0F3` |
| `--border-strong` | `oklch(34.85% 0 89.9)` | `oklch(85.32% 0.0094 258.3)` | `BorderStrong` | `#3A3A3A` / `#CBCFD5` |
| `--text` | `oklch(89.75% 0 89.9)` | `oklch(22% 0.0097 285.6)` | `Text` | `#DDDDDD` / `#1A1A1F` |
| `--text-muted` | `oklch(71.5% 0 89.9)` | `oklch(49% 0.0234 264.4)` | `Muted` | `#A3A3A3` / `#5A616E` |
| `--text-faint` | `oklch(46.76% 0 89.9)` | `oklch(71.37% 0.0192 261.3)` | `Faint` | `#5A5A5A` / `#9CA3AF` |
| `--accent` | `oklch(76.49% 0.102 246.4)` | `oklch(64.18% 0.128 249.9)` | `Accent` | `#7BB9F0` / `#4991D7` |
| `--accent-strong` | `oklch(64.18% 0.128 249.9)` | `oklch(58.91% 0.1555 253.4)` | `AccentStrong` | `#4991D7` / `#2C7ED6` |
| `--accent-on-soft` | `oklch(76.49% 0.102 246.4)` | `oklch(45% 0.14 252)` | `AccentOnSoft` | `#7BB9F0` / `#00559F` |
| `--accent-soft` | `oklch(76.49% 0.102 246.4 / 0.14)` | `oklch(64.18% 0.128 249.9 / 0.1)` | `AccentSoft` | `#7BB9F0 / 0.14` / `#4991D7 / 0.1` |
| `--accent-text` | `oklch(19.78% 0.0278 255.9)` | `oklch(100% 0 89.9)` | `AccentText` | `#0D1622` / `#FFFFFF` |
| `--ok` | `oklch(69.51% 0.1809 145.6)` | `oklch(52% 0.1388 149.6)` | `Success` | `#3FB950` / `#107E39` |
| `--warn` | `oklch(75.61% 0.1084 79.9)` | `oklch(62.36% 0.1269 68.9)` | `Warning` | `#D4A85B` / `#B8771C` |
| `--err` | `oklch(70.56% 0.1614 20.7)` | `oklch(56.4% 0.178 24.9)` | `Danger` | `#F47174` / `#C93C3C` |

`--panel`、`--surface-raised`、`--bg-elevated` 都引用 `--bg-elev`，对应 Native `Surface`；但历史 `--surface` 引用的是 `--bg`，必须对应 Native `Background`。**同名 `surface` 不能机械映射为 `Surface`。** 历史输入器 `.chat-composer` 使用 `--bg-elev`，此处使用 Native `Surface` 才符合来源。

对表中 OKLCH 值按标准 OKLab→线性 sRGB→sRGB 曲线换算、裁剪并四舍五入到 8 位通道得到的检查结果如下。这仅比较令牌，不包含材质混合、色彩管理、抗锯齿或字体像素：

主表所有映射的 RGB 通道均与历史 OKLCH 四舍五入后的 sRGB8 相同；alpha 仍按上表各自原值核对。

其余状态与材质必须单独处理：

| 历史令牌 | 深色 | 浅色 | 当前框架差异 |
| --- | --- | --- | --- |
| 透明 hover / pressed / selected / selected-hover / selected-pressed | 白色 alpha `0.06 / 0.10 / 0.15 / 0.19 / 0.23` | 黑色相同 alpha | 不能直接用实色 `Hover` / `Selected` 替换原生材质上的叠加层 |
| `--ok-soft` | `oklch(69.51% 0.1809 145.6 / 0.14)` | `oklch(54.34% 0.1388 149.6 / 0.10)` | 无独立 `SuccessSoft` 角色；浅色原值亮度也不同于 `--ok` 的 52% |
| `--warn-soft` | `oklch(75.61% 0.1084 79.9 / 0.16)` | `oklch(62.36% 0.1269 68.9 / 0.12)` | `WarningSoft` 保留对应 alpha |
| `--err-soft` | `oklch(70.56% 0.1614 20.7 / 0.14)` | `oklch(56.4% 0.178 24.9 / 0.10)` | 无独立基础 `DangerSoft`，已有 hover / pressed 为 0.18 / 0.22 |
| `--err-solid` | `oklch(55.63% 0.1427 20.3)` | `oklch(52.5% 0.1669 25)` | 不能用正文 `Danger` 颜色冒充实心危险按钮背景 |
| `--err-solid-hover` | `oklch(59.34% 0.1435 19.8)` | `oklch(49.34% 0.1586 25.1)` | 需要对应实心危险交互状态；文字为纯白 |
| `--lilia-backdrop-opacity` / raised opacity / CSS blur | `0.64 / 0.82 / 24px` | 相同；tint 改用 `96.7% 0.0029 264.5` | 是材质配方，须与 NanaUI 平台材质请求、实际 outcome 一起验收 |
| `--scrim` | `oklch(0% 0 0 / 0.45)` | 继承深色原值 | 对话框遮罩和菜单阴影不能混为同一角色 |

历史阴影原值：

- 卡片 `--shadow-surface`：深色 `0 10px 30px -24px oklch(0% 0 0 / 0.62)`；浅色 `0 10px 26px -24px oklch(21.01% 0.0318 264.7 / 0.24)`。
- 对话框 `--shadow-dialog`：深色 `0 14px 40px oklch(0% 0 0 / 0.45)`；浅色 `0 14px 40px oklch(21.01% 0.0318 264.7 / 0.28)`。
- 菜单 `--shadow-menu`：深色 `0 10px 28px -10px oklch(0% 0 0 / 0.55)`；浅色 `0 10px 28px -14px oklch(21.01% 0.0318 264.7 / 0.30)`。
- 产品输入器另有 `0 4px 16px -8px rgba(0, 0, 0, 0.45)`，来自应用 `chat.css`，不是全局卡片阴影。

## 3. 字体与圆角：必须区分声明和运行默认值

历史 `base.css` 默认正文 `14px / 1.55`、字重 400；关闭字体合成，启用 `cv11`、`ss01`、`ss03`。侧栏行、设置标签等组件明确覆盖为 13px，不能把整页默认字号改成 13px 后宣称等价。Native `UI_BASE_TEXT_SIZE = 13`，组件内需按照实际历史选择器比较，行距也不可只比较字号。

历史 `--font-sans` 完整顺序：

```text
"Noto Sans SC", "Segoe UI Variable Text", "Segoe UI", "Microsoft YaHei UI",
"Microsoft YaHei", system-ui, -apple-system, BlinkMacSystemFont, "PingFang SC",
"Source Han Sans SC", Roboto, "Helvetica Neue", Arial, sans-serif,
"Segoe UI Emoji", "Apple Color Emoji", "Segoe UI Symbol"
```

历史 `--font-mono`：

```text
ui-monospace, "SF Mono", "Cascadia Mono", "Cascadia Code", "JetBrains Mono",
Menlo, Consolas, "Liberation Mono", monospace
```

`fonts.css` 声明 Noto Sans SC 400 / 500 / 600 / 700，分别加载 `/fonts/noto-sans-sc-chinese-simplified-<weight>-normal.woff2`。Native `full` 加载 Regular / Medium / SemiBold / Bold TTF 并把 sans-serif 默认设为 Noto Sans SC。家族和字重档位一致；文件封装、字形集合、fallback、字体特性与 shaping 不可由名称推定逐像素相同。历史代码字体还开启 `calt`、`ss01`、`ss02` 和 contextual ligatures，当前未找到逐项等价配置证据。

历史 CSS 静态基数是 16px，但 `useCornerStyle.ts` 规定 macOS 新用户默认 `round + 8px`，Windows / Linux 默认 `smooth + 16px`。应用 `mainBootstrap.ts` 启动时调用 `installCornerStyle()`，会把平台默认或已存偏好写入 `--app-corner-radius`。偏好允许 0–20px；截图基线必须记录该值。

| 圆角档位 | CSS 原始表达式 | macOS 无存量偏好 | Windows / Linux 无存量偏好 | Native `UI_METRICS` |
| --- | --- | --- | --- | --- |
| xs | 基数 × 0.5 | 4 | 8 | 2 |
| sm | 基数 × 0.75 | 6 | 12 | 6 |
| md | 基数 | 8 | 16 | 10 |
| lg | 基数 × 1.25 | 10 | 20 | 14 |
| pill | 999px | 999 | 999 | 产品 pill builder 为 999 |

源码提取时应用输入器固定 `COMPOSER_CARD_RADIUS = 16`，与历史 macOS 默认 md=8 不同；本轮应用还原已修为 8。不能据此把所有 Native 圆角整体替换为 8。应先固定历史 appearance fixture，再按组件的 xs / sm / md / lg 消费关系恢复。通用令牌调整属于 NanaUI；本文不修改框架或应用代码。

## 4. 间距与组件尺寸映射

| 部位 / 来源 | 历史默认逻辑像素 | Native 对应与差异 |
| --- | --- | --- |
| 标题栏；LiliaUI `app-shell.css` | 36 高 | 应用标题栏应保持 36，和系统窗口外框高度分开记录 |
| 主内容；应用 `shell.css` `.shell__main` | 上下 20、左右 24；背景 `--bg` | 属于应用布局，使用 Stack padding；不能用框架 panel 默认 14/16 替代 |
| 侧栏框；LiliaUI `sidebar.css` | 上右下左 `10 / 8 / 10 / 12`，gap 14 | pin `SidebarFrame` 四边和 gap 相同 |
| 侧栏行；LiliaUI `.sb-tree__row` | 高 28，左右 padding 10，gap 6；13px/500；md 圆角 | `UI_METRICS.navigation_row_height=28`；pin SidebarRow 左右 padding 8，圆角 sm=6，存在 padding 和圆角消费差异 |
| 普通卡片；LiliaUI `page.css` | padding 上下14/左右16，margin-bottom12，md 圆角；默认无边框 | `UI_METRICS.panel_padding_y=14/x=16`；Outlined 才应有 1px border，不能给所有卡片强加描边 |
| 页面标题；LiliaUI `page.css` | h1 18px/600；标题区 gap12，margin-bottom16 | 应用标题 Text 需按该层级校准 |
| 普通控件；LiliaUI `base.css` | 高32，padding6/10，sm 圆角；按钮 gap6、字重500 | Native control_height=32、control_padding_x=10；field_padding=6/9，左右相差1 |
| Segmented；LiliaUI `page.css` | 容器高34，padding2/gap2；子项高28、左右12 | Native compact_control_height=28、selection_padding_x=12；容器需单独比较 |
| 输入器；应用 `chat.css` `.chat-composer` | padding10/gap7、1px border、md圆角、raised背景 | 当前 `composer_card()` padding10/gap7/1px/Surface 对应；macOS圆角已从16修为8，见上节 |
| 输入正文；应用 `chat.css` | textarea min30/max74，padding4/6；rich-input max92 | 当前默认 TextArea 最大72、field padding6/9 与历史不同；富引用布局不能仅按 textarea 对齐 |
| 输入区域；应用 `.chat-controls` | `min(100%,860px)` | 当前 `CHAT_CONTENT_MAX_WIDTH=860` 对应 |
| 时间线；应用 `.agent-timeline` | `min(100%,760px+28px)`，右 padding28；line-height1.45 | 788 是外宽，内容仍需扣除右轨道28；不能把788当纯正文宽 |
| 发送 / 优化；应用 `chat.css` | 30×30、pill圆角 | 当前 `COMPOSER_SEND_SIZE=30` 对应；历史发送背景AccentSoft/文字Accent，需同时比较ButtonKind映射 |
| 设置窄屏；LiliaUI `page.css` | 900px以下label/control纵排，正文输入宽360，range宽274 | 当前 Native 应按窗口逻辑宽度恢复折行，不能把所有控件永久纵排 |

Native 其他明确令牌：compact padding-x7、icon-button28、list-item padding-x9/y6、motion fast120ms / standard240ms。历史普通控件过渡120ms、侧栏transform240ms与此一致；具体缓动曲线、显示隐藏状态仍须逐控件核对。

## 5. 使用方式与验收边界

1. 固定 macOS 平台、深/浅主题、逻辑窗口尺寸、scale factor、corner style/radius、backdrop mode/target 和是否有保存偏好；先记录这些条件再比较图片。
2. 颜色从历史 OKLCH 原值与 NanaUI semantic role 映射开始，处理 `--surface` 名称冲突和状态透明度；不把独立状态色映射成普通正文颜色。
3. 字体先核对 Noto 字重与fallback，再比较实际文本边界、行高与选区；标题/控件13px和正文14px分开验收。
4. 圆角、文本、边距修改应按控件消费关系进入 NanaUI 或应用布局，不为单张截图添加散落裸色。
5. 本次确认源码可取、数据可追溯。macOS同版本、同数据历史运行截图仍未取得；以上源码映射不替代最终历史像素签署。


## 后续基准追踪

应用正式NanaUI基准已随并行工作更新为 `d0f3ccf705e59f30f05642ad761d4d4c08cab27a`。
本轮还原候选基于该对象，补丁SHA-256为 `07a485ba32da5d6730e6bccf6c22bda2aaac3a6924752ed8c107ae149b06918d`。
上文永久链接保留首次调查对象，避免把历史提取值误称为每次工作区快照。
已核对b86→d0的 `style_model.rs` 与 `theme.rs` 核心令牌没有变更；
框架sidebar与字体代码存在后续改动，因此实际控件及塑形验收以固定候选的真实产物为准。
