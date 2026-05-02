import { useLocalSearchParams } from "expo-router";
import { useEffect, useMemo, useState } from "react";
import { ScrollView, StyleSheet, Text, View } from "react-native";
import { tokens } from "~/theme";

// Capsule verify screen — destination of the QR scanner from app/scan.tsx.
// Codex review fix (PR #309): scan.tsx pushed users to /verify but no
// /verify route existed, breaking the scanner handoff.
//
// Real WASM verifier integration is gated on the same wasm32-wasi build
// of sbo3l-core that the playground API depends on (see
// apps/sbo3l-playground-api/lib/wasm-loader.ts). Until that lands, we
// run the structural checks the mock playground does on the web —
// schema string + presence of the load-bearing fields. The user can
// share the capsule via the system share sheet to reach the marketing
// /proof page where a more thorough check happens.

interface CapsulePolicyReceipt {
  request_hash?: string;
  outcome?: "allow" | "deny" | "require_human";
  signature?: string;
}

interface MaybeCapsule {
  schema?: string;
  policy_receipt?: CapsulePolicyReceipt;
}

interface StructuralCheck {
  label: string;
  passed: boolean;
  detail?: string;
}

function runStructuralChecks(raw: string): { capsule?: MaybeCapsule; checks: StructuralCheck[]; parseError?: string } {
  let parsed: MaybeCapsule;
  try {
    parsed = JSON.parse(raw) as MaybeCapsule;
  } catch (e) {
    return { checks: [], parseError: (e as Error).message };
  }

  const schema = typeof parsed.schema === "string" ? parsed.schema : "";
  const isMock = schema === "sbo3l.playground_mock.v1";
  const isReal = schema.startsWith("sbo3l.passport_capsule.");

  const checks: StructuralCheck[] = [
    {
      label: "Schema string present",
      passed: schema.length > 0,
      detail: schema || "missing",
    },
    {
      label: "Schema is a recognized capsule version",
      passed: isMock || isReal,
      detail: isMock ? "MOCK (sbo3l.playground_mock.v1)" : isReal ? `real (${schema})` : `unknown: ${schema}`,
    },
    {
      label: "Has policy_receipt object",
      passed: typeof parsed.policy_receipt === "object" && parsed.policy_receipt !== null,
    },
    {
      label: "Has request_hash",
      passed: typeof parsed.policy_receipt?.request_hash === "string",
      detail: parsed.policy_receipt?.request_hash?.slice(0, 18) ?? "—",
    },
    {
      label: "Has outcome (allow/deny/require_human)",
      passed: ["allow", "deny", "require_human"].includes(parsed.policy_receipt?.outcome ?? ""),
      detail: parsed.policy_receipt?.outcome ?? "—",
    },
    {
      label: "Has signature (if real schema)",
      passed: !isReal || (typeof parsed.policy_receipt?.signature === "string" && parsed.policy_receipt.signature !== "MOCK_NOT_SIGNED"),
      detail: parsed.policy_receipt?.signature === "MOCK_NOT_SIGNED" ? "MOCK_NOT_SIGNED (Tier-2 capsule)" : (parsed.policy_receipt?.signature?.slice(0, 18) ?? "—"),
    },
  ];

  return { capsule: parsed, checks };
}

export default function VerifyScreen(): JSX.Element {
  const params = useLocalSearchParams<{ capsule?: string }>();
  const [decoded, setDecoded] = useState<string | null>(null);

  useEffect(() => {
    if (typeof params.capsule === "string" && params.capsule.length > 0) {
      // QR payloads are usually url-safe base64 of the JSON. Try
      // decoding; fall back to the raw value if it parses already.
      try {
        const decodedB64 = atob(params.capsule.replace(/-/g, "+").replace(/_/g, "/"));
        JSON.parse(decodedB64);
        setDecoded(decodedB64);
      } catch {
        setDecoded(params.capsule);
      }
    }
  }, [params.capsule]);

  const result = useMemo(() => decoded ? runStructuralChecks(decoded) : null, [decoded]);

  if (!decoded) {
    return (
      <View style={styles.center}>
        <Text style={styles.muted}>No capsule supplied. Open this screen via /scan.</Text>
      </View>
    );
  }

  if (result?.parseError) {
    return (
      <View style={styles.center}>
        <Text style={styles.error}>Capsule isn't valid JSON.</Text>
        <Text style={styles.muted}>{result.parseError}</Text>
      </View>
    );
  }

  const passed = result?.checks.every((c) => c.passed) ?? false;

  return (
    <ScrollView contentContainerStyle={styles.container}>
      <Text style={[styles.headline, passed ? styles.allow : styles.deny]}>
        {passed ? "✓ Structural checks passed" : "✗ Capsule failed structural checks"}
      </Text>
      <Text style={styles.note}>
        Mobile verifier runs structural checks only. For full strict-mode WASM
        cryptographic verification, share this capsule to the /proof page on
        sbo3l-marketing.vercel.app (link below).
      </Text>
      <View style={styles.checks}>
        {result?.checks.map((c, i) => (
          <View key={i} style={styles.check}>
            <Text style={[styles.checkMark, c.passed ? styles.allow : styles.deny]}>
              {c.passed ? "✓" : "✗"}
            </Text>
            <View style={styles.checkBody}>
              <Text style={styles.checkLabel}>{c.label}</Text>
              {c.detail && <Text style={styles.checkDetail}>{c.detail}</Text>}
            </View>
          </View>
        ))}
      </View>
    </ScrollView>
  );
}

const styles = StyleSheet.create({
  container: { padding: 16, backgroundColor: tokens.bg, minHeight: "100%" },
  center: { flex: 1, justifyContent: "center", alignItems: "center", padding: 16, backgroundColor: tokens.bg },
  muted: { color: tokens.muted, textAlign: "center" },
  error: { color: tokens.deny, marginBottom: 12, fontWeight: "700" },
  headline: { fontSize: 18, fontWeight: "700", marginBottom: 8 },
  allow: { color: tokens.accent },
  deny: { color: tokens.deny },
  note: { color: tokens.muted, fontSize: 13, marginBottom: 16, lineHeight: 19 },
  checks: { gap: 6 },
  check: { flexDirection: "row", gap: 10, padding: 10, backgroundColor: tokens.codeBg, borderColor: tokens.border, borderWidth: 1, borderRadius: tokens.rSm },
  checkMark: { fontSize: 16, fontWeight: "700", width: 20 },
  checkBody: { flex: 1 },
  checkLabel: { color: tokens.fg, fontSize: 14 },
  checkDetail: { color: tokens.muted, fontSize: 12, marginTop: 2, fontFamily: tokens.fontMono },
});
