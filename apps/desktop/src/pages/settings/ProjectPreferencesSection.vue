<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref } from "vue";
import {
  AlertTriangle,
  CheckCircle2,
  Copy,
  FolderOpen,
  FolderTree,
  Github,
  Link2,
  LoaderCircle,
  Unplug,
} from "lucide-vue-next";
import type {
  GitHubBindingStatus,
  GitHubDeviceFlowStart,
  ProjectSettings,
} from "@lilia/contracts";
import {
  getGitHubBindingStatus,
  getProjectSettings,
  openUrl,
  pickFolder,
  pollGitHubDeviceFlow,
  setProjectSettings,
  startGitHubDeviceFlow,
  unbindGitHub,
} from "../../services/projects";

const projectSettings = ref<ProjectSettings>({ cloneParentDir: null });
const projectError = ref<string | null>(null);
const savingProject = ref(false);

const bindingStatus = ref<GitHubBindingStatus | null>(null);
const bindingBusy = ref(false);
const bindingError = ref<string | null>(null);
const deviceFlow = ref<GitHubDeviceFlowStart | null>(null);
const copiedCode = ref(false);
let pollTimer: ReturnType<typeof setTimeout> | null = null;

const isBound = computed(() => bindingStatus.value?.state === "bound");
const boundLogin = computed(() => bindingStatus.value?.binding?.login ?? "");
const boundScopes = computed(() => bindingStatus.value?.binding?.scopes ?? []);
const deviceCodeExpired = computed(() =>
  deviceFlow.value ? Date.now() >= deviceFlow.value.expiresAt : false,
);

function clearPollTimer() {
  if (pollTimer) {
    clearTimeout(pollTimer);
    pollTimer = null;
  }
}

async function loadProjectSettings() {
  try {
    projectSettings.value = await getProjectSettings();
  } catch (err) {
    projectError.value = `读取项目偏好失败：${String(err)}`;
  }
}

async function loadBindingStatus() {
  try {
    bindingStatus.value = await getGitHubBindingStatus();
  } catch (err) {
    bindingError.value = `读取 GitHub 绑定状态失败：${String(err)}`;
  }
}

async function persistProjectSettings() {
  savingProject.value = true;
  try {
    await setProjectSettings(projectSettings.value);
  } catch (err) {
    projectError.value = `保存项目偏好失败：${String(err)}`;
  } finally {
    savingProject.value = false;
  }
}

async function pickCloneParent() {
  projectError.value = null;
  try {
    const picked = await pickFolder({
      title: "选择默认 clone 父目录",
      defaultPath: projectSettings.value.cloneParentDir,
    });
    if (!picked) return;
    projectSettings.value = { ...projectSettings.value, cloneParentDir: picked };
    await persistProjectSettings();
  } catch (err) {
    projectError.value = `选择文件夹失败：${String(err)}`;
  }
}

async function schedulePoll(intervalSeconds: number) {
  clearPollTimer();
  pollTimer = setTimeout(() => {
    void continuePolling(intervalSeconds);
  }, Math.max(intervalSeconds, 1) * 1000);
}

async function continuePolling(intervalSeconds?: number) {
  if (!deviceFlow.value) return;
  if (deviceCodeExpired.value) {
    bindingBusy.value = false;
    bindingError.value = "GitHub 设备码已过期，请重新开始绑定。";
    deviceFlow.value = null;
    clearPollTimer();
    return;
  }

  try {
    const result = await pollGitHubDeviceFlow(
      deviceFlow.value.deviceCode,
      intervalSeconds ?? deviceFlow.value.intervalSeconds,
    );
    if (result.status === "authorized") {
      bindingStatus.value = result.bindingStatus;
      bindingBusy.value = false;
      bindingError.value = null;
      deviceFlow.value = null;
      copiedCode.value = false;
      clearPollTimer();
      await loadProjectSettings();
      return;
    }
    if (result.status === "expired") {
      bindingBusy.value = false;
      bindingError.value = result.error
        ? `GitHub 绑定失败：${result.error}`
        : "GitHub 设备码已过期，请重新开始绑定。";
      deviceFlow.value = null;
      clearPollTimer();
      return;
    }
    await schedulePoll(result.intervalSeconds);
  } catch (err) {
    bindingBusy.value = false;
    bindingError.value = `轮询 GitHub 授权失败：${String(err)}`;
    clearPollTimer();
  }
}

async function startBinding() {
  bindingError.value = null;
  copiedCode.value = false;
  bindingBusy.value = true;
  clearPollTimer();
  try {
    deviceFlow.value = await startGitHubDeviceFlow();
    await schedulePoll(deviceFlow.value.intervalSeconds);
  } catch (err) {
    bindingBusy.value = false;
    bindingError.value = `启动 GitHub 绑定失败：${String(err)}`;
  }
}

async function copyCodeAndOpenBrowser() {
  if (!deviceFlow.value) return;
  bindingError.value = null;
  try {
    await navigator.clipboard.writeText(deviceFlow.value.userCode);
    copiedCode.value = true;
    await openUrl(deviceFlow.value.verificationUri);
  } catch (err) {
    bindingError.value = `复制设备码或打开浏览器失败：${String(err)}`;
  }
}

