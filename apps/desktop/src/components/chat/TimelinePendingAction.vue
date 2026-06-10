<script setup lang="ts">
import { computed, ref, watch } from "vue";
import type { AskUserResult } from "@lilia/contracts";
import { useAskUserInteraction } from "../../composables/useAskUserInteraction";
import { useEditableToolCommand } from "../../composables/useEditableToolCommand";
import type {
  PendingAgentAction,
  PendingAgentActionResolution,
} from "../../composables/usePendingAgentActions";
import { useToolConsentPresentation } from "../../composables/useToolConsentPresentation";
import type { ToolConsentDecision } from "../../services/chat";
import AskUserInlinePrompt from "./AskUserInlinePrompt.vue";
import ToolConsentInlinePrompt from "./ToolConsentInlinePrompt.vue";

const props = defineProps<{
  action: PendingAgentAction;
}>();

const emit = defineEmits<{
  resolve: [resolution: PendingAgentActionResolution];
}>();

const freeformText = ref("");
const toolExpanded = ref(false);
const toolMessage = ref("");
const toolSubmitting = ref<ToolConsentDecision | null>(null);
const codexSubmitting = ref(false);
const mcpValues = ref<Record<string, unknown>>({});
const mcpJsonText = ref("{}");

const actionKey = computed(() =>
  props.action.kind === "tool_consent"
    ? `tool:${props.action.requestId}`
    : props.action.kind === "title_update"
      ? `title:${props.action.requestId}`
      : props.action.kind === "mcp_elicitation"
        ? `mcp:${props.action.requestId}`
        : props.action.kind === "permission_approval"
          ? `permission:${props.action.requestId}`
          : `ask:${props.action.ask.id}`,
);
const activeAsk = computed(() =>
  props.action.kind === "ask_user" || props.action.kind === "plan_approval"
    ? props.action.ask
    : null,
);
const {
  askIndex,
  askTotal,
  askQuestion,
  askDismissable,
  askIsLast,
  canGoPrev,
  askTitle,
  askOptionsWithId,
  askHasPreview,
  askFocusedOption,
  askOtherSelected,
  canAskSubmit,
  activeOptionId,
  singlePick,
  multiPicks,
  focusOption,
  highlightOption,
  clearOptionHighlight,
  selectSingleOption,
  toggleMulti,
  submitAsk,
  submitAskFreeform,
  confirmAskNo,
  skipAsk,
  backAsk,
  cancelAsk,
} = useAskUserInteraction(activeAsk, freeformText, resolveAsk);

const toolRequest = computed(() =>
  props.action.kind === "tool_consent" ? props.action.request : null,
);
const titleUpdateAction = computed(() =>
  props.action.kind === "title_update" ? props.action : null,
);
const { toolDanger, toolIcon, toolHeadline, toolInputJson, toolSubtitle } =
  useToolConsentPresentation(toolRequest);
const {
  commandDraft: toolCommandDraft,
  isEditingCommand: isEditingToolCommand,
  hasEditableCommand,
  commandIsEmpty: toolCommandIsEmpty,
  updatedCommandInput,
  beginCommandEdit,
  cancelCommandEdit,
} = useEditableToolCommand(toolRequest);
const hasFreeformText = computed(() => freeformText.value.trim().length > 0);
const hasToolMessage = computed(() => toolMessage.value.trim().length > 0);
const mcpAction = computed(() =>
  props.action.kind === "mcp_elicitation" ? props.action : null,
);
const permissionAction = computed(() =>
  props.action.kind === "permission_approval" ? props.action : null,
);
const mcpFields = computed(() => {
  const schema = mcpAction.value?.payload.requestedSchema;
  if (!schema || typeof schema !== "object" || Array.isArray(schema)) return [];
  const row = schema as Record<string, unknown>;
  const properties = row.properties;
  if (!properties || typeof properties !== "object" || Array.isArray(properties)) return [];
  const required = Array.isArray(row.required)
    ? new Set(row.required.filter((item): item is string => typeof item === "string"))
    : new Set<string>();
  return Object.entries(properties as Record<string, unknown>).map(([key, value]) => {
    const field = value && typeof value === "object" && !Array.isArray(value)
      ? value as Record<string, unknown>
      : {};
    const options = enumOptions(field);
    return {
      key,
      label: stringValue(field.title) || key,
      description: stringValue(field.description),
      type: stringValue(field.type) || "string",
      required: required.has(key),
      options,
      multi: stringValue(field.type) === "array",
      defaultValue: field.default,
    };
  });
});
const canSubmitMcp = computed(() => {
  if (mcpAction.value?.payload.mode !== "form") return true;
  if (mcpFields.value.length === 0) {
    try {
      JSON.parse(mcpJsonText.value || "{}");
      return true;
    } catch {
      return false;
    }
  }
  return mcpFields.value.every((field) => {
    if (!field.required) return true;
    const value = mcpValues.value[field.key];
    if (Array.isArray(value)) return value.length > 0;
    if (typeof value === "boolean") return true;
    return String(value ?? "").trim().length > 0;
  });
});
const permissionJson = computed(() =>
  JSON.stringify(permissionAction.value?.payload.permissions ?? {}, null, 2),
);

