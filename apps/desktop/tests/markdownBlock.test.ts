import { render, waitFor } from "@testing-library/vue";
import { describe, expect, it, vi } from "vitest";
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
  it("渲染 markdown 表格并保留列对齐", () => {
    const view = render(MarkdownBlock, {
      props: {
        content: [
          "| 名称 | 数量 | 比例 | 状态 |",
          "| :--- | ---: | :---: | --- |",
          "| **Alpha** | 42 | $\\frac{1}{2}$ | `ok` |",
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

  it("渲染行内和块级 LaTeX", () => {
    const view = render(MarkdownBlock, {
      props: {
        content: [
          "行内公式 $E=mc^2$ 与 \\(\\frac{1}{2}\\)。",
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
