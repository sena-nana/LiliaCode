<script setup lang="ts">
import { onMounted } from "vue";
import { ConfirmDialog } from "@lilia/ui";
import { useDesktopAppUpdater } from "../composables/useDesktopAppUpdater";

const props = withDefaults(defineProps<{
  autoCheck?: boolean;
  isProduction?: boolean;
  windowLabel?: string;
}>(), {
  autoCheck: true,
});

const updater = useDesktopAppUpdater({
  autoCheck: props.autoCheck,
  isProduction: props.isProduction,
  windowLabel: props.windowLabel,
});

onMounted(() => {
  updater.startAutoCheck();
});
</script>

<template>
  <ConfirmDialog
    :open="updater.open.value"
    :title="updater.title.value"
    :message="updater.message.value"
    :confirm-text="updater.confirmText.value"
    :cancel-text="updater.phase.value === 'failed' ? '稍后' : '暂不更新'"
    :busy="updater.busy.value"
    :busy-text="updater.busyText.value"
    @confirm="updater.confirmUpdate"
    @cancel="updater.dismissUpdate"
  />
</template>
