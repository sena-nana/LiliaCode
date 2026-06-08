# Lilia 样式标准

## 入口分层

- `apps/desktop/src/styles/index.css`：字体、主题令牌、reset、全局排版、滚动条和低层工具类。
- `apps/desktop/src/styles/components.css`：标准组件类，包含 `.ui-button`、`.ui-input`、`.ui-segmented`、`.ui-tabs`、`.ui-list`、`.ui-badge`、`.ui-pill`、`.ui-card` 等。
- `apps/desktop/src/styles/shell.css`：首屏应用壳层、标题栏、侧栏和全局浮层壳。
- `apps/desktop/src/styles/chat.css`：聊天页、timeline、composer、markdown、pending action 和 todo float。
- `apps/desktop/src/styles/pages/*.css`：路由级页面样式，只在对应页面或路由组件中导入。

## 标准类

- 按钮使用 `.ui-button`，主操作加 `.ui-button--primary`，普通操作加 `.ui-button--ghost`，危险操作加 `.ui-button--danger`，纯图标按钮加 `.ui-icon-button`；按钮不使用边框区分类型，普通按钮常态透明背景，主按钮使用低饱和蓝色，危险按钮使用低饱和红色。
- 输入使用 `.ui-input`，多行输入再加 `.ui-textarea`。
- 分段控制使用 `.ui-segmented`，直接子 `button` 由标准样式接管。
- Tab 使用 `.ui-tabs` 和 `.ui-tabs__tab`。
- 列表容器使用 `.ui-list`，可点击或可选行使用 `.ui-list-item`。
- 紧凑状态使用 `.ui-badge`，数量或短状态可用 `.ui-pill`；语义色使用标准 modifier。
- 页面 BEM 类只负责布局、区域命名和少量页面专属尺寸，不重新定义按钮、输入、tab、badge、列表项的基础外观；页面按钮扩展不得用边框作为类型或状态区分。

## 扩展规则

- 新界面先组合标准类；标准类无法表达时，才在页面 CSS 中扩展。
- 扩展必须从 `DESIGN.md` 的令牌派生，不硬编码页面专属颜色。
- 不在 SFC 内新增 scoped 组件样式，除非样式确实只属于单个组件且无法成为标准类或页面样式。
- 可选页面样式在对应异步路由或页面组件中导入，避免回到单一全局 CSS。

## 评审清单

- 控件是否使用 `.ui-*` 标准类作为基础。
- 深浅主题是否只依赖共享令牌。
- 页面样式是否只承担布局和页面语义。
- 新增 CSS 是否放在正确入口，聊天和路由页面样式是否按需加载。
- 按钮、输入、tab、badge、列表项、菜单和弹层的 hover、active、disabled、focus 状态是否与现有标准一致。
