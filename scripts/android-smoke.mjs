#!/usr/bin/env node

import { existsSync, readFileSync } from "node:fs";
import http from "node:http";
import https from "node:https";
import { join, resolve } from "node:path";
import { spawn, spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";
import {
  amStartSucceeded,
  androidSmokeOptions,
  avdCpuArch,
  avdNeedsHardwareAcceleration,
  bridgeDispatchUrl,
  formatAndroidDeviceHelp,
  newTrustedDeviceEndpointIds,
  pairedTrustedDeviceEndpointIds,
  pairingUriBridgeProbe,
  parseAdbDevices,
  remoteResumeEnvelope,
  selectOnlineDevice,
  smokePairingUri,
} from "./android-smoke-lib.mjs";

const root = resolve(fileURLToPath(new URL("..", import.meta.url)));
const appId = "com.lilia.remote";
const debugApk = join(root, "apps", "android", "app", "build", "outputs", "apk", "debug", "app-debug.apk");
const options = selectOptions();
const androidHome = findAndroidHome();
const adb = findAdb();
const emulator = findEmulator();

if (!adb) {
  fail("adb was not found. Run yarn android:doctor and install Android platform-tools first.");
}

const pairing = selectPairingUri();
const bridgeProbe = selectBridgeProbe(pairing);
const bridgeStatusBeforePairing = await verifyBridgeBeforePairing(bridgeProbe);
startAvdIfRequested();
const selectedDevice = selectOnlineDeviceFromAdb();
const adbBaseArgs = selectedDevice ? ["-s", selectedDevice.serial] : [];

console.log(`Android smoke target: ${selectedDevice.serial}`);
console.log(
  pairing.synthetic
    ? "Using synthetic pairing URI; this verifies deep link routing but does not prove desktop pairing."
    : "Using LILIA_REMOTE_PAIRING_URI for pairing deep link.",
);

if (!options.skipInstall) {
  runGradle([":app:assembleDebug"]);
  if (!existsSync(debugApk)) {
    fail(`Debug APK was not found after assembleDebug: ${debugApk}`);
  }
  runAdbStep([...adbBaseArgs, "install", "-r", debugApk], {
    title: "Installing Lilia Remote debug APK",
  });
} else {
  console.log("Skipping debug APK install because --skip-install was provided.");
}

const launchResult = runAdbStep([...adbBaseArgs, "shell", `am start -W -n ${appId}/.MainActivity`], {
  title: "Launching Lilia Remote",
});
assertAmStartSucceeded(launchResult, "main activity launch");

const deepLinkResult = runAdbStep(
  [
    ...adbBaseArgs,
    "shell",
    [
      "am",
      "start",
      "-W",
      "-a",
      "android.intent.action.VIEW",
      "-d",
      quoteForAdbShell(pairing.uri),
      appId,
    ].join(" "),
  ],
  { title: "Launching pairing deep link" },
);
assertAmStartSucceeded(deepLinkResult, "pairing deep link launch");
const trustedEndpointId = await verifyBridgeAfterPairing(bridgeProbe, bridgeStatusBeforePairing);
await verifyBridgeResume(bridgeProbe, trustedEndpointId);

const pid = runAdb([...adbBaseArgs, "shell", `pidof ${appId}`], {
  title: "Checking app process",
  allowFailure: true,
  capture: true,
}).stdout.trim();

if (!pid) {
  dumpDeviceDiagnostics("app process was not found");
  fail(`Smoke launch completed, but ${appId} is not running after the deep link launch.`);
}

console.log("");
console.log(`Android smoke passed. ${appId} is running as pid ${pid}.`);

function selectOnlineDeviceFromAdb() {
  const devicesResult = runAdb(["devices", "-l"], {
    capture: true,
    title: "Inspecting adb devices",
  });
  const devices = parseAdbDevices(devicesResult.stdout);
  try {
    return selectOnlineDevice(devices, process.env.ANDROID_SERIAL);
  } catch (err) {
    const message = err instanceof Error ? err.message : String(err);
    fail(`${message}${deviceHelp()}`);
  }
}

function selectOptions() {
  try {
    return androidSmokeOptions(process.argv.slice(2), process.env);
  } catch (err) {
    fail(err instanceof Error ? err.message : String(err));
  }
}

function selectPairingUri() {
  try {
    return smokePairingUri(process.env.LILIA_REMOTE_PAIRING_URI, {
      requirePairing: options.requirePairing,
    });
  } catch (err) {
    fail(err instanceof Error ? err.message : String(err));
  }
}

function selectBridgeProbe(pairing) {
  try {
    return pairingUriBridgeProbe(pairing);
  } catch (err) {
    fail(err instanceof Error ? err.message : String(err));
  }
}

async function verifyBridgeBeforePairing(probe) {
  if (!probe) return null;
  console.log("");
  console.log(`Checking desktop remote bridge: ${probe.statusUrl}`);
  const status = await fetchBridgeStatus(probe.statusUrl);
  if (!status.ok) {
    fail(`Desktop remote bridge returned ok=false before pairing: ${JSON.stringify(status.error ?? status)}`);
  }
  const activeTicketId = status.status?.activeTicket?.id;
  if (activeTicketId !== probe.ticketId) {
    fail(`Desktop remote bridge active ticket is ${activeTicketId ?? "(none)"}, expected ${probe.ticketId}.`);
  }
  console.log(`Desktop bridge has active pairing ticket ${probe.ticketId}.`);
  return status;
}

async function verifyBridgeAfterPairing(probe, beforeStatus) {
  if (!probe) return null;
  console.log("");
  console.log("Waiting for desktop bridge to consume pairing ticket");
  const deadline = Date.now() + 30_000;
  let lastStatus = null;
  while (Date.now() < deadline) {
    lastStatus = await fetchBridgeStatus(probe.statusUrl);
    const activeTicketId = lastStatus.status?.activeTicket?.id;
    const newTrustedEndpointIds = newTrustedDeviceEndpointIds(beforeStatus, lastStatus);
    const pairedEndpointIds = pairedTrustedDeviceEndpointIds(beforeStatus, lastStatus);
    if (lastStatus.ok && activeTicketId !== probe.ticketId && pairedEndpointIds.length > 0) {
      const endpointReason = newTrustedEndpointIds.includes(pairedEndpointIds[0])
        ? "new trusted endpoint"
        : "updated trusted endpoint";
      console.log(
        `Desktop bridge consumed pairing ticket ${probe.ticketId}; ${endpointReason}: ${pairedEndpointIds[0]}.`,
      );
      return pairedEndpointIds[0];
    }
    sleepMs(1_000);
  }
  fail(`Timed out waiting for desktop bridge to consume pairing ticket ${probe.ticketId} and add or update a trusted device. Last status: ${JSON.stringify(lastStatus)}`);
}

async function verifyBridgeResume(probe, deviceEndpointId) {
  if (!probe || !deviceEndpointId) return;
  const dispatchUrl = bridgeDispatchUrl(probe.statusUrl);
  console.log("");
  console.log(`Verifying desktop bridge dispatch: connection.resume for ${deviceEndpointId}`);
  let response;
  try {
    response = await postJson(dispatchUrl, remoteResumeEnvelope(deviceEndpointId), 5_000);
  } catch (err) {
    fail(`Failed to dispatch connection.resume at ${dispatchUrl}: ${err instanceof Error ? err.message : String(err)}`);
  }
  if (!response.ok || response.payload?.accepted !== true) {
    fail(`Desktop bridge rejected connection.resume for ${deviceEndpointId}: ${JSON.stringify(response)}`);
  }
  console.log(`Desktop bridge accepted connection.resume for ${deviceEndpointId}.`);
}

async function fetchBridgeStatus(statusUrl) {
  try {
    return await getJson(statusUrl, 5_000);
  } catch (err) {
    fail(`Failed to read desktop remote bridge status at ${statusUrl}: ${err instanceof Error ? err.message : String(err)}`);
  }
}

function startAvdIfRequested() {
  if (!options.startAvd) return;
  if (!emulator) {
    fail(`Cannot start AVD because emulator was not found.${deviceHelp()}`);
  }
  const avds = listAvds();
  const avdName = options.avdName || avds[0];
  if (!avdName) {
    fail("Cannot start AVD because no Android Virtual Devices were found.");
  }
  if (!avds.includes(avdName)) {
    fail(`ANDROID_AVD=${avdName} was requested, but emulator -list-avds does not include it.${deviceHelp()}`);
  }
  ensureAvdCanStart(avdName);

  const emulatorArgs = ["-avd", avdName, "-no-snapshot"];
  if (!options.avdWindowed) {
    emulatorArgs.push("-no-window", "-no-audio", "-gpu", "swiftshader_indirect");
  }
  console.log("");
  console.log(`Starting Android Virtual Device: ${avdName}`);
  console.log(`Using emulator: ${emulator}`);
  const child = spawn(emulator, emulatorArgs, {
    cwd: root,
    detached: true,
    stdio: "ignore",
  });
  child.unref();
  waitForBootCompleted();
}

function ensureAvdCanStart(avdName) {
  const configText = readAvdConfig(avdName);
  if (!configText) return;
  const accel = emulatorAcceleration();
  if (!accel?.ok && avdNeedsHardwareAcceleration(configText)) {
    const arch = avdCpuArch(configText) || "x86/x86_64";
    fail(
      [
        `Cannot start AVD ${avdName}: ${arch} emulation requires hardware acceleration.`,
        `Emulator acceleration: ${accel?.detail ?? "unavailable"}`,
        "Install/enable Android Emulator Hypervisor Driver or use an online physical device.",
      ].join("\n"),
    );
  }
}

function runGradle(args) {
  const gradle = process.platform === "win32"
    ? join(root, "apps", "android", "gradlew.bat")
    : join(root, "apps", "android", "gradlew");
  runCommand(gradle, ["--no-daemon", "-p", join(root, "apps", "android"), ...args], {
    title: `Running Gradle ${args.join(" ")}`,
    shell: process.platform === "win32",
  });
}

function runAdb(args, options = {}) {
  return runCommand(adb, args, options);
}

function runAdbStep(args, options = {}) {
  return runAdb(args, {
    ...options,
    capture: true,
    diagnoseOnFailure: true,
    echoOutput: true,
  });
}

function waitForBootCompleted() {
  console.log("Waiting for Android device to boot");
  runAdb(["wait-for-device"], {
    title: "Waiting for adb device",
    timeoutMs: 120_000,
  });

  const deadline = Date.now() + 180_000;
  let lastOutput = "";
  while (Date.now() < deadline) {
    const result = runAdb(["shell", "getprop", "sys.boot_completed"], {
      allowFailure: true,
      capture: true,
    });
    lastOutput = `${result.stdout ?? ""}${result.stderr ?? ""}`.trim();
    if (result.status === 0 && result.stdout.trim() === "1") {
      console.log("Android device boot completed.");
      return;
    }
    sleepMs(3_000);
  }
  fail(`Timed out waiting for Android boot completion. Last adb output: ${lastOutput || "(no output)"}`);
}

function runCommand(command, args, options = {}) {
  if (options.title) {
    console.log("");
    console.log(options.title);
  }

  const result = spawnSync(command, args, {
    cwd: root,
    encoding: "utf8",
    shell: options.shell ?? false,
    stdio: options.capture ? "pipe" : "inherit",
    timeout: options.timeoutMs,
  });

  if (options.capture && options.echoOutput) {
    printCommandOutput(result);
  }

  if (result.error && !options.allowFailure) {
    if (options.diagnoseOnFailure) dumpDeviceDiagnostics("command failed to start");
    if (result.error.code === "ETIMEDOUT") {
      fail(`${command} ${args.join(" ")} timed out after ${options.timeoutMs ?? "unknown"}ms.`);
    }
    fail(`${command} failed to start: ${result.error.message}`);
  }
  if (result.status !== 0 && !options.allowFailure) {
    if (options.diagnoseOnFailure) dumpDeviceDiagnostics(`command exited with ${result.status ?? "unknown"}`);
    fail(`${command} ${args.join(" ")} failed with exit code ${result.status ?? "unknown"}.`);
  }
  return result;
}

function printCommandOutput(result) {
  const stdout = result.stdout?.trim();
  const stderr = result.stderr?.trim();
  if (stdout) console.log(stdout);
  if (stderr) console.error(stderr);
}

function assertAmStartSucceeded(result, label) {
  const output = [result.stdout, result.stderr]
    .filter(Boolean)
    .join("\n");
  if (amStartSucceeded(output)) return;
  dumpDeviceDiagnostics(`${label} reported an error`);
  fail(`${label} reported an error. See diagnostics above.`);
}

function dumpDeviceDiagnostics(reason) {
  console.error("");
  console.error(`Android smoke diagnostics (${reason})`);

  runDiagnosticAdb("Installed package", ["shell", `pm path ${appId}`]);
  runDiagnosticAdb("Main activity resolution", [
    "shell",
    `cmd package resolve-activity --brief -n ${appId}/.MainActivity`,
  ]);
  runDiagnosticAdb("Deep link resolution", [
    "shell",
    [
      "cmd",
      "package",
      "resolve-activity",
      "--brief",
      "-a",
      "android.intent.action.VIEW",
      "-d",
      quoteForAdbShell(pairing.uri),
      appId,
    ].join(" "),
  ]);
  runDiagnosticAdb("App process", ["shell", `pidof ${appId}`]);
  runDiagnosticAdb("Recent app logcat", [
    "logcat",
    "-d",
    "-t",
    "120",
    "AndroidRuntime:E",
    "ActivityTaskManager:I",
    "ActivityManager:I",
    "*:S",
  ]);
}

function runDiagnosticAdb(title, args) {
  const result = runAdb([...adbBaseArgs, ...args], {
    allowFailure: true,
    capture: true,
  });
  console.error("");
  console.error(`${title}:`);
  const output = [result.stdout, result.stderr]
    .filter(Boolean)
    .join("")
    .trim();
  console.error(output || "(no output)");
}

function findAdb() {
  const envCandidates = [
    process.env.ADB,
    androidHome ? join(androidHome, "platform-tools", "adb.exe") : null,
    "adb",
  ].filter(Boolean);

  return envCandidates.find((candidate) => {
    if (candidate !== "adb" && !existsSync(candidate)) return false;
    const result = spawnSync(candidate, ["version"], { cwd: root, encoding: "utf8" });
    return result.status === 0;
  }) ?? null;
}

function findEmulator() {
  const envCandidates = [
    process.env.EMULATOR,
    androidHome ? join(androidHome, "emulator", "emulator.exe") : null,
    "emulator",
  ].filter(Boolean);

  return envCandidates.find((candidate) => {
    if (candidate !== "emulator" && !existsSync(candidate)) return false;
    const result = spawnSync(candidate, ["-version"], { cwd: root, encoding: "utf8" });
    return result.status === 0;
  }) ?? null;
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

function deviceHelp() {
  return formatAndroidDeviceHelp({
    avds: listAvds(),
    emulatorPath: emulator ?? "emulator",
    accelStatus: emulatorAcceleration(),
  });
}

function listAvds() {
  if (!emulator) return [];
  const result = spawnSync(emulator, ["-list-avds"], {
    cwd: root,
    encoding: "utf8",
  });
  if (result.status !== 0) return [];
  return (result.stdout ?? "")
    .split(/\r?\n/)
    .map((line) => line.trim())
    .filter(Boolean);
}

function readAvdConfig(avdName) {
  const androidUserHome = process.env.ANDROID_USER_HOME ||
    (process.env.USERPROFILE ? join(process.env.USERPROFILE, ".android") : null) ||
    (process.env.HOME ? join(process.env.HOME, ".android") : null);
  if (!androidUserHome) return "";
  const avdIni = join(androidUserHome, "avd", `${avdName}.ini`);
  if (!existsSync(avdIni)) return "";
  const iniText = readFileSync(avdIni, "utf8");
  const pathLine = iniText
    .split(/\r?\n/)
    .map((line) => line.trim())
    .find((line) => line.startsWith("path="));
  const avdPath = pathLine?.slice("path=".length).trim();
  if (!avdPath) return "";
  const configPath = join(avdPath, "config.ini");
  if (!existsSync(configPath)) return "";
  return readFileSync(configPath, "utf8");
}

function emulatorAcceleration() {
  if (!emulator) return null;
  const result = spawnSync(emulator, ["-accel-check"], {
    cwd: root,
    encoding: "utf8",
  });
  const output = `${result.stdout ?? ""}${result.stderr ?? ""}`;
  const detail = output
    .split(/\r?\n/)
    .map((line) => line.trim())
    .find((line) => line && line !== "accel:" && line !== "accel" && !/^\d+$/.test(line));
  return {
    ok: result.status === 0,
    detail: detail ?? "emulator acceleration status unavailable",
  };
}

function getJson(url, timeoutMs) {
  return new Promise((resolvePromise, reject) => {
    const client = url.startsWith("https:") ? https : http;
    const request = client.get(url, { timeout: timeoutMs }, (response) => {
      const chunks = [];
      response.setEncoding("utf8");
      response.on("data", (chunk) => chunks.push(chunk));
      response.on("end", () => {
        const text = chunks.join("");
        if ((response.statusCode ?? 0) < 200 || (response.statusCode ?? 0) > 299) {
          reject(new Error(`HTTP ${response.statusCode}: ${text || "(empty response)"}`));
          return;
        }
        try {
          resolvePromise(JSON.parse(text));
        } catch (err) {
          reject(new Error(`non-JSON response: ${err instanceof Error ? err.message : String(err)}`));
        }
      });
    });
    request.on("timeout", () => {
      request.destroy(new Error(`timed out after ${timeoutMs}ms`));
    });
    request.on("error", reject);
  });
}

function postJson(url, payload, timeoutMs) {
  return new Promise((resolvePromise, reject) => {
    const body = JSON.stringify(payload);
    const parsed = new URL(url);
    const client = parsed.protocol === "https:" ? https : http;
    const request = client.request(parsed, {
      method: "POST",
      timeout: timeoutMs,
      headers: {
        "content-type": "application/json",
        "content-length": Buffer.byteLength(body),
      },
    }, (response) => {
      const chunks = [];
      response.setEncoding("utf8");
      response.on("data", (chunk) => chunks.push(chunk));
      response.on("end", () => {
        const text = chunks.join("");
        if ((response.statusCode ?? 0) < 200 || (response.statusCode ?? 0) > 299) {
          reject(new Error(`HTTP ${response.statusCode}: ${text || "(empty response)"}`));
          return;
        }
        try {
          resolvePromise(JSON.parse(text));
        } catch (err) {
          reject(new Error(`non-JSON response: ${err instanceof Error ? err.message : String(err)}`));
        }
      });
    });
    request.on("timeout", () => {
      request.destroy(new Error(`timed out after ${timeoutMs}ms`));
    });
    request.on("error", reject);
    request.end(body);
  });
}

function sleepMs(ms) {
  spawnSync(process.execPath, ["-e", `Atomics.wait(new Int32Array(new SharedArrayBuffer(4)), 0, 0, ${ms})`], {
    cwd: root,
    encoding: "utf8",
  });
}

function quoteForAdbShell(value) {
  return `'${value.replace(/'/g, "'\\''")}'`;
}

function fail(message) {
  console.error("");
  console.error(message);
  process.exit(1);
}
