import { gzipSync } from "node:zlib";
import { readdir, readFile } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const assetsDir = path.join(repoRoot, "apps", "desktop", "dist", "assets");
const rawWarningLimit = 500 * 1024;
const gzipBudget = 120 * 1024;
const mermaidParserException = /^mermaid-parser-/;

function formatKb(bytes) {
  return `${(bytes / 1024).toFixed(2)} kB`;
}

function isJavaScriptAsset(name) {
  return name.endsWith(".js");
}

function isAnonymousChunk(name) {
  return /^chunk-[A-Z0-9]/i.test(name);
}

async function collectChunks() {
  const names = await readdir(assetsDir);
  const chunks = await Promise.all(
    names
      .filter(isJavaScriptAsset)
      .map(async (name) => {
        const file = path.join(assetsDir, name);
        const bytes = await readFile(file);
        return {
          name,
          raw: bytes.byteLength,
          gzip: gzipSync(bytes).byteLength,
        };
      }),
  );
  return chunks.sort((a, b) => b.raw - a.raw);
}

function reportTopChunks(chunks) {
  console.log("Top desktop JavaScript chunks:");
  for (const chunk of chunks.slice(0, 20)) {
    console.log(
      `${chunk.name.padEnd(56)} raw ${formatKb(chunk.raw).padStart(10)} gzip ${formatKb(chunk.gzip).padStart(10)}`,
    );
  }
}

function validateChunks(chunks) {
  const failures = [];
  const warnings = [];

  for (const chunk of chunks) {
    const mermaidException = mermaidParserException.test(chunk.name);
    if (mermaidException) {
      if (chunk.gzip > gzipBudget) {
        warnings.push(
          `${chunk.name} exceeds the ${formatKb(gzipBudget)} gzip budget, allowed as an async Mermaid parser exception.`,
        );
      }
      continue;
    }

    if (chunk.raw > rawWarningLimit) {
      failures.push(`${chunk.name} exceeds the ${formatKb(rawWarningLimit)} raw chunk budget.`);
    }
    if (isAnonymousChunk(chunk.name) && chunk.raw > 250 * 1024) {
      failures.push(`${chunk.name} is an oversized anonymous chunk; add a named split point.`);
    }
  }

  for (const warning of warnings) {
    console.warn(`Warning: ${warning}`);
  }

  if (failures.length > 0) {
    for (const failure of failures) {
      console.error(`Error: ${failure}`);
    }
    process.exitCode = 1;
  }
}

try {
  const chunks = await collectChunks();
  reportTopChunks(chunks);
  validateChunks(chunks);
} catch (error) {
  console.error(`Failed to inspect desktop bundle: ${error?.message ?? error}`);
  process.exitCode = 1;
}
