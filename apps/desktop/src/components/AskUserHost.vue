<script setup lang="ts">
/**
 * 全局问询宿主：监听 useAskUser 的当前 spec，挂一份 AskUserDialog。
 * 与 ContextMenuHost 同模式，需在 App 根渲染一次。
 */
import AskUserDialog from "./AskUserDialog.vue";
import { resolveAskUser, useAskUser } from "../composables/useAskUser";

const { state } = useAskUser();
</script>

<template>
  <AskUserDialog
    v-if="state.current"
    :key="state.current.spec.questions.map((q) => q.id).join('|')"
    :spec="state.current.spec"
    @resolve="resolveAskUser"
  />
</template>
