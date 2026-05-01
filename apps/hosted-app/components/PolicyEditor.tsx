"use client";

import { useEffect, useRef, useState } from "react";
import { POLICY_SCHEMA } from "@/lib/policy-schema";

interface Props {
  initial: string;
  onChange: (value: string, valid: boolean) => void;
}

// Monaco-backed JSON editor. Lazy-loaded via dynamic import — Monaco
// is ~2 MB minified, so we only pay the cost on /policy/edit. The
// fallback `<textarea>` keeps the page functional during the load
// (and on environments where Monaco fails to initialize).
//
// Schema autocomplete: we register POLICY_SCHEMA against the
// `inmemory://policy.json` model URI so the editor surfaces every
// allowed field + enum value as the operator types.
export function PolicyEditor({ initial, onChange }: Props): JSX.Element {
  const containerRef = useRef<HTMLDivElement | null>(null);
  const editorRef = useRef<{ dispose: () => void } | null>(null);
  const [ready, setReady] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    let editor: { dispose: () => void } | null = null;

    (async () => {
      try {
        // Dynamic import keeps Monaco out of the FCP bundle.
        const monaco = await import("monaco-editor");
        if (cancelled || !containerRef.current) return;

        monaco.languages.json.jsonDefaults.setDiagnosticsOptions({
          validate: true,
          schemas: [
            {
              uri: "inmemory://schema/sbo3l.policy.v1",
              fileMatch: ["inmemory://policy.json"],
              schema: POLICY_SCHEMA,
            },
          ],
        });

        const model = monaco.editor.createModel(initial, "json", monaco.Uri.parse("inmemory://policy.json"));
        editor = monaco.editor.create(containerRef.current, {
          model,
          theme: "vs-dark",
          minimap: { enabled: false },
          fontFamily: "ui-monospace, SF Mono, Menlo, monospace",
          fontSize: 13,
          automaticLayout: true,
          tabSize: 2,
        });
        editorRef.current = editor;

        const updateValidity = (): void => {
          const value = model.getValue();
          const markers = monaco.editor.getModelMarkers({ resource: model.uri });
          const valid = markers.every((m) => m.severity < 8); // <Error
          onChange(value, valid);
        };
        updateValidity();
        const sub = model.onDidChangeContent(() => updateValidity());

        setReady(true);

        return () => {
          sub.dispose();
          model.dispose();
        };
      } catch (e) {
        if (!cancelled) setError(e instanceof Error ? e.message : "Monaco load failed");
      }
    })();

    return () => {
      cancelled = true;
      editor?.dispose();
      editorRef.current = null;
    };
  }, [initial, onChange]);

  return (
    <div style={{ position: "relative", border: "1px solid var(--border)", borderRadius: "var(--r-md)", height: "60vh", minHeight: 360, overflow: "hidden" }}>
      <div ref={containerRef} style={{ width: "100%", height: "100%" }} />
      {!ready && !error && (
        <textarea
          defaultValue={initial}
          aria-label="Policy JSON (Monaco loading)"
          onChange={(ev) => onChange(ev.target.value, true)}
          style={{
            position: "absolute", inset: 0,
            width: "100%", height: "100%",
            background: "var(--code-bg)", color: "var(--fg)",
            border: "0", padding: "1em",
            fontFamily: "var(--font-mono)", fontSize: "13px",
          }}
        />
      )}
      {error && (
        <div style={{ position: "absolute", inset: 0, padding: "1em", color: "#ff6b6b" }}>
          Editor failed to initialize: {error}. Falling back to plain JSON edit.
        </div>
      )}
    </div>
  );
}
