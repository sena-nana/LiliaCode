<script setup lang="ts">
import { onMounted, ref } from "vue";
import { AlertTriangle, Keyboard, Save, Trash2 } from "lucide-vue-next";
import type { PopupWindowSettings } from "@lilia/contracts";
import {
  getPopupWindowSettings,
  setPopupWindowSettings,
} from "../../services/popupWindows";

const popupWindowSettings = ref<PopupWindowSettings>({ shortcut: null });
const savingPopupWindow = ref(false);
const popupWindowError = ref<string | null>(null);
const popupWindowDirty = ref(false);

async function loadPopupWindowSettings() {
  try {
    const settings = await getPopupWindowSettings();
    if (!popupWindowDirty.value) {
      popupWindowSettings.value = settings;
    }
  } catch (err) {
    popupWindowError.value = `读取弹出窗口设置失败：${String(err)}`;
  }
}

function shortcutLabel(shortcut: string | null): string {
  return shortcut?.trim() || "未设置";
}

function normalizeShortcutKey(key: string): string | null {
  if (key.length === 1) return key.toUpperCase();
  const aliases: Record<string, string> = {
    " ": "Space",
    ArrowUp: "ArrowUp",
    ArrowDown: "ArrowDown",
    ArrowLeft: "ArrowLeft",
    ArrowRight: "ArrowRight",
  };
  return aliases[key] ?? (key.length > 1 ? key : null);
}

function onShortcutCapture(event: KeyboardEvent) {
  if (event.key === "Tab") return;
  event.preventDefault();
  event.stopPropagation();
  popupWindowError.value = null;
  if (event.key === "Escape") {
    (event.currentTarget as HTMLInputElement | null)?.blur();
    return;
  }
  if (event.key === "Backspace" || event.key === "Delete") {
    popupWindowDirty.value = true;
    popupWindowSettings.value = { shortcut: null };
    return;
  }
  const key = normalizeShortcutKey(event.key);
  if (!key || ["Control", "Shift", "Alt", "Meta"].includes(event.key)) return;
  const parts: string[] = [];
  if (event.ctrlKey) parts.push("Ctrl");
  if (event.altKey) parts.push("Alt");
  if (event.shiftKey) parts.push("Shift");
  if (event.metaKey) parts.push("Meta");
  parts.push(key);
  popupWindowDirty.value = true;
  popupWindowSettings.value = { shortcut: parts.join("+") };
}

function clearPopupShortcut() {
  popupWindowDirty.value = true;
  popupWindowSettings.value = { shortcut: null };
  popupWindowError.value = null;
}

async function savePopupWindowSettings() {
  savingPopupWindow.value = true;
  popupWindowError.value = null;
  try {
    await setPopupWindowSettings(popupWindowSettings.value);
    popupWindowDirty.value = false;
  } catch (err) {
    popupWindowError.value = `保存弹出窗口设置失败：${String(err)}`;
  } finally {
    savingPopupWindow.value = false;
  }
}

onMounted(loadPopupWindowSettings);
</script>

<template>
  <div class="card">
    <h2>
      <span class="card-h2__title">
        <Keyboard :size="14" aria-hidden="true" />
        弹出窗口
      </span>
    </h2>

    <div class="settings-row">
      <div class="settings-row__label">全局快捷键</div>
      <div style="display: flex; gap: 8px; align-items: center;">
        <input
          type="text"
          class="text-input"
          readonly
          :value="shortcutLabel(popupWindowSettings.shortcut)"
          aria-label="弹出窗口快捷键"
          title="聚焦后按组合键录入，Backspace 或 Delete 清空"
          @keydown="onShortcutCapture"
        />
        <button
          type="button"
          class="ghost"
          :disabled="savingPopupWindow"
          @click="clearPopupShortcut"
        >
          <Trash2 :size="12" aria-hidden="true" />
          清空
        </button>
        <button
          type="button"
          class="ghost"
          :disabled="savingPopupWindow"
          @click="savePopupWindowSettings"
        >
          <Save :size="12" aria-hidden="true" />
          {{ savingPopupWindow ? "保存中…" : "保存" }}
        </button>
      </div>
    </div>

    <div v-if="popupWindowError" class="conn-banner conn-banner--err">
      <AlertTriangle :size="16" aria-hidden="true" />
      <div>
        <div class="conn-banner__title">弹出窗口</div>
        <div class="conn-banner__hint">{{ popupWindowError }}</div>
      </div>
    </div>
  </div>
</template>
