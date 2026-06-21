export function parseAdbDevices(output) {
  return output
    .split(/\r?\n/)
    .map((line) => line.trim())
    .filter((line) => line && !line.startsWith("List of devices"))
    .map((line) => {
      const [serial, state, ...details] = line.split(/\s+/);
      return { serial, state: state ?? "unknown", details: details.join(" ") };
    })
    .filter((device) => device.serial);
}

export function selectOnlineDevice(devices, requestedSerial) {
  const requested = requestedSerial?.trim();
  if (requested) {
    const device = devices.find((item) => item.serial === requested);
    if (!device) {
      throw new Error(`ANDROID_SERIAL=${requested} was requested, but adb does not list that device.`);
    }
    if (device.state !== "device") {
      throw new Error(`ANDROID_SERIAL=${requested} is ${device.state}, not online.`);
    }
    return device;
  }

  const onlineDevices = devices.filter((device) => device.state === "device");
  if (onlineDevices.length === 1) {
    return onlineDevices[0];
  }
  if (onlineDevices.length > 1) {
    throw new Error(
      [
        "Multiple online Android devices were found. Set ANDROID_SERIAL to choose one.",
        "",
        formatDeviceList(devices),
      ].join("\n"),
    );
  }

  const nonOnlineHint = devices.length > 0 ? `\n\nCurrent adb devices:\n${formatDeviceList(devices)}` : "";
  const unauthorizedHint = formatUnauthorizedDeviceHint(devices);
  throw new Error(
    [
      "No online Android device detected.",
      "Connect a physical device with USB debugging enabled, or install/enable Android Emulator Hypervisor Driver and start an AVD.",
      unauthorizedHint,
      nonOnlineHint,
    ].filter(Boolean).join("\n"),
  );
}

export function formatAndroidDeviceHelp({
  avds = [],
  emulatorPath = "emulator",
  accelStatus = null,
} = {}) {
  const lines = [];
  const normalizedAvds = avds.map((avd) => avd.trim()).filter(Boolean);

  if (normalizedAvds.length > 0) {
    lines.push("", "Available Android Virtual Devices:");
    for (const avd of normalizedAvds) {
      lines.push(`- ${avd}`);
    }
    lines.push("");
    lines.push("Start an AVD before rerunning yarn android:smoke, or let smoke start it:");
    lines.push(`  "${emulatorPath}" -avd ${normalizedAvds[0]} -no-snapshot`);
    lines.push(`  yarn android:smoke --start-avd --avd=${normalizedAvds[0]}`);
  }

  if (accelStatus?.detail) {
    lines.push("");
    lines.push(`Emulator acceleration: ${accelStatus.detail}`);
    if (!accelStatus.ok) {
      lines.push("x86_64 AVDs require hardware acceleration on Windows.");
    }
  }

  return lines.join("\n");
}

export function androidSmokeOptions(args = [], env = {}) {
  const options = {
    skipInstall: false,
    requirePairing: env.ANDROID_SMOKE_REQUIRE_PAIRING === "1",
    startAvd: env.ANDROID_SMOKE_START_AVD === "1",
    avdName: env.ANDROID_AVD?.trim() || "",
    avdWindowed: env.ANDROID_SMOKE_AVD_WINDOWED === "1",
  };

  for (const arg of args) {
    if (arg === "--skip-install") {
      options.skipInstall = true;
      continue;
    }
    if (arg === "--require-pairing") {
      options.requirePairing = true;
      continue;
    }
    if (arg === "--start-avd") {
      options.startAvd = true;
      continue;
    }
    if (arg === "--windowed-avd") {
      options.avdWindowed = true;
      continue;
    }
    if (arg.startsWith("--avd=")) {
      options.avdName = arg.slice("--avd=".length).trim();
      options.startAvd = true;
      continue;
    }
    throw new Error(`Unknown android smoke option: ${arg}`);
  }

  return options;
}

export function avdCpuArch(configText) {
  const entries = parseAvdConfig(configText);
  return entries.get("hw.cpu.arch") || entries.get("abi.type") || "";
}

export function avdNeedsHardwareAcceleration(configText) {
  const arch = avdCpuArch(configText).toLowerCase();
  return arch === "x86" || arch === "x86_64";
}

export function formatDeviceList(devices) {
  if (devices.length === 0) return "(none)";
  return devices
    .map((device) => {
      const detail = device.details ? ` ${device.details}` : "";
      return `- ${device.serial} ${device.state}${detail}`;
    })
    .join("\n");
}

export function amStartSucceeded(output) {
  if (/Status:\s*ok/i.test(output) && !/\bError:|Exception:|unable to resolve Intent/i.test(output)) {
    return true;
  }
  return !/\bError:|Exception:|unable to resolve Intent|Activity not started/i.test(output);
}

