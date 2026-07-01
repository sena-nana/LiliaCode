import type { vContextMenu } from "@lilia/ui";

declare module "*.vue" {
  import type { DefineComponent } from "vue";
  const component: DefineComponent<{}, {}, any>;
  export default component;
}

declare module "vue" {
  interface GlobalDirectives {
    vContextMenu: typeof vContextMenu;
  }
}

