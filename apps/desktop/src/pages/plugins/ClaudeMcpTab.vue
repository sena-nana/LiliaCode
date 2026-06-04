<script setup lang="ts">
import { Check, FolderOpen, Pencil, Plus, Trash2 } from "lucide-vue-next";
import type { ClaudeMcpServer } from "../../services/plugins";

defineProps<{
  servers: ClaudeMcpServer[];
  configPath: string | null;
}>();

const emit = defineEmits<{
  "open-config": [];
  create: [];
  edit: [server: ClaudeMcpServer];
  toggle: [server: ClaudeMcpServer];
  remove: [server: ClaudeMcpServer];
}>();
</script>

<template>
  <div class="card">
    <div class="plugins-toolbar">
      <span class="plugins-toolbar__hint">
        来自 <code>{{ configPath || "~/.lilia/config/claude-mcp-servers.json" }}</code>
      </span>
      <button type="button" class="ghost" @click="emit('open-config')">
        <FolderOpen :size="12" aria-hidden="true" /> 打开配置
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
            <Check :size="11" aria-hidden="true" /> 已启用
          </span>
          <span v-else class="plugins-list__badge plugins-list__badge--mute">已停用</span>
          <span v-if="s.envKeys.length" class="plugins-list__badge plugins-list__badge--mute">
            env {{ s.envKeys.length }}
          </span>
        </div>
        <div class="plugins-list__meta">
          <code>{{ s.command }} {{ s.args.join(' ') }}</code>
        </div>
        <p v-if="s.envKeys.length" class="plugins-list__desc">
          {{ s.envKeys.join(", ") }}
        </p>
        <div class="plugins-list__actions">
          <button type="button" class="ghost" @click="emit('toggle', s)">
            {{ s.enabled ? "停用" : "启用" }}
          </button>
          <button type="button" class="ghost" @click="emit('edit', s)">
            <Pencil :size="12" aria-hidden="true" /> 编辑
          </button>
          <button type="button" class="ghost danger" @click="emit('remove', s)">
            <Trash2 :size="12" aria-hidden="true" /> 删除
          </button>
        </div>
      </li>
    </ul>
    <p v-else class="plugins-empty">
      还没有外部 Claude MCP server。
    </p>
  </div>
</template>
