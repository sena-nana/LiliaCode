#!/usr/bin/env node

import { existsSync, readdirSync, readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import path from "node:path";
import process from "node:process";
import { gzipSync } from "node:zlib";

const repoRoot = fileURLToPath(new URL("..", import.meta.url));
const distRoot = path.join(repoRoot, "apps", "desktop", "dist");
const assetsRoot = path.join(distRoot, "assets");
const warningLimitBytes = 500 * 1024;
const checkMode = process.argv.includes("--check");

const oversizedAllowedGroups = new Set(["Mermaid lazy path"]);
const kib = (value) => value * 1024;
const budget = (totalBytes, totalGzipBytes, maxFileBytes, maxFileGzipBytes) => ({
  totalBytes: kib(totalBytes),
  totalGzipBytes: kib(totalGzipBytes),
  maxFileBytes: kib(maxFileBytes),
  maxFileGzipBytes: kib(maxFileGzipBytes),
});

const groups = [
  {
    label: "main entry",
    budget: budget(220, 75, 90, 35),
    patterns: [
      /^index\.html$/,
      /^index-/,
      /^mainBootstrap-/,
      /^AppShell-/,
      /^router-/,
      /^vue-router-/,
      /^runtime-(?:core|dom)/,
    ],
  },
  {
    label: "chat path",
    budget: budget(380, 115, 90, 28),
    patterns: [
      /^TaskDetail/,
      /^Chat(?:Transcript|Composer|SidebarHost|ScrollMap|Suggestions)/,
      /^AgentTimeline-/,
      /^Timeline/,
      /^MarkdownBlock-/,
      /^chat-/,
    ],
  },
  {
    label: "Mermaid lazy path",
    lazy: true,
    budget: budget(3400, 950, 720, 170),
    patterns: [
      /mermaid/i,
      /diagram/i,
      /^cytoscape/i,
      /^dagre/i,
      /^rough\.esm-/,
    ],
  },
  {
    label: "KaTeX lazy path",
    lazy: true,
    budget: budget(1550, 1080, 300, 95),
    patterns: [
      /^KaTeX_/,
      /katex/i,
      /^mathRender-/,
      /^useDeferredMathRender-/,
      /^MarkdownMath/,
    ],
  },
  {
    label: "VueFlow lazy path",
    lazy: true,
    budget: budget(190, 65, 175, 58),
    patterns: [
      /vue-flow/i,
      /^AutomationCanvasPane-/,
    ],
  },
];

function readFiles() {
  if (!existsSync(distRoot) || !existsSync(assetsRoot)) {
    throw new Error("Desktop dist is missing. Run `yarn --cwd apps/desktop exec vite build` first.");
  }

  const files = [
    path.join(distRoot, "index.html"),
    ...readdirSync(assetsRoot).map((fileName) => path.join(assetsRoot, fileName)),
  ];
  return files
    .map((filePath) => {
      const content = readFileSync(filePath);
      return {
        name: path.relative(distRoot, filePath).replace(/\\/g, "/"),
        baseName: path.basename(filePath),
        bytes: content.byteLength,
        gzipBytes: gzipSync(content).byteLength,
      };
    });
}

function readInitialAssetNames() {
  const indexHtml = readFileSync(path.join(distRoot, "index.html"), "utf8");
  const assetNames = new Set();
  for (const match of indexHtml.matchAll(/\b(?:src|href)=["']([^"']+)["']/g)) {
    const assetMatch = match[1].match(/(?:^|\/)assets\/([^/?#]+)/);
    if (assetMatch) {
      assetNames.add(decodeURIComponent(assetMatch[1]));
    }
  }
  return assetNames;
}

function matchesGroup(file, group) {
  return group.patterns.some((pattern) => pattern.test(file.baseName));
}

function findGroups(file) {
  return groups.filter((group) => matchesGroup(file, group));
}

function formatSize(bytes) {
  return `${(bytes / 1024).toFixed(2)} KiB`;
}

function summarizeGroup(files) {
  const totalBytes = files.reduce((sum, file) => sum + file.bytes, 0);
  const totalGzipBytes = files.reduce((sum, file) => sum + file.gzipBytes, 0);
  const largestFile = files.reduce((largest, file) => (
    !largest || file.bytes > largest.bytes ? file : largest
  ), null);
  const largestGzipFile = files.reduce((largest, file) => (
    !largest || file.gzipBytes > largest.gzipBytes ? file : largest
  ), null);
  return { totalBytes, totalGzipBytes, largestFile, largestGzipFile };
}

function printGroup(group, files, summary) {
  console.log(`\n${group.label}`);
  console.log(`  files: ${files.length}`);
  console.log(`  total: ${formatSize(summary.totalBytes)} / gzip ${formatSize(summary.totalGzipBytes)}`);
  if (checkMode && group.budget) {
    console.log(`  budget: total <= ${formatSize(group.budget.totalBytes)} / gzip <= ${formatSize(group.budget.totalGzipBytes)}`);
    console.log(`  budget: largest <= ${formatSize(group.budget.maxFileBytes)} / gzip <= ${formatSize(group.budget.maxFileGzipBytes)}`);
  }

  for (const file of files.slice(0, 8)) {
    console.log(`  ${formatSize(file.bytes).padStart(10)} gzip ${formatSize(file.gzipBytes).padStart(10)}  ${file.name}`);
  }
  if (files.length > 8) {
    console.log(`  ... ${files.length - 8} more`);
  }
}

function collectBudgetFailures(group, summary) {
  if (!group.budget || summary.totalBytes === 0) {
    return [];
  }
  const failures = [];
  const checks = [
    ["total", summary.totalBytes, group.budget.totalBytes, null],
    ["total gzip", summary.totalGzipBytes, group.budget.totalGzipBytes, null],
    ["largest file", summary.largestFile?.bytes ?? 0, group.budget.maxFileBytes, summary.largestFile],
    ["largest gzip file", summary.largestGzipFile?.gzipBytes ?? 0, group.budget.maxFileGzipBytes, summary.largestGzipFile],
  ];
  for (const [label, actual, limit, file] of checks) {
    if (actual > limit) {
      const fileText = file ? ` (${file.name})` : "";
      failures.push(`${group.label} ${label}${fileText} is ${formatSize(actual)}, limit ${formatSize(limit)}`);
    }
  }
  return failures;
}

function collectInitialLazyFailures(allFiles, initialAssetNames) {
  return allFiles
    .filter((file) => initialAssetNames.has(file.baseName))
    .flatMap((file) => findGroups(file)
      .filter((group) => group.lazy)
      .map((group) => `${file.name} matches ${group.label} but is referenced by index.html`));
}

function collectUnexpectedOversizedFailures(oversized) {
  return oversized
    .filter((file) => !findGroups(file).some((group) => oversizedAllowedGroups.has(group.label)))
    .map((file) => `${file.name} is ${formatSize(file.bytes)} and is not an allowed lazy oversized chunk`);
}

function main() {
  const allFiles = readFiles();
  const initialAssetNames = readInitialAssetNames();
  const missingGroups = [];
  const failures = [];

  console.log("Desktop bundle baseline");
  console.log(`dist: ${path.relative(repoRoot, distRoot).replace(/\\/g, "/")}`);

  for (const group of groups) {
    const files = allFiles
      .filter((file) => matchesGroup(file, group))
      .sort((left, right) => right.bytes - left.bytes);
    if (files.length === 0) {
      missingGroups.push(group.label);
    }
    const summary = summarizeGroup(files);
    printGroup(group, files, summary);
    failures.push(...collectBudgetFailures(group, summary));
  }

  const oversized = allFiles
    .filter((file) => file.bytes > warningLimitBytes)
    .sort((left, right) => right.bytes - left.bytes);
  console.log("\nchunks over 500 KiB");
  if (oversized.length === 0) {
    console.log("  none");
  } else {
    for (const file of oversized) {
      console.log(`  ${formatSize(file.bytes).padStart(10)} gzip ${formatSize(file.gzipBytes).padStart(10)}  ${file.name}`);
    }
  }

  failures.push(...collectInitialLazyFailures(allFiles, initialAssetNames));
  failures.push(...collectUnexpectedOversizedFailures(oversized));

  if (missingGroups.length > 0) {
    failures.unshift(`Missing expected bundle groups: ${missingGroups.join(", ")}`);
  }

  if (checkMode && failures.length > 0) {
    console.error("\nDesktop bundle regression gate failed:");
    for (const failure of failures) {
      console.error(`  - ${failure}`);
    }
    process.exit(1);
  }

  if (checkMode) {
    console.log("\nDesktop bundle regression gate passed.");
  } else if (missingGroups.length > 0) {
    console.error(`\nMissing expected bundle groups: ${missingGroups.join(", ")}`);
    process.exit(1);
  }
}

try {
  main();
} catch (error) {
  console.error(error instanceof Error ? error.message : String(error));
  process.exit(1);
}
