import { isRecord, stringOrNull } from "./utils.mjs";

const FALLBACKS = new Set(["diagnostic", "unsupported", "ignore"]);

function normalizeExperimentalProviderOption(option, backend) {
  if (!isRecord(option) || stringOrNull(option.provider) !== backend) return null;
  const fallback = FALLBACKS.has(stringOrNull(option.fallback))
    ? stringOrNull(option.fallback)
    : "diagnostic";
  return {
    fallback,
    capability: stringOrNull(option.capability)?.trim() || "unknown",
    payloadKeys: isRecord(option.payload) ? Object.keys(option.payload) : [],
  };
}

function emitExperimentalProviderDiagnostic(protocol, backend, option, status) {
  protocol.emitTimeline({
    kind: "diagnostic",
    status,
    title: status === "error"
      ? "Unsupported experimental provider option"
      : "Ignored experimental provider option",
    summary: `${backend} adapter does not support experimental capability: ${option.capability}`,
    payload: {
      backend,
      subkind: "experimental_provider_option",
      ...option,
    },
    sourceId: `${backend}:experimental-provider-option:${option.capability}`,
  });
}

export function handleExperimentalProviderOptions(cmd, context, backend) {
  for (const raw of cmd?.runtimeOptions?.experimentalProviderOptions || []) {
    const option = normalizeExperimentalProviderOption(raw, backend);
    if (!option || option.fallback === "ignore") continue;
    if (option.fallback === "unsupported") {
      emitExperimentalProviderDiagnostic(context.protocol, backend, option, "error");
      throw new Error(`${backend} experimental provider capability is unsupported: ${option.capability}`);
    }
    emitExperimentalProviderDiagnostic(context.protocol, backend, option, "info");
  }
}
