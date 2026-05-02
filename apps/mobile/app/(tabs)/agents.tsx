import { StyleSheet, Text, View } from "react-native";
import { tokens } from "~/theme";

// Agents list — placeholder until /api/t/<slug>/agents lands in the
// hosted-app proxy. Mobile read-only by design (registration is web-
// only via the hosted-app /admin/agents flow).
export default function AgentsList(): JSX.Element {
  return (
    <View style={styles.container}>
      <Text style={styles.muted}>Agent registration is web-only. View agents at app.sbo3l.dev/admin/agents.</Text>
    </View>
  );
}

const styles = StyleSheet.create({
  container: { flex: 1, backgroundColor: tokens.bg, padding: 24, justifyContent: "center" },
  muted: { color: tokens.muted, textAlign: "center", lineHeight: 22 },
});
