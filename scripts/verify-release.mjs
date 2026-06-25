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

function requireText(source, text, snippet, requirement) {
  if (!text.includes(snippet)) {
    reportError(`${source} must ${requirement}.`);
  }
}

function requireSnippets(source, text, entries) {
  for (const [snippet, requirement] of entries) {
    requireText(source, text, snippet, requirement);
  }
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

function verifyReleaseTemplate(expectedVersion, productName) {
  const source = "docs/github/release-template.md";
  const template = readText(source);
  const expectedInstaller = `${productName}_${expectedVersion}_x64-setup.exe`;
  const knownLimitations = [
    ["## 已知限制", "include a known limitations section"],
    ["当前只发布 Windows 安装包", "state that only the Windows installer is published"],
    ["当前安装包没有代码签名", "state that the installer is unsigned"],
    ["当前不包含 Tauri updater 自动更新能力", "state that Tauri updater auto-update is not included"],
    ["当前不发布 macOS 公证包、macOS 安装包或 Linux 安装包", "state that macOS/Linux packages are not published"],
  ];
  const windowsVerification = [
    ["LiliaCode_<version>_x64-setup.exe", "document the Windows installer naming pattern"],
    [`安装包文件名：${expectedInstaller}`, `include a validation record entry for ${expectedInstaller}`],
    ["## Windows 安装验证", "include the Windows install verification checklist"],
    ["从 draft Release 下载 Windows 安装包", "include the draft Release download check"],
    ["yarn release:smoke:windows --tag <tag>", "include the repeatable draft Release smoke command"],
    ["yarn release:smoke:windows --installer <安装包路径>", "include the repeatable local installer smoke command"],
    ["启动 LiliaCode", "include the launch verification check"],
    ["liliacode <测试项目路径>", "include the liliacode CLI verification check"],
    ["卸载后新的 PowerShell 或 cmd 中 `liliacode` 不再可用", "include the uninstall CLI cleanup check"],
    ["## Windows 安装验证记录", "include a Windows verification record section"],
    ["验证人：", "include a verifier entry"],
    ["Windows 环境：", "include a Windows environment entry"],
    ["安装：", "include an install result entry"],
    ["启动：", "include a launch result entry"],
    ["卸载：", "include an uninstall result entry"],
  ];

  requireSnippets(source, template, knownLimitations);
  requireSnippets(source, template, windowsVerification);
}

function verifyReleaseDocs() {
  const developmentGuidePath = "docs/guide/development.md";
  const developmentGuide = readText(developmentGuidePath);
  const phaseRoadmapPath = "docs/github/phase-roadmap.md";
  const phaseRoadmap = readText(phaseRoadmapPath);
  const releaseWorkflowPath = ".github/workflows/release.yml";
  const releaseWorkflow = readText(releaseWorkflowPath);

  requireSnippets(developmentGuidePath, developmentGuide, [
    ["`release:check` 会自动核对版本号", "describe the automated release:check gate"],
    ["`release:smoke:windows` 可重复执行", "describe the repeatable Windows installer smoke gate"],
    ["安装、启动、`liliacode <测试项目路径>` 和卸载后的 CLI 清理", "describe the automated Windows smoke coverage"],
    ["Windows 安装验证记录", "describe the Windows verification record gate"],
  ]);
  requireSnippets(phaseRoadmapPath, phaseRoadmap, [
    ["`yarn release:check --tag <tag>`", "record the release gate script entrypoint"],
    [
      "`yarn release:smoke:windows --tag <tag>` 可对 Windows draft Release 安装包重复执行安装、启动、`liliacode <测试项目路径>` 和卸载后的 CLI 清理 smoke",
      "record the repeatable Windows installer smoke entrypoint",
    ],
  ]);
  requireSnippets(releaseWorkflowPath, releaseWorkflow, [
    ["assetNamePattern: LiliaCode_[version]_[arch][setup][ext]", "keep the Tauri action asset naming pattern"],
    ["Smoke Windows installer", "run the Windows installer smoke after publishing the draft Release"],
    ["corepack yarn release:smoke:windows --tag", "keep the Windows installer smoke workflow entrypoint"],
    ["LiliaCode_<version>_x64-setup.*", "keep the draft Release checklist installer naming expectation"],
    ["Windows 安装验证记录", "keep the draft Release verification record entry"],
    ["liliacode <测试项目路径>", "keep the draft Release CLI verification check"],
    ["当前只发布 Windows 安装包", "keep the draft Release Windows-only limitation"],
    ["当前安装包没有代码签名", "keep the draft Release unsigned-installer limitation"],
    ["当前不包含 Tauri updater 自动更新能力", "keep the draft Release no-updater limitation"],
    ["当前不发布 macOS 公证包、macOS 安装包或 Linux 安装包", "keep the draft Release non-Windows package limitation"],
  ]);
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

  verifyReleaseTemplate(expectedVersion, tauriConfig.productName);
  verifyReleaseDocs();

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
    console.log("[release:check] Release notes known limitations and Windows verification record entry are present.");
    console.log("[release:check] Automated before publishing: run release:smoke:windows against the draft Release installer and record the result in the Release body.");
  }
}

for (const error of errors) {
  console.error(`[release:check] ${error}`);
}

if (errors.length > 0) {
  process.exitCode = 1;
}
