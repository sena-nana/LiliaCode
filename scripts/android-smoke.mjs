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

startAvdIfRequested();
const selectedDevice = selectOnlineDeviceFromAdb();
const adbBaseArgs = selectedDevice ? ["-s", selectedDevice.serial] : [];
let pairing = null;
const mockRegression = options.mockRemoteRegression ? await startMockRemoteRegression() : null;
pairing = mockRegression?.primaryPairing ?? selectPairingUri();
const bridgeProbe = selectBridgeProbe(pairing);
const bridgeStatusBeforePairing = await verifyBridgeBeforePairing(bridgeProbe);

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

const deepLinkResult = launchPairingDeepLink(pairing.uri, "Launching pairing deep link");
assertAmStartSucceeded(deepLinkResult, "pairing deep link launch");
const trustedEndpointId = await verifyBridgeAfterPairing(bridgeProbe, bridgeStatusBeforePairing);
await verifyBridgeResume(bridgeProbe, trustedEndpointId);
if (mockRegression) {
  await runMockRemoteRegression(mockRegression, trustedEndpointId);
}

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

async function startMockRemoteRegression() {
  console.log("");
  console.log("Starting Android remote regression mock bridges");
  const pcA = await createMockBridge({
    key: "pc-a",
    pcName: "Smoke PC A",
    pcEndpointId: "pc-smoke-a",
    ticketId: "smoke-ticket-a",
    challenge: "smoke-challenge-a",
    taskId: "smoke-task-shared",
    taskTitle: "Smoke Shared Task",
    retryEventId: "smoke-retry-event-a",
  });
  const pcB = await createMockBridge({
    key: "pc-b",
    pcName: "Smoke PC B",
    pcEndpointId: "pc-smoke-b",
    ticketId: "smoke-ticket-b",
    challenge: "smoke-challenge-b",
    taskId: "smoke-task-shared",
    taskTitle: "Smoke Shared Task",
    retryEventId: "smoke-retry-event-b",
  });
  const regression = {
    pcA,
    pcB,
    primaryPairing: pcA.pairing,
    close() {
      pcA.close();
      pcB.close();
    },
  };
  process.on("exit", () => regression.close());

  for (const bridge of [pcA, pcB]) {
    runAdbStep([...adbBaseArgs, "reverse", `tcp:${bridge.port}`, `tcp:${bridge.port}`], {
      title: `Exposing ${bridge.pcName} mock bridge to Android via adb reverse`,
    });
  }
  console.log(`Mock remote regression bridges are ready on ports ${pcA.port} and ${pcB.port}.`);
  return regression;
}

async function runMockRemoteRegression(regression, primaryEndpointId) {
  const { pcA, pcB } = regression;
  const endpointId = primaryEndpointId || pcA.trustedEndpointId();
  if (!endpointId) {
    fail("Mock remote regression could not determine the paired Android endpoint id.");
  }

  console.log("");
  console.log("Running Android remote regression smoke checks");
  await pcA.waitForDispatch("connection.resume", (request) => request.androidEndpointId === endpointId);
  await pcA.waitForDispatch("tasks.list");

  await tapText(pcA.taskTitle);
  await pcA.waitForDispatch("tasks.get", (request) => request.taskId === pcA.taskId);
  await pcA.waitForDispatch("timeline.snapshot", (request) => request.taskId === pcA.taskId);
  await pcA.waitForDispatch("interaction.pending.read", (request) => request.taskId === pcA.taskId);
  await waitForText(pcA.taskTitle);
  console.log("Opened mock task on Smoke PC A.");

  const pcBProbe = selectBridgeProbe(pcB.pairing);
  const pcBStatusBeforePairing = await verifyBridgeBeforePairing(pcBProbe);
  const pcBDeepLink = launchPairingDeepLink(pcB.pairing.uri, "Launching second mock PC pairing deep link");
  assertAmStartSucceeded(pcBDeepLink, "second mock PC pairing deep link launch");
  const secondEndpointId = await verifyBridgeAfterPairing(pcBProbe, pcBStatusBeforePairing);
  await verifyBridgeResume(pcBProbe, secondEndpointId);
  await pcB.waitForDispatch("connection.resume");
  await pcB.waitForDispatch("tasks.list");

  await tapText(pcB.taskTitle);
  await pcB.waitForDispatch("tasks.get", (request) => request.taskId === pcB.taskId);
  await waitForText(pcB.pcName);
  console.log("Opened mock task on Smoke PC B.");

  const pcATaskGetsBeforeSwitch = pcA.dispatchCount("tasks.get");
  await tapText(pcB.pcName);
  await tapText(pcA.pcName);
  await pcA.waitForDispatch(
    "tasks.get",
    (request) => request.taskId === pcA.taskId,
    { after: pcATaskGetsBeforeSwitch },
  );
  await waitForText(pcA.pcName);
  console.log("Verified PC switch reopens the current task on Smoke PC A.");

  const retryCountBefore = pcA.dispatchCount("chat.retry");
  await tapTimelineRetry();
  await pcA.waitForDispatch(
    "chat.retry",
    (request) => request.taskId === pcA.taskId && request.eventId === pcA.retryEventId,
    { after: retryCountBefore },
  );
  console.log(`Verified precise retry dispatch includes eventId=${pcA.retryEventId}.`);

  const resumeCountBefore = pcA.dispatchCount("connection.resume");
  const taskGetCountBefore = pcA.dispatchCount("tasks.get");
  runAdbStep([...adbBaseArgs, "shell", "input keyevent KEYCODE_HOME"], {
    title: "Sending app to background",
  });
  sleepMs(1_000);
  const resumeResult = runAdbStep([...adbBaseArgs, "shell", `am start -W -n ${appId}/.MainActivity`], {
    title: "Returning app to foreground",
  });
  assertAmStartSucceeded(resumeResult, "foreground restore launch");
  await pcA.waitForDispatch("connection.resume", null, { after: resumeCountBefore });
  await pcA.waitForDispatch(
    "tasks.get",
    (request) => request.taskId === pcA.taskId,
    { after: taskGetCountBefore },
  );
  console.log("Verified foreground restore resumes the bridge and refreshes the active task.");
}

