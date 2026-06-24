<script setup lang="ts">
import { RouterLink } from "vue-router";
import {
  GitBranch,
  Settings,
} from "lucide-vue-next";
import ProviderConnectionBadge from "../ProviderConnectionBadge.vue";
</script>

<template>
  <div class="sb-footer">
    <RouterLink
      to="/settings"
      class="sb-footer__btn"
      active-class="is-active"
      data-agent-id="sidebar.footer.settings"
      title="设置"
      aria-label="设置"
    >
      <Settings :size="14" aria-hidden="true" />
    </RouterLink>

    <RouterLink
      to="/automations"
      class="sb-footer__btn"
      active-class="is-active"
      data-agent-id="sidebar.footer.automations"
      title="自动化"
      aria-label="自动化"
    >
      <GitBranch :size="14" aria-hidden="true" />
    </RouterLink>

    <ProviderConnectionBadge
      to="/settings"
      popover-id="sb-conn-quota-popover"
    />
  </div>

</template>
