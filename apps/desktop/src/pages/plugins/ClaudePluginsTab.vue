<script setup lang="ts">
import { Check } from "lucide-vue-next";
import type { ClaudePlugin } from "../../services/plugins";

defineProps<{ plugins: ClaudePlugin[] }>();

const emit = defineEmits<{
  toggle: [plugin: ClaudePlugin];
}>();
</script>

<template>
  <div class="card">
    <p class="plugins-section-hint">
      来自 <code>~/.claude/plugins/</code> 的 marketplace 插件（beta）。
    </p>
    <ul v-if="plugins.length" class="plugins-list">
      <li
        v-for="p in plugins"
        :key="p.path"
        class="plugins-list__item"
        :class="{ 'is-disabled': !p.enabled }"
      >
        <div class="plugins-list__head">
          <span class="plugins-list__name">{{ p.name }}</span>
          <span v-if="p.enabled" class="plugins-list__badge plugins-list__badge--ok">
            <Check :size="11" aria-hidden="true" /> 已启用
          </span>
          <span v-else class="plugins-list__badge plugins-list__badge--mute">已停用</span>
          <span class="plugins-list__badge plugins-list__badge--mute">v{{ p.version || "—" }}</span>
        </div>
        <p class="plugins-list__desc">{{ p.description || "（无描述）" }}</p>
        <div class="plugins-list__meta"><code>{{ p.path }}</code></div>
        <div class="plugins-list__actions">
          <button type="button" class="ghost" @click="emit('toggle', p)">
            {{ p.enabled ? "停用" : "启用" }}
          </button>
        </div>
      </li>
    </ul>
    <p v-else class="plugins-empty">没有发现已安装的 plugin。</p>
  </div>
</template>
