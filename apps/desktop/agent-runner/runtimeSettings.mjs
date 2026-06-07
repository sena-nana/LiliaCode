export function normalizeRuntimePermission(permission) {
  switch (permission) {
    case "full":
    case "readonly":
    case "ask":
      return permission;
    default:
      return null;
  }
}
