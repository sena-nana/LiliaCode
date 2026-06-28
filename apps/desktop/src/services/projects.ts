/**
 * 项目相关服务：包装「添加项目」入口需要的所有 Tauri command + 系统对话框。
 *
 * `pickFolder` 直接 invoke Tauri dialog 插件命令而非走 @tauri-apps/plugin-dialog wrapper，
 * 保持与 Tauri 端权限申明一致的最小依赖面。
 */
import { invoke } from "../tauri/runtime";
import type {
  GitHubBindingStatus,
  GitHubDeviceFlowPollResult,
  GitHubDeviceFlowStart,
  GitHubRepoPage,
  GitHubRepoSummary,
  ProjectSettings,
} from "@lilia/contracts";
import { TAURI_PLUGIN_DIALOG_OPEN_COMMAND } from "../tauri/pluginCommands";
import {
  GIT_CLONE_REPO_COMMAND,
  GITHUB_CLONE_REPO_COMMAND,
  GITHUB_GET_BINDING_STATUS_COMMAND,
  GITHUB_LIST_REPOS_COMMAND,
  GITHUB_POLL_DEVICE_FLOW_COMMAND,
  GITHUB_START_DEVICE_FLOW_COMMAND,
  GITHUB_UNBIND_COMMAND,
  PROJECT_GET_SETTINGS_COMMAND,
  PROJECT_SET_SETTINGS_COMMAND,
  SYSTEM_OPEN_IN_VSCODE_COMMAND,
  SYSTEM_OPEN_PATH_COMMAND,
  SYSTEM_OPEN_URL_COMMAND,
} from "@lilia/contracts";

export type {
  GitHubBindingStatus,
  GitHubDeviceFlowPollResult,
  GitHubDeviceFlowStart,
  GitHubRepoPage,
  GitHubRepoSummary,
  ProjectSettings,
};

const GITHUB_REPO_CACHE_TTL_MS = 5 * 60 * 1000;

let githubRepoCache: {
  items: GitHubRepoSummary[];
  nextPage: number | null;
  fetchedAt: number;
} | null = null;
let githubRepoPreloadPromise: Promise<GitHubRepoPage> | null = null;
let githubRepoCacheGeneration = 0;
let githubRepoFirstPageRequestId = 0;

function cloneRepoPage(page: GitHubRepoPage): GitHubRepoPage {
  return {
    items: page.items.map((repo) => ({ ...repo })),
    nextPage: page.nextPage,
  };
}

function writeGitHubRepoCache(page: GitHubRepoPage) {
  githubRepoCache = {
    items: page.items.map((repo) => ({ ...repo })),
    nextPage: page.nextPage,
    fetchedAt: Date.now(),
  };
}

export function readCachedGitHubRepos(): GitHubRepoPage | null {
  if (!githubRepoCache) return null;
  return cloneRepoPage(githubRepoCache);
}

export function clearGitHubRepoCache() {
  githubRepoCacheGeneration += 1;
  githubRepoCache = null;
  githubRepoPreloadPromise = null;
}

export function preloadGitHubRepos(opts: { force?: boolean } = {}): Promise<GitHubRepoPage> {
  const now = Date.now();
  if (
    !opts.force &&
    githubRepoCache &&
    now - githubRepoCache.fetchedAt < GITHUB_REPO_CACHE_TTL_MS
  ) {
    return Promise.resolve(cloneRepoPage(githubRepoCache));
  }
  if (!opts.force && githubRepoPreloadPromise) return githubRepoPreloadPromise;
  githubRepoPreloadPromise = listGitHubRepos(1).finally(() => {
    githubRepoPreloadPromise = null;
  });
  return githubRepoPreloadPromise;
}

function preloadGitHubReposSilently() {
  void preloadGitHubRepos().catch(() => undefined);
}

interface DialogOpenOptions {
  directory?: boolean;
  multiple?: boolean;
  title?: string;
  defaultPath?: string;
}