function stringValue(value: unknown): string {
  return typeof value === "string" ? value : "";
}

function enumOptions(field: Record<string, unknown>): Array<{ value: string; label: string }> {
  const rawEnum = Array.isArray(field.enum) ? field.enum : null;
  if (rawEnum) {
    return rawEnum
      .map((value) => typeof value === "string" ? { value, label: value } : null)
      .filter((value): value is { value: string; label: string } => value !== null);
  }
  const oneOf = Array.isArray(field.oneOf)
    ? field.oneOf
    : Array.isArray((field.items as Record<string, unknown> | undefined)?.anyOf)
      ? (field.items as Record<string, unknown>).anyOf as unknown[]
      : null;
  if (!oneOf) return [];
  return oneOf
    .map((item) => {
      if (!item || typeof item !== "object" || Array.isArray(item)) return null;
      const row = item as Record<string, unknown>;
      const value = stringValue(row.const);
      if (!value) return null;
      return { value, label: stringValue(row.title) || value };
    })
    .filter((value): value is { value: string; label: string } => value !== null);
}

function mcpFieldInputType(type: string): string {
  if (type === "number" || type === "integer") return "number";
  return "text";
}

function mcpMultiSelected(key: string, value: string): boolean {
  const current = mcpValues.value[key];
  return Array.isArray(current) && current.includes(value);
}

function toggleMcpMultiValue(key: string, value: string) {
  const current = mcpValues.value[key];
  const values = Array.isArray(current)
    ? current.filter((item): item is string => typeof item === "string")
    : [];
  mcpValues.value[key] = values.includes(value)
    ? values.filter((item) => item !== value)
    : [...values, value];
}

function mcpContentFromForm(): Record<string, unknown> {
  if (mcpFields.value.length === 0) {
    const parsed = JSON.parse(mcpJsonText.value || "{}");
    return parsed && typeof parsed === "object" && !Array.isArray(parsed) ? parsed : {};
  }
  const content: Record<string, unknown> = {};
  for (const field of mcpFields.value) {
    const raw = mcpValues.value[field.key];
    if (field.type === "number" || field.type === "integer") {
      const number = typeof raw === "number" ? raw : Number(raw);
      if (Number.isFinite(number)) content[field.key] = field.type === "integer"
        ? Math.trunc(number)
        : number;
      continue;
    }
    if (field.type === "boolean") {
      content[field.key] = raw === true;
      continue;
    }
    if (Array.isArray(raw)) {
      content[field.key] = raw;
      continue;
    }
    const text = String(raw ?? "");
    if (text || field.required) content[field.key] = text;
  }
  return content;
}

function resolveAsk(result: AskUserResult) {
  if (props.action.kind !== "ask_user" && props.action.kind !== "plan_approval") return;
  emit("resolve", {
    kind: props.action.kind,
    requestId: props.action.requestId,
    askId: props.action.ask.id,
    result,
  });
}

function decideTool(decision: ToolConsentDecision) {
  if (props.action.kind !== "tool_consent" || toolSubmitting.value) return;
  if (decision === "allow" && toolCommandIsEmpty.value) return;
  toolSubmitting.value = decision;
  const updatedInput = decision === "allow" ? updatedCommandInput.value : undefined;
  emit("resolve", {
    kind: "tool_consent",
    requestId: props.action.requestId,
    decision,
    message: decision === "deny"
      ? toolMessage.value.trim() || "用户拒绝了此次工具调用"
      : undefined,
    ...(updatedInput ? { updatedInput } : {}),
  });
}

function decideTitleUpdate(decision: "accept" | "decline") {
  if (props.action.kind !== "title_update") return;
  emit("resolve", {
    kind: "title_update",
    requestId: props.action.requestId,
    decision,
  });
}

function decideMcp(action: "accept" | "decline" | "cancel") {
  if (props.action.kind !== "mcp_elicitation" || codexSubmitting.value) return;
  if (action === "accept" && !canSubmitMcp.value) return;
  codexSubmitting.value = true;
  emit("resolve", {
    kind: "mcp_elicitation",
    requestId: props.action.requestId,
    action,
    ...(action === "accept" && props.action.payload.mode === "form"
      ? { content: mcpContentFromForm() }
      : {}),
  });
}

