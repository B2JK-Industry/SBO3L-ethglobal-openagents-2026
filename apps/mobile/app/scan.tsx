import { BarCodeScanner, BarCodeScannerResult } from "expo-barcode-scanner";
import { useEffect, useState } from "react";
import { StyleSheet, Text, TouchableOpacity, View } from "react-native";
import { useRouter } from "expo-router";
import { tokens } from "~/theme";

// Capsule QR scanner. The QR encodes a base64-url Passport capsule
// payload. We hand it off to the verify route which mounts the WASM
// verifier (same code path as the hosted-app /demo/3-verify-yourself).

export default function ScanScreen(): JSX.Element {
  const [permission, setPermission] = useState<boolean | null>(null);
  const [scanned, setScanned] = useState(false);
  const router = useRouter();

  useEffect(() => {
    BarCodeScanner.requestPermissionsAsync().then(({ status }) => setPermission(status === "granted"));
  }, []);

  if (permission === null) return <View style={styles.center}><Text style={styles.muted}>Requesting camera permission…</Text></View>;
  if (permission === false) return <View style={styles.center}><Text style={styles.error}>Camera permission denied. Enable in Settings.</Text></View>;

  const onScan = ({ data }: BarCodeScannerResult): void => {
    if (scanned) return;
    setScanned(true);
    router.push({ pathname: "/verify", params: { capsule: data } });
  };

  return (
    <View style={styles.container}>
      <BarCodeScanner onBarCodeScanned={onScan} style={StyleSheet.absoluteFillObject} />
      <View style={styles.frame}><Text style={styles.frameText}>Align Passport QR inside the frame</Text></View>
      {scanned && (
        <TouchableOpacity onPress={() => setScanned(false)} style={styles.retry}>
          <Text style={styles.retryText}>Tap to scan again</Text>
        </TouchableOpacity>
      )}
    </View>
  );
}

const styles = StyleSheet.create({
  container: { flex: 1, backgroundColor: tokens.bg },
  center: { flex: 1, justifyContent: "center", alignItems: "center", padding: 16 },
  muted: { color: tokens.muted },
  error: { color: tokens.deny },
  frame: { position: "absolute", left: 40, right: 40, top: "30%", height: 280, borderColor: tokens.accent, borderWidth: 2, borderRadius: tokens.rLg, justifyContent: "flex-end", padding: 8 },
  frameText: { color: tokens.accent, fontSize: 12, textAlign: "center" },
  retry: { position: "absolute", bottom: 40, alignSelf: "center", padding: 14, backgroundColor: tokens.codeBg, borderRadius: tokens.rMd },
  retryText: { color: tokens.fg, fontSize: 14 },
});
