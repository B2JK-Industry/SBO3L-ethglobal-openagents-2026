// JS glue for the in-browser passport-capsule verifier.
//
// Lazy-loads Dev 1's wasm-bindgen output from /wasm/sbo3l_core.js + the
// associated /wasm/sbo3l_core_bg.wasm. If the module is missing (Dev 1
// slice not yet shipped), surfaces a graceful fallback so /proof still
// works as an information page.

export interface CheckResult {
  name: string;
  passed: boolean;
  detail?: string;
}

export interface VerifyResult {
  capsule_id?: string;
  checks: CheckResult[];
  rc: 0 | 1 | 2;
  total_ms: number;
}

interface WasmModule {
  default: (path?: string) => Promise<unknown>;
  verify_capsule_strict: (json: string) => unknown;
}

let modulePromise: Promise<WasmModule | null> | null = null;

async function loadModule(): Promise<WasmModule | null> {
  if (modulePromise) return modulePromise;
  modulePromise = (async () => {
    try {
      // @ts-expect-error — emitted by wasm-pack at build time; no static types until Dev 1's PR lands
      const mod = (await import("/wasm/sbo3l_core.js")) as WasmModule;
      await mod.default("/wasm/sbo3l_core_bg.wasm");
      return mod;
    } catch (err) {
      console.warn("WASM verifier module not yet available:", err);
      return null;
    }
  })();
  return modulePromise;
}

export async function isVerifierAvailable(): Promise<boolean> {
  const mod = await loadModule();
  return mod !== null;
}

export async function verifyCapsule(capsuleJson: string): Promise<VerifyResult | null> {
  const mod = await loadModule();
  if (!mod) return null;
  const t0 = performance.now();
  const raw = mod.verify_capsule_strict(capsuleJson) as VerifyResult;
  return { ...raw, total_ms: performance.now() - t0 };
}

// Pretty-printer for results — mounted into the result list by ProofVerifier.astro.
export function renderResult(result: VerifyResult, host: HTMLElement): void {
  host.innerHTML = "";
  const summary = document.createElement("p");
  summary.className = "verify-summary";
  const passed = result.checks.filter((c) => c.passed).length;
  const total = result.checks.length;
  summary.textContent = `${passed} / ${total} checks passed (rc=${result.rc}) — ${result.total_ms.toFixed(1)} ms`;
  host.append(summary);

  const list = document.createElement("ul");
  list.className = "verify-checks";
  for (const c of result.checks) {
    const li = document.createElement("li");
    li.className = c.passed ? "pass" : "fail";
    li.textContent = `${c.passed ? "✓" : "✗"} ${c.name}${c.detail ? ` — ${c.detail}` : ""}`;
    list.append(li);
  }
  host.append(list);
}
