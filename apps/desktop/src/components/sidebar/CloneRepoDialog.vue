<script setup lang="ts">
import { computed, nextTick, ref } from "vue";
import { Github, FolderOpen } from "lucide-vue-next";
import { homeDir } from "@tauri-apps/api/path";
import type { Project } from "@lilia/contracts";
import {
  createProject,
  deriveProjectName,
} from "../../services/projectsStore";
import {
  getProjectSettings,
  gitCloneRepo,
  pickFolder,
} from "../../services/projects";

const props = defineProps<{ open: boolean }>();
const emit = defineEmits<{
  close: [];
  cloned: [project: Project];
  error: [msg: string];
}>();

const cloneUrl = ref("");
const cloneParent = ref("");
const cloneInput = ref<HTMLInputElement | null>(null);
const cloneBusy = ref(false);
const cloneError = ref<string | null>(null);

async function init() {
  cloneUrl.value = "";
  cloneError.value = null;
  cloneBusy.value = false;
  try {
    const s = await getProjectSettings();
    if (s.cloneParentDir && s.cloneParentDir.trim()) {
      cloneParent.value = s.cloneParentDir.trim();
    } else {
      cloneParent.value = await safeHomeDir();
    }
  } catch {
    cloneParent.value = await safeHomeDir();
  }
  await nextTick();
  cloneInput.value?.focus();
}

async function safeHomeDir(): Promise<string> {
  try { return await homeDir(); }
  catch { return ""; }
}

async function pickCloneParent() {
  try {
    const picked = await pickFolder({
      title: "选择 clone 目标父目录",
      defaultPath: cloneParent.value || null,
    });
    if (picked) cloneParent.value = picked;
  } catch (err) {
    cloneError.value = `选择文件夹失败：${String(err)}`;
  }
}

const cloneTargetPreview = computed(() => {
  const url = cloneUrl.value.trim();
  const parent = cloneParent.value.trim();
  if (!url || !parent) return "";
  const cleaned = url.replace(/\.git$/i, "").replace(/\/+$/, "");
  const base = cleaned.split(/[/:]/).pop()?.trim() || "repo";
  const sep = parent.includes("\\") ? "\\" : "/";
  const normalizedParent = parent.replace(/[\\/]+$/, "");
  return `${normalizedParent}${sep}${base}`;
});

async function confirmClone() {
  if (cloneBusy.value) return;
  cloneError.value = null;
  const url = cloneUrl.value.trim();
  const parent = cloneParent.value.trim();
  if (!url) {
    cloneError.value = "请填写仓库 URL";
    return;
  }
  if (!parent) {
    cloneError.value = "请选择 clone 目标父目录";
    return;
  }
  cloneBusy.value = true;
  try {
    const cloned = await gitCloneRepo(url, parent);
    const project = createProject({
      name: deriveProjectName(cloned) || "新项目",
      cwd: cloned,
    });
    emit("cloned", project);
    emit("close");
  } catch (err) {
    cloneError.value = String(err);
  } finally {
    cloneBusy.value = false;
  }
}

defineExpose({ init });
</script>

<template>
  <Teleport to="body">
    <Transition name="search-palette">
      <div v-if="open" class="search-palette" role="dialog" aria-modal="true" aria-label="从 GitHub clone"
        @click.self="emit('close')">
        <div class="search-palette__card dialog__card">
          <div class="dialog__header">
            <Github :size="14" aria-hidden="true" />
            <span>从 Git 仓库克隆</span>
          </div>
          <div class="dialog__body">
            <label>
              <span>仓库 URL</span>
              <input ref="cloneInput" v-model="cloneUrl" type="text" class="text-input"
                placeholder="https://github.com/owner/repo.git"
                @keydown.enter.prevent="confirmClone" />
            </label>
            <label>
              <span>目标父目录</span>
              <div class="dialog__field-row">
                <input :value="cloneParent" type="text" class="text-input"
                  placeholder="选择克隆到哪个目录下" readonly />
                <button type="button" class="ghost" :disabled="cloneBusy" @click="pickCloneParent">
                  <FolderOpen :size="12" aria-hidden="true" /> 选择
                </button>
              </div>
            </label>
            <p v-if="cloneTargetPreview" class="plugins-create__hint">
              将克隆到 <code>{{ cloneTargetPreview }}</code>
            </p>
            <p v-if="cloneError" class="plugins-create__error">{{ cloneError }}</p>
          </div>
          <div class="dialog__actions">
            <button type="button" class="ghost" :disabled="cloneBusy" @click="emit('close')">取消</button>
            <button type="button" class="primary" :disabled="cloneBusy" @click="confirmClone">
              {{ cloneBusy ? "克隆中…" : "克隆并添加" }}
            </button>
          </div>
        </div>
      </div>
    </Transition>
  </Teleport>
</template>
