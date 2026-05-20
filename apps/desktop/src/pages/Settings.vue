<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import {
  Moon, Sun, CheckCircle2, XCircle, Loader2,
  Plug, AlertTriangle, Save, Network,
} from "lucide-vue-next";
import { useTheme } from "../composables/useTheme";
import { useConnectionStatus } from "../composables/useConnectionStatus";
import {
  getCCSwitchConfig,
  getProviderConfig,
  getRouterMode,
  setCCSwitchConfig,
  setProviderConfig,
  setRouterMode,
  type BackendEnvStatus,
} from "../services/chat";
import type {
  CCSwitchConfig,
  ChatBackendKind,
  ConnectionMode,
  ProviderConfig,
  RouterMode,
} from "@lilia/contracts";

const { theme, setTheme } = useTheme();
const {
  statusFor, routerFor, probing, refresh, nodeAvailable, codexCliAvailable, ccSwitch,
} = useConnectionStatus();

const claudeStatus = computed<BackendEnvStatus | null>(() => statusFor("claude"));
const codexStatus = computed<BackendEnvStatus | null>(() => statusFor("codex"));

async function probe() { await refresh(); }
onMounted(probe);

// 三态卡片头：连接模式 + 一行解释。
function buildModeMeta(
  s: BackendEnvStatus | null,
  backend: ChatBackendKind,
): { label: string; hint: string; ok: boolean } | null {
  if (!s) return null;
  const url = s.effectiveUrl ?? "—";
  const isClaude = backend === "claude";
  const router = routerFor(backend);
  const map: Record<ConnectionMode, { label: string; hint: string; ok: boolean }> = {
    "cc-switch": {
      label: "经 CC-Switch 代理",
      hint: `请求会发往 ${url}，由 CC-Switch 转发到它的 active provider。`,
      ok: true,
    },
    custom: {
      label: "自定义 Base URL",
      hint: `使用 ${url}（来源：下方直连配置）`,
      ok: true,
    },
    direct: {
      label: isClaude ? "Anthropic 官方 API" : "OpenAI 官方 API",
      hint: `直连 ${url}，使用 ${isClaude ? "ANTHROPIC_API_KEY" : "OPENAI_API_KEY"} 验证。`,
      ok: true,
    },
    unconfigured: {
      label: "未配置",
      hint: router === "direct"
        ? `直连模式下未填写 ${isClaude ? "Claude" : "Codex"} 的 baseUrl/apiKey。请在下方填入。`
        : `CC-Switch 路由不可达。请检查上方代理 URL，或在「路由」处切到直连。`,
      ok: false,
    },
  };
  return map[s.connectionMode];
}

const claudeMeta = computed(() => buildModeMeta(claudeStatus.value, "claude"));
const codexMeta = computed(() => buildModeMeta(codexStatus.value, "codex"));

// ---- 各 backend 的路由模式（独立选择） ----

const claudeRouter = ref<RouterMode>("cc-switch");
const codexRouter = ref<RouterMode>("cc-switch");
const savingClaudeRouter = ref(false);
const savingCodexRouter = ref(false);

const routerOptions: { value: RouterMode; label: string; hint: string }[] = [
  { value: "cc-switch", label: "CC-Switch", hint: "经上方代理转发" },
  { value: "direct", label: "直连", hint: "用本卡的 baseUrl + apiKey 直连" },
];

async function chooseRouter(backend: ChatBackendKind, mode: RouterMode) {
  const cur = backend === "claude" ? claudeRouter : codexRouter;
  const saving = backend === "claude" ? savingClaudeRouter : savingCodexRouter;
  if (saving.value || cur.value === mode) return;
  const prev = cur.value;
  cur.value = mode;
  saving.value = true;
  try {
    await setRouterMode(backend, mode);
    await refresh();
  } catch (err) {
    console.error("[settings] setRouterMode failed", err);
    cur.value = prev;
  } finally {
    saving.value = false;
  }
}

// ---- 直连配置表单（per-backend）+ CC-Switch（单 URL）----

function emptyConfig(backend: ChatBackendKind): ProviderConfig {
  return { backend, baseUrl: null, apiKey: null };
}

const claudeForm = ref<ProviderConfig>(emptyConfig("claude"));
const codexForm = ref<ProviderConfig>(emptyConfig("codex"));
const ccSwitchForm = ref<CCSwitchConfig>({ baseUrl: "http://127.0.0.1:15721" });
const savingClaude = ref(false);
const savingCodex = ref(false);
const savingCCSwitch = ref(false);

