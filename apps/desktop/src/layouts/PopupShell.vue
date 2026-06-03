<script setup lang="ts">
import { RouterView } from "vue-router";
import PopupTitleBar from "../components/PopupTitleBar.vue";
</script>

<template>
  <div class="popup-shell">
    <PopupTitleBar />
    <main class="popup-shell__main">
      <RouterView />
    </main>
  </div>
</template>
