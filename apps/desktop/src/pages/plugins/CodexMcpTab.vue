<script setup lang="ts">
import { Check, FolderOpen, Pencil, Plus, Trash2 } from "lucide-vue-next";
import type { CodexMcpServer } from "../../services/plugins";

defineProps<{
  servers: CodexMcpServer[];
  configPath: string | null;
}>();

const emit = defineEmits<{
  "open-config": [];
  create: [];
  edit: [server: CodexMcpServer];
  toggle: [server: CodexMcpServer];
  remove: [server: CodexMcpServer];
}>();

function transportLabel(server: CodexMcpServer) {
  if (server.transport === "stdio") return "stdio";
  if (server.transport === "http") return "HTTP";
  if (server.transport === "oauth") return "OAuth";
  return "未知";
}

function serverSummary(server: CodexMcpServer) {
  const stdio = [server.command, ...server.args].filter(Boolean).join(" ").trim();
  return stdio || `${transportLabel(server)} MCP server`;
}
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
      <button type="button" class="primary" @click="emit('create')">
        <Plus :size="14" aria-hidden="true" /> 新增 MCP
      </button>
    </div>
    <ul v-if="servers.length" class="plugins-list">
      <li
        v-for="s in servers"
        :key="s.name"
        class="plugins-list__item"
        :class="{ 'is-disabled': !s.enabled }"
      >
        <div class="plugins-list__head">
          <span class="plugins-list__name">{{ s.name }}</span>
          <span v-if="s.enabled" class="plugins-list__badge plugins-list__badge--ok">
            <Check :size="11" aria-hidden="true" /> 已注册
          </span>
          <span v-else class="plugins-list__badge plugins-list__badge--mute">已停用</span>
          <span class="plugins-list__badge plugins-list__badge--mute">
            {{ transportLabel(s) }}
          </span>
          <span v-if="!s.editable" class="plugins-list__badge plugins-list__badge--mute">只读</span>
          <span v-if="s.envKeys.length" class="plugins-list__badge plugins-list__badge--mute">
            env {{ s.envKeys.length }}
          </span>
        </div>
        <div class="plugins-list__meta">
          <code>{{ serverSummary(s) }}</code>
        </div>
        <p v-if="s.envKeys.length" class="plugins-list__desc">
          {{ s.envKeys.join(", ") }}
        </p>
        <div class="plugins-list__actions">
          <template v-if="s.editable">
            <button type="button" class="ghost" @click="emit('toggle', s)">
              {{ s.enabled ? "停用" : "启用" }}
            </button>
            <button type="button" class="ghost" @click="emit('edit', s)">
              <Pencil :size="12" aria-hidden="true" /> 编辑
            </button>
            <button type="button" class="ghost danger" @click="emit('remove', s)">
              <Trash2 :size="12" aria-hidden="true" /> 删除
            </button>
          </template>
          <button v-else type="button" class="ghost" @click="emit('open-config')">
            <FolderOpen :size="12" aria-hidden="true" /> 打开配置
          </button>
        </div>
      </li>
    </ul>
    <p v-else class="plugins-empty">
      config.toml 里还没有 mcp_servers 节。
    </p>
  </div>
</template>
