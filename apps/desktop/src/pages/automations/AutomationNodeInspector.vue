<script setup lang="ts">
import { computed } from "vue";
import type { AutomationNode } from "@lilia/contracts";
import { PRIORITY_OPTIONS } from "./constants";
import type { AutomationFlowNode } from "./types";

const props = defineProps<{
  selectedNode: AutomationFlowNode | null;
  configString: (key: string) => string;
  configBoolean: (key: string) => boolean;
}>();

const emit = defineEmits<{
  "update-title": [value: string];
  "update-config": [key: string, value: unknown];
}>();

const TOOL_ACTIONS_WITH_TITLE = new Set(["record_timeline", "create_task"]);
const TOOL_ACTIONS_WITH_TEXT = new Set(["add_todo", "send_guide"]);
const TOOL_ACTIONS_WITH_STATUS = new Set([
  "record_timeline",
  "create_task",
  "update_task_status",
]);

const selectedAutomationNode = computed<AutomationNode | null>(() => props.selectedNode?.data.node ?? null);

function selectedToolAction() {
  return props.configString("action") || "record_timeline";
}

function selectedToolUsesTaskId() {
  return selectedToolAction() !== "create_task";
}

function selectedToolUsesProjectId() {
  return selectedToolAction() === "create_task";
}

function selectedToolUsesTimelineFields() {
  return selectedToolAction() === "record_timeline";
}

function selectedToolUsesGuidePriority() {
  return selectedToolAction() === "send_guide";
}

function selectedToolUsesTitle() {
  return TOOL_ACTIONS_WITH_TITLE.has(selectedToolAction());
}

function selectedToolUsesText() {
  return TOOL_ACTIONS_WITH_TEXT.has(selectedToolAction());
}

function selectedToolUsesStatus() {
  return TOOL_ACTIONS_WITH_STATUS.has(selectedToolAction());
}
</script>

