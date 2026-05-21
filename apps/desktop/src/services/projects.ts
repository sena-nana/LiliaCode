/**
 * 项目相关服务：包装「添加项目」入口需要的所有 Tauri command + 系统对话框。
 *
 * `pickFolder` 直接 invoke `plugin:dialog|open` 而非走 @tauri-apps/plugin-dialog wrapper——
 * wrapper 因 workspace 内 @openai/codex-sdk 版本错位无法 yarn 装，但 invoke 频道仍可达。
 */
import { invoke } from "@tauri-apps/api/core";
import type { ProjectSettings } from "@lilia/contracts";

export type { ProjectSettings };

interface DialogOpenOptions {
  directory?: boolean;
  multiple?: boolean;
  title?: string;
  defaultPath?: string;
}

/**
 * 弹系统文件夹选择器。用户取消时返回 null；不允许多选，返回 `string | null`。
 */
export async function pickFolder(opts: {
  title?: string;
  defaultPath?: string | null;
} = {}): Promise<string | null> {
  const options: DialogOpenOptions = {
    directory: true,
    multiple: false,
    title: opts.title,
  };
  if (opts.defaultPath) options.defaultPath = opts.defaultPath;
  const picked = await invoke<string | string[] | null>(
    "plugin:dialog|open",
    { options },
  );
  if (!picked) return null;
  return Array.isArray(picked) ? (picked[0] ?? null) : picked;
}

/** `git clone <url> <parentDir>/<derived-name>`，成功后返回克隆出的绝对路径。 */
export function gitCloneRepo(url: string, parentDir: string): Promise<string> {
  return invoke<string>("git_clone_repo", { url, parentDir });
}

export function getProjectSettings(): Promise<ProjectSettings> {
  return invoke<ProjectSettings>("project_get_settings");
}

export function setProjectSettings(settings: ProjectSettings): Promise<void> {
  return invoke<void>("project_set_settings", { settings });
}
