/* tslint:disable */
/* eslint-disable */

/**
 * Crate version exposed to JS so the marketing site can show
 * "verifier built from sbo3l-core v0.1.0" honestly.
 */
export function sbo3l_core_version(): string;

/**
 * Structural verify entry point. JS calls
 * `verify_capsule_json(capsuleJsonString)`. Resolves to `null` on
 * success; rejects with the `capsule.<code>` string on failure.
 */
export function verify_capsule_json(capsule_json: string): any;

/**
 * Strict (cryptographic) verify entry point. JS calls
 * `verify_capsule_strict_json(capsuleJsonString)` and gets a
 * structured object back:
 *
 * ```ignore
 * {
 *   ok: boolean,             // true iff every check passed (no failures, no skips)
 *   any_failed: boolean,     // true iff at least one check failed
 *   checks: [
 *     { label: "structural", outcome: "PASSED" | "SKIPPED" | "FAILED", detail?: string },
 *     ...
 *   ]
 * }
 * ```
 *
 * No auxiliary inputs are accepted — this is the v2 self-contained
 * path. v1 capsules + v2 capsules with missing embedded fields will
 * see SKIPPED outcomes for the aux-dependent checks, which is the
 * expected honest-disclosure behaviour from F-6.
 */
export function verify_capsule_strict_json(capsule_json: string): any;

export type InitInput = RequestInfo | URL | Response | BufferSource | WebAssembly.Module;

export interface InitOutput {
    readonly memory: WebAssembly.Memory;
    readonly sbo3l_core_version: (a: number) => void;
    readonly verify_capsule_json: (a: number, b: number, c: number) => void;
    readonly verify_capsule_strict_json: (a: number, b: number, c: number) => void;
    readonly __wbindgen_export: (a: number, b: number) => number;
    readonly __wbindgen_export2: (a: number, b: number, c: number, d: number) => number;
    readonly __wbindgen_export3: (a: number) => void;
    readonly __wbindgen_add_to_stack_pointer: (a: number) => number;
    readonly __wbindgen_export4: (a: number, b: number, c: number) => void;
}

export type SyncInitInput = BufferSource | WebAssembly.Module;

/**
 * Instantiates the given `module`, which can either be bytes or
 * a precompiled `WebAssembly.Module`.
 *
 * @param {{ module: SyncInitInput }} module - Passing `SyncInitInput` directly is deprecated.
 *
 * @returns {InitOutput}
 */
export function initSync(module: { module: SyncInitInput } | SyncInitInput): InitOutput;

/**
 * If `module_or_path` is {RequestInfo} or {URL}, makes a request and
 * for everything else, calls `WebAssembly.instantiate` directly.
 *
 * @param {{ module_or_path: InitInput | Promise<InitInput> }} module_or_path - Passing `InitInput` directly is deprecated.
 *
 * @returns {Promise<InitOutput>}
 */
export default function __wbg_init (module_or_path?: { module_or_path: InitInput | Promise<InitInput> } | InitInput | Promise<InitInput>): Promise<InitOutput>;
