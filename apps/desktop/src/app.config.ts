import type { LiliaAppConfig } from "@lilia/ui";
import desktopPackage from "../package.json";

export const appConfig = {
  appName: "lilia",
  productTitle: "Lilia",
  version: desktopPackage.version,
  storageKeyPrefix: "lilia",
} satisfies LiliaAppConfig;
