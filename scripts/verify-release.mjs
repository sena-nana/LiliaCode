import fs from "node:fs";
import path from "node:path";
import process from "node:process";

const repoRoot = process.cwd();
const versionTagPattern = /^v(\d+\.\d+\.\d+(?:-[0-9A-Za-z][0-9A-Za-z-]*(?:\.[0-9A-Za-z][0-9A-Za-z-]*)*)?)$/;
const errors = [];

function readText(relativePath) {
  return fs.readFileSync(path.join(repoRoot, relativePath), "utf8");
}

function readJson(relativePath) {
  return JSON.parse(readText(relativePath));
}

function getArgValue(name) {
  const index = process.argv.indexOf(name);
  if (index === -1) {
    return undefined;
  }
  return process.argv[index + 1];
}

function reportError(message) {
  errors.push(message);
}

function requireCargoVersion(cargoToml) {
  const versionMatch = cargoToml.match(/^\s*version\s*=\s*"([^"]+)"\s*$/m);
  if (!versionMatch) {
    throw new Error("apps/desktop/src-tauri/Cargo.toml is missing package version.");
  }
  return versionMatch[1];
}

function describeBundleTargets(targets) {
  if (Array.isArray(targets)) {
    return targets.join(", ");
  }
  return String(targets ?? "");
}

function hasWindowsInstallerTarget(targets) {
  if (targets === "all") {
    return true;
  }
  if (!Array.isArray(targets)) {
    return targets === "nsis" || targets === "msi";
  }
  return targets.includes("all") || targets.includes("nsis") || targets.includes("msi");
}

function hasBundleTarget(targets, target) {
  if (targets === "all") {
    return true;
  }
  if (Array.isArray(targets)) {
    return targets.includes("all") || targets.includes(target);
  }
  return targets === target;
}

function verifyWindowsCliInstallerHook(tauriConfig) {
  const hookPath = tauriConfig.bundle?.windows?.nsis?.installerHooks;
  if (!hookPath) {
    reportError("Tauri NSIS installerHooks must be configured so Windows installs the liliacode CLI entry.");
    return;
  }

  const normalizedHookPath = hookPath.replaceAll("\\", "/").replace(/^\.\//, "");
  const hookRelativePath = path.posix.join("apps/desktop/src-tauri", normalizedHookPath);
  const hookSource = readText(hookRelativePath);
  const requiredSnippets = [
    ['FileOpen $0 "$INSTDIR\\liliacode.cmd" w', "create liliacode.cmd during install"],
    ['FileWrite $0 "start """" ""$INSTDIR\\LiliaCode.exe"" %*$\\r$\\n"', "forward liliacode arguments to LiliaCode.exe"],
    ["[Environment]::SetEnvironmentVariable('Path', ($$parts -join ';'), 'User')", "write the user PATH for liliacode"],
    ['!insertmacro LILIA_RUN_PATH_SCRIPT "install"', "add the install directory to PATH after install"],
    ['!insertmacro LILIA_RUN_PATH_SCRIPT "uninstall"', "remove the install directory from PATH before uninstall"],
    ['Delete "$INSTDIR\\liliacode.cmd"', "delete liliacode.cmd during uninstall"],
  ];

  for (const [snippet, requirement] of requiredSnippets) {
    if (!hookSource.includes(snippet)) {
      reportError(`${hookRelativePath} must ${requirement}.`);
    }
  }
}

const tag = getArgValue("--tag") ?? process.env.RELEASE_TAG ?? process.env.GITHUB_REF_NAME;
const tagMatch = tag?.match(versionTagPattern);

if (!tagMatch) {
  reportError(`Release tag must match vX.Y.Z or vX.Y.Z-prerelease, got ${tag ? `"${tag}"` : "nothing"}.`);
} else {
  const expectedVersion = tagMatch[1];
  const rootPackage = readJson("package.json");
  const desktopPackage = readJson("apps/desktop/package.json");
  const tauriConfig = readJson("apps/desktop/src-tauri/tauri.conf.json");
  const cargoVersion = requireCargoVersion(readText("apps/desktop/src-tauri/Cargo.toml"));

  const versions = [
    ["package.json", rootPackage.version],
    ["apps/desktop/package.json", desktopPackage.version],
    ["apps/desktop/src-tauri/Cargo.toml", cargoVersion],
    ["apps/desktop/src-tauri/tauri.conf.json", tauriConfig.version],
  ];

  for (const [source, actualVersion] of versions) {
    if (actualVersion !== expectedVersion) {
      reportError(`${source} version ${actualVersion} does not match ${tag}.`);
    }
  }

  if (tauriConfig.productName !== "LiliaCode") {
    reportError(`Tauri productName must be LiliaCode, got "${tauriConfig.productName}".`);
  }

  if (tauriConfig.bundle?.active !== true) {
    reportError("Tauri bundle.active must be true for release packaging.");
  }

  if (!hasWindowsInstallerTarget(tauriConfig.bundle?.targets)) {
    reportError(
      `Tauri bundle.targets must include all, nsis, or msi for a Windows installer, got "${describeBundleTargets(
        tauriConfig.bundle?.targets,
      )}".`,
    );
  }

  if (hasBundleTarget(tauriConfig.bundle?.targets, "nsis")) {
    verifyWindowsCliInstallerHook(tauriConfig);
  }

  if (errors.length === 0) {
    const productName = tauriConfig.productName;
    console.log("[release:check] Release metadata is valid.");
    console.log(`[release:check] Tag: ${tag}`);
    console.log(`[release:check] Version: ${expectedVersion}`);
    console.log(`[release:check] Release name: ${productName} ${tag}`);
    console.log("[release:check] Asset pattern: LiliaCode_[version]_[arch][setup][ext]");
    const installers = [];
    if (hasBundleTarget(tauriConfig.bundle?.targets, "nsis")) {
      installers.push(`${productName}_${expectedVersion}_x64-setup.exe`);
    }
    if (hasBundleTarget(tauriConfig.bundle?.targets, "msi")) {
      installers.push(`${productName}_${expectedVersion}_x64.msi`);
    }
    console.log(`[release:check] Expected Windows installers: ${installers.join(" and/or ")}`);
  }
}

for (const error of errors) {
  console.error(`[release:check] ${error}`);
}

if (errors.length > 0) {
  process.exitCode = 1;
}
