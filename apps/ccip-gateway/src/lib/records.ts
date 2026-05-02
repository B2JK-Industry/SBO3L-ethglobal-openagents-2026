/**
 * Static record source for the CCIP-Read gateway. T-4-1 ships a JSON
 * file at `apps/ccip-gateway/data/records.json` keyed by FQDN; this
 * module loads + indexes it once at module init for O(1) lookups by
 * namehash. T-4-3 swaps in a SQLite-backed live source.
 */

import recordsRaw from "../../data/records.json";
import { namehash } from "./ens";

type RecordsFile = Record<string, Record<string, string>>;

interface AgentEntry {
  fqdn: string;
  records: Record<string, string>;
}

/** namehash → AgentEntry lookup table. Built at module init. */
const byNode = new Map<string, AgentEntry>();

(function buildIndex() {
  const file = recordsRaw as unknown as RecordsFile;
  for (const [fqdn, records] of Object.entries(file)) {
    if (fqdn.startsWith("_")) continue; // skip _comment etc.
    const node = namehash(fqdn).toLowerCase();
    byNode.set(node, { fqdn, records });
  }
})();

/**
 * Return the records for a given namehash, or `null` if no agent
 * exists at that node.
 */
export function lookupByNode(
  node: `0x${string}`,
): AgentEntry | null {
  return byNode.get(node.toLowerCase()) ?? null;
}

/** All known FQDNs — useful for diagnostics + the landing page. */
export function knownFqdns(): string[] {
  return Array.from(byNode.values())
    .map((e) => e.fqdn)
    .sort();
}

/** Total number of agents in the static source. */
export function recordsCount(): number {
  return byNode.size;
}
