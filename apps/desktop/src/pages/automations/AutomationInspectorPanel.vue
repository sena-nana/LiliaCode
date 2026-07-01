<script setup lang="ts">
import { computed } from "vue";
import CircleHelp from "@lucide/vue/dist/esm/icons/circle-question-mark.mjs";
import Play from "@lucide/vue/dist/esm/icons/play.mjs";
import {
  DEFAULT_AUTOMATION_HUMAN_PROMPT,
  AUTOMATION_WAITING_USER_STATUS,
  automationRunStatusTone,
  normalizeAutomationRunStatus,
  taskStatusLabel,
} from "@lilia/contracts";
import type {
  AutomationRunNodeState,
  AutomationRunSummary,
  AutomationScopeFilter,
  ChatBackendKind,
  Project,
} from "@lilia/contracts";
import {
  BACKEND_OPTIONS,
  TASK_STATUS_OPTIONS,
  TRIGGER_EVENT_KIND_OPTIONS,
} from "./constants";
import AutomationNodeInspector from "./AutomationNodeInspector.vue";
import type { AutomationFlowNode } from "./types";

const props = defineProps<{
  scope: AutomationScopeFilter;
  projectRows: Project[];
  selectedNode: AutomationFlowNode | null;
  configString: (key: string) => string;
  configBoolean: (key: string) => boolean;
  runs: AutomationRunSummary[];
  selectedRunId: string | null;
  selectedRunNodeId: string | null;
  selectedRunNodeStates: AutomationRunNodeState[];
  selectedRunNodeState: AutomationRunNodeState | null;
  resuming: boolean;
}>();

const emit = defineEmits<{
  "toggle-scope-include-inbox": [];
  "toggle-scope-list": [key: "projectIds" | "taskStatuses" | "eventKinds", value: string];
  "toggle-scope-backend": [value: ChatBackendKind];
  "update-title": [value: string];
  "update-config": [key: string, value: unknown];
  "select-run": [run: AutomationRunSummary];
  "select-run-node-state": [state: AutomationRunNodeState];
  "resume-selected-run": [];
}>();

const selectedRunHumanPrompt = computed(() => {
  const output = props.selectedRunNodeState?.output;
  const prompt = output && typeof output.prompt === "string" ? output.prompt.trim() : "";
  return prompt || DEFAULT_AUTOMATION_HUMAN_PROMPT;
});

function formatDuration(state: AutomationRunNodeState | null): string {
  if (!state?.startedAt || !state.finishedAt) return "-";
  return `${Math.max(0, state.finishedAt - state.startedAt)} ms`;
}

function formatJson(value: unknown): string {
  return JSON.stringify(value ?? null, null, 2);
}

function runContextMeta(run: AutomationRunSummary): string {
  const items = [
    run.projectId ? `项目 ${run.projectId}` : "收集箱",
    run.taskId ? `任务 ${run.taskId}` : null,
    run.backend ? `后端 ${run.backend}` : null,
    run.eventKind ? `事件 ${run.eventKind}` : null,
  ].filter(Boolean);
  return items.join(" · ");
}

function runStatusClass(status: string) {
  return `ui-badge--${automationRunStatusTone(normalizeAutomationRunStatus(status))}`;
}

function formatTime(value: number | null): string {
  if (!value) return "-";
  return new Date(value).toLocaleString();
}
</script>

