<script setup lang="ts">
import { onMounted, ref } from "vue";
import { AlertTriangle, FolderOpen, FolderTree } from "lucide-vue-next";
import type { ProjectSettings } from "@lilia/contracts";
import {
  getProjectSettings,
  pickFolder,
  setProjectSettings,
} from "../../services/projects";

const projectSettings = ref<ProjectSettings>({ cloneParentDir: null });
const savingProject = ref(false);
const projectError = ref<string | null>(null);

async function loadProjectSettings() {
  try {
    projectSettings.value = await getProjectSettings();
  } catch (err) {
    projectError.value = `读取项目偏好失败：${String(err)}`;
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

onMounted(loadProjectSettings);
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
      <div class="settings-row__label">
        <div>Clone 默认父目录</div>
      </div>
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

    <div v-if="projectError" class="conn-banner conn-banner--err">
      <AlertTriangle :size="16" aria-hidden="true" />
      <div>
        <div class="conn-banner__title">项目偏好</div>
        <div class="conn-banner__hint">{{ projectError }}</div>
      </div>
    </div>
  </div>
</template>