async function createMockBridge(config) {
  const state = {
    trustedEndpointId: "",
    lastSeenAt: null,
    dispatches: [],
  };
  let activeTicket = {
    id: config.ticketId,
    pcName: config.pcName,
    pcEndpoint: { endpointId: config.pcEndpointId, relayUrl: null, directAddresses: [] },
    protocolVersion: 1,
    challenge: config.challenge,
    expiresAt: Date.now() + 10 * 60_000,
    pairingUri: "",
  };
  const capabilities = {
    protocolVersion: 1,
    minProtocolVersion: 1,
    alpn: "lilia.remote-control.v1",
    supportsPairing: true,
    supportsTaskInbox: true,
    supportsTimelineSubscription: true,
    supportsChatSend: true,
    supportsInteractionResponse: true,
    supportsInterrupt: true,
  };
  const task = {
    id: config.taskId,
    taskId: config.taskId,
    title: config.taskTitle,
    projectName: "Android smoke",
    status: "waiting",
    dependsOn: [],
    createdAt: 1_720_000_000_000,
  };
  const peer = () => ({
    id: `${config.key}-android`,
    kind: "android",
    displayName: "Android smoke device",
    endpointId: state.trustedEndpointId,
    protocolVersion: 1,
    trusted: true,
    firstPairedAt: state.lastSeenAt ?? Date.now(),
    lastSeenAt: state.lastSeenAt,
    revokedAt: null,
  });
  const timeline = () => [
    {
      id: `${config.key}-user-message`,
      kind: "message",
      status: "success",
      turnId: `${config.key}-turn`,
      payload: { role: "user", content: `Retry payload for ${config.pcName}` },
    },
    {
      id: config.retryEventId,
      kind: "error",
      title: "Retry anchor",
      status: "failed",
      turnId: `${config.key}-turn`,
      payload: { retryContext: { content: `Retry payload for ${config.pcName}` } },
    },
  ];
  const responseEnvelope = (envelope, payload) => ({
    id: `mock-${Date.now()}`,
    requestId: envelope.id,
    protocolVersion: 1,
    sentAt: Date.now(),
    ok: true,
    payload,
  });
  const dispatchPayload = (request) => {
    switch (request.type) {
      case "connection.resume":
        state.lastSeenAt = Date.now();
        return { type: "connection.resume", accepted: true, peer: peer() };
      case "provider.status.read":
        return { type: "provider.status", backend: "mock", ready: true };
      case "tasks.list":
        return { type: "tasks.list", tasks: [task] };
      case "tasks.get":
        return { type: "tasks.get", task, runtime: { phase: "waiting", processSessionId: null } };
      case "timeline.snapshot":
      case "timeline.subscribe":
        return { type: request.type, taskId: config.taskId, events: timeline() };
      case "interaction.pending.read":
        return { type: "interaction.pending", interactions: [] };
      case "chat.retry":
        return { type: "chat.retry", result: { accepted: true } };
      default:
        return { type: request.type ?? "unknown" };
    }
  };
  const server = http.createServer(async (request, response) => {
    try {
      if (request.method === "GET" && request.url === "/status") {
        writeJson(response, {
          ok: true,
          status: {
            hostEnabled: true,
            state: "listening",
            pcName: config.pcName,
            endpoint: { endpointId: config.pcEndpointId, relayUrl: null, directAddresses: [] },
            activeTicket,
            trustedDevices: state.trustedEndpointId ? [peer()] : [],
            capabilities,
          },
        });
        return;
      }
      if (request.method === "POST" && request.url === "/pair") {
        const body = await readRequestJson(request);
        if (body.ticketId !== config.ticketId || body.challenge !== config.challenge) {
          writeJson(response, {
            ok: false,
            error: { code: "unauthorized", message: "Mock pairing ticket mismatch" },
          });
          return;
        }
        state.trustedEndpointId = String(body.androidEndpoint?.endpointId ?? "").trim();
        state.lastSeenAt = Date.now();
        activeTicket = null;
        writeJson(response, { ok: true, peer: peer() });
        return;
      }
      if (request.method === "POST" && request.url === "/dispatch") {
        const envelope = await readRequestJson(request);
        const remoteRequest = envelope.request ?? {};
        state.dispatches.push({
          at: Date.now(),
          envelope,
          request: remoteRequest,
        });
        if (!state.trustedEndpointId || envelope.deviceId !== state.trustedEndpointId) {
          writeJson(response, {
            id: `mock-${Date.now()}`,
            requestId: envelope.id,
            protocolVersion: 1,
            sentAt: Date.now(),
            ok: false,
            error: { code: "unauthorized", message: "Mock bridge does not trust this Android endpoint" },
          });
        } else {
          writeJson(response, responseEnvelope(envelope, dispatchPayload(remoteRequest)));
        }
        return;
      }
      writeJson(response, { ok: false, error: { code: "invalidRequest", message: "Not found" } }, 404);
    } catch (err) {
      writeJson(
        response,
        {
          ok: false,
          error: {
            code: "internal",
            message: err instanceof Error ? err.message : String(err),
          },
        },
        500,
      );
    }
  });
  await new Promise((resolvePromise) => server.listen(0, "127.0.0.1", resolvePromise));
  const port = server.address().port;
  const bridgeUrl = `http://127.0.0.1:${port}`;
  const pairingUri = [
    "lilia-remote://pair?v=1",
    `ticket=${encodeURIComponent(config.ticketId)}`,
    `challenge=${encodeURIComponent(config.challenge)}`,
    `endpoint=${encodeURIComponent(config.pcEndpointId)}`,
    `name=${encodeURIComponent(config.pcName)}`,
    `bridge=${encodeURIComponent(bridgeUrl)}`,
  ].join("&");
  activeTicket.pairingUri = pairingUri;
  return {
    ...config,
    port,
    pairing: { uri: pairingUri, synthetic: false },
    trustedEndpointId: () => state.trustedEndpointId,
    dispatchCount: (type) => state.dispatches.filter((entry) => entry.request.type === type).length,
    waitForDispatch: async (type, predicate = null, { after = 0, timeoutMs = 15_000 } = {}) => {
      const deadline = Date.now() + timeoutMs;
      while (Date.now() < deadline) {
        const matches = state.dispatches.filter((entry) => entry.request.type === type);
        const candidate = matches.slice(after).find((entry) => !predicate || predicate(entry.request, entry.envelope));
        if (candidate) return candidate;
        sleepMs(250);
      }
      const seen = state.dispatches.map((entry) => entry.request.type).join(", ") || "(none)";
      fail(`Timed out waiting for mock dispatch ${type}. Seen dispatches: ${seen}`);
    },
    close: () => server.close(),
  };
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

function launchPairingDeepLink(uri, title) {
  return runAdbStep(
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
        quoteForAdbShell(uri),
        appId,
      ].join(" "),
    ],
    { title },
  );
}

