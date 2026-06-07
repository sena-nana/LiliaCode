/**
 * 项目相关服务：包装「添加项目」入口需要的所有 Tauri command + 系统对话框。
 *
 * `pickFolder` 直接 invoke `plugin:dialog|open` 而非走 @tauri-apps/plugin-dialog wrapper，
 * 保持与 Tauri 端权限申明一致的最小依赖面。
 */
import { invoke } from "@tauri-apps/api/core";
import type {
  GitHubBindingStatus,
  GitHubDeviceFlowPollResult,
  GitHubDeviceFlowStart,
  GitHubRepoPage,
  GitHubRepoSummary,
  ProjectSettings,
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

export function gitHubCloneRepo(
  repo: string,
  parentDir: string,
): Promise<string> {
  return invoke<string>("github_clone_repo", { repo, parentDir });
}

export function isGitHubBindingExpiredError(err: unknown): boolean {
  const message = String(err);
  return message.includes("GitHub 绑定已失效") ||
    message.includes("HTTP 401") ||
    message.includes("HTTP 403") ||
    message.toLowerCase().includes("bad credentials");
}

export function getProjectSettings(): Promise<ProjectSettings> {
  return invoke<ProjectSettings>("project_get_settings");
}

export function setProjectSettings(settings: ProjectSettings): Promise<void> {
  return invoke<void>("project_set_settings", { settings });
}

export function openInFileManager(path: string): Promise<void> {
  return invoke<void>("system_open_path", { path });
}

export function openUrl(url: string): Promise<void> {
  return invoke<void>("system_open_url", { url });
}

/** PATH 里没 `code` 时 Rust 端会返回错误。 */
export function openInVSCode(path: string): Promise<void> {
  return invoke<void>("system_open_in_vscode", { path });
}

export async function getGitHubBindingStatus(): Promise<GitHubBindingStatus> {
  const status = await invoke<GitHubBindingStatus>("github_get_binding_status");
  if (status.state === "bound") {
    preloadGitHubReposSilently();
  } else {
    clearGitHubRepoCache();
  }
  return status;
}

export function startGitHubDeviceFlow(): Promise<GitHubDeviceFlowStart> {
  return invoke<GitHubDeviceFlowStart>("github_start_device_flow");
}

export function pollGitHubDeviceFlow(
  deviceCode: string,
  intervalSeconds?: number | null,
): Promise<GitHubDeviceFlowPollResult> {
  return invoke<GitHubDeviceFlowPollResult>("github_poll_device_flow", {
    deviceCode,
    intervalSeconds: intervalSeconds ?? null,
  });
}

export function unbindGitHub(): Promise<void> {
  return invoke<void>("github_unbind").then(() => {
    clearGitHubRepoCache();
  });
}

export async function listGitHubRepos(page?: number | null): Promise<GitHubRepoPage> {
  const pageNo = page ?? null;
  const generation = githubRepoCacheGeneration;
  const firstPageRequestId = (pageNo ?? 1) === 1
    ? ++githubRepoFirstPageRequestId
    : githubRepoFirstPageRequestId;
  const result = await invoke<GitHubRepoPage>("github_list_repos", { page: pageNo })
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
