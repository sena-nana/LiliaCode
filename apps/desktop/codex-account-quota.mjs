import { readCodexAccountQuotaStatus } from "./agent-runner/codex/accountQuota.mjs";

function writeJson(payload) {
  process.stdout.write(`${JSON.stringify(payload)}\n`);
}

async function main() {
  writeJson(await readCodexAccountQuotaStatus());
}

main().catch((err) => {
  writeJson({
    available: false,
    connectionMode: "codex-account",
    limitId: null,
    limitName: null,
    planType: null,
    rateLimitReachedType: null,
    fiveHour: null,
    weekly: null,
    sparkFiveHour: null,
    sparkWeekly: null,
    fetchedAt: Date.now(),
    error: err?.message || String(err),
  });
  process.exit(1);
});
