<script setup lang="ts">
import { ref } from "vue";
import { ExternalLink, Globe } from "lucide-vue-next";
import type { ChatSidebarContext } from "../../composables/useChatSidebar";

defineProps<ChatSidebarContext>();

const draftUrl = ref("about:blank");
const activeUrl = ref("about:blank");

function openIab() {
  activeUrl.value = draftUrl.value.trim() || "about:blank";
}
</script>

<template>
  <div class="iab-panel">
    <form class="iab-panel__bar" @submit.prevent="openIab">
      <label class="iab-panel__field">
        <Globe :size="14" aria-hidden="true" />
        <input
          v-model="draftUrl"
          type="text"
          spellcheck="false"
          autocomplete="off"
          aria-label="IAB URL"
          placeholder="about:blank"
        />
      </label>
      <button type="submit" class="iab-panel__button">
        <ExternalLink :size="14" aria-hidden="true" />
        <span>打开 IAB</span>
      </button>
    </form>
    <iframe
      class="iab-panel__frame"
      :src="activeUrl"
      title="Lilia IAB"
      referrerpolicy="no-referrer"
    />
  </div>
</template>
