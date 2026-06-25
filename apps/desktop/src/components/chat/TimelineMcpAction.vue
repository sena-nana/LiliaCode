<script setup lang="ts">
import type { PendingAgentAction } from "../../composables/pendingAgentActions";
import type { TimelineMcpFormField, TimelineMcpFormValues } from "./timelineMcpForm";

type McpAction = Extract<PendingAgentAction, { kind: "mcp_elicitation" }>;

withDefaults(defineProps<{
  action: McpAction;
  autoDecisionText?: string;
  canSubmit: boolean;
  disabled?: boolean;
  fieldInputType: (type: string) => string;
  fields: TimelineMcpFormField[];
  jsonText: string;
  multiSelected: (key: string, value: string) => boolean;
  values: TimelineMcpFormValues;
}>(), {
  autoDecisionText: "",
  disabled: false,
});

const emit = defineEmits<{
  accept: [];
  cancel: [];
  decline: [];
  toggleMultiValue: [key: string, value: string];
  updateField: [key: string, value: unknown];
  updateJsonText: [value: string];
}>();
</script>

<template>
  <section
    class="timeline-pending-action timeline-pending-action--codex"
    role="region"
    aria-label="MCP 确认"
    data-agent-id="timeline.mcp"
  >
    <div class="timeline-pending-action__stack">
      <div class="timeline-pending-action__title-preview">
        {{ action.payload.serverName }} · {{ action.payload.message }}
      </div>
      <a
        v-if="action.payload.mode === 'url' && action.payload.url"
        class="timeline-pending-action__link"
        data-agent-id="timeline.mcp.url"
        :href="action.payload.url"
        target="_blank"
        rel="noreferrer"
      >
        {{ action.payload.url }}
      </a>
      <div
        v-else-if="action.payload.mode === 'form'"
        class="timeline-pending-action__fields"
      >
        <label
          v-for="field in fields"
          :key="field.key"
          class="timeline-pending-action__field"
        >
          <span class="timeline-pending-action__field-label">
            {{ field.label }}{{ field.required ? " *" : "" }}
          </span>
          <select
            v-if="field.options.length && !field.multi"
            :value="String(values[field.key] ?? '')"
            class="timeline-pending-action__input"
            :data-agent-id="`timeline.mcp.field.${field.key}`"
            @change="emit('updateField', field.key, ($event.target as HTMLSelectElement).value)"
          >
            <option
              v-for="option in field.options"
              :key="option.value"
              :value="option.value"
            >
              {{ option.label }}
            </option>
          </select>
          <div v-else-if="field.options.length && field.multi" class="timeline-pending-action__checks">
            <label
              v-for="option in field.options"
              :key="option.value"
              class="timeline-pending-action__check"
            >
              <input
                :checked="multiSelected(field.key, option.value)"
                type="checkbox"
                :value="option.value"
                :data-agent-id="`timeline.mcp.field.${field.key}.${option.value}`"
                @change="emit('toggleMultiValue', field.key, option.value)"
              />
              <span>{{ option.label }}</span>
            </label>
          </div>
          <label v-else-if="field.type === 'boolean'" class="timeline-pending-action__check">
            <input
              :checked="values[field.key] === true"
              type="checkbox"
              :data-agent-id="`timeline.mcp.field.${field.key}`"
              @change="emit('updateField', field.key, ($event.target as HTMLInputElement).checked)"
            />
            <span>{{ field.description || "启用" }}</span>
          </label>
          <input
            v-else
            :value="String(values[field.key] ?? '')"
            class="timeline-pending-action__input"
            :type="fieldInputType(field.type)"
            :data-agent-id="`timeline.mcp.field.${field.key}`"
            @input="emit('updateField', field.key, ($event.target as HTMLInputElement).value)"
          />
        </label>
        <textarea
          v-if="fields.length === 0"
          :value="jsonText"
          class="timeline-pending-action__input timeline-pending-action__input--json"
          rows="3"
          placeholder="JSON content"
          data-agent-id="timeline.mcp.json"
          @input="emit('updateJsonText', ($event.target as HTMLTextAreaElement).value)"
        />
      </div>
      <div
        v-if="autoDecisionText"
        class="composer-inline__auto-decision timeline-pending-action__auto-decision"
        role="status"
      >
        {{ autoDecisionText }}
      </div>
      <div class="composer-inline__actions">
        <button
          type="button"
          class="ui-button ui-button--ghost composer-inline__btn"
          data-agent-id="timeline.mcp.cancel"
          :disabled="disabled"
          @click="emit('cancel')"
        >
          取消
        </button>
        <button
          type="button"
          class="ui-button ui-button--ghost composer-inline__btn"
          data-agent-id="timeline.mcp.decline"
          :disabled="disabled"
          @click="emit('decline')"
        >
          拒绝
        </button>
        <button
          type="button"
          class="ui-button ui-button--primary composer-inline__btn"
          data-agent-id="timeline.mcp.accept"
          :disabled="disabled || !canSubmit"
          @click="emit('accept')"
        >
          同意
        </button>
      </div>
    </div>
  </section>
</template>