async function waitForText(text, timeoutMs = 15_000) {
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    const nodes = dumpWindowNodes().filter((node) => node.text === text || node.contentDesc === text);
    if (nodes.length > 0) return nodes;
    sleepMs(300);
  }
  fail(`Timed out waiting for Android UI text: ${text}`);
}

async function tapText(text, { occurrence = "first", timeoutMs = 15_000 } = {}) {
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    const nodes = dumpWindowNodes().filter((node) => node.text === text || node.contentDesc === text);
    const node = selectNodeOccurrence(nodes, occurrence);
    if (node) {
      tapBounds(node.bounds, `text "${text}"`);
      return;
    }
    sleepMs(300);
  }
  fail(`Timed out waiting to tap Android UI text: ${text}`);
}

async function tapTimelineRetry() {
  const deadline = Date.now() + 15_000;
  let scrolled = false;
  while (Date.now() < deadline) {
    const retryNodes = dumpWindowNodes().filter((node) => node.text === "Retry" || node.contentDesc === "Retry");
    if (retryNodes.length >= 2 || (scrolled && retryNodes.length === 1)) {
      tapBounds(retryNodes[retryNodes.length - 1].bounds, "timeline Retry");
      return;
    }
    runAdbStep([...adbBaseArgs, "shell", "input swipe 540 1500 540 700 350"], {
      title: "Scrolling task detail to timeline retry",
    });
    scrolled = true;
    sleepMs(500);
  }
  fail("Timed out waiting for timeline retry button.");
}

