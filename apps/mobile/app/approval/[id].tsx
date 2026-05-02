import { useState } from "react";
import { Alert, StyleSheet, Text, TouchableOpacity, View } from "react-native";
import { useLocalSearchParams, useRouter } from "expo-router";
import { api } from "~/lib/api";
import { biometricGate } from "~/lib/auth";
import { tokens } from "~/theme";

const TENANT_SLUG = "acme";

export default function ApprovalDetail(): JSX.Element {
  const { id } = useLocalSearchParams<{ id: string }>();
  const router = useRouter();
  const [submitting, setSubmitting] = useState(false);

  const resolve = async (decision: "allow" | "deny"): Promise<void> => {
    if (!id) return;
    const ok = await biometricGate(decision === "allow" ? "Confirm approval" : "Confirm denial");
    if (!ok) return;
    setSubmitting(true);
    try {
      await api.resolveApproval(TENANT_SLUG, id, decision);
      router.back();
    } catch (e) {
      Alert.alert("Error", (e as Error).message);
      setSubmitting(false);
    }
  };

  return (
    <View style={styles.container}>
      <Text style={styles.h1}>Decision {id}</Text>
      <Text style={styles.muted}>Re-authenticate with biometrics before resolving.</Text>
      <View style={styles.row}>
        <TouchableOpacity disabled={submitting} onPress={() => resolve("deny")} style={[styles.btn, styles.deny]}>
          <Text style={styles.btnText}>Deny</Text>
        </TouchableOpacity>
        <TouchableOpacity disabled={submitting} onPress={() => resolve("allow")} style={[styles.btn, styles.allow]}>
          <Text style={styles.btnText}>Approve</Text>
        </TouchableOpacity>
      </View>
    </View>
  );
}

const styles = StyleSheet.create({
  container: { flex: 1, backgroundColor: tokens.bg, padding: 16 },
  h1: { color: tokens.fg, fontSize: 22, fontWeight: "700", marginBottom: 8 },
  muted: { color: tokens.muted, fontSize: 13, marginBottom: 24 },
  row: { flexDirection: "row", gap: 10 },
  btn: { flex: 1, padding: 16, borderRadius: tokens.rMd, alignItems: "center" },
  deny: { backgroundColor: tokens.deny },
  allow: { backgroundColor: tokens.accent },
  btnText: { color: tokens.bg, fontSize: 16, fontWeight: "700" },
});