<template>
  <div class="automations-page__inspector-body">
    <section class="automations-page__section">
      <h3 class="automations-page__section-title">作用域</h3>
      <label class="ui-switch">
        <input
          :checked="scope.includeInbox"
          type="checkbox"
          data-agent-id="automations.scope.include-inbox"
          @change="emit('toggle-scope-include-inbox')"
        />
        <span>包含收集箱</span>
      </label>
      <div class="automations-page__field">
        <label>项目</label>
        <div class="automations-page__chips">
          <button
            v-for="project in projectRows"
            :key="project.id"
            type="button"
            class="automations-page__chip"
            :class="{ 'is-active': scope.projectIds.includes(project.id) }"
            :data-agent-id="`automations.scope.project.${project.id}`"
            @click="emit('toggle-scope-list', 'projectIds', project.id)"
          >
            {{ project.name }}
          </button>
        </div>
      </div>
      <div class="automations-page__field">
        <label>任务状态</label>
        <div class="automations-page__chips">
          <button
            v-for="status in TASK_STATUS_OPTIONS"
            :key="status"
            type="button"
            class="automations-page__chip"
            :class="{ 'is-active': scope.taskStatuses.includes(status) }"
            :data-agent-id="`automations.scope.task-status.${status}`"
            @click="emit('toggle-scope-list', 'taskStatuses', status)"
          >
            {{ taskStatusLabel(status) }}
          </button>
        </div>
      </div>
      <div class="automations-page__field">
        <label>Agent 后端</label>
        <div class="automations-page__chips">
          <button
            v-for="backend in BACKEND_OPTIONS"
            :key="backend"
            type="button"
            class="automations-page__chip"
            :class="{ 'is-active': scope.backends.includes(backend) }"
            :data-agent-id="`automations.scope.backend.${backend}`"
            @click="emit('toggle-scope-backend', backend)"
          >
            {{ backend }}
          </button>
        </div>
      </div>
      <div class="automations-page__field">
        <label>事件 kind 过滤</label>
        <div class="automations-page__chips">
          <button
            v-for="eventKind in TRIGGER_EVENT_KIND_OPTIONS"
            :key="eventKind"
            type="button"
            class="automations-page__chip"
            :class="{ 'is-active': scope.eventKinds.includes(eventKind) }"
            :data-agent-id="`automations.scope.event-kind.${eventKind}`"
            @click="emit('toggle-scope-list', 'eventKinds', eventKind)"
          >
            {{ eventKind }}
          </button>
        </div>
      </div>
    </section>

    <section class="automations-page__section">
      <h3 class="automations-page__section-title">节点</h3>
      <AutomationNodeInspector
        :selected-node="selectedNode"
        :config-string="configString"
        :config-boolean="configBoolean"
        @update-title="emit('update-title', $event)"
        @update-config="(key, value) => emit('update-config', key, value)"
      />
    </section>

    <section class="automations-page__section">
      <h3 class="automations-page__section-title">运行历史</h3>
      <div v-if="!runs.length" class="automations-page__notice">没有运行记录</div>
      <div v-else class="automations-page__runs">
        <button
          v-for="run in runs"
          :key="run.id"
          type="button"
          class="automations-page__run"
          :class="{ 'is-active': run.id === selectedRunId }"
          :data-agent-id="`automations.run.${run.id}`"
          @click="emit('select-run', run)"
        >
          <span class="automations-page__run-main">
            <span class="automations-page__run-title">{{ run.triggerKind }}</span>
            <span class="automations-page__run-meta">{{ formatTime(run.startedAt) }}</span>
            <span class="automations-page__run-meta">{{ runContextMeta(run) }}</span>
            <span v-if="run.error" class="automations-page__run-error">{{ run.error }}</span>
          </span>
          <span class="ui-badge automations-page__status" :class="runStatusClass(run.status)">
            {{ run.status }}
          </span>
        </button>
      </div>
    </section>

    <section class="automations-page__section">
      <h3 class="automations-page__section-title">节点状态</h3>
      <div v-if="!selectedRunNodeStates.length" class="automations-page__notice">选择运行记录后查看节点状态</div>
      <div v-else class="automations-page__node-states">
        <button
          v-for="state in selectedRunNodeStates"
          :key="state.id"
          type="button"
          class="automations-page__node-state"
          :class="{ 'is-active': state.nodeId === selectedRunNodeId }"
          :data-agent-id="`automations.run-node.${state.nodeId}`"
          @click="emit('select-run-node-state', state)"
        >
          <span>{{ state.nodeId }}</span>
          <span class="ui-badge automations-page__status" :class="runStatusClass(state.status)">
            {{ state.status }}
          </span>
        </button>
      </div>
      <div v-if="selectedRunNodeState" class="automations-page__replay">
        <section
          v-if="selectedRunNodeState.status === AUTOMATION_WAITING_USER_STATUS"
          class="composer-inline composer-inline--ask automations-page__human-ask"
          role="region"
          aria-label="自动化等待确认"
        >
          <header class="composer-inline__header">
            <span class="composer-inline__icon" aria-hidden="true">
              <CircleHelp :size="14" />
            </span>
            <span class="composer-inline__title">自动化等待确认</span>
            <span class="composer-inline__source">Automation</span>
          </header>
          <div class="composer-inline__body">
            <div class="composer-inline__question">
              <span class="composer-inline__chip">人工节点</span>
              <p class="composer-inline__qtext">{{ selectedRunHumanPrompt }}</p>
            </div>
          </div>
          <footer class="composer-inline__actions">
            <span class="composer-inline__spacer" />
            <button
              type="button"
              class="ui-button ui-button--primary composer-inline__btn"
              data-agent-id="automations.run.resume"
              :disabled="resuming"
              @click="emit('resume-selected-run')"
            >
              <Play :size="14" aria-hidden="true" />
              {{ resuming ? "继续中" : "继续" }}
            </button>
          </footer>
        </section>
        <ul class="kv">
          <li>
            <span>耗时</span>
            <span>{{ formatDuration(selectedRunNodeState) }}</span>
          </li>
          <li v-if="selectedRunNodeState.error">
            <span>错误</span>
            <span>{{ selectedRunNodeState.error }}</span>
          </li>
        </ul>
        <details open>
          <summary>输入</summary>
          <pre>{{ formatJson(selectedRunNodeState.input) }}</pre>
        </details>
        <details open>
          <summary>输出</summary>
          <pre>{{ formatJson(selectedRunNodeState.output) }}</pre>
        </details>
      </div>
    </section>
  </div>
</template>

