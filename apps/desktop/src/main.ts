import { createApp } from "vue";
import App from "./App.vue";
import { router } from "./router";
// 触发主题 composable 的模块级初始化：从 localStorage 读出当前主题并写到 <html>。
import "./composables/useTheme";
import "./styles.css";

const app = createApp(App);
app.use(router);
app.mount("#root");
