import { useEffect, useState } from "react";
import { ActivityIndicator, ScrollView, StyleSheet, Text, View } from "react-native";
import { Link } from "expo-router";
import { api, type Tenant } from "~/lib/api";
import { tokens } from "~/theme";

interface Membership { tenant: Tenant; role: string }

export default function Dashboard(): JSX.Element {
  const [memberships, setMemberships] = useState<Membership[] | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    api.me()
      .then((res) => setMemberships(res.memberships))
      .catch((e: Error) => setError(e.message));
  }, []);

  if (error) return <View style={styles.center}><Text style={styles.error}>{error}</Text><Link href="/signin" style={styles.link}>Sign in</Link></View>;
  if (!memberships) return <View style={styles.center}><ActivityIndicator color={tokens.accent} /></View>;

  return (
    <ScrollView contentContainerStyle={styles.container}>
      <Text style={styles.h1}>Tenants</Text>
      {memberships.length === 0 ? (
        <Text style={styles.muted}>No tenant memberships. Ask an admin to invite you.</Text>
      ) : memberships.map(({ tenant, role }) => (
        <View key={tenant.slug} style={styles.card}>
          <Text style={styles.tenantName}>{tenant.display_name}</Text>
          <Text style={styles.muted}>tier {tenant.tier} · role {role}</Text>
        </View>
      ))}
    </ScrollView>
  );
}

const styles = StyleSheet.create({
  container: { padding: 16, backgroundColor: tokens.bg, minHeight: "100%" },
  center: { flex: 1, justifyContent: "center", alignItems: "center", backgroundColor: tokens.bg },
  h1: { color: tokens.fg, fontSize: 22, fontWeight: "700", marginBottom: 12 },
  muted: { color: tokens.muted, fontSize: 13 },
  error: { color: tokens.deny, marginBottom: 12 },
  link: { color: tokens.accent, fontSize: 16, marginTop: 8 },
  card: { backgroundColor: tokens.codeBg, borderColor: tokens.border, borderWidth: 1, borderRadius: tokens.rMd, padding: 12, marginBottom: 10 },
  tenantName: { color: tokens.fg, fontSize: 16, fontWeight: "600", marginBottom: 4 },
});
