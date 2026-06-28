#!/usr/bin/env node
import { execFileSync } from "node:child_process";
import { existsSync, mkdirSync, readFileSync } from "node:fs";
import { writeFile } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";

const DEFAULT_BRANCH = "main";
const UNKNOWN_LICENSE = "未声明";
const ROOT_DIR = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const OUTPUT_PATH = path.resolve(ROOT_DIR, "src/generated/openSourceLicenseManifest.json");

function readJson(filePath, fallback = null) {
  if (!existsSync(filePath)) return fallback;
  return JSON.parse(readFileSync(filePath, "utf8"));
}

function toHttpsRepoUrl(rawUrl) {
  if (!rawUrl) return "";
  const trimmed = String(rawUrl).trim();
  if (!trimmed) return "";

  let normalized = trimmed.replace(/^git\+/, "").replace(/\/$/, "").replace(/\.git$/, "");
  if (/^git@[^:]+:/.test(normalized)) {
    normalized = normalized.replace(/^git@([^:]+):(.+)$/, "https://$1/$2");
  } else if (normalized.startsWith("ssh://")) {
    normalized = normalized.replace(/^ssh:\/\//, "https://");
  }

  return normalized;
}

function normalizeRepositoryUrl(raw) {
  if (typeof raw === "string") return toHttpsRepoUrl(raw);
  if (raw && typeof raw === "object" && typeof raw.url === "string") return toHttpsRepoUrl(raw.url);
  return "";
}

function normalizeLicense(raw) {
  if (typeof raw === "string") return raw.trim() || UNKNOWN_LICENSE;
  if (Array.isArray(raw)) {
    const values = raw
      .map(normalizeLicense)
      .filter((item) => item && item !== UNKNOWN_LICENSE);
    if (values.length === 0) return UNKNOWN_LICENSE;
    return [...new Set(values)].sort().join("/");
  }
  if (raw && typeof raw === "object") {
    if (typeof raw.type === "string" && raw.type.trim()) return raw.type.trim();
    if (typeof raw.name === "string" && raw.name.trim()) return raw.name.trim();
  }
  return UNKNOWN_LICENSE;
}

function normalizePath(p) {
  return path.resolve(p).replace(/\\/g, "/");
}

function summarizeByLicense(items) {
  const map = new Map();
  for (const item of items) {
    const key = normalizeLicense(item.license || UNKNOWN_LICENSE);
    map.set(key, (map.get(key) ?? 0) + 1);
  }
  return Object.fromEntries([...map.entries()].sort(([a], [b]) => a.localeCompare(b)));
}

function findPackageJsonPath(dependencyName) {
  const candidates = [
    path.resolve(ROOT_DIR, "node_modules", dependencyName, "package.json"),
    path.resolve(ROOT_DIR, "..", "node_modules", dependencyName, "package.json"),
  ];
  for (const candidate of candidates) {
    if (existsSync(candidate)) return candidate;
  }
  return null;
}

function collectNpmDependencies(packageJson) {
  const collected = new Map();

  const addFromSection = (deps = {}) => {
    for (const [name, req] of Object.entries(deps)) {
      if (collected.has(name)) continue;
      const localPackage = readJson(findPackageJsonPath(name) || "", null);
      collected.set(name, {
        name,
        version: localPackage?.version ?? String(req),
        license: normalizeLicense(localPackage?.license),
      });
    }
  };

  addFromSection(packageJson.dependencies);
  addFromSection(packageJson.devDependencies);

  return [...collected.values()].sort((left, right) => left.name.localeCompare(right.name, "en"));
}

function collectRustDependencies() {
  const metadataRaw = execFileSync(
    "cargo",
    ["metadata", "--offline", "--format-version", "1", "--manifest-path", "src-tauri/Cargo.toml"],
    { cwd: ROOT_DIR, encoding: "utf8", maxBuffer: 1024 * 1024 * 100 },
  );
  const metadata = JSON.parse(metadataRaw);
  const packages = metadata?.packages ?? [];
  const rootPath = normalizePath(path.resolve(ROOT_DIR, "src-tauri/Cargo.toml"));
  const rootPackage = packages.find((pkg) => normalizePath(pkg.manifest_path) === rootPath);
  if (!rootPackage) {
    throw new Error("未找到 src-tauri/Cargo.toml 对应的 cargo metadata 包。");
  }

  const byName = new Map();
  for (const pkg of packages) {
    if (!byName.has(pkg.name)) byName.set(pkg.name, pkg);
  }

  const directDeps = (rootPackage.dependencies ?? [])
    .filter((dep) => dep.kind === undefined || dep.kind === null || dep.kind === "normal" || dep.kind === "build" || dep.kind === "dev")
    .map((dep) => {
      const packageMeta = byName.get(dep.name) ?? {};
      return {
        name: dep.name,
        version: packageMeta.version ?? dep.req ?? UNKNOWN_LICENSE,
        license: normalizeLicense(packageMeta.license),
      };
    });

  return [...new Map(directDeps.map((dep) => [dep.name, dep])).values()].sort((left, right) =>
    left.name.localeCompare(right.name, "en"),
  );
}

function buildLicenseUrl(repositoryUrl, homepage) {
  const repoUrl = toHttpsRepoUrl(repositoryUrl);
  if (repoUrl) return `${repoUrl}/blob/${DEFAULT_BRANCH}/LICENSE`;
  const home = toHttpsRepoUrl(homepage);
  return home ? `${home}/blob/${DEFAULT_BRANCH}/LICENSE` : "";
}

async function main() {
  const packageJson = readJson(path.resolve(ROOT_DIR, "package.json"), {});
  const tauriConfig = readJson(path.resolve(ROOT_DIR, "src-tauri/tauri.conf.json"), {});
  const repository = normalizeRepositoryUrl(packageJson.repository);

  const npmDependencies = collectNpmDependencies(packageJson);
  const rustDependencies = collectRustDependencies();
  const manifest = {
    generatedAt: new Date().toISOString(),
    app: {
      name: tauriConfig?.productName || packageJson.name || "LiliaCode",
      version: packageJson.version || tauriConfig?.version || "1.0.0-beta.1",
      license: normalizeLicense(packageJson.license),
      licenseUrl: buildLicenseUrl(repository, packageJson.homepage),
    },
    npmDependencies,
    rustDependencies,
    totals: {
      npm: summarizeByLicense(npmDependencies),
      rust: summarizeByLicense(rustDependencies),
    },
  };

  mkdirSync(path.dirname(OUTPUT_PATH), { recursive: true });
  await writeFile(OUTPUT_PATH, `${JSON.stringify(manifest, null, 2)}\n`);
  console.log(`Generated ${path.relative(ROOT_DIR, OUTPUT_PATH)}`);
}

await main();

