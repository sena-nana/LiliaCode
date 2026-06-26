import fs from "node:fs";
import path from "node:path";
import process from "node:process";
import { spawnSync } from "node:child_process";

function fail(message) {
  throw new Error(`[tauri:install] ${message}`);
}

function runCommand(command, args = []) {
  const result = spawnSync(command, args, {
    encoding: "utf8",
    stdio: "inherit",
    windowsHide: true,
  });
  if (result.error) {
    fail(`${command} 启动失败: ${result.error.message}`);
  }
  if (result.status !== 0) {
    fail(`${command} 执行失败，退出码 ${result.status}`);
  }
}

function resolveInstaller() {
  if (process.platform !== "win32") {
    fail("当前仅支持在 Windows 下自动安装 Tauri 打包产物。");
  }

  const releaseDir = path.join(process.cwd(), "apps/desktop/src-tauri/target/release");
  if (!fs.existsSync(releaseDir)) {
    fail(`未找到构建目录: ${releaseDir}`);
  }

  const candidates = collectInstallers(releaseDir);
  if (candidates.length === 0) {
    fail(`未找到可用安装包于: ${releaseDir}`);
  }

  const preferredMsi = candidates.filter((file) =>
    /LiliaCode_.*_x64\.msi$/i.test(path.basename(file))
  );
  const preferredExe = candidates.filter((file) =>
    /LiliaCode_.*_x64-setup\.exe$/i.test(path.basename(file))
  );

  const finalCandidates = preferredMsi.length > 0 ? preferredMsi : preferredExe;
  const fallbackCandidates = finalCandidates.length > 0 ? finalCandidates : candidates;

  return fallbackCandidates.reduce((latest, file) => {
    return fs.statSync(file).mtimeMs > fs.statSync(latest).mtimeMs ? file : latest;
  });
}

function collectInstallers(dir) {
  const files = [];
  for (const entry of fs.readdirSync(dir, { withFileTypes: true })) {
    const fullPath = path.join(dir, entry.name);
    if (entry.isDirectory()) {
      files.push(...collectInstallers(fullPath));
    } else if (entry.isFile() && /\.(exe|msi)$/i.test(fullPath)) {
      files.push(fullPath);
    }
  }
  return files;
}

function install(installerPath) {
  const ext = path.extname(installerPath).toLowerCase();
  if (ext === ".msi") {
    runCommand("msiexec", ["/i", installerPath, "/qn"]);
    return;
  }
  if (ext === ".exe") {
    runCommand(installerPath, ["/S"]);
    return;
  }
  fail(`不支持的安装包类型: ${ext}`);
}

function main() {
  const installer = resolveInstaller();
  console.log(`[tauri:install] Running installer: ${installer}`);
  install(installer);
}

try {
  main();
} catch (error) {
  console.error(error.message);
  process.exitCode = 1;
}
