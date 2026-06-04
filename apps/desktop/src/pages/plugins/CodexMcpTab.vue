<script setup lang="ts">
import { Check, FolderOpen } from "lucide-vue-next";
import type { CodexMcpServer } from "../../services/plugins";

defineProps<{
  servers: CodexMcpServer[];
  configPath: string | null;
}>();

const emit = defineEmits<{
  "open-config": [];
}>();
</script>

<template>
  <div class="card">
    <div class="plugins-toolbar">
      <span class="plugins-toolbar__hint">
        来自 <code>{{ configPath || "~/.codex/config.toml" }}</code> 的 mcp_servers 节
      </span>
      <button type="button" class="ghost" @click="emit('open-config')">
        <FolderOpen :size="12" aria-hidden="true" /> 打开 config.toml
      </button>
    </div>
    <ul v-if="servers.length" class="plugins-list">
      <li v-for="s in servers" :key="s.name" class="plugins-list__item">
        <div class="plugins-list__head">
          <span class="plugins-list__name">{{ s.name }}</span>
          <span class="plugins-list__badge plugins-list__badge--ok">
            <Check :size="11" aria-hidden="true" /> 已注册
          </span>
        </div>
        <div class="plugins-list__meta">
          <code>{{ s.command }} {{ s.args.join(' ') }}</code>
        </div>
      </li>
    </ul>
    <p v-else class="plugins-empty">
      config.toml 里还没有 mcp_servers 节，点击「打开 config.toml」开始添加。
    </p>
  </div>
</template>
