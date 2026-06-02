import { fireEvent, render, waitFor } from "@testing-library/vue";
import { beforeEach, describe, expect, it, vi } from "vitest";
import MarkdownBlock from "../src/components/chat/MarkdownBlock.vue";

const mermaidMock = vi.hoisted(() => ({
  initialize: vi.fn(),
  render: vi.fn(async (_id: string, source: string) => ({
    svg: `<svg data-testid="mermaid-svg"><text>${source}</text></svg>`,
  })),
}));

vi.mock("mermaid", () => ({
  default: mermaidMock,
}));

describe("MarkdownBlock", () => {
  beforeEach(() => {
    mermaidMock.initialize.mockClear();
    mermaidMock.render.mockClear();
  });


  it("渲染 markdown 表格并保留列对齐", () => {
    const view = render(MarkdownBlock, {
      props: {
        content: [
          "| 名称 | 数量 | 比例 | 状态 |",
          "| :--- | ---: | :---: | --- |",
          "| **Alpha** | 42 | \\(\\frac{1}{2}\\) | `ok` |",
        ].join("\n"),
      },
    });

    expect(view.container.querySelector(".markdown-block__table")).toBeInTheDocument();
    expect(view.getByText("Alpha").closest("strong")).toBeInTheDocument();
    expect(view.getByText("ok").closest("code")).toBeInTheDocument();
    expect(view.getByText("名称").closest("th")).toHaveStyle({ textAlign: "left" });
    expect(view.getByText("42").closest("td")).toHaveStyle({ textAlign: "right" });
    expect(view.getByText("比例").closest("th")).toHaveStyle({ textAlign: "center" });
    expect(view.container.querySelector(".markdown-block__table .katex")).toBeInTheDocument();
  });


  it("渲染明确边界的行内和块级 LaTeX", () => {
    const view = render(MarkdownBlock, {
      props: {
        content: [
          "行内公式 \\(E=mc^2\\) 与 \\(\\frac{1}{2}\\)。",
          "",
          "$$",
          "a^2 + b^2 = c^2",
          "$$",
        ].join("\n"),
      },
    });

    expect(view.container.querySelectorAll(".markdown-block__math-inline .katex"))
      .toHaveLength(2);
    expect(view.container.querySelector(".markdown-block__math-block .katex-display"))
      .toBeInTheDocument();
  });


  it("渲染任务列表并支持嵌套与续行", () => {
    const view = render(MarkdownBlock, {
      props: {
        content: [
          "- [ ] 待办",
          "  续行说明",
          "  - 子项",
          "- [x] 完成",
        ].join("\n"),
      },
    });

    const checkboxes = view.container.querySelectorAll<HTMLInputElement>(
      ".markdown-block__task-checkbox",
    );
    expect(checkboxes).toHaveLength(2);
    expect(checkboxes[0]).toBeDisabled();
    expect(checkboxes[0]?.checked).toBe(false);
    expect(checkboxes[1]?.checked).toBe(true);
    expect(view.container.querySelector(".markdown-block__list .markdown-block__list"))
      .toHaveTextContent("子项");
    expect(view.container.querySelector(".markdown-block__list li")).toHaveTextContent(
      "待办 续行说明",
    );
  });


  it("渲染硬换行、删除线和自动链接", () => {
    const view = render(MarkdownBlock, {
      props: {
        content: [
          "第一行\\",
          "第二行  ",
          "第三行",
          "",
          "删除 ~~旧内容~~，链接 https://example.com/path).",
          "<mailto:test@example.com> 和 `https://code.example`",
        ].join("\n"),
      },
    });

    expect(view.container.querySelectorAll(".markdown-block__paragraph br")).toHaveLength(2);
    expect(view.container.querySelector("del")).toHaveTextContent("旧内容");
    expect(view.getByRole("link", { name: "https://example.com/path" }))
      .toHaveAttribute("href", "https://example.com/path");
    expect(view.getByRole("link", { name: "mailto:test@example.com" }))
      .toHaveAttribute("href", "mailto:test@example.com");
    expect(view.getByText("https://code.example").closest("code")).toBeInTheDocument();

    const singleLineView = render(MarkdownBlock, {
      props: {
        content: "单行 https://example.com",
        singleLine: true,
      },
    });
    expect(singleLineView.getByRole("link", { name: "https://example.com" }))
      .toHaveAttribute("href", "https://example.com");
  });

  it("Markdown 图片点击会请求打开图片查看器", async () => {
    const view = render(MarkdownBlock, {
      props: {
        content: "![截图](https://example.com/shot.png)",
      },
    });

    await fireEvent.click(view.getByRole("button", { name: "查看图片 截图" }));

    expect(view.emitted("open-image")?.[0]?.[0]).toEqual({
      src: "https://example.com/shot.png",
      name: "截图",
      path: "https://example.com/shot.png",
    });

    const singleLineView = render(MarkdownBlock, {
      props: {
        content: "![截图](https://example.com/shot.png)",
        singleLine: true,
      },
    });
    expect(singleLineView.container.querySelector(".markdown-block__image-button")).toBeNull();
  });


  it("把 mermaid fenced code 渲染为图表", async () => {
    const view = render(MarkdownBlock, {
      props: {
        content: [
          "```mermaid",
          "graph TD",
          "  A --> B",
          "```",
        ].join("\n"),
      },
    });

    await waitFor(() => {
      expect(mermaidMock.render).toHaveBeenCalledWith(
        expect.stringMatching(/^markdown-mermaid-m\d+-block-0-/),
        "graph TD\n  A --> B",
      );
    });

    expect(mermaidMock.initialize).toHaveBeenCalledTimes(1);
    expect(view.container.querySelector('[data-testid="mermaid-svg"]')).toBeInTheDocument();
  });
});