function formatUnauthorizedDeviceHint(devices) {
  const unauthorizedDevices = devices
    .filter((device) => device.state === "unauthorized")
    .map((device) => device.serial);
  if (unauthorizedDevices.length === 0) return "";
  return [
    "",
    `USB debugging authorization is pending for: ${unauthorizedDevices.join(", ")}`,
    "Unlock the phone, accept the RSA fingerprint dialog, then rerun yarn android:smoke.",
    "If no dialog appears, revoke USB debugging authorizations on the phone and reconnect USB.",
  ].join("\n");
}

export function smokePairingUri(value, options = {}) {
  const uri = value?.trim();
  if (uri) {
    if (!uri.startsWith("lilia-remote://pair?")) {
      throw new Error("LILIA_REMOTE_PAIRING_URI must start with lilia-remote://pair?");
    }
    return { uri, synthetic: false };
  }
  if (options.requirePairing) {
    throw new Error("LILIA_REMOTE_PAIRING_URI is required for real Android pairing smoke.");
  }
  return {
    uri: "lilia-remote://pair?v=1&ticket=smoke&challenge=smoke&endpoint=pc-smoke&name=Smoke%20PC&bridge=http%3A%2F%2F127.0.0.1%3A41478",
    synthetic: true,
  };
}

export function pairingUriBridgeProbe(pairing) {
  if (pairing.synthetic) return null;
  const parsed = new URL(pairing.uri);
  const ticketId = parsed.searchParams.get("ticket")?.trim();
  const bridgeUrl = parsed.searchParams.get("bridge")?.trim();
  if (!ticketId) {
    throw new Error("LILIA_REMOTE_PAIRING_URI is missing ticket.");
  }
  if (!bridgeUrl) {
    throw new Error("LILIA_REMOTE_PAIRING_URI is missing bridge.");
  }
  const bridge = new URL(bridgeUrl);
  if (bridge.protocol !== "http:" && bridge.protocol !== "https:") {
    throw new Error("LILIA_REMOTE_PAIRING_URI bridge must use HTTP(S).");
  }
  return {
    ticketId,
    statusUrl: new URL("/status", bridge).toString(),
  };
}

export function bridgeDispatchUrl(statusUrl) {
  return new URL("/dispatch", statusUrl).toString();
}

export function remoteResumeEnvelope(deviceEndpointId, requestId = "android-smoke-resume", sentAt = Date.now()) {
  const endpointId = String(deviceEndpointId ?? "").trim();
  if (!endpointId) {
    throw new Error("deviceEndpointId is required for connection.resume smoke dispatch.");
  }
  return {
    id: requestId,
    protocolVersion: 1,
    sentAt,
    deviceId: endpointId,
    request: {
      type: "connection.resume",
      androidEndpointId: endpointId,
    },
  };
}

export function trustedDeviceEndpointIds(statusPayload) {
  const devices = statusPayload?.status?.trustedDevices;
  if (!Array.isArray(devices)) return [];
  return devices
    .filter((device) => device && device.trusted !== false)
    .map((device) => String(device.endpointId ?? "").trim())
    .filter(Boolean);
}

export function newTrustedDeviceEndpointIds(beforeStatus, afterStatus) {
  const before = new Set(trustedDeviceEndpointIds(beforeStatus));
  return trustedDeviceEndpointIds(afterStatus).filter((endpointId) => !before.has(endpointId));
}

export function changedTrustedDeviceEndpointIds(beforeStatus, afterStatus) {
  const before = trustedDeviceEntriesByEndpointId(beforeStatus);
  return trustedDeviceEntries(afterStatus)
    .filter((device) => device.endpointId && device.trusted !== false)
    .filter((device) => {
      const previous = before.get(device.endpointId);
      return previous && device.lastSeenAt != null && device.lastSeenAt !== previous.lastSeenAt;
    })
    .map((device) => device.endpointId);
}

export function pairedTrustedDeviceEndpointIds(beforeStatus, afterStatus) {
  return uniqueEndpointIds([
    ...newTrustedDeviceEndpointIds(beforeStatus, afterStatus),
    ...changedTrustedDeviceEndpointIds(beforeStatus, afterStatus),
  ]);
}

function trustedDeviceEntries(statusPayload) {
  const devices = statusPayload?.status?.trustedDevices;
  if (!Array.isArray(devices)) return [];
  return devices
    .map((device) => ({
      endpointId: String(device?.endpointId ?? "").trim(),
      trusted: device?.trusted,
      lastSeenAt: device?.lastSeenAt ?? null,
    }))
    .filter((device) => device.endpointId);
}

function trustedDeviceEntriesByEndpointId(statusPayload) {
  return new Map(
    trustedDeviceEntries(statusPayload).map((device) => [device.endpointId, device]),
  );
}

function uniqueEndpointIds(endpointIds) {
  return [...new Set(endpointIds.map((endpointId) => endpointId.trim()).filter(Boolean))];
}

function parseAvdConfig(configText) {
  const entries = new Map();
  for (const line of configText.split(/\r?\n/)) {
    const trimmed = line.trim();
    if (!trimmed || trimmed.startsWith("#")) continue;
    const [key, ...rest] = trimmed.split("=");
    if (!key || rest.length === 0) continue;
    entries.set(key.trim(), rest.join("=").trim());
  }
  return entries;
}
