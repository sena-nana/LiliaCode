//! Lilia · 插件 / 技能管理。
//!
//! 桥接两套不一样的扩展机制：
//!
//! - **Claude Code**
//!   - Skills：`<root>/.claude/skills/<name>/SKILL.md`，frontmatter 至少有 `name` 和 `description`。
//!     Lilia 额外承认自定义字段 `disabled: true` 表示关闭——Claude 官方不读这个字段，
//!     但 Lilia 在启动 agent 子进程时按它决定本轮传给 SDK 的 skills 列表。
//!   - Plugins (marketplace beta)：`<root>/.claude/plugins/<name>/plugin.json`，用
//!     `disabled: true` 控制是否传给 SDK 的 plugins 列表。
//!   - 外部 MCP：Lilia 自管 `<LILIA_HOME>/config/claude-mcp-servers.json`，
//!     当前只支持用户级 stdio server。
//! - **Codex**：`~/.codex/config.toml` 的 `[mcp_servers.<name>]` 节，一期做只读 + 「打开配置」。
//!
//! 文件解析全部手写极小子集（避免引入 yaml / toml 依赖）。解析失败按「跳过 + 记 warning」
//! 处理，不让单个坏文件阻塞整个面板。

mod claude_mcp;
mod claude_plugins;
mod claude_skills;
mod codex_mcp;
mod commands;
mod paths;
mod runtime;
mod types;

pub use commands::*;
pub(crate) use runtime::runtime_extensions;
