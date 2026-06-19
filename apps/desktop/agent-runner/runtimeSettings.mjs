export function normalizeRuntimePermission(permission) {
  switch (permission) {
    case "full":
    case "readonly":
    case "ask":
    case "free":
      return permission;
    default:
      return null;
  }
}
