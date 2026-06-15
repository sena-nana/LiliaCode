<script setup lang="ts">
import { computed } from "vue";
import { RouterLink, useRoute } from "vue-router";
import {
  GitBranch,
  Import,
  Puzzle,
  Settings,
} from "lucide-vue-next";
import ProviderConnectionBadge from "../ProviderConnectionBadge.vue";

const route = useRoute();

const importRoute = computed(() => {
  const value = route.params.projectId;
  const projectId = Array.isArray(value) ? value[0] : value;
  return projectId ? { path: "/import", query: { projectId } } : "/import";
});
</script>

<template>
  <div class="sb-footer">
    <RouterLink to="/settings" class="sb-footer__btn" active-class="is-active" title="设置" aria-label="设置">
      <Settings :size="14" aria-hidden="true" />
    </RouterLink>

    <RouterLink to="/plugins" class="sb-footer__btn" active-class="is-active" title="插件 / 技能" aria-label="插件 / 技能">
      <Puzzle :size="14" aria-hidden="true" />
    </RouterLink>

    <RouterLink to="/automations" class="sb-footer__btn" active-class="is-active" title="自动化" aria-label="自动化">
      <GitBranch :size="14" aria-hidden="true" />
    </RouterLink>

    <RouterLink
      :to="importRoute"
      class="sb-footer__btn"
      active-class="is-active"
      title="从 Claude / Codex 导入对话"
      aria-label="从 Claude / Codex 导入对话"
    >
      <Import :size="14" aria-hidden="true" />
    </RouterLink>

    <ProviderConnectionBadge
      to="/settings"
      popover-id="sb-conn-quota-popover"
    />
  </div>

</template>
