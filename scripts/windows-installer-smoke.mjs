import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import process from "node:process";
import { spawn, spawnSync } from "node:child_process";

const repoRoot = process.cwd();
const processName = "lilia.exe";
const defaultTimeoutMs = 300_000;
const bundledRuntimeFiles = [
  "codex-account-quota.mjs",
  path.join("agent-runner", "codex", "accountQuota.mjs"),
  path.join("agent-runner", "codex", "appServer.mjs"),
];

function getArgValue(name) {
  const index = process.argv.indexOf(name);
  if (index === -1) return undefined;
  return process.argv[index + 1];
}

function log(message) {
  console.log(`[release:smoke:windows] ${message}`);
}

function fail(message) {
  throw new Error(message);
}

function run(command, args, options = {}) {
  const result = spawnSync(command, args, {
    cwd: options.cwd ?? repoRoot,
    env: options.env ?? process.env,
    encoding: "utf8",
    stdio: options.stdio ?? "pipe",
    windowsHide: true,
  });
  if (result.error) {
    fail(`${command} failed to start: ${result.error.message}`);
  }
  if (result.status !== 0) {
    const stdout = result.stdout?.trim();
    const stderr = result.stderr?.trim();
    fail(`${command} ${args.join(" ")} exited ${result.status}.${stdout ? `\nstdout:\n${stdout}` : ""}${stderr ? `\nstderr:\n${stderr}` : ""}`);
  }
  return result;
}

function readWindowsPath(scope) {
  const result = run("powershell.exe", [
    "-NoProfile",
    "-ExecutionPolicy",
    "Bypass",
    "-Command",
    `[Environment]::GetEnvironmentVariable('Path', '${scope}')`,
  ]);
  return result.stdout.trim();
}

function freshWindowsEnv(extra = {}) {
  const machinePath = readWindowsPath("Machine");
  const userPath = readWindowsPath("User");
  return {
    ...process.env,
    Path: [machinePath, userPath].filter(Boolean).join(";"),
    PATH: [machinePath, userPath].filter(Boolean).join(";"),
    ...extra,
  };
}

function waitUntil(description, predicate, timeoutMs = defaultTimeoutMs) {
  const started = Date.now();
  while (Date.now() - started < timeoutMs) {
    if (predicate()) return;
    Atomics.wait(new Int32Array(new SharedArrayBuffer(4)), 0, 0, 500);
  }
  fail(`Timed out waiting for ${description}.`);
}

function isProcessRunning() {
  const result = spawnSync("tasklist.exe", ["/FI", `IMAGENAME eq ${processName}`, "/NH"], {
    encoding: "utf8",
    stdio: "pipe",
    windowsHide: true,
  });
  if (result.error || result.status !== 0) return false;
  return result.stdout.toLowerCase().includes(processName.toLowerCase());
}

function stopAppIfRunning() {
  if (!isProcessRunning()) return;
  spawnSync("taskkill.exe", ["/F", "/IM", processName, "/T"], {
    encoding: "utf8",
    stdio: "pipe",
    windowsHide: true,
  });
  waitUntil(`${processName} to exit`, () => !isProcessRunning(), 15_000);
}

function quoteCmdArg(value) {
  return `"${value.replace(/"/g, '""')}"`;
}

function displayPath(value) {
  if (value.startsWith("\\\\?\\UNC\\")) return `\\\\${value.slice("\\\\?\\UNC\\".length)}`;
  if (value.startsWith("\\\\?\\")) return value.slice("\\\\?\\".length);
  return value;
}

function fileContainsText(filePath, needles) {
  try {
    const data = fs.readFileSync(filePath);
    return needles.some((needle) => data.includes(Buffer.from(needle, "utf8")));
  } catch {
    return false;
  }
}

function storeContainsProjectPath(liliaHome, projectPath) {
  const dbDir = path.join(liliaHome, "db");
  const dbFiles = ["lilia.db", "lilia.db-wal"].map((name) => path.join(dbDir, name));
  const canonical = displayPath(fs.realpathSync.native(projectPath));
  const needles = Array.from(new Set([canonical, canonical.replaceAll("/", "\\")]));
  return dbFiles.some((filePath) => fileContainsText(filePath, needles));
}

function ensureInstallerPath() {
  const explicitInstaller = getArgValue("--installer") ?? process.env.RELEASE_INSTALLER_PATH;
  if (explicitInstaller) {
    const installer = path.resolve(explicitInstaller);
    if (!fs.existsSync(installer)) fail(`Installer not found: ${installer}`);
    return installer;
  }

  const tag = getArgValue("--tag") ?? process.env.RELEASE_TAG ?? process.env.GITHUB_REF_NAME;
  if (!tag) {
    fail("Pass --installer <path> for a local smoke run, or --tag <tag> to download from a draft Release.");
  }

  const downloadDir = path.join(os.tmpdir(), `lilia-release-smoke-assets-${process.pid}`);
  fs.mkdirSync(downloadDir, { recursive: true });
  log(`Downloading draft Release installer for ${tag} into ${downloadDir}`);
  const ghEnv = {
    ...process.env,
    GH_TOKEN: process.env.GH_TOKEN ?? process.env.GITHUB_TOKEN,
  };
  const ghArgs = ["release", "download", tag, "--pattern", "LiliaCode_*_x64-setup.exe", "--dir", downloadDir, "--clobber"];
  const repo = getArgValue("--repo") ?? process.env.GITHUB_REPOSITORY;
  if (repo) ghArgs.push("--repo", repo);
  run("gh.exe", ghArgs, { env: ghEnv });

  const installers = fs.readdirSync(downloadDir)
    .filter((name) => /^LiliaCode_.+_x64-setup\.exe$/i.test(name))
    .map((name) => path.join(downloadDir, name));
  if (installers.length !== 1) {
    fail(`Expected exactly one LiliaCode_*_x64-setup.exe in ${downloadDir}, found ${installers.length}.`);
  }
  return installers[0];
}

