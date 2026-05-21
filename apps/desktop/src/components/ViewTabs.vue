<script setup lang="ts">
import { MessagesSquare, LayoutDashboard, GitBranch, CheckSquare } from "lucide-vue-next";

type ViewKey = "sessions" | "kanban" | "architecture" | "todo";

interface Props {
  projectId: string;
  /** 当前激活的 tab。目前只有 sessions 可用。 */
  active: ViewKey;
}

defineProps<Props>();

const tabs: Array<{ key: ViewKey; label: string; icon: any; disabled: boolean }> = [
  { key: "sessions", label: "Sessions", icon: MessagesSquare, disabled: false },
  { key: "kanban", label: "看板", icon: LayoutDashboard, disabled: true },
  { key: "architecture", label: "架构图", icon: GitBranch, disabled: true },
  { key: "todo", label: "Todo", icon: CheckSquare, disabled: true },
];
</script>

<template>
  <div class="view-tabs" role="tablist" aria-label="项目视图">
    <button
      v-for="t in tabs"
      :key="t.key"
      type="button"
      class="view-tabs__tab"
      :class="{ 'is-active': active === t.key }"
      :disabled="t.disabled"
      :aria-selected="active === t.key"
      :title="t.disabled ? '即将上线' : t.label"
      role="tab"
    >
      <component :is="t.icon" :size="14" aria-hidden="true" />
      <span>{{ t.label }}</span>
    </button>
  </div>
</template>
