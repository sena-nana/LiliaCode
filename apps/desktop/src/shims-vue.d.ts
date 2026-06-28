declare module "*.vue" {
  import type { DefineComponent } from "vue";
  const component: DefineComponent<{}, {}, any>;
  export default component;
}

import type { vContextMenu } from "./directives/contextMenu";

declare module "vue" {
  interface GlobalDirectives {
    vContextMenu: typeof vContextMenu;
  }
}