function ensureTestProject() {
  const explicitProject = getArgValue("--test-project");
  const projectPath = explicitProject
    ? path.resolve(explicitProject)
    : path.join(os.tmpdir(), `lilia-release-smoke-project-${process.pid}`);
  fs.mkdirSync(projectPath, { recursive: true });
  fs.writeFileSync(path.join(projectPath, ".lilia-smoke"), "release smoke\n", "utf8");
  return projectPath;
}

function main() {
  if (process.argv.includes("--help")) {
    console.log("Usage: yarn release:smoke:windows --installer <path> [--test-project <path>]");
    console.log("   or: yarn release:smoke:windows --tag <tag> [--repo owner/repo]");
    return;
  }

  if (process.platform !== "win32") {
    fail("Windows installer smoke can only run on Windows.");
  }
  if (isProcessRunning()) {
    fail(`${processName} is already running. Close it before running this smoke script so cleanup is scoped to this run.`);
  }

  const installer = ensureInstallerPath();
  const installDir = path.join(os.tmpdir(), `lilia-release-smoke-install-${process.pid}`);
  const liliaHome = path.join(os.tmpdir(), `lilia-release-smoke-home-${process.pid}`);
  const testProject = ensureTestProject();
  const installedExe = path.join(installDir, processName);
  const cliCmd = path.join(installDir, "liliacode.cmd");
  const uninstaller = path.join(installDir, "uninstall.exe");
  let installed = false;

  log(`Installer: ${installer}`);
  log(`Install dir: ${installDir}`);
  log(`LILIA_HOME: ${liliaHome}`);
  log(`Test project: ${testProject}`);

  try {
    log("Installing silently");
    run(installer, ["/S", `/D=${installDir}`]);
    installed = true;
    waitUntil("installed lilia.exe", () => fs.existsSync(installedExe));
    waitUntil("installed liliacode.cmd", () => fs.existsSync(cliCmd));
    for (const filename of bundledRuntimeFiles) {
      const resourcePath = path.join(installDir, filename);
      waitUntil(`installed runtime resource ${filename}`, () => fs.existsSync(resourcePath));
    }

    const runtimeEnv = freshWindowsEnv({ LILIA_HOME: liliaHome });
    if (!fs.existsSync(cliCmd)) {
      fail(`Smoke install did not create expected liliacode command:\n${cliCmd}`);
    }
    log("liliacode command file is present in install directory");

    log("Launching installed app");
    const app = spawn(installedExe, [], {
      env: runtimeEnv,
      detached: false,
      stdio: "ignore",
      windowsHide: false,
    });
    app.unref();
    waitUntil(`${processName} to start`, () => isProcessRunning());
    log("Installed app process started");

    log("Opening test project through liliacode");
    run("cmd.exe", ["/d", "/s", "/c", `${quoteCmdArg(cliCmd)} ${quoteCmdArg(testProject)}`], { env: runtimeEnv });
    waitUntil("CLI project path in the Lilia store", () => storeContainsProjectPath(liliaHome, testProject));
    log("CLI project path reached the Lilia store");

    log("Stopping installed app before uninstall");
    stopAppIfRunning();

    log("Uninstalling silently");
    waitUntil("uninstaller.exe", () => fs.existsSync(uninstaller));
    run(uninstaller, ["/S"]);
    waitUntil("liliacode.cmd removal", () => !fs.existsSync(cliCmd), 30_000);
    installed = false;

    if (fs.existsSync(cliCmd)) {
      fail(`liliacode.cmd still exists after uninstall:\n${cliCmd}`);
    }
    log("CLI command is removed after uninstall");
    log("Windows installer smoke passed");
  } finally {
    stopAppIfRunning();
    if (installed && fs.existsSync(uninstaller)) {
      spawnSync(uninstaller, ["/S"], { encoding: "utf8", stdio: "pipe", windowsHide: true });
    }
    fs.rmSync(liliaHome, { recursive: true, force: true });
    if (fs.existsSync(installDir)) {
      fs.rmSync(installDir, { recursive: true, force: true });
    }
    if (!getArgValue("--test-project")) {
      fs.rmSync(testProject, { recursive: true, force: true });
    }
  }
}

try {
  main();
} catch (err) {
  console.error(`[release:smoke:windows] ${err.message}`);
  process.exitCode = 1;
}
