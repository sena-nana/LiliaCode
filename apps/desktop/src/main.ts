import { createApp } from "vue";
import App from "./App.vue";
import { router } from "./router";
// 副作用 import：触发主题 composable 在模块级初始化。
import "./composables/useTheme";
import "./styles.css";

const app = createApp(App);
app.use(router);
app.mount("#root");
