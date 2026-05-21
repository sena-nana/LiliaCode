import { createApp } from "vue";
import App from "./App.vue";
import { router } from "./router";
// 副作用 import：触发主题 composable 在模块级初始化。
import "./composables/useTheme";
import { installContextMenu } from "./composables/useContextMenu";
import { vContextMenu } from "./directives/contextMenu";
import "./styles.css";

installContextMenu();

const app = createApp(App);
app.use(router);
app.directive("context-menu", vContextMenu);
app.mount("#root");