function decidePermission(decision: "allow" | "deny") {
  if (props.action.kind !== "permission_approval" || codexSubmitting.value) return;
  codexSubmitting.value = true;
  emit("resolve", {
    kind: "permission_approval",
    requestId: props.action.requestId,
    decision,
  });
}

watch(actionKey, () => {
  toolExpanded.value = false;
  toolMessage.value = "";
  toolSubmitting.value = null;
  codexSubmitting.value = false;
  mcpJsonText.value = "{}";
  mcpValues.value = {};
  for (const field of mcpFields.value) {
    if (field.defaultValue !== undefined) {
      mcpValues.value[field.key] = field.defaultValue;
    } else if (field.type === "boolean") {
      mcpValues.value[field.key] = false;
    } else if (field.multi) {
      mcpValues.value[field.key] = [];
    } else {
      mcpValues.value[field.key] = "";
    }
  }
}, { immediate: true });
</script>

<template>
  <ToolConsentInlinePrompt
    v-if="props.action.kind === 'tool_consent' && toolRequest"
    root-class="timeline-pending-action composer-inline composer-inline--tool"
    :active-tool-consent="toolRequest"
    :tool-danger="toolDanger"
    :tool-icon="toolIcon"
    :tool-headline="toolHeadline"
    :tool-input-json="toolInputJson"
    :tool-subtitle="toolSubtitle"
    :tool-expanded="toolExpanded"
    :is-editing-tool-command="isEditingToolCommand"
    :has-editable-command="hasEditableCommand"
    :tool-command-draft="toolCommandDraft"
    @update-tool-expanded="toolExpanded = $event"
    @update-tool-command-draft="toolCommandDraft = $event"
    @begin-command-edit="beginCommandEdit"
  >
    <div class="timeline-pending-action__row">
      <textarea
        v-model="toolMessage"
        class="timeline-pending-action__input"
        rows="1"
        placeholder="拒绝理由"
      />
      <div class="composer-inline__actions">
        <button
          type="button"
          class="ui-button ui-button--ghost composer-inline__btn"
          :disabled="toolSubmitting !== null || (!isEditingToolCommand && !hasToolMessage)"
          @click="isEditingToolCommand ? cancelCommandEdit() : decideTool('deny')"
        >
          {{ toolSubmitting === "deny" ? "处理中..." : isEditingToolCommand ? "取消" : hasToolMessage ? "修改" : "忽略" }}
        </button>
        <button
          type="button"
          class="ui-button ui-button--primary composer-inline__btn"
          :disabled="toolSubmitting !== null || toolCommandIsEmpty"
          @click="decideTool('allow')"
        >
          {{ toolSubmitting === "allow" ? "处理中..." : toolDanger ? "同意执行" : "同意" }}
        </button>
      </div>
    </div>
  </ToolConsentInlinePrompt>

  <section
    v-else-if="titleUpdateAction"
    class="timeline-pending-action timeline-pending-action--title"
    role="region"
    aria-label="标题更新确认"
  >
    <span class="timeline-pending-action__title-preview">
      {{ titleUpdateAction.proposedTitle }}
    </span>
    <div class="composer-inline__actions">
      <button
        type="button"
        class="ui-button ui-button--ghost composer-inline__btn"
        @click="decideTitleUpdate('decline')"
      >
        忽略
      </button>
      <button
        type="button"
        class="ui-button ui-button--primary composer-inline__btn"
        @click="decideTitleUpdate('accept')"
      >
        同意
      </button>
    </div>
  </section>

  <section
    v-else-if="mcpAction"
    class="timeline-pending-action timeline-pending-action--codex"
    role="region"
    aria-label="MCP 确认"
  >
    <div class="timeline-pending-action__stack">
      <div class="timeline-pending-action__title-preview">
        {{ mcpAction.payload.serverName }} · {{ mcpAction.payload.message }}
      </div>
      <a
        v-if="mcpAction.payload.mode === 'url' && mcpAction.payload.url"
        class="timeline-pending-action__link"
        :href="mcpAction.payload.url"
        target="_blank"
        rel="noreferrer"
      >
        {{ mcpAction.payload.url }}
      </a>
      <div
        v-else-if="mcpAction.payload.mode === 'form'"
        class="timeline-pending-action__fields"
      >
        <label
          v-for="field in mcpFields"
          :key="field.key"
          class="timeline-pending-action__field"
        >
          <span class="timeline-pending-action__field-label">
            {{ field.label }}{{ field.required ? " *" : "" }}
          </span>
          <select
            v-if="field.options.length && !field.multi"
            v-model="mcpValues[field.key]"
            class="timeline-pending-action__input"
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
                :checked="mcpMultiSelected(field.key, option.value)"
                type="checkbox"
                :value="option.value"
                @change="toggleMcpMultiValue(field.key, option.value)"
              />
              <span>{{ option.label }}</span>
            </label>
          </div>
          <label v-else-if="field.type === 'boolean'" class="timeline-pending-action__check">
            <input v-model="mcpValues[field.key]" type="checkbox" />
            <span>{{ field.description || "启用" }}</span>
          </label>
          <input
            v-else
            v-model="mcpValues[field.key]"
            class="timeline-pending-action__input"
            :type="mcpFieldInputType(field.type)"
          />
        </label>
        <textarea
          v-if="mcpFields.length === 0"
          v-model="mcpJsonText"
          class="timeline-pending-action__input timeline-pending-action__input--json"
          rows="3"
          placeholder="JSON content"
        />
      </div>
      <div class="composer-inline__actions">
        <button
          type="button"
          class="ui-button ui-button--ghost composer-inline__btn"
          :disabled="codexSubmitting"
          @click="decideMcp('cancel')"
        >
          取消
        </button>
        <button
          type="button"
          class="ui-button ui-button--ghost composer-inline__btn"
          :disabled="codexSubmitting"
          @click="decideMcp('decline')"
        >
          拒绝
        </button>
        <button
          type="button"
          class="ui-button ui-button--primary composer-inline__btn"
          :disabled="codexSubmitting || !canSubmitMcp"
          @click="decideMcp('accept')"
        >
          同意
        </button>
      </div>
    </div>
  </section>

  <section
    v-else-if="permissionAction"
    class="timeline-pending-action timeline-pending-action--codex"
    role="region"
    aria-label="权限确认"
  >
    <div class="timeline-pending-action__stack">
      <div class="timeline-pending-action__title-preview">
        {{ permissionAction.payload.reason || "Codex 请求额外权限" }}
      </div>
      <div class="timeline-pending-action__meta">{{ permissionAction.payload.cwd }}</div>
      <pre class="timeline-code-block">{{ permissionJson }}</pre>
      <div class="composer-inline__actions">
        <button
          type="button"
          class="ui-button ui-button--ghost composer-inline__btn"
          :disabled="codexSubmitting"
          @click="decidePermission('deny')"
        >
          拒绝
        </button>
        <button
          type="button"
          class="ui-button ui-button--primary composer-inline__btn"
          :disabled="codexSubmitting"
          @click="decidePermission('allow')"
        >
          同意
        </button>
      </div>
    </div>
  </section>

  <section
    v-else-if="props.action.kind === 'plan_approval'"
    class="timeline-pending-action timeline-pending-action--plan"
    role="region"
    :aria-label="askTitle"
  >
    <textarea
      v-model="freeformText"
      class="timeline-pending-action__input"
      rows="1"
      placeholder="修改要求"
    />
    <div class="composer-inline__actions">
      <button
        type="button"
        class="ui-button ui-button--ghost composer-inline__btn"
        :disabled="!hasFreeformText"
        @click="submitAskFreeform()"
      >
        {{ hasFreeformText ? "修改" : "忽略" }}
      </button>
      <button type="button" class="ui-button ui-button--primary composer-inline__btn" @click="submitAsk">
        同意
      </button>
    </div>
  </section>

  <AskUserInlinePrompt
    v-else-if="activeAsk && askQuestion"
    v-model:freeform-text="freeformText"
    root-class="timeline-pending-action composer-inline composer-inline--ask"
    input-class="timeline-pending-action__input composer-inline__other-input"
    :active-ask="activeAsk"
    :ask-question="askQuestion"
    :ask-title="askTitle"
    :ask-index="askIndex"
    :ask-total="askTotal"
    :ask-dismissable="askDismissable"
    :ask-is-last="askIsLast"
    :ask-options-with-id="askOptionsWithId"
    :ask-has-preview="askHasPreview"
    :ask-focused-option="askFocusedOption"
    :active-option-id="activeOptionId"
    :single-pick="singlePick"
    :multi-picks="multiPicks"
    :can-go-prev="canGoPrev"
    :can-ask-submit="canAskSubmit"
    :ask-other-selected="askOtherSelected"
    show-choice-footer
    @cancel-ask="cancelAsk"
    @highlight-option="highlightOption"
    @clear-option-highlight="clearOptionHighlight"
    @focus-option="focusOption"
    @select-single-option="selectSingleOption"
    @toggle-multi="toggleMulti"
    @skip-ask="skipAsk"
    @back-ask="backAsk"
    @confirm-ask-no="confirmAskNo"
    @submit-ask="submitAsk"
  />
</template>
