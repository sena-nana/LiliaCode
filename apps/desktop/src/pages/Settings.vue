<script setup lang="ts">
const claudeHome = "C:\\Users\\wangjunxue\\.claude";
</script>

<template>
  <section>
    <div class="page-header">
      <div>
        <h1>设置</h1>
        <p>当前仅占位，后续接入真实存储。</p>
      </div>
    </div>

    <div class="card">
      <h2>通用</h2>
      <ul class="kv">
        <li><span>主题</span><span>跟随系统</span></li>
        <li><span>语言</span><span>简体中文</span></li>
      </ul>
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
