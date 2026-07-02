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

const groups = [
  {
    label: "main entry",
    patterns: [
      /^index\.html$/,
      /^mainBootstrap-/,
      /^AppShell-/,
      /^router-/,
      /^vue-router-/,
      /^runtime-(?:core|dom)/,
    ],
  },
  {
    label: "chat path",
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

function matchesGroup(file, group) {
  return group.patterns.some((pattern) => pattern.test(file.baseName));
}

function formatSize(bytes) {
  return `${(bytes / 1024).toFixed(2)} KiB`;
}

function printGroup(group, files) {
  const totalBytes = files.reduce((sum, file) => sum + file.bytes, 0);
  const totalGzipBytes = files.reduce((sum, file) => sum + file.gzipBytes, 0);
  console.log(`\n${group.label}`);
  console.log(`  files: ${files.length}`);
  console.log(`  total: ${formatSize(totalBytes)} / gzip ${formatSize(totalGzipBytes)}`);

  for (const file of files.slice(0, 8)) {
    console.log(`  ${formatSize(file.bytes).padStart(10)} gzip ${formatSize(file.gzipBytes).padStart(10)}  ${file.name}`);
  }
  if (files.length > 8) {
    console.log(`  ... ${files.length - 8} more`);
  }
}

function main() {
  const allFiles = readFiles();
  const missingGroups = [];

  console.log("Desktop bundle baseline");
  console.log(`dist: ${path.relative(repoRoot, distRoot).replace(/\\/g, "/")}`);

  for (const group of groups) {
    const files = allFiles
      .filter((file) => matchesGroup(file, group))
      .sort((left, right) => right.bytes - left.bytes);
    if (files.length === 0) {
      missingGroups.push(group.label);
    }
    printGroup(group, files);
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

  if (missingGroups.length > 0) {
    console.error(`\nMissing expected lazy bundle groups: ${missingGroups.join(", ")}`);
    process.exit(1);
  }
}

try {
  main();
} catch (error) {
  console.error(error instanceof Error ? error.message : String(error));
  process.exit(1);
}
