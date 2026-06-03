import { markStartup } from "./services/startupTrace";

markStartup("main.ts start");
void import("./mainBootstrap").then(({ mountLiliaApp }) => {
  markStartup("bootstrap imported");
  mountLiliaApp();
});
