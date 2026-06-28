import { fireEvent, render, waitFor } from "@testing-library/vue";
import { createMemoryHistory, createRouter } from "vue-router";
import { describe, expect, it } from "vitest";
import { GITHUB_CLONE_REPO_COMMAND } from "@lilia/contracts";
import CloneRepoDialog from "../src/components/sidebar/CloneRepoDialog.vue";
import {
  mockInvoke,
  setMockGitHubBindingStatus,
  setMockGitHubRepos,
  setMockGitHubReposError,
} from "./tauriMock";

function mockBoundGitHubAccount() {
  setMockGitHubBindingStatus({
    state: "bound",
    binding: {
      login: "octocat",
      avatarUrl: null,
      boundAt: 1,
      scopes: ["repo", "read:user"],
      clientIdSource: "bundled",
    },
  });
}

async function renderCloneRepoDialog() {
  const router = createRouter({
    history: createMemoryHistory(),
    routes: [
      { path: "/", component: { template: "<div />" } },
      { path: "/settings", component: { template: "<div />" } },
    ],
  });
  await router.push("/");
  await router.isReady();
  const view = render(CloneRepoDialog, {
    global: {
      plugins: [router],
    },
  });
  return { ...view, router };
}

describe("CloneRepoDialog", () => {
  it("未绑定时支持 owner/repo 直接克隆", async () => {
    const view = await renderCloneRepoDialog();

    await waitFor(() => {
      expect(
        view.getByPlaceholderText("owner/repo 或 https://github.com/owner/repo.git"),
      ).toBeInTheDocument();
      expect(view.getByDisplayValue("C:\\Users\\mock")).toBeInTheDocument();
    });

    await fireEvent.update(
      view.getByPlaceholderText("owner/repo 或 https://github.com/owner/repo.git"),
      "sena-nana/Lilia",
    );
    await fireEvent.click(view.getByRole("button", { name: "克隆并添加" }));

    await waitFor(() => {
      expect(
        mockInvoke.mock.calls.some(([cmd, args]) =>
          cmd === GITHUB_CLONE_REPO_COMMAND &&
          typeof args === "object" &&
          args !== null &&
          "repo" in args &&
          args.repo === "sena-nana/Lilia"
        ),
      ).toBe(true);
    });
  });

  it("已绑定时默认展示绑定账号仓库列表，并可选择克隆", async () => {
    mockBoundGitHubAccount();
    setMockGitHubRepos({
      1: {
        items: [
          {
            id: 1,
            name: "Lilia",
            fullName: "octocat/Lilia",
            ownerLogin: "octocat",
            private: true,
            description: "Desktop app",
            defaultBranch: "main",
            updatedAt: "2026-06-07T00:00:00Z",
            cloneUrl: "https://github.com/octocat/Lilia.git",
            htmlUrl: "https://github.com/octocat/Lilia",
          },
          {
            id: 2,
            name: "Tools",
            fullName: "octocat/Tools",
            ownerLogin: "octocat",
            private: false,
            description: "Utilities",
            defaultBranch: "main",
            updatedAt: "2026-06-06T00:00:00Z",
            cloneUrl: "https://github.com/octocat/Tools.git",
            htmlUrl: "https://github.com/octocat/Tools",
          },
        ],
        nextPage: null,
      },
    });

    const view = await renderCloneRepoDialog();

    await waitFor(() => {
      expect(view.getByPlaceholderText("搜索仓库，或直接输入 owner/repo")).toBeInTheDocument();
      expect(view.getByText(/当前绑定账号：/)).toBeInTheDocument();
      expect(view.getByDisplayValue("C:\\Users\\mock")).toBeInTheDocument();
    });

    const input = view.getByPlaceholderText("搜索仓库，或直接输入 owner/repo");
    await fireEvent.focus(input);

    await waitFor(() => {
      expect(view.getByText("octocat/Lilia")).toBeInTheDocument();
      expect(view.getByText("octocat/Tools")).toBeInTheDocument();
    });

    await fireEvent.click(view.getByText("octocat/Lilia"));
    await fireEvent.click(view.getByRole("button", { name: "克隆并添加" }));

    await waitFor(() => {
      expect(
        mockInvoke.mock.calls.some(([cmd, args]) =>
          cmd === GITHUB_CLONE_REPO_COMMAND &&
          typeof args === "object" &&
          args !== null &&
          "repo" in args &&
          args.repo === "octocat/Lilia"
        ),
      ).toBe(true);
    });
  });

  it("仓库列表遇到失效绑定时提示重新绑定入口", async () => {
    mockBoundGitHubAccount();
    setMockGitHubReposError("GitHub 绑定已失效，请重新绑定");

    const view = await renderCloneRepoDialog();

    await waitFor(() => {
      expect(view.getByText("GitHub 绑定已失效，请重新绑定。")).toBeInTheDocument();
    });

    const input = view.getByPlaceholderText("搜索仓库，或直接输入 owner/repo");
    await fireEvent.focus(input);

    expect(
      view.getByText("GitHub 绑定已失效，请重新绑定后再加载账号仓库。"),
    ).toBeInTheDocument();

    await fireEvent.click(view.getAllByRole("button", { name: "重新绑定 GitHub" })[0]!);

    await waitFor(() => {
      expect(view.router.currentRoute.value.path).toBe("/settings");
      expect(view.router.currentRoute.value.query.tab).toBe("project");
    });
  });

});

