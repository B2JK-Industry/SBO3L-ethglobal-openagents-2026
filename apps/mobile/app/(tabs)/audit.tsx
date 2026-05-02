import { useEffect, useState } from "react";
import { ActivityIndicator, FlatList, StyleSheet, Text, View } from "react-native";
import { api, type AuditEvent } from "~/lib/api";
import { tokens } from "~/theme";

const TENANT_SLUG = "acme";

export default function AuditFeed(): JSX.Element {
  const [events, setEvents] = useState<AuditEvent[] | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    api.audit(TENANT_SLUG, 100)
      .then(setEvents)
      .catch((e: Error) => setError(e.message));
  }, []);

  if (error) return <View style={styles.center}><Text style={styles.err}>{error}</Text></View>;
  if (!events) return <View style={styles.center}><ActivityIndicator color={tokens.accent} /></View>;

  return (
    <FlatList
      data={events}
      keyExtractor={(e) => e.event_id}
      contentContainerStyle={styles.list}
      renderItem={({ item }) => (
        <View style={styles.row}>
          <Text style={styles.ts}>{new Date(item.ts_ms).toLocaleTimeString()}</Text>
          <Text style={[styles.kind, item.decision === "deny" && styles.deny]}>{item.kind}</Text>
          <Text style={styles.body}>{item.agent_id}{item.deny_code ? ` · ${item.deny_code}` : ""}</Text>
        </View>
      )}
    />
  );
}

const styles = StyleSheet.create({
  list: { padding: 16, backgroundColor: tokens.bg, minHeight: "100%" },
  center: { flex: 1, justifyContent: "center", alignItems: "center", backgroundColor: tokens.bg },
  err: { color: tokens.deny },
  row: { backgroundColor: tokens.codeBg, borderColor: tokens.border, borderWidth: 1, borderRadius: tokens.rSm, padding: 10, marginBottom: 6 },
  ts: { color: tokens.muted, fontSize: 11 },
  kind: { color: tokens.fg, fontSize: 13, fontWeight: "600", marginVertical: 2 },
  deny: { color: tokens.deny },
  body: { color: tokens.muted, fontSize: 12 },
});
