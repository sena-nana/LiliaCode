import { fireEvent, render, screen, waitFor } from "@testing-library/vue";
import { defineComponent, ref } from "vue";
import { describe, expect, it } from "vitest";
import SearchDropdown from "../src/components/SearchDropdown.vue";

function renderSearchDropdown() {
  return render(defineComponent({
    components: { SearchDropdown },
    setup() {
      const value = ref("");
      const open = ref(false);
      return { open, value };
    },
    template: `
      <SearchDropdown
        v-model="value"
        v-model:open="open"
        placeholder="搜索"
        :close-on-outside="true"
        :close-on-escape="true"
      >
        <button type="button" class="search-dropdown__item" role="option">结果 A</button>
      </SearchDropdown>
    `,
  }));
}

describe("SearchDropdown", () => {
  it("teleports the menu to body and closes on outside click", async () => {
    const view = renderSearchDropdown();
    const input = view.getByPlaceholderText("搜索");

    await fireEvent.focus(input);

    const listbox = await screen.findByRole("listbox");
    expect(document.body.contains(listbox)).toBe(true);
    expect(view.container.contains(listbox)).toBe(false);

    await fireEvent.pointerDown(document.body);

    await waitFor(() => {
      expect(screen.queryByRole("listbox")).not.toBeInTheDocument();
    });
  });
});
