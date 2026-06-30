import { fireEvent, render } from "@testing-library/vue";
import { computed, defineComponent, h, nextTick, reactive, ref } from "vue";
import { describe, expect, it, vi } from "vitest";
import type { ToolConsentRequest } from "@lilia/contracts";
import { useFocusOnActivation } from "@lilia/ui";
import { useInlineRename } from "../src/composables/useInlineRename";
import { useEditableToolCommand } from "../src/composables/useEditableToolCommand";
import { toolConsentRequestFixture as toolConsentRequest } from "./interactionTestHelpers";

describe("interaction composables", () => {
  it("focuses and selects the active input after the DOM has updated", async () => {
    const focus = vi.spyOn(HTMLTextAreaElement.prototype, "focus").mockImplementation(() => {});
    const select = vi.spyOn(HTMLTextAreaElement.prototype, "select").mockImplementation(() => {});

    const Host = defineComponent({
      props: { active: Boolean },
      setup(props) {
        const input = ref<HTMLTextAreaElement | null>(null);
        useFocusOnActivation(input, () => props.active, true);
        return () => props.active ? h("textarea", { ref: input }) : h("button", "open");
      },
    });

    const view = render(Host, { props: { active: false } });
    await view.rerender({ active: true });
    await nextTick();

    expect(focus).toHaveBeenCalledTimes(1);
    expect(select).toHaveBeenCalledTimes(1);
    focus.mockRestore();
    select.mockRestore();
  });

  it("cancels stale activation focus when the component changes state first", async () => {
    const focus = vi.spyOn(HTMLTextAreaElement.prototype, "focus").mockImplementation(() => {});
    const active = ref(false);

    const Host = defineComponent({
      setup() {
        const input = ref<HTMLTextAreaElement | null>(null);
        useFocusOnActivation(input, () => active.value);
        return () => active.value ? h("textarea", { ref: input }) : h("button", "open");
      },
    });

    render(Host);
    active.value = true;
    active.value = false;
    await nextTick();

    expect(focus).not.toHaveBeenCalled();
    focus.mockRestore();
  });

  it("keeps inline rename state and commit behavior outside the tree component", async () => {
    const commit = vi.fn();
    const project = reactive({ id: "p1", name: "Lilia" });
    let startRename: (() => void) | null = null;

    const Host = defineComponent({
      setup() {
        const rename = useInlineRename({
          currentId: () => project.id,
          currentValue: () => project.name,
          commit,
        });
        startRename = rename.startRename;
        return { project, ...rename };
      },
      template: `
        <input
          v-if="editingId === project.id"
          :ref="bindEditingInput"
          v-model="editingValue"
          @keydown="onEditingKeydown"
          @blur="commitRename"
        />
      `,
    });

    const view = render(Host);
    startRename?.();
    await nextTick();
    const input = view.getByRole("textbox");
    await fireEvent.update(input, "  Next Lilia  ");
    await fireEvent.keyDown(input, { key: "Enter" });

    expect(commit).toHaveBeenCalledWith("p1", "Next Lilia");
    expect(view.queryByRole("textbox")).toBeNull();
  });

  it("cancels inline rename without committing", async () => {
    const commit = vi.fn();
    let rename: ReturnType<typeof useInlineRename> | null = null;

    const Host = defineComponent({
      setup() {
        rename = useInlineRename({
          currentId: () => "p1",
          currentValue: () => "Lilia",
          commit,
        });
        return { ...rename };
      },
      template: `<input v-if="editingId" :ref="bindEditingInput" />`,
    });

    render(Host);
    rename?.startRename();
    rename?.cancelRename();
    await nextTick();

    expect(commit).not.toHaveBeenCalled();
    expect(rename?.editingId.value).toBeNull();
  });

  it("centralizes editable tool command draft state", async () => {
    const request = ref<ToolConsentRequest | null>(toolConsentRequest());
    const editor = useEditableToolCommand(computed(() => request.value));
    await nextTick();

    editor.beginCommandEdit();
    expect(editor.commandDraft.value).toBe("pwd");

    editor.commandDraft.value = "ls";
    expect(editor.updatedCommandInput.value).toEqual({ command: "ls" });

    editor.commandDraft.value = "   ";
    expect(editor.commandIsEmpty.value).toBe(true);
    expect(editor.updatedCommandInput.value).toBeUndefined();

    request.value = toolConsentRequest({
      requestId: "tool-2",
      input: { command: "git status" },
    });
    await nextTick();

    expect(editor.isEditingCommand.value).toBe(false);
    expect(editor.commandDraft.value).toBe("git status");
  });
});

