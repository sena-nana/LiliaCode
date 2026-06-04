<script setup lang="ts">
import { Check, Folder, Layers, Plus, Trash2 } from "lucide-vue-next";
import type { ClaudeSkill, PluginScope } from "../../services/plugins";
import Dropdown from "../../components/Dropdown.vue";

defineProps<{
  scope: PluginScope;
  projectCwd: string | null;
  projectOptions: Array<{ value: string; label: string; hint: string }>;
  currentSkills: ClaudeSkill[];
  currentScopeHint: string;
}>();

const emit = defineEmits<{
  "update:scope": [scope: PluginScope];
  "project-change": [cwd: string];
  create: [];
  toggle: [skill: ClaudeSkill];
  remove: [skill: ClaudeSkill];
}>();
</script>

<template>
  <div class="card card--allow-overflow">
    <div class="plugins-toolbar">
      <div class="segmented" role="radiogroup" aria-label="Skill scope">
        <button
          type="button" role="radio"
          :aria-checked="scope === 'user'"
          :class="{ 'is-active': scope === 'user' }"
          @click="emit('update:scope', 'user')"
        >
          <Layers :size="12" aria-hidden="true" /> 全局
        </button>
        <button
          type="button" role="radio"
          :aria-checked="scope === 'project'"
          :class="{ 'is-active': scope === 'project' }"
          @click="emit('update:scope', 'project')"
        >
          <Layers :size="12" aria-hidden="true" /> 项目
        </button>
      </div>

      <Dropdown
        v-if="scope === 'project' && projectCwd"
        :model-value="projectCwd"
        :options="projectOptions"
        :icon="Folder"
        placement="bottom"
        @update:model-value="emit('project-change', $event)"
      />
      <span v-else-if="scope === 'project'" class="muted">尚未配置项目</span>

      <span class="plugins-toolbar__hint">{{ currentScopeHint }}</span>

      <button type="button" class="primary" @click="emit('create')">
        <Plus :size="14" aria-hidden="true" /> 新建 Skill
      </button>
    </div>

    <ul v-if="currentSkills.length" class="plugins-list">
      <li
        v-for="s in currentSkills"
        :key="s.path"
        class="plugins-list__item"
        :class="{ 'is-disabled': !s.enabled }"
      >
        <div class="plugins-list__head">
          <span class="plugins-list__name">{{ s.name }}</span>
          <span v-if="s.enabled" class="plugins-list__badge plugins-list__badge--ok">
            <Check :size="11" aria-hidden="true" /> 已启用
          </span>
          <span v-else class="plugins-list__badge plugins-list__badge--mute">已停用</span>
        </div>
        <p class="plugins-list__desc">{{ s.description || "（无描述）" }}</p>
        <div class="plugins-list__meta">
          <code>{{ s.path }}</code>
        </div>
        <div class="plugins-list__actions">
          <button type="button" class="ghost" @click="emit('toggle', s)">
            {{ s.enabled ? "停用" : "启用" }}
          </button>
          <button type="button" class="ghost danger" @click="emit('remove', s)">
            <Trash2 :size="12" aria-hidden="true" /> 删除
          </button>
        </div>
      </li>
    </ul>
    <p v-else class="plugins-empty">
      {{ scope === 'user' ? '全局' : '项目' }} skill 目录里还没有任何 SKILL.md。
    </p>
  </div>
</template>
