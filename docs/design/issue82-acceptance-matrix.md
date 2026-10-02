# Issue82 桌面收尾验收矩阵

这份矩阵是 Issue82 的验收入口。`implemented` 表示代码路径已接入，`behavior-tested` 表示有领域或应用行为测试，`native-verified` 只在 `agent-debug` 或真实窗口证据存在时使用。未达到最后一种状态的条目不能写成桌面能力已交付。

| ID | 可见入口／业务命令 | 代表 target ID／命令 | 权威状态 | 关键异常与隔离条件 | 证据 |
| --- | --- | --- | --- | --- | --- |
| M-01 | 启动、项目打开、单实例转发 | `lilia.project.*`, CLI `open`/`handoff` | Product project/session | 无效目录、重复启动、旧窗口身份 | `agent-debug-runs/lilia-*/observe.json` |
| M-02 | 数据设置：预览、显式导入、重置 | Settings `data`, import plan/execute | Import plan/report、Product DB | 旧项目/任务/历史/配置、旧 schema、WAL/SHM、目标冲突、失败回滚、重复执行 | import tests + `agent-debug` |
| M-03 | 任务、对话、时间线、审批、草稿 | `lilia.task-session.*`, `timeline.load-earlier` | Product projection + Agent session | 任务切换不串状态、长时间线分页、晚到事件、重启恢复 | desktop behavior tests + native replay |
| M-04 | 主题、设置、窄窗口、DPI | `settings.*`, viewport matrix | Window/session state | 明暗主题、960×600、1440×900、窄窗口、高 DPI、返回焦点 | screenshot matrix |
| M-05 | 管理页面与 UiModule 生命周期 | WindowRoute、module message | WindowRoute/module projection | 主窗与任务窗隔离、离页清理、事件缺口重读 | module tests + native replay |
| M-06 | 文件、编辑器、多视图、冲突 | document/resource commands | Document/resource revision | 同文档多视图、未保存内容、外部变更、过期语言服务结果 | editor behavior tests + Windows replay |
| M-07 | 终端与 PTY | terminal session/input/resize | Terminal session/output | 多终端隔离、resize、复制粘贴、中断、退出、大输出边界 | terminal tests + Windows replay |
| M-08 | 浏览器与 Agent 操作 | browser observe/type/click/scroll/screenshot | Browser tab/page version | 页面版本失效、接管/恢复、审批拒绝、取消、文件/新窗口、DPI/IME | browser profile + Windows replay |
| M-09 | 性能 corpus | `cargo xtask performance` | Native/WGPU process | 冷启动、输入帧、调整尺寸、千条时间线、CPU、RSS | `performance.json` |

## 证据规则

- 测试通过只证明对应行为，不替代真实窗口绘制、焦点、DPI 或系统权限验收。
- 截图必须记录逻辑尺寸、像素尺寸、缩放因子、主题、源码 revision 和实际依赖指纹。
- 性能报告同时记录绝对门禁和历史基线；基线平台、依赖或测量契约不匹配时必须标记 `not_comparable`，不能把绝对门禁通过解释成无回退。
- #77–#81 的剩余工作在此矩阵中显式追踪，但不因矩阵存在而自动关闭对应 Issue。