async function handleUnbind() {
  bindingBusy.value = true;
  bindingError.value = null;
  clearPollTimer();
  try {
    await unbindGitHub();
    bindingStatus.value = await getGitHubBindingStatus();
    deviceFlow.value = null;
    copiedCode.value = false;
    await loadProjectSettings();
  } catch (err) {
    bindingError.value = `解绑 GitHub 失败：${String(err)}`;
  } finally {
    bindingBusy.value = false;
  }
}

onMounted(async () => {
  await Promise.all([loadProjectSettings(), loadBindingStatus()]);
});

onBeforeUnmount(() => {
  clearPollTimer();
});
</script>

<template>
  <div class="card">
    <h2>
      <span class="card-h2__title">
        <FolderTree :size="14" aria-hidden="true" />
        项目
      </span>
    </h2>

    <div class="settings-row">
      <div class="settings-row__label">Clone 默认父目录</div>
      <div style="display: flex; gap: 8px; align-items: center; flex-wrap: wrap; justify-content: flex-end;">
        <span class="muted" style="max-width: 320px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap;">
          {{ projectSettings.cloneParentDir || "未设置（用家目录）" }}
        </span>
        <button type="button" class="ghost" :disabled="savingProject" @click="pickCloneParent">
          <FolderOpen :size="12" aria-hidden="true" />
          选择
        </button>
      </div>
    </div>

    <div class="settings-row" style="align-items: flex-start;">
      <div class="settings-row__label">
        <div style="display: inline-flex; align-items: center; gap: 6px;">
          <Github :size="14" aria-hidden="true" />
          GitHub 绑定
        </div>
      </div>
      <div style="display: flex; flex-direction: column; align-items: flex-end; gap: 8px; min-width: min(420px, 100%);">
        <div v-if="isBound && bindingStatus?.binding" class="conn-banner conn-banner--ok" style="margin: 0; width: min(420px, 100%);">
          <CheckCircle2 :size="16" aria-hidden="true" />
          <div style="min-width: 0;">
            <div class="conn-banner__title">已绑定 GitHub</div>
            <div class="conn-banner__hint">
              当前账号：{{ boundLogin }}
              <template v-if="boundScopes.length"> · 权限：{{ boundScopes.join("、") }}</template>
            </div>
          </div>
        </div>

        <div v-else-if="deviceFlow" class="conn-banner conn-banner--probing" style="margin: 0; width: min(420px, 100%);">
          <LoaderCircle :size="16" aria-hidden="true" :class="{ 'is-spinning': bindingBusy }" />
          <div style="min-width: 0;">
            <div class="conn-banner__title">等待 GitHub 授权</div>
            <div class="conn-banner__hint">
              设备码：<code>{{ deviceFlow.userCode }}</code>
              <span v-if="copiedCode"> · 已复制</span>
              <span v-if="deviceCodeExpired"> · 已过期</span>
            </div>
          </div>
        </div>

        <div v-else class="muted" style="font-size: 12px;">
          未绑定 GitHub
        </div>

        <div style="display: flex; gap: 8px; flex-wrap: wrap; justify-content: flex-end;">
          <button
            v-if="!isBound"
            type="button"
            class="primary"
            :disabled="bindingBusy"
            @click="startBinding"
          >
            <LoaderCircle v-if="bindingBusy && !deviceFlow" :size="12" aria-hidden="true" class="is-spinning" />
            <Github v-else :size="12" aria-hidden="true" />
            {{ deviceFlow ? "重新开始绑定" : "绑定 GitHub" }}
          </button>

          <button
            v-if="deviceFlow"
            type="button"
            class="ghost"
            :disabled="bindingBusy && !deviceFlow"
            @click="copyCodeAndOpenBrowser"
          >
            <Copy :size="12" aria-hidden="true" />
            复制设备码并打开浏览器
          </button>

          <button
            v-if="deviceFlow"
            type="button"
            class="ghost"
            :disabled="bindingBusy && !deviceFlow"
            @click="openUrl(deviceFlow.verificationUri)"
          >
            <Link2 :size="12" aria-hidden="true" />
            仅打开 GitHub
          </button>

          <button
            v-if="isBound"
            type="button"
            class="ghost danger"
            :disabled="bindingBusy"
            @click="handleUnbind"
          >
            <Unplug :size="12" aria-hidden="true" />
            解绑
          </button>
        </div>
      </div>
    </div>

    <div v-if="bindingError" class="conn-banner conn-banner--err">
      <AlertTriangle :size="16" aria-hidden="true" />
      <div>
        <div class="conn-banner__title">GitHub 绑定</div>
        <div class="conn-banner__hint">{{ bindingError }}</div>
      </div>
    </div>

    <div v-if="projectError" class="conn-banner conn-banner--err">
      <AlertTriangle :size="16" aria-hidden="true" />
      <div>
        <div class="conn-banner__title">项目偏好</div>
        <div class="conn-banner__hint">{{ projectError }}</div>
      </div>
    </div>
  </div>
</template>