async function loadForms() {
  try {
    const [c, x, cc, rc, rx] = await Promise.all([
      getProviderConfig("claude"),
      getProviderConfig("codex"),
      getCCSwitchConfig(),
      getRouterMode("claude"),
      getRouterMode("codex"),
    ]);
    claudeForm.value = c;
    codexForm.value = x;
    ccSwitchForm.value = cc;
    claudeRouter.value = rc;
    codexRouter.value = rx;
  } catch (err) {
    console.error("[settings] load configs failed", err);
  }
}
onMounted(loadForms);

async function saveBackend(backend: ChatBackendKind) {
  const form = backend === "claude" ? claudeForm.value : codexForm.value;
  const cfg: ProviderConfig = {
    ...form,
    baseUrl: form.baseUrl?.trim() || null,
    apiKey: form.apiKey?.trim() || null,
  };
  const setSaving = backend === "claude" ? savingClaude : savingCodex;
  setSaving.value = true;
  try {
    await setProviderConfig(cfg);
    await refresh();
  } catch (err) {
    console.error("[settings] setProviderConfig failed", err);
  } finally {
    setSaving.value = false;
  }
}

async function saveCCSwitch() {
  const cfg: CCSwitchConfig = {
    baseUrl: ccSwitchForm.value.baseUrl?.trim() || null,
  };
  savingCCSwitch.value = true;
  try {
    await setCCSwitchConfig(cfg);
    await refresh();
  } catch (err) {
    console.error("[settings] setCCSwitchConfig failed", err);
  } finally {
    savingCCSwitch.value = false;
  }
}

// 直连卡里的 baseUrl/apiKey 字段仅在该 backend 选了 direct 路由时有意义；
// 切到 cc-switch 时折叠隐藏，避免误以为它在生效。
const claudeShowDirect = computed(() => claudeRouter.value === "direct");
const codexShowDirect = computed(() => codexRouter.value === "direct");
</script>