function selectNodeOccurrence(nodes, occurrence) {
  if (nodes.length === 0) return null;
  if (occurrence === "last") return nodes[nodes.length - 1];
  if (typeof occurrence === "number") return nodes[occurrence] ?? null;
  return nodes[0];
}

function tapBounds(bounds, label) {
  if (!bounds) {
    fail(`Cannot tap ${label}; UI node has no bounds.`);
  }
  const x = Math.round((bounds.left + bounds.right) / 2);
  const y = Math.round((bounds.top + bounds.bottom) / 2);
  runAdbStep([...adbBaseArgs, "shell", `input tap ${x} ${y}`], {
    title: `Tapping ${label}`,
  });
}

function dumpWindowNodes() {
  runAdb([...adbBaseArgs, "shell", "uiautomator dump /sdcard/lilia-smoke-window.xml"], {
    allowFailure: true,
    capture: true,
  });
  const result = runAdb([...adbBaseArgs, "exec-out", "cat", "/sdcard/lilia-smoke-window.xml"], {
    allowFailure: true,
    capture: true,
  });
  const xml = result.stdout ?? "";
  return parseUiAutomatorNodes(xml);
}

function parseUiAutomatorNodes(xml) {
  const nodes = [];
  for (const match of xml.matchAll(/<node\b[^>]*>/g)) {
    const tag = match[0];
    nodes.push({
      text: decodeXmlAttribute(attributeValue(tag, "text")),
      contentDesc: decodeXmlAttribute(attributeValue(tag, "content-desc")),
      bounds: parseBounds(attributeValue(tag, "bounds")),
    });
  }
  return nodes;
}

function attributeValue(tag, name) {
  const match = tag.match(new RegExp(`${name}="([^"]*)"`));
  return match?.[1] ?? "";
}

function decodeXmlAttribute(value) {
  return value
    .replace(/&quot;/g, "\"")
    .replace(/&apos;/g, "'")
    .replace(/&lt;/g, "<")
    .replace(/&gt;/g, ">")
    .replace(/&amp;/g, "&");
}

function parseBounds(value) {
  const match = value.match(/\[(\d+),(\d+)]\[(\d+),(\d+)]/);
  if (!match) return null;
  return {
    left: Number(match[1]),
    top: Number(match[2]),
    right: Number(match[3]),
    bottom: Number(match[4]),
  };
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
  const diagnosticPairingUri = pairing?.uri ?? "lilia-remote://pair";

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
      quoteForAdbShell(diagnosticPairingUri),
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

function readRequestJson(request) {
  return new Promise((resolvePromise, reject) => {
    const chunks = [];
    request.setEncoding("utf8");
    request.on("data", (chunk) => chunks.push(chunk));
    request.on("end", () => {
      try {
        resolvePromise(JSON.parse(chunks.join("") || "{}"));
      } catch (err) {
        reject(new Error(`invalid JSON request: ${err instanceof Error ? err.message : String(err)}`));
      }
    });
    request.on("error", reject);
  });
}

function writeJson(response, payload, statusCode = 200) {
  const body = JSON.stringify(payload);
  response.writeHead(statusCode, {
    "content-type": "application/json; charset=utf-8",
    "content-length": Buffer.byteLength(body),
  });
  response.end(body);
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
