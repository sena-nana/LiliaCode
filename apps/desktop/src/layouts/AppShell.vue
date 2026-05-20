<script setup lang="ts">
import { RouterView, useRoute, useRouter } from "vue-router";
import { computed } from "vue";
import TitleBar from "../components/TitleBar.vue";
import ActivityBar from "./ActivityBar.vue";
import SecondaryPanel from "./SecondaryPanel.vue";

const route = useRoute();
const router = useRouter();

const activeSection = computed<"projects" | "settings">(() => {
  if (route.path.startsWith("/settings")) return "settings";
  return "projects";
});

function go(section: "projects" | "settings") {
  if (section === "projects") router.push("/");
  else router.push("/settings");
}
</script>

<template>
  <div class="shell">
    <TitleBar />
    <ActivityBar :active="activeSection" @navigate="go" />
    <SecondaryPanel :section="activeSection" />
    <main class="shell__main">
      <RouterView />
    </main>
  </div>
</template>