<template>
  <section>
    <div class="page-header">
      <div>
        <h1>设置</h1>
        <p>每个 backend 独立选择「经 CC-Switch」或「直连」；CC-Switch 代理 URL 两者共用。</p>
      </div>
    </div>

    <div class="card">
      <h2>外观</h2>
      <div class="settings-row">
        <div class="settings-row__label">
          <div>主题</div>
          <div class="settings-row__hint">选择应用配色，立即生效并记忆到本地。</div>
        </div>
        <div class="segmented" role="radiogroup" aria-label="主题">
          <button
            type="button"
            role="radio"
            :aria-checked="theme === 'dark'"
            :class="{ 'is-active': theme === 'dark' }"
            @click="setTheme('dark')"
          >
            <Moon :size="14" aria-hidden="true" />
            暗色
          </button>
          <button
            type="button"
            role="radio"
            :aria-checked="theme === 'light'"
            :class="{ 'is-active': theme === 'light' }"
            @click="setTheme('light')"
          >
            <Sun :size="14" aria-hidden="true" />
            浅色
          </button>
        </div>
      </div>
      <div class="settings-row">
        <div class="settings-row__label">
          <div>语言</div>
          <div class="settings-row__hint">界面语言</div>
        </div>
        <span class="muted">简体中文</span>
      </div>
    </div>

    <div class="card">
      <h2>运行环境</h2>
      <p class="muted" style="margin: 0 0 10px; font-size: 12px;">
        所有 backend 共用同一个 Node 子进程承载 SDK；Codex 还需要本机有 <code>codex</code> CLI（@openai/codex-sdk 是 wrapper）。
      </p>
      <div class="settings-row">
        <div class="settings-row__label">
          <div>Node.js</div>
          <div class="settings-row__hint">SDK 是 Node 包，须能在 PATH 中找到 <code>node</code>（v18+）。</div>
        </div>
        <span v-if="probing" class="status-pill status-pill--probing">
          <Loader2 :size="12" class="status-pill__spin" aria-hidden="true" />
          检查中
        </span>
        <span v-else-if="nodeAvailable" class="status-pill status-pill--ok">
          <CheckCircle2 :size="12" aria-hidden="true" />
          可用
        </span>
        <span v-else class="status-pill status-pill--err">
          <XCircle :size="12" aria-hidden="true" />
          未找到
        </span>
      </div>
      <div class="settings-row">
        <div class="settings-row__label">
          <div>Codex CLI</div>
          <div class="settings-row__hint">使用 Codex backend 时必需：<code>npm i -g @openai/codex</code>。</div>
        </div>
        <span v-if="probing" class="status-pill status-pill--probing">
          <Loader2 :size="12" class="status-pill__spin" aria-hidden="true" />
          检查中
        </span>
        <span v-else-if="codexCliAvailable" class="status-pill status-pill--ok">
          <CheckCircle2 :size="12" aria-hidden="true" />
          可用
        </span>
        <span v-else class="status-pill status-pill--muted">
          <XCircle :size="12" aria-hidden="true" />
          未找到
        </span>
      </div>
      <div class="settings-row">
        <div class="settings-row__label">
          <div>重新检测</div>
          <div class="settings-row__hint">启动 CC-Switch、改环境变量或装完 codex 后点一下，不用重启 Lilia。</div>
        </div>
        <button type="button" class="ghost" :disabled="probing" @click="probe">
          重新检测
        </button>
      </div>
    </div>

    <div class="card">
      <h2>
        <Network :size="14" aria-hidden="true" style="vertical-align: -2px; margin-right: 6px;" />
        CC-Switch 代理
      </h2>
      <p class="muted" style="margin: 0 0 10px; font-size: 12px;">
        <a href="https://github.com/farion1231/cc-switch" target="_blank" rel="noopener noreferrer">CC-Switch</a>
        是本地代理工具，能集中管理多套 API 凭据并 failover。Claude 与 Codex 共用同一个代理 URL，上游网关负责协议路由。
      </p>

      <div class="settings-row">
        <div class="settings-row__label">
          <div>代理 URL</div>
          <div class="settings-row__hint">默认 <code>http://127.0.0.1:15721</code>。</div>
        </div>
        <input
          type="text"
          class="text-input"
          placeholder="http://127.0.0.1:15721"
          :value="ccSwitchForm.baseUrl ?? ''"
          @input="(e) => (ccSwitchForm.baseUrl = (e.target as HTMLInputElement).value)"
        />
      </div>
      <div class="settings-row">
        <div class="settings-row__label">
          <div>代理可达性</div>
          <div class="settings-row__hint">实时 TCP 拨号 150ms 超时；保存后会自动重新检测。</div>
        </div>
        <span v-if="probing" class="status-pill status-pill--probing">
          <Loader2 :size="12" class="status-pill__spin" aria-hidden="true" />
          检查中
        </span>
        <span v-else-if="ccSwitch?.reachable" class="status-pill status-pill--ok">
          <CheckCircle2 :size="12" aria-hidden="true" />
          可达
        </span>
        <span v-else class="status-pill status-pill--muted">
          <XCircle :size="12" aria-hidden="true" />
          {{ ccSwitch?.baseUrl ? "未运行" : "未配置" }}
        </span>
      </div>
      <div class="settings-row">
        <div class="settings-row__label">
          <div>保存</div>
          <div class="settings-row__hint">保存后立即重新检测代理状态。</div>
        </div>
        <button type="button" class="ghost" :disabled="savingCCSwitch" @click="saveCCSwitch">
          <Save :size="12" aria-hidden="true" />
          {{ savingCCSwitch ? "保存中…" : "保存" }}
        </button>
      </div>
    </div>

    <div class="card">
      <h2>Claude</h2>
      <p class="muted" style="margin: 0 0 10px; font-size: 12px;">
        通过 Claude Agent SDK 与 Claude 通信。下面先选路由方式：经 CC-Switch 还是直连。
      </p>

      <div v-if="probing" class="conn-banner conn-banner--probing">
        <Loader2 :size="14" class="status-pill__spin" aria-hidden="true" />
        <div><div class="conn-banner__title">检查中…</div></div>
      </div>
      <div
        v-else-if="claudeMeta"
        class="conn-banner"
        :class="claudeMeta.ok ? 'conn-banner--ok' : 'conn-banner--err'"
      >
        <component :is="claudeMeta.ok ? Plug : AlertTriangle" :size="16" aria-hidden="true" />
        <div>
          <div class="conn-banner__title">{{ claudeMeta.label }}</div>
          <div class="conn-banner__hint">{{ claudeMeta.hint }}</div>
        </div>
      </div>

      <div class="settings-row">
        <div class="settings-row__label">
          <div>路由</div>
          <div class="settings-row__hint">
            {{ routerOptions.find(o => o.value === claudeRouter)?.hint }}
          </div>
        </div>
        <div class="segmented" role="radiogroup" aria-label="Claude 路由">
          <button
            v-for="opt in routerOptions"
            :key="opt.value"
            type="button"
            role="radio"
            :aria-checked="claudeRouter === opt.value"
            :class="{ 'is-active': claudeRouter === opt.value }"
            :disabled="savingClaudeRouter"
            :title="opt.hint"
            @click="chooseRouter('claude', opt.value)"
          >
            {{ opt.label }}
          </button>
        </div>
      </div>
      <template v-if="claudeShowDirect">
        <div class="settings-row">
          <div class="settings-row__label">
            <div>Base URL</div>
            <div class="settings-row__hint">留空走 Anthropic 官方 API。</div>
          </div>
          <input
            type="text"
            class="text-input"
            placeholder="https://api.anthropic.com"
            :value="claudeForm.baseUrl ?? ''"
            @input="(e) => (claudeForm.baseUrl = (e.target as HTMLInputElement).value)"
          />
        </div>
        <div class="settings-row">
          <div class="settings-row__label">
            <div>API Key</div>
            <div class="settings-row__hint">仅在内存与本地 store 中存储。</div>
          </div>
          <input
            type="password"
            class="text-input"
            placeholder="sk-ant-…"
            :value="claudeForm.apiKey ?? ''"
            @input="(e) => (claudeForm.apiKey = (e.target as HTMLInputElement).value)"
          />
        </div>
        <div class="settings-row">
          <div class="settings-row__label">
            <div>保存直连配置</div>
            <div class="settings-row__hint">保存后立即重新检测连接。</div>
          </div>
          <button type="button" class="ghost" :disabled="savingClaude" @click="saveBackend('claude')">
            <Save :size="12" aria-hidden="true" />
            {{ savingClaude ? "保存中…" : "保存" }}
          </button>
        </div>
      </template>
    </div>

    <div class="card">
      <h2>Codex</h2>
      <p class="muted" style="margin: 0 0 10px; font-size: 12px;">
        通过 <code>@openai/codex-sdk</code> 调本机 <code>codex</code> CLI。下面先选路由方式：经 CC-Switch 还是直连。
      </p>

      <div v-if="probing" class="conn-banner conn-banner--probing">
        <Loader2 :size="14" class="status-pill__spin" aria-hidden="true" />
        <div><div class="conn-banner__title">检查中…</div></div>
      </div>
      <div
        v-else-if="codexMeta"
        class="conn-banner"
        :class="codexMeta.ok ? 'conn-banner--ok' : 'conn-banner--err'"
      >
        <component :is="codexMeta.ok ? Plug : AlertTriangle" :size="16" aria-hidden="true" />
        <div>
          <div class="conn-banner__title">{{ codexMeta.label }}</div>
          <div class="conn-banner__hint">{{ codexMeta.hint }}</div>
        </div>
      </div>

      <div class="settings-row">
        <div class="settings-row__label">
          <div>路由</div>
          <div class="settings-row__hint">
            {{ routerOptions.find(o => o.value === codexRouter)?.hint }}
          </div>
        </div>
        <div class="segmented" role="radiogroup" aria-label="Codex 路由">
          <button
            v-for="opt in routerOptions"
            :key="opt.value"
            type="button"
            role="radio"
            :aria-checked="codexRouter === opt.value"
            :class="{ 'is-active': codexRouter === opt.value }"
            :disabled="savingCodexRouter"
            :title="opt.hint"
            @click="chooseRouter('codex', opt.value)"
          >
            {{ opt.label }}
          </button>
        </div>
      </div>
      <template v-if="codexShowDirect">
        <div class="settings-row">
          <div class="settings-row__label">
            <div>Base URL</div>
            <div class="settings-row__hint">留空走 OpenAI 官方。第三方兼容服务在此覆盖。</div>
          </div>
          <input
            type="text"
            class="text-input"
            placeholder="https://api.openai.com/v1"
            :value="codexForm.baseUrl ?? ''"
            @input="(e) => (codexForm.baseUrl = (e.target as HTMLInputElement).value)"
          />
        </div>
        <div class="settings-row">
          <div class="settings-row__label">
            <div>API Key</div>
            <div class="settings-row__hint">仅在内存与本地 store 中存储。</div>
          </div>
          <input
            type="password"
            class="text-input"
            placeholder="sk-…"
            :value="codexForm.apiKey ?? ''"
            @input="(e) => (codexForm.apiKey = (e.target as HTMLInputElement).value)"
          />
        </div>
        <div class="settings-row">
          <div class="settings-row__label">
            <div>保存直连配置</div>
            <div class="settings-row__hint">保存后立即重新检测连接。</div>
          </div>
          <button type="button" class="ghost" :disabled="savingCodex" @click="saveBackend('codex')">
            <Save :size="12" aria-hidden="true" />
            {{ savingCodex ? "保存中…" : "保存" }}
          </button>
        </div>
      </template>
    </div>

    <div class="card">
      <h2>关于</h2>
      <ul class="kv">
        <li><span>名称</span><span>Lilia</span></li>
        <li><span>版本</span><span>0.1.0</span></li>
      </ul>
    </div>
  </section>
</template>
