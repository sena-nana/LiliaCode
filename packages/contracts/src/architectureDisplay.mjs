import { compactLine, isRecord, pick, readRecord } from "./toolUtils.mjs";

export function readArchitectureChangeRecords(value) {
  const raw = Array.isArray(value) ? value : readRecord(value).changes;
  return (Array.isArray(raw) ? raw : []).filter(isRecord);
}

export function architectureChangeDisplayText(value, options = {}) {
  const change = readRecord(value);
  const type = compactLine(pick(change, ["type"]), 80);
  const separator = options.separator ?? "：";
  const fallback = options.fallback ?? "";
  if (!type) return fallback;
  if (type === "set_summary") return options.summaryLabel ?? "更新架构摘要";
  if (type === "remove_node") {
    return joinLabel(options.removeNodeLabel ?? "移除节点", compactLine(pick(change, ["nodeId"]), 120), separator);
  }
  if (type === "remove_edge") {
    return joinLabel(options.removeEdgeLabel ?? "移除关系", compactLine(pick(change, ["edgeId"]), 120), separator);
  }
  if (type === "upsert_node") {
    const node = readRecord(change.node);
    return joinLabel(
      options.upsertNodeLabel ?? "更新节点",
      compactLine(pick(node, ["label", "id"]), 120) || options.unnamedNode || "",
      separator,
    );
  }
  if (type === "upsert_edge") {
    const edge = readRecord(change.edge);
    const directLabel = compactLine(pick(edge, ["label", "id"]), 120);
    const endpointLabel = options.includeEdgeEndpoints
      ? [compactLine(pick(edge, ["from"]), 80), compactLine(pick(edge, ["to"]), 80)]
        .filter(Boolean)
        .join(" -> ")
      : "";
    return joinLabel(
      options.upsertEdgeLabel ?? "更新关系",
      directLabel || endpointLabel || options.unnamedEdge || "",
      separator,
    );
  }
  return fallback || type;
}

export function architectureChangeCompactLabel(value) {
  return architectureChangeDisplayText(value, {
    fallback: "架构变更",
    includeEdgeEndpoints: true,
    removeEdgeLabel: "删除关系",
    removeNodeLabel: "删除节点",
    separator: " ",
    summaryLabel: "更新项目摘要",
  });
}

function joinLabel(label, value, separator) {
  const text = compactLine(value, 120);
  return text ? `${label}${separator}${text}` : label;
}
