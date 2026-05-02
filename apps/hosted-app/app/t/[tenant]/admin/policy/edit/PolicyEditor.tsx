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

export function PolicyEditor({ initialYaml, version, signedBy, updatedAt, tenantSlug }: Props): JSX.Element {
  const [yaml, setYaml] = useState(initialYaml);
  const [state, setState] = useState<SaveState>("idle");
  const [errorMsg, setErrorMsg] = useState("");
  const dirty = yaml !== initialYaml;

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
      setState("saved");
      setErrorMsg(`Saved as draft v${version + 1} (mock — daemon round-trip pending P3.5).`);
    }, 600);
  };

  return (
    <div style={{ display: "grid", gap: "1em" }}>
      <header style={{ display: "flex", justifyContent: "space-between", alignItems: "baseline", flexWrap: "wrap", gap: "0.6em" }}>
        <span style={{ color: "var(--muted)", fontSize: "0.9em", fontFamily: "var(--font-mono)" }}>
          {tenantSlug} · v{version} · signed by <code>{signedBy}</code> · updated {new Date(updatedAt).toLocaleDateString()}
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
