#!/usr/bin/env node

import assert from "node:assert/strict";
import {
  amStartSucceeded,
  androidSmokeOptions,
  avdCpuArch,
  avdNeedsHardwareAcceleration,
  bridgeDispatchUrl,
  changedTrustedDeviceEndpointIds,
  formatAndroidDeviceHelp,
  formatDeviceList,
  newTrustedDeviceEndpointIds,
  pairedTrustedDeviceEndpointIds,
  pairingUriBridgeProbe,
  parseAdbDevices,
  remoteResumeEnvelope,
  selectOnlineDevice,
  smokePairingUri,
} from "./android-smoke-lib.mjs";

const devices = parseAdbDevices(`
List of devices attached
emulator-5554 device product:sdk_gphone64_x86_64 model:sdk_gphone64_x86_64
R58M123 unauthorized usb:1-1
offline-1 offline
`);

assert.equal(devices.length, 3);
assert.equal(devices[0].serial, "emulator-5554");
assert.equal(devices[0].state, "device");
assert.match(devices[0].details, /model:sdk_gphone64/);
assert.equal(devices[1].state, "unauthorized");

assert.equal(selectOnlineDevice(devices, "").serial, "emulator-5554");
assert.equal(selectOnlineDevice(devices, "emulator-5554").serial, "emulator-5554");
assert.throws(
  () => selectOnlineDevice(devices, "R58M123"),
  /ANDROID_SERIAL=R58M123 is unauthorized, not online/,
);
assert.throws(
  () => selectOnlineDevice(devices, "missing"),
  /ANDROID_SERIAL=missing was requested/,
);
assert.throws(
  () => selectOnlineDevice([{ serial: "a", state: "device", details: "" }, { serial: "b", state: "device", details: "" }], ""),
  /Multiple online Android devices/,
);
assert.throws(
  () => selectOnlineDevice([{ serial: "R58M123", state: "unauthorized", details: "" }], ""),
  /No online Android device detected/,
);
assert.throws(
  () => selectOnlineDevice([{ serial: "R58M123", state: "unauthorized", details: "" }], ""),
  /USB debugging authorization is pending for: R58M123/,
);

assert.equal(
  formatDeviceList([{ serial: "R58M123", state: "unauthorized", details: "usb:1-1" }]),
  "- R58M123 unauthorized usb:1-1",
);

assert.equal(
  amStartSucceeded(`
Starting: Intent { act=android.intent.action.VIEW dat=lilia-remote://pair/... }
Warning: Activity not started, intent has been delivered to currently running top-most instance.
Status: ok
Complete
`),
  true,
);
assert.equal(amStartSucceeded("Error: Activity class does not exist."), false);
assert.equal(amStartSucceeded("Error: unable to resolve Intent"), false);

assert.equal(smokePairingUri("").synthetic, true);
assert.match(smokePairingUri("").uri, /^lilia-remote:\/\/pair\?/);
assert.throws(
  () => smokePairingUri("", { requirePairing: true }),
  /LILIA_REMOTE_PAIRING_URI is required for real Android pairing smoke/,
);
assert.deepEqual(
  smokePairingUri(" lilia-remote://pair?v=1&ticket=real "),
  { uri: "lilia-remote://pair?v=1&ticket=real", synthetic: false },
);
assert.throws(
  () => smokePairingUri("https://example.test/pair"),
  /LILIA_REMOTE_PAIRING_URI must start with lilia-remote:\/\/pair\?/,
);

const deviceHelp = formatAndroidDeviceHelp({
  avds: ["LiliaRemoteApi36"],
  emulatorPath: "C:\\Android\\Sdk\\emulator\\emulator.exe",
  accelStatus: {
    ok: false,
    detail: "Android Emulator hypervisor driver is not installed on this machine",
  },
});
assert.match(deviceHelp, /Available Android Virtual Devices/);
assert.match(deviceHelp, /LiliaRemoteApi36/);
assert.match(deviceHelp, /-avd LiliaRemoteApi36 -no-snapshot/);
assert.match(deviceHelp, /yarn android:smoke --start-avd --avd=LiliaRemoteApi36/);
assert.match(deviceHelp, /x86_64 AVDs require hardware acceleration/);

