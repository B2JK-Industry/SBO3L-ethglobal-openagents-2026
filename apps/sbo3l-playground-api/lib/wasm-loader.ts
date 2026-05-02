// sbo3l-core WASM loader — runs the real Rust decision engine
// inside a Vercel Function (Node 24 LTS, wasm32-wasi target).
//
// SKELETON. Building the WASM module is a separate task that needs
// the sbo3l-core maintainer to add a wasm32-wasi build target +
// expose the C-ABI exports (decide_aprp, build_capsule). Once that
// lands, this file loads the .wasm bundle, instantiates the
// runtime, and provides a typed wrapper around the FFI calls.

export interface WasmDecideInput {
  aprp_canonical_json: string;
  policy_toml: string;
  signing_key_pem: string;
  audit_prev_hash: string; // hex
}

export interface WasmDecideOutput {
  outcome: "allow" | "deny" | "require_human";
  deny_code?: string;
  matched_rule?: string;
  request_hash: string;
  policy_hash: string;
  audit_event_id: string;
  capsule_json: string; // signed
}

// TODO: cache the instantiated module across invocations within the
// same Vercel Function container (Vercel reuses warm containers).
// Cold-start budget: ≤500ms (R16 P9 brief). Keep the wasm bundle
// under 2 MB so the cold init isn't the dominant cost.
let _instance: unknown = null;

export async function decideAprpWasm(_input: WasmDecideInput): Promise<WasmDecideOutput> {
  // TODO:
  //   if (!_instance) {
  //     const wasm = await fetch(new URL("./sbo3l-core.wasm", import.meta.url));
  //     _instance = await WebAssembly.instantiateStreaming(wasm, imports);
  //   }
  //   const { decide_aprp } = (_instance as any).exports;
  //   ... marshal Wasm types, call, unmarshal ...
  throw new Error("wasm-loader.decideAprpWasm: skeleton — needs sbo3l-core wasm32-wasi build (separate task in next round)");
}

export async function isWasmReady(): Promise<boolean> {
  return _instance !== null;
}
