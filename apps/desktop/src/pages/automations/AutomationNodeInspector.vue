<script setup lang="ts">
import { computed } from "vue";
import {
  AUTOMATION_LOGIC_KINDS,
  AUTOMATION_LOGIC_KIND_LABELS,
  AUTOMATION_TRIGGER_KIND_LABELS,
  AUTOMATION_TRIGGER_KINDS,
  AUTOMATION_TOOL_ACTIONS,
  AUTOMATION_TOOL_ACTION_LABELS,
  AUTOMATION_TOOL_PRIORITIES,
  CHAT_BACKENDS,
  CHAT_BACKEND_LABELS,
  DEFAULT_CHAT_BACKEND,
  DEFAULT_AUTOMATION_LOGIC_KIND,
  DEFAULT_AUTOMATION_LOGIC_PATH,
  DEFAULT_AUTOMATION_TOOL_ACTION,
  DEFAULT_AUTOMATION_TOOL_PRIORITY,
  DEFAULT_MODEL_BY_BACKEND,
  PERMISSION_MODES,
  automationToolActionUsesField,
  normalizeAutomationLogicKind,
  normalizeAutomationToolAction,
  normalizePermissionMode,
  type AutomationNode,
  type AutomationToolConfigField,
} from "@lilia/contracts";
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

const selectedAutomationNode = computed<AutomationNode | null>(() => props.selectedNode?.data.node ?? null);
const chatBackendOptions = CHAT_BACKENDS.map((backend) => ({
  value: backend,
  label: CHAT_BACKEND_LABELS[backend],
}));
const modelPlaceholder = CHAT_BACKENDS.map((backend) => DEFAULT_MODEL_BY_BACKEND[backend]).join(" / ");

function selectedToolAction() {
  return normalizeAutomationToolAction(props.configString("action"));
}

function selectedToolUsesField(field: AutomationToolConfigField) {
  return automationToolActionUsesField(selectedToolAction(), field);
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
          <option v-for="kind in AUTOMATION_TRIGGER_KINDS" :key="kind" :value="kind">
            {{ AUTOMATION_TRIGGER_KIND_LABELS[kind] }}
          </option>
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
          :value="configString('backend') || DEFAULT_CHAT_BACKEND"
          @change="emit('update-config', 'backend', ($event.target as HTMLSelectElement).value)"
        >
          <option v-for="option in chatBackendOptions" :key="option.value" :value="option.value">
            {{ option.label }}
          </option>
        </select>
      </div>
      <div class="automations-page__field">
        <label for="automation-agent-model">模型</label>
        <input
          id="automation-agent-model"
          :value="configString('model')"
          :placeholder="modelPlaceholder"
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
          :value="normalizePermissionMode(configString('permission'))"
          @change="emit('update-config', 'permission', ($event.target as HTMLSelectElement).value)"
        >
          <option v-for="mode in PERMISSION_MODES" :key="mode" :value="mode">{{ mode }}</option>
        </select>
      </div>
    </template>
    <template v-else-if="selectedAutomationNode?.kind === 'logic'">
      <div class="automations-page__field">
        <label>逻辑</label>
        <select
          :value="normalizeAutomationLogicKind(configString('logic'))"
          @change="emit('update-config', 'logic', ($event.target as HTMLSelectElement).value)"
        >
          <option v-for="kind in AUTOMATION_LOGIC_KINDS" :key="kind" :value="kind">
            {{ AUTOMATION_LOGIC_KIND_LABELS[kind] }}
          </option>
        </select>
      </div>
      <div class="automations-page__field">
        <label>路径</label>
        <input
          :value="configString('path')"
          :placeholder="DEFAULT_AUTOMATION_LOGIC_PATH"
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
        v-if="normalizeAutomationLogicKind(configString('logic'), DEFAULT_AUTOMATION_LOGIC_KIND) === 'switch'"
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
          :value="configString('action') || DEFAULT_AUTOMATION_TOOL_ACTION"
          @change="emit('update-config', 'action', ($event.target as HTMLSelectElement).value)"
        >
          <option v-for="action in AUTOMATION_TOOL_ACTIONS" :key="action" :value="action">
            {{ AUTOMATION_TOOL_ACTION_LABELS[action] }}
          </option>
        </select>
      </div>
      <div
        v-if="selectedToolUsesField('taskId')"
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
        v-if="selectedToolUsesField('projectId')"
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
        v-if="selectedToolUsesField('title')"
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
        v-if="selectedToolUsesField('text')"
        class="automations-page__field"
      >
        <label for="automation-tool-text">{{ selectedToolUsesField('priority') ? '引导内容' : 'Todo 内容' }}</label>
        <textarea
          id="automation-tool-text"
          class="ui-input ui-textarea"
          :value="configString('text')"
          placeholder="自动化 Todo"
          @input="emit('update-config', 'text', ($event.target as HTMLTextAreaElement).value)"
        />
      </div>
      <div
        v-if="selectedToolUsesField('summary')"
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
        v-if="selectedToolUsesField('status')"
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
        v-if="selectedToolUsesField('backend')"
        class="automations-page__field"
      >
        <label>记录后端</label>
        <select
          :value="configString('backend') || DEFAULT_CHAT_BACKEND"
          @change="emit('update-config', 'backend', ($event.target as HTMLSelectElement).value)"
        >
          <option v-for="option in chatBackendOptions" :key="option.value" :value="option.value">
            {{ option.label }}
          </option>
        </select>
      </div>
      <div
        v-if="selectedToolUsesField('priority')"
        class="automations-page__field"
      >
        <label for="automation-tool-priority">优先级</label>
        <select
          id="automation-tool-priority"
          :value="configString('priority') || DEFAULT_AUTOMATION_TOOL_PRIORITY"
          @change="emit('update-config', 'priority', ($event.target as HTMLSelectElement).value)"
        >
          <option v-for="priority in AUTOMATION_TOOL_PRIORITIES" :key="priority" :value="priority">
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
