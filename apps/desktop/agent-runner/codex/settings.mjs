import { stringOrNull } from "../utils.mjs";

const CODEX_PERMISSION_PROFILE_IDS = {
  readOnly: ":read-only",
  workspaceWrite: ":workspace",
  dangerFullAccess: ":danger-no-sandbox",
};

export function codexPermissionProfileId(value, { allowAppServerId = false } = {}) {
  const profile = stringOrNull(value);
  if (!profile || profile === "default") return null;
  return CODEX_PERMISSION_PROFILE_IDS[profile] ||
    (allowAppServerId && profile.startsWith(":") ? profile : null);
}
