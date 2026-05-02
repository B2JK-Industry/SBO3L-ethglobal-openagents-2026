import { useState } from "react";
import { ActivityIndicator, StyleSheet, Text, TouchableOpacity, View } from "react-native";
import { useRouter } from "expo-router";
import { signIn } from "~/lib/auth";
import { tokens } from "~/theme";

export default function SignIn(): JSX.Element {
  const router = useRouter();
  const [busy, setBusy] = useState(false);

  const onSignIn = async (): Promise<void> => {
    setBusy(true);
    const ok = await signIn();
    setBusy(false);
    if (ok) router.replace("/");
  };

  return (
    <View style={styles.container}>
      <Text style={styles.brand}>SBO3L</Text>
      <Text style={styles.tagline}>Don't give your agent a wallet.</Text>
      <Text style={styles.taglineAccent}>Give it a mandate.</Text>
      <TouchableOpacity onPress={onSignIn} disabled={busy} style={styles.btn}>
        {busy ? <ActivityIndicator color={tokens.bg} /> : <Text style={styles.btnText}>Sign in with GitHub</Text>}
      </TouchableOpacity>
    </View>
  );
}

const styles = StyleSheet.create({
  container: { flex: 1, backgroundColor: tokens.bg, padding: 32, justifyContent: "center", alignItems: "center" },
  brand: { color: tokens.accent, fontSize: 14, fontFamily: tokens.fontMono, letterSpacing: 4, marginBottom: 24 },
  tagline: { color: tokens.fg, fontSize: 22, fontWeight: "700", textAlign: "center" },
  taglineAccent: { color: tokens.accent, fontSize: 22, fontWeight: "700", textAlign: "center", marginBottom: 48 },
  btn: { backgroundColor: tokens.accent, paddingVertical: 14, paddingHorizontal: 32, borderRadius: tokens.rMd, alignSelf: "stretch", alignItems: "center" },
  btnText: { color: tokens.bg, fontSize: 16, fontWeight: "700" },
});
