import { fireEvent, render } from "@testing-library/vue";
import { beforeEach, describe, expect, it, vi } from "vitest";
import TodoFloat from "../src/components/todo/TodoFloat.vue";

const todoServiceMock = vi.hoisted(() => ({
  deleteTodo: vi.fn(),
  listTodos: vi.fn(async () => []),
  onTodoChanged: vi.fn(async () => vi.fn()),
}));

vi.mock("../src/services/todos", () => ({
  deleteTodo: todoServiceMock.deleteTodo,
  listTodos: todoServiceMock.listTodos,
  onTodoChanged: todoServiceMock.onTodoChanged,
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

async function flushAsyncLifecycle() {
  await Promise.resolve();
  await Promise.resolve();
}

describe("TodoFloat", () => {
  beforeEach(() => {
    todoServiceMock.deleteTodo.mockReset();
    todoServiceMock.listTodos.mockReset();
    todoServiceMock.onTodoChanged.mockReset();
    todoServiceMock.deleteTodo.mockResolvedValue(undefined);
    todoServiceMock.listTodos.mockResolvedValue([]);
    todoServiceMock.onTodoChanged.mockResolvedValue(vi.fn());
  });

  it("does not render an empty Lilia Goal row when goal is unset", () => {
    const view = render(TodoFloat, {
      props: {
        taskId: "task-1",
        showGoal: true,
        goal: null,
      },
    });

    expect(view.container.querySelector(".todo-float__section--goal")).toBeNull();
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

  it("卸载期间完成 todo changed listener 注册时会立即反注册", async () => {
    const unlisten = vi.fn();
    let resolveTodoListener: ((unlisten: () => void) => void) | null = null;
    todoServiceMock.onTodoChanged.mockImplementationOnce(
      () => new Promise((resolve) => {
        resolveTodoListener = resolve;
      }),
    );

    const view = render(TodoFloat, {
      props: {
        taskId: "task-1",
      },
    });
    await flushAsyncLifecycle();
    expect(todoServiceMock.onTodoChanged).toHaveBeenCalledTimes(1);

    view.unmount();
    expect(unlisten).not.toHaveBeenCalled();
    expect(resolveTodoListener).not.toBeNull();
    resolveTodoListener?.(unlisten);
    await flushAsyncLifecycle();

    expect(unlisten).toHaveBeenCalledTimes(1);
  });

  it("任务切换后忽略较早完成的删除错误", async () => {
    const guide = {
      id: "guide-1",
      taskId: "task-1",
      text: "整理测试",
      done: false,
      order: 0,
      source: "lilia",
      priority: "normal",
      guideStatus: "pending",
      createdAt: 1,
      updatedAt: 1,
    };
    let rejectDelete: (error: Error) => void = () => {};
    todoServiceMock.listTodos.mockImplementation(async (taskId: string) => taskId === "task-1" ? [guide] : []);
    todoServiceMock.deleteTodo.mockReturnValueOnce(
      new Promise((_, reject) => {
        rejectDelete = reject;
      }),
    );

    const view = render(TodoFloat, {
      props: {
        taskId: "task-1",
      },
    });

    expect(await view.findByText("整理测试")).toBeInTheDocument();
    await fireEvent.click(view.getByRole("button", { name: /删除引导/ }));
    await view.rerender({ taskId: "task-2" });
    rejectDelete(new Error("旧任务删除失败"));
    await flushAsyncLifecycle();

    expect(view.queryByText("旧任务删除失败")).toBeNull();
  });
});