<template>
  <template v-if="selectedNode">
    <div class="automations-page__field">
      <label>标题</label>
      <input
        :value="selectedAutomationNode?.title"
        @input="emit('update-title', ($event.target as HTMLInputElement).value)"
      />
    </div>
    <template v-if="selectedAutomationNode?.kind === 'trigger'">
      <div class="automations-page__field">
        <label>触发类型</label>
        <select
          :value="configString('triggerKind') || 'manual'"
          @change="emit('update-config', 'triggerKind', ($event.target as HTMLSelectElement).value)"
        >
          <option value="manual">手动触发</option>
          <option value="task_changed">任务变化</option>
          <option value="timeline_event">时间线事件</option>
          <option value="todo_changed">Todo 变化</option>
          <option value="interaction_request">Agent 交互请求</option>
        </select>
      </div>
    </template>
    <template v-else-if="selectedAutomationNode?.kind === 'agent'">
      <label class="ui-switch">
        <input
          :checked="configBoolean('createTask')"
          type="checkbox"
          @change="emit('update-config', 'createTask', ($event.target as HTMLInputElement).checked)"
        />
        <span>新建任务</span>
      </label>
      <div class="automations-page__field">
        <label>Task ID</label>
        <input
          :value="configString('taskId')"
          placeholder="${trigger.taskId}"
          @input="emit('update-config', 'taskId', ($event.target as HTMLInputElement).value)"
        />
      </div>
      <div class="automations-page__field">
        <label>项目 ID</label>
        <input
          :value="configString('projectId')"
          placeholder="${trigger.projectId}"
          @input="emit('update-config', 'projectId', ($event.target as HTMLInputElement).value)"
        />
      </div>
      <div class="automations-page__field">
        <label>任务标题</label>
        <input
          :value="configString('title')"
          placeholder="自动化 Agent 任务"
          @input="emit('update-config', 'title', ($event.target as HTMLInputElement).value)"
        />
      </div>
      <div class="automations-page__field">
        <label>后端</label>
        <select
          :value="configString('backend') || 'claude'"
          @change="emit('update-config', 'backend', ($event.target as HTMLSelectElement).value)"
        >
          <option value="claude">Claude</option>
          <option value="codex">Codex</option>
        </select>
      </div>
      <div class="automations-page__field">
        <label for="automation-agent-model">模型</label>
        <input
          id="automation-agent-model"
          :value="configString('model')"
          placeholder="claude-sonnet-4-6 / gpt-5.5"
          @input="emit('update-config', 'model', ($event.target as HTMLInputElement).value)"
        />
      </div>
      <div class="automations-page__field">
        <label for="automation-agent-project-cwd">工作目录</label>
        <input
          id="automation-agent-project-cwd"
          :value="configString('projectCwd')"
          placeholder="${trigger.projectCwd}"
          @input="emit('update-config', 'projectCwd', ($event.target as HTMLInputElement).value)"
        />
      </div>
      <div class="automations-page__field">
        <label>Prompt</label>
        <textarea
          class="ui-input ui-textarea"
          :value="configString('prompt')"
          @input="emit('update-config', 'prompt', ($event.target as HTMLTextAreaElement).value)"
        />
      </div>
      <div class="automations-page__field">
        <label>权限</label>
        <select
          :value="configString('permission') || 'ask'"
          @change="emit('update-config', 'permission', ($event.target as HTMLSelectElement).value)"
        >
          <option value="ask">ask</option>
          <option value="readonly">readonly</option>
          <option value="full">full</option>
        </select>
      </div>
    </template>
    <template v-else-if="selectedAutomationNode?.kind === 'logic'">
      <div class="automations-page__field">
        <label>逻辑</label>
        <select
          :value="configString('logic') || 'condition'"
          @change="emit('update-config', 'logic', ($event.target as HTMLSelectElement).value)"
        >
          <option value="condition">条件</option>
          <option value="switch">Switch</option>
          <option value="stop">停止运行</option>
        </select>
      </div>
      <div class="automations-page__field">
        <label>路径</label>
        <input
          :value="configString('path')"
          placeholder="trigger.kind"
          @input="emit('update-config', 'path', ($event.target as HTMLInputElement).value)"
        />
      </div>
      <div class="automations-page__field">
        <label>等于</label>
        <input
          :value="configString('equals')"
          placeholder="task_changed"
          @input="emit('update-config', 'equals', ($event.target as HTMLInputElement).value)"
        />
      </div>
      <div
        v-if="configString('logic') === 'switch'"
        class="automations-page__field"
      >
        <label>分支值</label>
        <input
          :value="configString('cases')"
          placeholder="done, blocked"
          @input="emit('update-config', 'cases', ($event.target as HTMLInputElement).value)"
        />
      </div>
    </template>
    <template v-else-if="selectedAutomationNode?.kind === 'tool'">
      <div class="automations-page__field">
        <label for="automation-tool-action">动作</label>
        <select
          id="automation-tool-action"
          :value="configString('action') || 'record_timeline'"
          @change="emit('update-config', 'action', ($event.target as HTMLSelectElement).value)"
        >
          <option value="record_timeline">记录运行</option>
          <option value="create_task">创建任务</option>
          <option value="update_task_status">更新任务状态</option>
          <option value="add_todo">添加 Todo</option>
          <option value="send_guide">发送引导</option>
        </select>
      </div>
      <div
        v-if="selectedToolUsesTaskId()"
        class="automations-page__field"
      >
        <label>Task ID</label>
        <input
          :value="configString('taskId')"
          placeholder="${trigger.taskId}"
          @input="emit('update-config', 'taskId', ($event.target as HTMLInputElement).value)"
        />
      </div>
      <div
        v-if="selectedToolUsesProjectId()"
        class="automations-page__field"
      >
        <label>项目 ID</label>
        <input
          :value="configString('projectId')"
          placeholder="${trigger.projectId}"
          @input="emit('update-config', 'projectId', ($event.target as HTMLInputElement).value)"
        />
      </div>
      <div
        v-if="selectedToolUsesTitle()"
        class="automations-page__field"
      >
        <label>标题</label>
        <input
          :value="configString('title')"
          placeholder="自动化任务"
          @input="emit('update-config', 'title', ($event.target as HTMLInputElement).value)"
        />
      </div>
      <div
        v-if="selectedToolUsesText()"
        class="automations-page__field"
      >
        <label for="automation-tool-text">{{ selectedToolUsesGuidePriority() ? '引导内容' : 'Todo 内容' }}</label>
        <textarea
          id="automation-tool-text"
          class="ui-input ui-textarea"
          :value="configString('text')"
          placeholder="自动化 Todo"
          @input="emit('update-config', 'text', ($event.target as HTMLTextAreaElement).value)"
        />
      </div>
      <div
        v-if="selectedToolUsesTimelineFields()"
        class="automations-page__field"
      >
        <label>摘要</label>
        <textarea
          class="ui-input ui-textarea"
          :value="configString('summary')"
          placeholder="写入时间线的摘要"
          @input="emit('update-config', 'summary', ($event.target as HTMLTextAreaElement).value)"
        />
      </div>
      <div
        v-if="selectedToolUsesStatus()"
        class="automations-page__field"
      >
        <label>状态</label>
        <input
          :value="configString('status')"
          placeholder="waiting"
          @input="emit('update-config', 'status', ($event.target as HTMLInputElement).value)"
        />
      </div>
      <div
        v-if="selectedToolUsesTimelineFields()"
        class="automations-page__field"
      >
        <label>记录后端</label>
        <select
          :value="configString('backend') || 'claude'"
          @change="emit('update-config', 'backend', ($event.target as HTMLSelectElement).value)"
        >
          <option value="claude">Claude</option>
          <option value="codex">Codex</option>
        </select>
      </div>
      <div
        v-if="selectedToolUsesGuidePriority()"
        class="automations-page__field"
      >
        <label for="automation-tool-priority">优先级</label>
        <select
          id="automation-tool-priority"
          :value="configString('priority') || 'normal'"
          @change="emit('update-config', 'priority', ($event.target as HTMLSelectElement).value)"
        >
          <option v-for="priority in PRIORITY_OPTIONS" :key="priority" :value="priority">
            {{ priority }}
          </option>
        </select>
      </div>
    </template>
    <template v-else-if="selectedAutomationNode?.kind === 'human'">
      <div class="automations-page__field">
        <label>提示</label>
        <textarea
          class="ui-input ui-textarea"
          :value="configString('prompt')"
          @input="emit('update-config', 'prompt', ($event.target as HTMLTextAreaElement).value)"
        />
      </div>
    </template>
  </template>
  <div v-else class="automations-page__notice">选择一个节点</div>
</template>
