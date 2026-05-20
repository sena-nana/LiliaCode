<script setup lang="ts">
import { Moon, Sun } from "lucide-vue-next";
import { useTheme } from "../composables/useTheme";

const { theme, setTheme } = useTheme();
const claudeHome = "C:\\Users\\wangjunxue\\.claude";
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
      <h2>Claude CLI</h2>
      <ul class="kv">
        <li><span>会话目录</span><span>{{ claudeHome }}</span></li>
        <li><span>状态</span><span class="muted">未连接（待实现）</span></li>
      </ul>
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