assert.deepEqual(
  androidSmokeOptions(["--skip-install", "--require-pairing", "--start-avd", "--avd=LiliaRemoteApi36", "--windowed-avd"], {}),
  {
    skipInstall: true,
    requirePairing: true,
    startAvd: true,
    avdName: "LiliaRemoteApi36",
    avdWindowed: true,
  },
);
assert.deepEqual(
  androidSmokeOptions([], {
    ANDROID_SMOKE_START_AVD: "1",
    ANDROID_SMOKE_REQUIRE_PAIRING: "1",
    ANDROID_AVD: "EnvAvd",
    ANDROID_SMOKE_AVD_WINDOWED: "1",
  }),
  {
    skipInstall: false,
    requirePairing: true,
    startAvd: true,
    avdName: "EnvAvd",
    avdWindowed: true,
  },
);
assert.throws(
  () => androidSmokeOptions(["--bad"], {}),
  /Unknown android smoke option: --bad/,
);

assert.equal(pairingUriBridgeProbe(smokePairingUri("")), null);
assert.deepEqual(
  pairingUriBridgeProbe({
    synthetic: false,
    uri: "lilia-remote://pair?v=1&ticket=ticket-1&challenge=c&endpoint=pc&bridge=http%3A%2F%2F192.168.1.12%3A41478",
  }),
  {
    ticketId: "ticket-1",
    statusUrl: "http://192.168.1.12:41478/status",
  },
);
assert.throws(
  () => pairingUriBridgeProbe({
    synthetic: false,
    uri: "lilia-remote://pair?v=1&ticket=ticket-1&challenge=c&endpoint=pc",
  }),
  /missing bridge/,
);
assert.equal(
  bridgeDispatchUrl("http://192.168.1.12:41478/status"),
  "http://192.168.1.12:41478/dispatch",
);
assert.deepEqual(
  remoteResumeEnvelope(" android-new ", "resume-1", 123),
  {
    id: "resume-1",
    protocolVersion: 1,
    sentAt: 123,
    deviceId: "android-new",
    request: {
      type: "connection.resume",
      androidEndpointId: "android-new",
    },
  },
);
assert.throws(
  () => remoteResumeEnvelope(" "),
  /deviceEndpointId is required/,
);

assert.deepEqual(
  newTrustedDeviceEndpointIds(
    {
      status: {
        trustedDevices: [
          { endpointId: "android-existing", trusted: true, lastSeenAt: 100 },
          { endpointId: "android-revoked", trusted: false },
        ],
      },
    },
    {
      status: {
        trustedDevices: [
          { endpointId: "android-existing", trusted: true, lastSeenAt: 100 },
          { endpointId: "android-revoked", trusted: false },
          { endpointId: "android-new", trusted: true, lastSeenAt: 200 },
        ],
      },
    },
  ),
  ["android-new"],
);
assert.deepEqual(
  changedTrustedDeviceEndpointIds(
    {
      status: {
        trustedDevices: [
          { endpointId: "android-existing", trusted: true, lastSeenAt: 100 },
          { endpointId: "android-revoked", trusted: false, lastSeenAt: 100 },
        ],
      },
    },
    {
      status: {
        trustedDevices: [
          { endpointId: "android-existing", trusted: true, lastSeenAt: 200 },
          { endpointId: "android-revoked", trusted: false, lastSeenAt: 200 },
        ],
      },
    },
  ),
  ["android-existing"],
);
assert.deepEqual(
  pairedTrustedDeviceEndpointIds(
    {
      status: {
        trustedDevices: [
          { endpointId: "android-existing", trusted: true, lastSeenAt: 100 },
        ],
      },
    },
    {
      status: {
        trustedDevices: [
          { endpointId: "android-existing", trusted: true, lastSeenAt: 200 },
          { endpointId: "android-new", trusted: true, lastSeenAt: 200 },
        ],
      },
    },
  ),
  ["android-new", "android-existing"],
);

const x86AvdConfig = `
abi.type = x86_64
hw.cpu.arch = x86_64
`;
assert.equal(avdCpuArch(x86AvdConfig), "x86_64");
assert.equal(avdNeedsHardwareAcceleration(x86AvdConfig), true);
assert.equal(
  avdNeedsHardwareAcceleration(`
abi.type = arm64-v8a
hw.cpu.arch = arm64
`),
  false,
);

console.log("android-smoke-lib tests passed");
