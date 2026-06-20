#!/usr/bin/env node

import { existsSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { join, resolve } from "node:path";
import { spawnSync } from "node:child_process";

const root = resolve(fileURLToPath(new URL("..", import.meta.url)));
const expectedNdk = "28.2.13676358";
const expectedTargets = [
  "aarch64-linux-android",
  "armv7-linux-androideabi",
  "i686-linux-android",
  "x86_64-linux-android",
];

const checks = [];
const androidHome = findAndroidHome();

checkCommand("java", ["-version"], { required: true, stderr: true });
checkJavaMajor();
checkCommand("adb", ["version"], { required: false });
checkCommand("emulator", ["-version"], { required: false });
checkAndroidHome(androidHome);
checkSdkTool(androidHome, "platform-tools", "adb.exe");
checkSdkTool(androidHome, join("emulator", "emulator.exe"));
checkSdkTool(androidHome, join("platforms", "android-36", "android.jar"));
checkSdkTool(androidHome, join("build-tools", "36.0.0", "aapt2.exe"));
checkSdkTool(androidHome, join("ndk", expectedNdk, "source.properties"));
checkSdkManager(androidHome);
checkRustTargets();

const maxName = Math.max(...checks.map((check) => check.name.length));
for (const check of checks) {
  const label = check.ok ? "OK" : check.required ? "FAIL" : "WARN";
  console.log(`${label.padEnd(4)} ${check.name.padEnd(maxName)} ${check.detail}`);
}

const failures = checks.filter((check) => check.required && !check.ok);
if (failures.length > 0) {
  console.error("");
  console.error("Android environment is not ready. Run scripts/setup-android-env.ps1 or install the missing items above.");
  process.exit(1);
}

console.log("");
console.log("Android environment checks passed.");

function addCheck(name, ok, detail, required = true) {
  checks.push({ name, ok, detail, required });
}

function run(command, args) {
  return spawnSync(command, args, {
    cwd: root,
    encoding: "utf8",
  });
}

function checkCommand(command, args, options = {}) {
  const result = run(command, args);
  const output = `${result.stdout ?? ""}${options.stderr ? result.stderr ?? "" : ""}`.trim();
  addCheck(
    command,
    result.status === 0,
    result.status === 0 ? firstLine(output) : "not found or failed",
    options.required ?? true,
  );
}

function checkJavaMajor() {
  const result = run("java", ["-version"]);
  const output = `${result.stdout ?? ""}${result.stderr ?? ""}`;
  const match = output.match(/version "(\d+)/);
  const major = Number.parseInt(match?.[1] ?? "", 10);
  addCheck("java>=21", Number.isFinite(major) && major >= 21, match?.[0] ?? "version not detected");
}

function checkAndroidHome(home) {
  addCheck("ANDROID_HOME", Boolean(home), home ?? "not set and no SDK found");
}

function checkSdkTool(home, relativePath, display = relativePath) {
  const path = home ? join(home, relativePath) : "";
  addCheck(display, Boolean(home && existsSync(path)), path || "Android SDK not found");
}

function checkSdkManager(home) {
  const candidates = home
    ? [
        join(home, "cmdline-tools", "latest", "bin", "sdkmanager.bat"),
        join(home, "cmdline-tools", "bin", "sdkmanager.bat"),
      ]
    : [];
  const found = candidates.find((candidate) => existsSync(candidate));
  addCheck("sdkmanager", Boolean(found), found ?? "Android command-line tools missing");
}

function checkRustTargets() {
  const result = run("rustup", ["target", "list", "--installed"]);
  const installed = new Set((result.stdout ?? "").split(/\r?\n/).map((line) => line.trim()).filter(Boolean));
  for (const target of expectedTargets) {
    addCheck(`rust:${target}`, installed.has(target), installed.has(target) ? "installed" : "missing");
  }
}

function findAndroidHome() {
  const envCandidates = [
    process.env.ANDROID_HOME,
    process.env.ANDROID_SDK_ROOT,
    process.env.LOCALAPPDATA ? join(process.env.LOCALAPPDATA, "Android", "Sdk") : null,
    "D:\\Android\\Sdk",
    "D:\\软件",
  ].filter(Boolean);

  return envCandidates.find((candidate) => {
    if (!existsSync(candidate)) return false;
    return existsSync(join(candidate, "platform-tools")) ||
      existsSync(join(candidate, "cmdline-tools", "latest", "bin", "sdkmanager.bat"));
  }) ?? null;
}

function firstLine(value) {
  return value.split(/\r?\n/).find(Boolean) ?? "available";
}
