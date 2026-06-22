#!/usr/bin/env node

import { existsSync, readdirSync } from "node:fs";
import { join, resolve } from "node:path";
import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";

const root = resolve(fileURLToPath(new URL("..", import.meta.url)));
const apkChecks = [
  {
    name: "debug",
    path: join(root, "apps", "android", "app", "build", "outputs", "apk", "debug", "app-debug.apk"),
  },
  {
    name: "release",
    path: join(root, "apps", "android", "app", "build", "outputs", "apk", "release", "app-release-unsigned.apk"),
  },
];

const aapt2 = findAapt2();
if (!aapt2) {
  fail("aapt2 was not found. Run yarn android:doctor and install Android build-tools 36.0.0 first.");
}

for (const apk of apkChecks) {
  inspectApk(apk);
}

console.log("");
console.log("Android APK manifest checks passed.");

function inspectApk(apk) {
  if (!existsSync(apk.path)) {
    fail(`${apk.name} APK was not found: ${apk.path}`);
  }

  const badging = runAapt2(["dump", "badging", apk.path]);
  const manifest = runAapt2(["dump", "xmltree", "--file", "AndroidManifest.xml", apk.path]);

  requireContains(badging, "package: name='com.lilia.remote'", apk.name);
  requireContains(badging, "minSdkVersion:'29'", apk.name);
  requireContains(badging, "targetSdkVersion:'36'", apk.name);
  requireContains(badging, "uses-permission: name='android.permission.INTERNET'", apk.name);
  requireContains(badging, "uses-permission: name='android.permission.ACCESS_NETWORK_STATE'", apk.name);
  requireContains(badging, "uses-permission: name='android.permission.CAMERA'", apk.name);
  requireContains(badging, "launchable-activity: name='com.lilia.remote.MainActivity'", apk.name);

  requireContains(manifest, 'A: package="com.lilia.remote"', apk.name);
  requireContains(manifest, 'A: http://schemas.android.com/apk/res/android:name(0x01010003)="com.lilia.remote.LiliaRemoteApplication"', apk.name);
  requireContains(manifest, "A: http://schemas.android.com/apk/res/android:usesCleartextTraffic(0x010104ec)=true", apk.name);
  requireContains(manifest, 'A: http://schemas.android.com/apk/res/android:name(0x01010003)="com.lilia.remote.MainActivity"', apk.name);
  requireContains(manifest, "A: http://schemas.android.com/apk/res/android:exported(0x01010010)=true", apk.name);
  requireContains(manifest, "A: http://schemas.android.com/apk/res/android:launchMode(0x0101001d)=1", apk.name);
  requireContains(manifest, 'A: http://schemas.android.com/apk/res/android:name(0x01010003)="android.intent.action.MAIN"', apk.name);
  requireContains(manifest, 'A: http://schemas.android.com/apk/res/android:name(0x01010003)="android.intent.category.LAUNCHER"', apk.name);
  requireContains(manifest, 'A: http://schemas.android.com/apk/res/android:name(0x01010003)="android.intent.action.VIEW"', apk.name);
  requireContains(manifest, 'A: http://schemas.android.com/apk/res/android:name(0x01010003)="android.intent.category.DEFAULT"', apk.name);
  requireContains(manifest, 'A: http://schemas.android.com/apk/res/android:name(0x01010003)="android.intent.category.BROWSABLE"', apk.name);
  requireContains(manifest, 'A: http://schemas.android.com/apk/res/android:scheme(0x01010027)="lilia-remote"', apk.name);
  requireContains(manifest, 'A: http://schemas.android.com/apk/res/android:host(0x01010028)="pair"', apk.name);
  requireContains(manifest, 'A: http://schemas.android.com/apk/res/android:name(0x01010003)="android.hardware.camera.any"', apk.name);
  requireContains(manifest, "A: http://schemas.android.com/apk/res/android:required(0x0101028e)=false", apk.name);

  console.log(`OK   ${apk.name.padEnd(7)} ${apk.path}`);
}

function runAapt2(args) {
  const result = spawnSync(aapt2, args, {
    cwd: root,
    encoding: "utf8",
  });
  if (result.status !== 0) {
    fail(`${aapt2} ${args.join(" ")} failed:\n${result.stderr || result.stdout || "(no output)"}`);
  }
  return result.stdout ?? "";
}

function requireContains(value, expected, label) {
  if (!value.includes(expected)) {
    fail(`${label} APK manifest check failed. Missing: ${expected}`);
  }
}

function findAapt2() {
  const androidHome = findAndroidHome();
  const envCandidates = [
    process.env.AAPT2,
    androidHome ? join(androidHome, "build-tools", "36.0.0", executable("aapt2")) : null,
    ...buildToolsCandidates(androidHome),
    "aapt2",
  ].filter(Boolean);

  return envCandidates.find((candidate) => {
    if (candidate !== "aapt2" && !existsSync(candidate)) return false;
    const result = spawnSync(candidate, ["version"], { cwd: root, encoding: "utf8" });
    return result.status === 0;
  }) ?? null;
}

function buildToolsCandidates(androidHome) {
  if (!androidHome) return [];
  const buildTools = join(androidHome, "build-tools");
  if (!existsSync(buildTools)) return [];
  return readdirSync(buildTools)
    .sort((a, b) => b.localeCompare(a, undefined, { numeric: true }))
    .map((version) => join(buildTools, version, executable("aapt2")));
}

function findAndroidHome() {
  const candidates = [
    process.env.ANDROID_HOME,
    process.env.ANDROID_SDK_ROOT,
    process.env.LOCALAPPDATA ? join(process.env.LOCALAPPDATA, "Android", "Sdk") : null,
    "D:\\Android\\Sdk",
    "D:\\软件",
  ].filter(Boolean);

  return candidates.find((candidate) => {
    if (!existsSync(candidate)) return false;
    return existsSync(join(candidate, "platform-tools")) ||
      existsSync(join(candidate, "cmdline-tools", "latest", "bin", "sdkmanager.bat"));
  }) ?? null;
}

function executable(name) {
  return process.platform === "win32" ? `${name}.exe` : name;
}

function fail(message) {
  console.error("");
  console.error(message);
  process.exit(1);
}
