<script setup lang="ts">
import { computed, onMounted } from "vue";
import {
  Moon, Sun, CheckCircle2, XCircle, Loader2,
  Plug, AlertTriangle,
} from "lucide-vue-next";
import { useTheme } from "../composables/useTheme";
import { useConnectionStatus } from "../composables/useConnectionStatus";
import type { ConnectionMode } from "../services/chat";

const { theme, setTheme } = useTheme();

// 与侧栏徽章共享同一个 ref，这里点「重新检测」侧栏也会跟着变。
const { status: env, probing, refresh } = useConnectionStatus();

async function probe() {
  await refresh();
}

onMounted(probe);

/** 三态卡片头：连接模式 + 一行解释，让用户一眼明白当前在和谁说话。 */
const modeMeta = computed<{
  label: string; hint: string; ok: boolean;
} | null>(() => {
  const e = env.value;
  if (!e) return null;
  const url = e.effectiveUrl ?? "—";
  const map: Record<ConnectionMode, { label: string; hint: string; ok: boolean }> = {
    "cc-switch": {
      label: "CC-Switch 本地代理",
      hint: `检测到 ${url} 在监听，请求会经 CC-Switch 转发到当前 active provider。`,
      ok: true,
    },
    custom: {
      label: "自定义 Base URL",
      hint: `使用环境变量 ANTHROPIC_BASE_URL=${url}`,
      ok: true,
    },
    direct: {
      label: "Anthropic 官方 API",
      hint: `直连 ${url}，使用环境变量里的 ANTHROPIC_API_KEY。`,
      ok: true,
    },
    unconfigured: {
      label: "未配置",
      hint: "未检测到 CC-Switch 本地代理（127.0.0.1:15721），也没有 ANTHROPIC_API_KEY。发送会失败。",
      ok: false,
    },
  };
  return map[e.connectionMode];
});
</script>

<template>
  <section>
    <div class="page-header">
      <div>
        <h1>设置</h1>
        <p>外观可立即生效；其他项后续会接入真实存储。</p>
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
      <h2>Claude 连接</h2>
      <p class="muted" style="margin: 0 0 10px; font-size: 12px;">
        Lilia 通过 Claude Agent SDK 与 Claude 通信。默认会先尝试连
        <a href="https://github.com/farion1231/cc-switch" target="_blank" rel="noopener noreferrer">CC-Switch</a>
        的本地代理（<code>127.0.0.1:15721</code>），探测失败再退回到用
        <code>ANTHROPIC_API_KEY</code> 直连官方 API。
      </p>

      <!-- 模式横幅：用户最先看的就是「现在到底连到哪了」。 -->
      <div
        v-if="probing"
        class="conn-banner conn-banner--probing"
      >
        <Loader2 :size="14" class="status-pill__spin" aria-hidden="true" />
        <div>
          <div class="conn-banner__title">检查中…</div>
        </div>
      </div>
      <div
        v-else-if="modeMeta"
        class="conn-banner"
        :class="modeMeta.ok ? 'conn-banner--ok' : 'conn-banner--err'"
      >
        <component
          :is="modeMeta.ok ? Plug : AlertTriangle"
          :size="16"
          aria-hidden="true"
        />
        <div>
          <div class="conn-banner__title">{{ modeMeta.label }}</div>
          <div class="conn-banner__hint">{{ modeMeta.hint }}</div>
        </div>
      </div>

      <div class="settings-row">
        <div class="settings-row__label">
          <div>CC-Switch 代理</div>
          <div class="settings-row__hint">扫描 127.0.0.1:15721；只要 CC-Switch 在跑就会被默认选中。</div>
        </div>
        <span v-if="probing" class="status-pill status-pill--probing">
          <Loader2 :size="12" class="status-pill__spin" aria-hidden="true" />
          检查中
        </span>
        <span v-else-if="env?.ccSwitchReachable" class="status-pill status-pill--ok">
          <CheckCircle2 :size="12" aria-hidden="true" />
          可达
        </span>
        <span v-else class="status-pill status-pill--err">
          <XCircle :size="12" aria-hidden="true" />
          未运行
        </span>
      </div>
      <div class="settings-row">
        <div class="settings-row__label">
          <div>ANTHROPIC_API_KEY</div>
          <div class="settings-row__hint">直连官方 API 时必填；走 CC-Switch 时可不设。</div>
        </div>
        <span v-if="probing" class="status-pill status-pill--probing">
          <Loader2 :size="12" class="status-pill__spin" aria-hidden="true" />
          检查中
        </span>
        <span v-else-if="env?.hasApiKey" class="status-pill status-pill--ok">
          <CheckCircle2 :size="12" aria-hidden="true" />
          已设置
        </span>
        <span v-else class="status-pill status-pill--muted">
          <XCircle :size="12" aria-hidden="true" />
          未设置
        </span>
      </div>
      <div class="settings-row">
        <div class="settings-row__label">
          <div>Node.js</div>
          <div class="settings-row__hint">SDK 是 Node 包，须能在 PATH 中找到 <code>node</code>（v18+）。</div>
        </div>
        <span v-if="probing" class="status-pill status-pill--probing">
          <Loader2 :size="12" class="status-pill__spin" aria-hidden="true" />
          检查中
        </span>
        <span v-else-if="env?.nodeAvailable" class="status-pill status-pill--ok">
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
          <div>重新检测</div>
          <div class="settings-row__hint">启动 CC-Switch 或修改环境变量后点一下，不用重启 Lilia。</div>
        </div>
        <button type="button" class="ghost" :disabled="probing" @click="probe">
          重新检测
        </button>
      </div>
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


