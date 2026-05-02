"use client";

import dynamic from "next/dynamic";
import { useState } from "react";

// Monaco is heavy + browser-only — dynamic import so it stays out of
// the server bundle and the SSR pass. The wrapper handles its own
// lazy AMD loader; we just feed it props.
const MonacoEditor = dynamic(
  () => import("@monaco-editor/react").then((m) => m.default),
  { ssr: false, loading: () => <p style={{ color: "var(--muted)", padding: "1em" }}>Loading editor…</p> },
);

interface Props {
  initialYaml: string;
  version: number;
  signedBy: string;
  updatedAt: string;
  tenantSlug: string;
}

type SaveState = "idle" | "validating" | "saving" | "saved" | "error";

// Codex review fix (PR #290): formatting an ISO timestamp with
// `new Date(...).toLocaleDateString()` during render produces different
// output between server pre-render (server's locale/timezone) and browser
// hydration (user's locale/timezone), which causes hydration mismatches
// and date text flicker. Format on the server with a fixed locale +
// UTC timezone so the initial client render matches the SSR HTML byte
// for byte.
function formatStableDate(iso: string): string {
  try {
    const d = new Date(iso);
    if (Number.isNaN(d.getTime())) return iso;
    return new Intl.DateTimeFormat("en-CA", { timeZone: "UTC", year: "numeric", month: "2-digit", day: "2-digit" }).format(d);
  } catch {
    return iso;
  }
}

export function PolicyEditor({ initialYaml, version, signedBy, updatedAt, tenantSlug }: Props): JSX.Element {
  const [yaml, setYaml] = useState(initialYaml);
  // Codex review fix (PR #290): the previous `dirty = yaml !== initialYaml`
  // computed against the immutable prop, so the UI kept showing
  // "unsaved changes" + an enabled Save button even after a successful
  // save told the user the draft persisted. After save we advance the
  // baseline + version so the dirty state reflects reality.
  const [savedBaseline, setSavedBaseline] = useState({ yaml: initialYaml, version });
  const [state, setState] = useState<SaveState>("idle");
  const [errorMsg, setErrorMsg] = useState("");
  const dirty = yaml !== savedBaseline.yaml;

  const onValidate = (): void => {
    setState("validating");
    setErrorMsg("");
    // Mock client-side validation. Real impl: POST to daemon
    // /v1/tenants/<slug>/policy/validate which returns parse errors
    // with line numbers so we can decorate Monaco markers.
    setTimeout(() => {
      if (!yaml.includes("schema: sbo3l.policy.v1")) {
        setState("error");
        setErrorMsg("Missing required field: schema: sbo3l.policy.v1");
        return;
      }
      setState("saved");
      setErrorMsg("Validation passed (mock — daemon-side validation pending P3.5).");
    }, 400);
  };

  const onSave = (): void => {
    setState("saving");
    // Mock save. Real impl: POST /v1/tenants/<slug>/policy with the
    // YAML body + If-Match: <version> for optimistic concurrency.
    setTimeout(() => {
      const newVersion = savedBaseline.version + 1;
      setSavedBaseline({ yaml, version: newVersion });
      setState("saved");
      setErrorMsg(`Saved as draft v${newVersion} (mock — daemon round-trip pending P3.5).`);
    }, 600);
  };

  return (
    <div style={{ display: "grid", gap: "1em" }}>
      <header style={{ display: "flex", justifyContent: "space-between", alignItems: "baseline", flexWrap: "wrap", gap: "0.6em" }}>
        <span style={{ color: "var(--muted)", fontSize: "0.9em", fontFamily: "var(--font-mono)" }}>
          {tenantSlug} · v{savedBaseline.version} · signed by <code>{signedBy}</code> · updated {formatStableDate(updatedAt)}
        </span>
        <span style={{ fontSize: "0.85em", color: dirty ? "var(--accent)" : "var(--muted)" }}>
          {dirty ? "● unsaved changes" : "○ no changes"}
        </span>
      </header>
      <div style={{ border: "1px solid var(--border)", borderRadius: "var(--r-md)", overflow: "hidden" }}>
        <MonacoEditor
          height="60vh"
          defaultLanguage="yaml"
          theme="vs-dark"
          value={yaml}
          onChange={(v) => setYaml(v ?? "")}
          options={{
            fontFamily: "var(--font-mono), Menlo, Monaco, monospace",
            fontSize: 13,
            minimap: { enabled: false },
            scrollBeyondLastLine: false,
            wordWrap: "on",
            tabSize: 2,
            renderWhitespace: "boundary",
            automaticLayout: true,
          }}
        />
      </div>
      <footer style={{ display: "flex", gap: "0.6em", alignItems: "center", flexWrap: "wrap" }}>
        <button onClick={onValidate} disabled={state === "validating" || state === "saving"} className="ghost">
          {state === "validating" ? "Validating…" : "Validate"}
        </button>
        <button onClick={onSave} disabled={!dirty || state === "saving" || state === "validating"}>
          {state === "saving" ? "Saving…" : "Save draft"}
        </button>
        {errorMsg && (
          <span style={{ fontSize: "0.85em", color: state === "error" ? "#f87171" : "var(--muted)" }}>{errorMsg}</span>
        )}
      </footer>
    </div>
  );
}
