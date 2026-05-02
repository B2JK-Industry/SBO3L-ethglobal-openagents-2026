import { useState } from "react";
import { Alert, ScrollView, StyleSheet, Switch, Text, TouchableOpacity, View } from "react-native";
import { useRouter } from "expo-router";
import { signOut } from "~/lib/auth";
import { registerForPushNotifications } from "~/lib/push";
import { tokens } from "~/theme";

const TENANT_SLUG = "acme";

export default function Settings(): JSX.Element {
  const router = useRouter();
  const [push, setPush] = useState(false);

  const togglePush = async (next: boolean): Promise<void> => {
    setPush(next);
    if (next) {
      const token = await registerForPushNotifications(TENANT_SLUG);
      if (!token) {
        Alert.alert("Push", "Permission denied or unavailable.");
        setPush(false);
      }
    }
  };

  const onSignOut = async (): Promise<void> => {
    await signOut();
    router.replace("/signin");
  };

  return (
    <ScrollView contentContainerStyle={styles.container}>
      <View style={styles.row}>
        <Text style={styles.label}>Push notifications</Text>
        <Switch value={push} onValueChange={togglePush} trackColor={{ true: tokens.accent }} />
      </View>
      <Text style={styles.muted}>
        Pushes fire on human_2fa decisions for your active tenant.
      </Text>
      <TouchableOpacity style={styles.signoutBtn} onPress={onSignOut}>
        <Text style={styles.signoutText}>Sign out</Text>
      </TouchableOpacity>
      <Text style={styles.version}>SBO3L mobile · v0.0.1</Text>
    </ScrollView>
  );
}

const styles = StyleSheet.create({
  container: { padding: 16, backgroundColor: tokens.bg, minHeight: "100%" },
  row: { flexDirection: "row", justifyContent: "space-between", alignItems: "center", paddingVertical: 12, borderBottomColor: tokens.border, borderBottomWidth: 1 },
  label: { color: tokens.fg, fontSize: 15 },
  muted: { color: tokens.muted, fontSize: 12, marginTop: 8, marginBottom: 32 },
  signoutBtn: { padding: 14, backgroundColor: tokens.codeBg, borderColor: tokens.border, borderWidth: 1, borderRadius: tokens.rMd, alignItems: "center" },
  signoutText: { color: tokens.deny, fontSize: 15, fontWeight: "600" },
  version: { color: tokens.muted, textAlign: "center", marginTop: 24, fontSize: 11 },
});
