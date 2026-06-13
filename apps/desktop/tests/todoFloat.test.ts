import { fireEvent, render } from "@testing-library/vue";
import { describe, expect, it, vi } from "vitest";
import TodoFloat from "../src/components/todo/TodoFloat.vue";

vi.mock("../src/services/todos", () => ({
  deleteTodo: vi.fn(),
  listTodos: vi.fn(async () => []),
  onTodoChanged: vi.fn(async () => vi.fn()),
}));

const goal = {
  threadId: "thread-1",
  objective: "完成 Thread Goal 接入",
  status: "active",
  tokenBudget: 100,
  tokensUsed: 20,
  timeUsedSeconds: 3,
  createdAt: 1,
  updatedAt: 2,
};

describe("TodoFloat", () => {
  it("does not render an empty Lilia Goal row when goal is unset", () => {
    const view = render(TodoFloat, {
      props: {
        taskId: "task-1",
        showGoal: true,
        goal: null,
      },
    });

    expect(view.queryByText("未设置 Lilia Goal")).toBeNull();
    expect(view.container.querySelector(".todo-float__section--goal")).toBeNull();
  });

  it("renders Lilia Goal above todo sections with a distinct row", () => {
    const view = render(TodoFloat, {
      props: {
        taskId: "task-1",
        showGoal: true,
        goal,
      },
    });

    expect(view.getByText("完成 Thread Goal 接入")).toBeTruthy();
    expect(view.getByText("进行中 · 20/100 tokens")).toBeTruthy();
    expect(view.container.querySelector(".todo-float__section--goal")).toBeTruthy();
    expect(view.container.querySelector(".todo-float__row--goal")).toBeTruthy();
    expect(view.container.querySelector(".todo-float__source--goal")).toBeTruthy();
  });

  it("emits goal actions from the goal row", async () => {
    const promptSpy = vi.spyOn(window, "prompt").mockReturnValue("新的目标");
    const view = render(TodoFloat, {
      props: {
        taskId: "task-1",
        showGoal: true,
        goal,
      },
    });

    await fireEvent.click(view.getByRole("button", { name: "设置 Lilia Goal" }));
    await fireEvent.click(view.getByRole("button", { name: "刷新 Lilia Goal" }));
    await fireEvent.click(view.getByRole("button", { name: "清除 Lilia Goal" }));

    expect(view.emitted("set-lilia-goal")?.[0]).toEqual(["新的目标"]);
    expect(view.emitted("refresh-lilia-goal")).toHaveLength(1);
    expect(view.emitted("clear-lilia-goal")).toHaveLength(1);
    promptSpy.mockRestore();
  });
});