/**
 * 弹系统文件夹选择器。用户取消时返回 null。
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
    TAURI_PLUGIN_DIALOG_OPEN_COMMAND,
    { options },
  );
  if (!picked) return null;
  return Array.isArray(picked) ? (picked[0] ?? null) : picked;
}

/** `git clone <url> <parentDir>/<derived-name>`，成功后返回克隆出的绝对路径。 */
export function gitCloneRepo(url: string, parentDir: string): Promise<string> {
  return invoke<string>(GIT_CLONE_REPO_COMMAND, { url, parentDir });
}

export function gitHubCloneRepo(
  repo: string,
  parentDir: string,
): Promise<string> {
  return invoke<string>(GITHUB_CLONE_REPO_COMMAND, { repo, parentDir });
}

export function isGitHubBindingExpiredError(err: unknown): boolean {
  const message = String(err);
  return message.includes("GitHub 绑定已失效") ||
    message.includes("HTTP 401") ||
    message.includes("HTTP 403") ||
    message.toLowerCase().includes("bad credentials");
}

export function getProjectSettings(): Promise<ProjectSettings> {
  return invoke<ProjectSettings>(PROJECT_GET_SETTINGS_COMMAND);
}

export function setProjectSettings(settings: ProjectSettings): Promise<void> {
  return invoke<void>(PROJECT_SET_SETTINGS_COMMAND, { settings });
}

export function openInFileManager(path: string): Promise<void> {
  return invoke<void>(SYSTEM_OPEN_PATH_COMMAND, { path });
}

export function openUrl(url: string): Promise<void> {
  return invoke<void>(SYSTEM_OPEN_URL_COMMAND, { url });
}

/** PATH 里没 `code` 时 Rust 端会返回错误。 */
export function openInVSCode(path: string): Promise<void> {
  return invoke<void>(SYSTEM_OPEN_IN_VSCODE_COMMAND, { path });
}

export async function getGitHubBindingStatus(): Promise<GitHubBindingStatus> {
  const status = await invoke<GitHubBindingStatus>(GITHUB_GET_BINDING_STATUS_COMMAND);
  if (status.state === "bound") {
    preloadGitHubReposSilently();
  } else {
    clearGitHubRepoCache();
  }
  return status;
}

export function startGitHubDeviceFlow(): Promise<GitHubDeviceFlowStart> {
  return invoke<GitHubDeviceFlowStart>(GITHUB_START_DEVICE_FLOW_COMMAND);
}

export function pollGitHubDeviceFlow(
  deviceCode: string,
  intervalSeconds?: number | null,
): Promise<GitHubDeviceFlowPollResult> {
  return invoke<GitHubDeviceFlowPollResult>(GITHUB_POLL_DEVICE_FLOW_COMMAND, {
    deviceCode,
    intervalSeconds: intervalSeconds ?? null,
  });
}

export function unbindGitHub(): Promise<void> {
  return invoke<void>(GITHUB_UNBIND_COMMAND).then(() => {
    clearGitHubRepoCache();
  });
}

export async function listGitHubRepos(page?: number | null): Promise<GitHubRepoPage> {
  const pageNo = page ?? null;
  const generation = githubRepoCacheGeneration;
  const firstPageRequestId = (pageNo ?? 1) === 1
    ? ++githubRepoFirstPageRequestId
    : githubRepoFirstPageRequestId;
  const result = await invoke<GitHubRepoPage>(GITHUB_LIST_REPOS_COMMAND, { page: pageNo })
    .catch((err) => {
      if (isGitHubBindingExpiredError(err)) {
        clearGitHubRepoCache();
      }
      throw err;
    });
  if (
    (pageNo ?? 1) === 1 &&
    generation === githubRepoCacheGeneration &&
    firstPageRequestId === githubRepoFirstPageRequestId
  ) {
    writeGitHubRepoCache(result);
  }
  return cloneRepoPage(result);
}

