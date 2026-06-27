import { createLazyLoadState } from "../../../utils/lazyLoadState";
import { measurePerfAsync } from "../../../utils/perf";

type MermaidApi = typeof import("mermaid")["default"];

export const MERMAID_EXPLICIT_RENDER_LENGTH = 1_200;
const EXTENSION_DIAGRAM_PATTERN =
  /^(?:architecture(?:-beta)?|eventmodeling|packet(?:-beta)?|radar-beta|railroad(?:-(?:ebnf|abnf|peg))?-beta|wardley-beta|cynefin-beta)(?:\s|:|$)/i;

let mermaidConfigured = false;
const mermaidModuleLoad = createLazyLoadState(() =>
  measurePerfAsync(
    "markdown.mermaid.module.load",
    async () => await import("mermaid"),
  )
);

function firstMeaningfulLine(source: string): string {
  return source
    .split(/\r?\n/)
    .map((line) => line.trim())
    .find((line) => line.length > 0 && !line.startsWith("%%")) ?? "";
}

function isExtensionMermaidDiagram(source: string): boolean {
  return EXTENSION_DIAGRAM_PATTERN.test(firstMeaningfulLine(source));
}

export function needsExplicitMermaidActivation(source: string): boolean {
  return source.trim().length >= MERMAID_EXPLICIT_RENDER_LENGTH || isExtensionMermaidDiagram(source);
}

async function getMermaid(): Promise<MermaidApi> {
  const module = await mermaidModuleLoad.load();
  return module.default;
}

function configureMermaid(mermaid: MermaidApi) {
  if (mermaidConfigured) return;
  mermaid.initialize({
    startOnLoad: false,
    securityLevel: "strict",
    theme: "base",
    themeVariables: {
      background: "transparent",
      mainBkg: "transparent",
      fontFamily: "var(--font-sans)",
      primaryColor: "transparent",
      primaryTextColor: "currentColor",
      lineColor: "currentColor",
      textColor: "currentColor",
    },
  });
  mermaidConfigured = true;
}

export function mermaidErrorMessage(error: unknown): string {
  return error instanceof Error ? error.message : "Mermaid 渲染失败。";
}

export async function renderMermaidDiagram(
  id: string,
  source: string,
) {
  const mermaid = await getMermaid();
  configureMermaid(mermaid);
  return await measurePerfAsync(
    "markdown.mermaid.render",
    async () => await mermaid.render(id, source),
    { detail: `${source.length} chars` },
  );
}
