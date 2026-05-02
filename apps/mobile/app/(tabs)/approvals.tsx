import { useEffect, useState } from "react";
import { ActivityIndicator, FlatList, StyleSheet, Text, TouchableOpacity, View } from "react-native";
import { Link } from "expo-router";
import { api, type PendingApproval } from "~/lib/api";
import { tokens } from "~/theme";

const TENANT_SLUG = "acme"; // TODO: read from tenant picker once multi-tenant context lands

export default function ApprovalsList(): JSX.Element {
  const [items, setItems] = useState<PendingApproval[] | null>(null);
  const [error, setError] = useState<string | null>(null);

  const reload = (): void => {
    api.approvals(TENANT_SLUG)
      .then(setItems)
      .catch((e: Error) => setError(e.message));
  };

  useEffect(reload, []);

  if (error) return <View style={styles.center}><Text style={styles.error}>{error}</Text></View>;
  if (!items) return <View style={styles.center}><ActivityIndicator color={tokens.accent} /></View>;
  if (items.length === 0) return <View style={styles.center}><Text style={styles.muted}>No pending approvals.</Text></View>;

  return (
    <FlatList
      data={items}
      keyExtractor={(it) => it.decision_id}
      contentContainerStyle={styles.list}
      onRefresh={reload}
      refreshing={false}
      renderItem={({ item }) => (
        <Link href={{ pathname: "/approval/[id]", params: { id: item.decision_id } }} asChild>
          <TouchableOpacity style={styles.card}>
            <Text style={styles.agent}>{item.agent_id}</Text>
            <Text style={styles.intent}>{item.intent}</Text>
            {item.amount_usd_cents !== undefined && (
              <Text style={styles.amount}>${(item.amount_usd_cents / 100).toFixed(2)}</Text>
            )}
            <Text style={styles.expires}>expires {new Date(item.expires_at).toLocaleString()}</Text>
          </TouchableOpacity>
        </Link>
      )}
    />
  );
}

const styles = StyleSheet.create({
  list: { padding: 16, backgroundColor: tokens.bg, minHeight: "100%" },
  center: { flex: 1, justifyContent: "center", alignItems: "center", backgroundColor: tokens.bg, padding: 16 },
  error: { color: tokens.deny },
  muted: { color: tokens.muted },
  card: { backgroundColor: tokens.codeBg, borderColor: tokens.border, borderWidth: 1, borderRadius: tokens.rMd, padding: 14, marginBottom: 10 },
  agent: { color: tokens.fg, fontSize: 14, fontWeight: "600", marginBottom: 4 },
  intent: { color: tokens.muted, fontSize: 13, marginBottom: 6 },
  amount: { color: tokens.accent, fontSize: 18, fontWeight: "700", marginBottom: 4 },
  expires: { color: tokens.muted, fontSize: 11 },
});
