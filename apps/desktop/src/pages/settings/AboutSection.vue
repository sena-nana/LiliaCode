<template>
  <div class="about-page">
    <section class="card about-main">
      <h2>关于</h2>
      <ul class="kv">
        <li><span>名称</span><span>{{ appName }}</span></li>
        <li class="about-version-row">
          <span>版本</span>
          <span>
            <span class="about-version-value">{{ appVersion }}</span>
            <button
              type="button"
              class="primary about-version-action"
              data-agent-id="settings.about.updater.check"
              :disabled="checking || installing"
              @click="checkForUpdate"
            >
              {{ checking ? "检查中" : "检查更新" }}
            </button>
            <button
              v-if="pendingUpdate"
              type="button"
              class="primary about-version-action"
              data-agent-id="settings.about.updater.install"
              :disabled="installing"
              @click="installUpdate"
            >
              {{ installing ? "安装中" : "下载并安装" }}
            </button>
          </span>
        </li>
      </ul>
    </section>
    <section class="card about-license-third-party" aria-label="第三方许可证协议">
      <template v-if="hasLicenseManifest">
        <details class="about-license-details">
          <summary class="about-license-summary">
            <span>第三方许可证协议</span>
            <ChevronRight
              :size="13"
              aria-hidden="true"
              class="sb-group-toggle__chevron"
            />
          </summary>
          <div v-if="npmDependencies.length">
            <ul class="kv about-license-list">
              <li v-for="dependency in npmDependencies" :key="`npm-dep-${dependency.name}`">
                <span>{{ dependency.name }}</span>
                <span>{{ dependency.license }}</span>
              </li>
            </ul>
          </div>
          <div v-if="rustDependencies.length">
            <ul class="kv about-license-list">
              <li v-for="dependency in rustDependencies" :key="`rust-dep-${dependency.name}`">
                <span>{{ dependency.name }}</span>
                <span>{{ dependency.license }}</span>
              </li>
            </ul>
          </div>
          <div v-if="!npmDependencies.length && !rustDependencies.length" class="about-license-empty">
            未发现可汇总的依赖记录。
          </div>
        </details>
      </template>
      <p v-else class="about-license-fallback">未生成许可清单，请重试 yarn about:licenses</p>
    </section>
  </div>
</template>

<script setup lang="ts">
import { computed, ref } from "vue";
import { ChevronRight } from "@lucide/vue";
import { check, type Update } from "@tauri-apps/plugin-updater";
import openSourceLicenseManifest from "../../generated/openSourceLicenseManifest.json";

interface OpenSourceLicenseManifestDependency {
  name: string;
  version: string;
  license: string;
}

interface OpenSourceLicenseManifest {
  app?: {
    name?: string;
    version?: string;
    license?: string;
  };
  npmDependencies?: OpenSourceLicenseManifestDependency[];
  rustDependencies?: OpenSourceLicenseManifestDependency[];
}

const manifest = openSourceLicenseManifest as OpenSourceLicenseManifest | null;

const hasLicenseManifest = computed(() => {
  return (
    Boolean(manifest?.app?.name) &&
    Array.isArray(manifest?.npmDependencies) &&
    Array.isArray(manifest?.rustDependencies)
  );
}
);

const appName = computed(() => manifest?.app?.name || "LiliaCode");
const appVersion = computed(() => manifest?.app?.version ?? "1.0.0-beta.1");
const npmDependencies = computed(() => manifest?.npmDependencies ?? []);
const rustDependencies = computed(() => manifest?.rustDependencies ?? []);
const checking = ref(false);
const installing = ref(false);
const pendingUpdate = ref<Update | null>(null);

async function checkForUpdate() {
  checking.value = true;
  pendingUpdate.value = null;
  try {
    const update = await check();
    pendingUpdate.value = update;
  } catch {
  } finally {
    checking.value = false;
  }
}

async function installUpdate() {
  if (!pendingUpdate.value) return;
  installing.value = true;
  try {
    await pendingUpdate.value.downloadAndInstall(() => {});
    pendingUpdate.value = null;
  } catch {
  } finally {
    installing.value = false;
  }
}
</script>

<style scoped>
.about-page {
  display: grid;
  gap: 12px;
}

.about-version-row > span:last-child {
  display: inline-flex;
  flex-wrap: wrap;
  gap: 8px;
  justify-content: flex-end;
  align-items: center;
}

.about-version-action {
  height: 22px;
  padding: 0 8px;
  font-size: 12px;
  line-height: 1;
}

.about-license-fallback {
  margin: 0 0 8px;
  color: var(--muted);
  font-size: 13px;
}

.about-license-empty {
  font-size: 13px;
  color: var(--muted);
}

.about-license-list {
  font-size: 12px;
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(220px, 1fr));
  gap: 2px 12px;
}

.about-license-list li {
  display: grid;
  grid-template-columns: minmax(0, 1fr) auto;
  gap: 6px;
  justify-content: space-between;
  border-bottom: 1px solid var(--border-soft);
  padding: 6px 0;
}

.about-license-details summary {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  list-style: none;
  margin: 6px 0;
  cursor: pointer;
  padding: 0;
  line-height: normal;
  color: var(--text-muted);
  font-size: 13px;
  font-weight: 600;
  text-transform: uppercase;
  letter-spacing: 0.5px;
}

.about-license-details > summary .sb-group-toggle__chevron {
  flex: 0 0 auto;
  transition: transform 0.12s ease;
  color: currentColor;
}

.about-license-details[open] > summary .sb-group-toggle__chevron {
  transform: rotate(90deg);
}

.about-license-details > summary::-webkit-details-marker {
  display: none;
}

.about-license-details summary ~ div,
.about-license-details summary ~ p,
.about-license-details summary ~ ul {
  margin-top: 6px;
}

.about-license-details summary:focus-visible {
  outline: 2px solid color-mix(in srgb, var(--accent) 58%, transparent);
  outline-offset: 2px;
}

@media (max-width: 720px) {
  .about-version-row > span:last-child {
    justify-content: flex-start;
  }
}
</style>
